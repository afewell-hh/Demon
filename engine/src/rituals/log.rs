use anyhow::{Context, Result};
use async_nats::jetstream::{self, consumer::PullConsumer, stream::Stream};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info};

const DEFAULT_STREAM_NAME: &str = "RITUAL_EVENTS";
const DEPRECATED_STREAM_NAME: &str = "DEMON_RITUAL_EVENTS"; // kept for compatibility
const STREAM_SUBJECTS: &str = "demon.ritual.v1.>";

#[derive(Debug, Clone)]
pub struct EventLog {
    #[allow(dead_code)]
    client: async_nats::Client,
    jetstream: jetstream::Context,
    stream: Stream,
    tenant_config: TenantConfig,
}

#[derive(Debug, Clone)]
struct TenantConfig {
    enabled: bool,
    default_tenant: String,
    dual_publish: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "event")]
pub enum RitualEvent {
    #[serde(rename = "ritual.started:v1")]
    Started {
        #[serde(rename = "ritualId")]
        ritual_id: String,
        #[serde(rename = "runId")]
        run_id: String,
        ts: String,
        spec: Value,
        #[serde(rename = "traceId", skip_serializing_if = "Option::is_none")]
        trace_id: Option<String>,
    },
    #[serde(rename = "ritual.state.transitioned:v1")]
    StateTransitioned {
        #[serde(rename = "ritualId")]
        ritual_id: String,
        #[serde(rename = "runId")]
        run_id: String,
        ts: String,
        #[serde(rename = "fromState")]
        from_state: String,
        #[serde(rename = "toState")]
        to_state: String,
        #[serde(rename = "traceId", skip_serializing_if = "Option::is_none")]
        trace_id: Option<String>,
    },
    #[serde(rename = "ritual.completed:v1")]
    Completed {
        #[serde(rename = "ritualId")]
        ritual_id: String,
        #[serde(rename = "runId")]
        run_id: String,
        ts: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        outputs: Option<Value>,
        #[serde(rename = "traceId", skip_serializing_if = "Option::is_none")]
        trace_id: Option<String>,
    },
    #[serde(rename = "policy.decision:v1")]
    PolicyDecision {
        #[serde(rename = "ritualId")]
        ritual_id: String,
        #[serde(rename = "runId")]
        run_id: String,
        ts: String,
        #[serde(rename = "tenantId")]
        tenant_id: String,
        capability: String,
        decision: Value,
        quota: Value,
    },
}

impl TenantConfig {
    fn from_env() -> Self {
        let enabled = std::env::var("TENANTING_ENABLED")
            .unwrap_or_else(|_| "0".to_string())
            .parse::<u8>()
            .unwrap_or(0)
            == 1;

        let default_tenant =
            std::env::var("TENANT_DEFAULT").unwrap_or_else(|_| "default".to_string());

        let dual_publish = std::env::var("TENANT_DUAL_PUBLISH")
            .unwrap_or_else(|_| "0".to_string())
            .parse::<u8>()
            .unwrap_or(0)
            == 1;

        debug!(
            "Engine tenant configuration - enabled: {}, default: {}, dual_publish: {}",
            enabled, default_tenant, dual_publish
        );

        Self {
            enabled,
            default_tenant,
            dual_publish,
        }
    }

    fn build_subject(&self, tenant_id: &str, ritual_id: &str, run_id: &str) -> String {
        if self.enabled {
            format!(
                "demon.ritual.v1.{}.{}.{}.events",
                tenant_id, ritual_id, run_id
            )
        } else {
            format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id)
        }
    }
}

impl EventLog {
    pub async fn new(nats_url: &str) -> Result<Self> {
        let client = async_nats::connect(nats_url)
            .await
            .context("Failed to connect to NATS")?;

        let jetstream = jetstream::new(client.clone());

        // Resolve stream name with precedence:
        // RITUAL_STREAM_NAME env -> existing DEMON_RITUAL_EVENTS (deprecated) -> default RITUAL_EVENTS
        let env_name = std::env::var("RITUAL_STREAM_NAME").ok();
        let mut stream = if let Some(name) = env_name {
            // Respect explicit name
            jetstream
                .get_or_create_stream(jetstream::stream::Config {
                    name: name.clone(),
                    subjects: vec![STREAM_SUBJECTS.to_string()],
                    retention: jetstream::stream::RetentionPolicy::Limits,
                    storage: jetstream::stream::StorageType::File,
                    duplicate_window: std::time::Duration::from_secs(120),
                    ..Default::default()
                })
                .await
                .with_context(|| format!("Failed to create/get stream '{}'", name))?
        } else {
            // No env override: prefer RITUAL_EVENTS; if a deprecated stream exists, use it with a warning
            match jetstream.get_stream(DEFAULT_STREAM_NAME).await {
                Ok(s) => s,
                Err(_) => match jetstream.get_stream(DEPRECATED_STREAM_NAME).await {
                    Ok(s) => {
                        info!(
                            "Using deprecated stream name '{}'; set RITUAL_STREAM_NAME or migrate to '{}'",
                            DEPRECATED_STREAM_NAME, DEFAULT_STREAM_NAME
                        );
                        s
                    }
                    Err(_) => jetstream
                        .get_or_create_stream(jetstream::stream::Config {
                            name: DEFAULT_STREAM_NAME.to_string(),
                            subjects: vec![STREAM_SUBJECTS.to_string()],
                            retention: jetstream::stream::RetentionPolicy::Limits,
                            storage: jetstream::stream::StorageType::File,
                            duplicate_window: std::time::Duration::from_secs(120),
                            ..Default::default()
                        })
                        .await
                        .context("Failed to create/get default stream")?,
                },
            }
        };

        let stream_info = stream.info().await.context("Failed to fetch stream info")?;
        info!("Connected to JetStream stream: {}", stream_info.config.name);

        let tenant_config = TenantConfig::from_env();

        Ok(Self {
            client,
            jetstream,
            stream,
            tenant_config,
        })
    }

    pub async fn append(&self, event: &RitualEvent, sequence: u64) -> Result<()> {
        self.append_with_tenant(event, sequence, None).await
    }

    pub async fn append_with_tenant(
        &self,
        event: &RitualEvent,
        sequence: u64,
        tenant_id: Option<&str>,
    ) -> Result<()> {
        let (ritual_id, run_id) = match event {
            RitualEvent::Started {
                ritual_id, run_id, ..
            }
            | RitualEvent::StateTransitioned {
                ritual_id, run_id, ..
            }
            | RitualEvent::Completed {
                ritual_id, run_id, ..
            }
            | RitualEvent::PolicyDecision {
                ritual_id, run_id, ..
            } => (ritual_id, run_id),
        };

        let resolved_tenant = tenant_id.unwrap_or(&self.tenant_config.default_tenant);
        let msg_id = format!("{}:{}", run_id, sequence);
        let payload = serde_json::to_vec(event).context("Failed to serialize event")?;

        let mut headers = async_nats::HeaderMap::new();
        headers.insert("Nats-Msg-Id", msg_id.as_str());

        // Primary subject (tenant-scoped if enabled)
        let subject = self
            .tenant_config
            .build_subject(resolved_tenant, ritual_id, run_id);

        let ack = self
            .jetstream
            .publish_with_headers(subject.clone(), headers.clone(), payload.clone().into())
            .await
            .context("Failed to publish event")?
            .await
            .context("Failed to get ack")?;

        debug!(
            "Published event with msg-id {} to {}, seq: {}",
            msg_id, subject, ack.sequence
        );

        // Dual publish to legacy subject if enabled
        if self.tenant_config.dual_publish && self.tenant_config.enabled {
            let legacy_subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);
            let legacy_msg_id = format!("legacy:{}:{}", run_id, sequence);

            let mut legacy_headers = async_nats::HeaderMap::new();
            legacy_headers.insert("Nats-Msg-Id", legacy_msg_id.as_str());

            let legacy_ack = self
                .jetstream
                .publish_with_headers(legacy_subject.clone(), legacy_headers, payload.into())
                .await
                .context("Failed to publish legacy event")?
                .await
                .context("Failed to get legacy ack")?;

            debug!(
                "Dual-published event with msg-id {} to legacy {}, seq: {}",
                legacy_msg_id, legacy_subject, legacy_ack.sequence
            );
        }

        Ok(())
    }

    pub async fn read_run(&self, ritual_id: &str, run_id: &str) -> Result<Vec<RitualEvent>> {
        self.read_run_with_tenant(ritual_id, run_id, None).await
    }

    pub async fn read_run_with_tenant(
        &self,
        ritual_id: &str,
        run_id: &str,
        tenant_id: Option<&str>,
    ) -> Result<Vec<RitualEvent>> {
        let resolved_tenant = tenant_id.unwrap_or(&self.tenant_config.default_tenant);
        let filter_subject = self
            .tenant_config
            .build_subject(resolved_tenant, ritual_id, run_id);

        // Create truly ephemeral pull consumer (no name = auto-generated)
        // This allows concurrent reads and prevents consumer conflicts
        let mut consumer: PullConsumer = self
            .stream
            .create_consumer(jetstream::consumer::pull::Config {
                name: None, // Let JetStream auto-generate ephemeral consumer name
                filter_subject: filter_subject.clone(),
                ack_policy: jetstream::consumer::AckPolicy::Explicit,
                ..Default::default()
            })
            .await
            .context("Failed to create ephemeral consumer")?;

        // Ensure consumer cleanup happens regardless of success or failure
        let result = self.read_messages_with_cleanup(&mut consumer, run_id).await;

        // Always attempt cleanup, even if reading failed
        if let Ok(info) = consumer.info().await {
            let _ = self.stream.delete_consumer(&info.name).await;
            debug!("Cleaned up ephemeral consumer: {}", info.name);
        }

        result
    }

    async fn read_messages_with_cleanup(
        &self,
        consumer: &mut PullConsumer,
        run_id: &str,
    ) -> Result<Vec<RitualEvent>> {
        let mut events = Vec::new();

        // Fetch messages in batches to avoid infinite blocking
        // Use reasonable batch size and timeout for each batch
        const BATCH_SIZE: usize = 100;
        const BATCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

        loop {
            // Fetch a batch of messages with timeout
            let batch_result = consumer
                .batch()
                .max_messages(BATCH_SIZE)
                .expires(BATCH_TIMEOUT)
                .messages()
                .await;

            let mut batch = match batch_result {
                Ok(batch) => batch,
                Err(e) => {
                    // Be very conservative - only treat very specific empty-batch conditions as completion
                    // All other errors should be propagated to avoid hiding real issues
                    let error_msg = format!("{}", e);
                    debug!("Batch fetch error: {}", error_msg);

                    // Only treat very specific timeout/empty conditions as expected completion
                    // All other errors must be propagated to avoid hiding operational issues
                    if error_msg.contains("Timed out") || error_msg.contains("TimedOut") {
                        // JetStream timeout - this is expected when no (more) messages available
                        // Handle both "Timed out" (with space) and "TimedOut" (no space) variants
                        debug!(
                            "JetStream timeout - no more messages available: {}",
                            error_msg
                        );
                        break;
                    } else if error_msg.contains("no messages available")
                        || error_msg.contains("no matching messages")
                        || error_msg.contains("empty batch")
                    {
                        // Explicit empty responses - expected completion
                        debug!("Empty batch response - no more messages: {}", error_msg);
                        break;
                    } else {
                        // All other errors (authorization, permissions, missing streams, network, etc.)
                        // must be propagated - do NOT treat as empty runs
                        return Err(anyhow::anyhow!("Batch fetch failed: {}", e));
                    }
                }
            };

            let mut batch_count = 0;
            let mut batch_empty = true;

            // Process all messages in this batch
            while let Some(msg_result) = batch.next().await {
                batch_empty = false;
                match msg_result {
                    Ok(msg) => {
                        let event: RitualEvent = serde_json::from_slice(&msg.message.payload)
                            .context("Failed to deserialize event")?;
                        events.push(event);
                        let _ = msg.ack().await; // Best effort ack
                        batch_count += 1;
                    }
                    Err(e) => {
                        // Message processing error - propagate it
                        return Err(anyhow::anyhow!("Failed to process message in batch: {}", e));
                    }
                }
            }

            debug!("Processed batch of {} messages", batch_count);

            // If batch was empty or smaller than requested, we've read all available messages
            if batch_empty || batch_count < BATCH_SIZE {
                break;
            }
        }

        debug!("Read {} events for run {}", events.len(), run_id);

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_started_event(ritual_id: &str, run_id: &str) -> RitualEvent {
        RitualEvent::Started {
            ritual_id: ritual_id.to_string(),
            run_id: run_id.to_string(),
            ts: Utc::now().to_rfc3339(),
            spec: serde_json::json!({
                "id": ritual_id,
                "name": "Test Ritual",
                "version": "1.0.0",
                "initial": "start",
                "states": {
                    "start": {
                        "type": "action",
                        "action": "echo",
                        "input": { "message": "test" },
                        "end": true
                    }
                }
            }),
            trace_id: Some("test-trace".to_string()),
        }
    }

    fn create_test_completed_event(ritual_id: &str, run_id: &str) -> RitualEvent {
        RitualEvent::Completed {
            ritual_id: ritual_id.to_string(),
            run_id: run_id.to_string(),
            ts: Utc::now().to_rfc3339(),
            outputs: Some(serde_json::json!({ "result": "success" })),
            trace_id: Some("test-trace".to_string()),
        }
    }

    #[tokio::test]
    #[ignore] // Requires NATS to be running
    async fn test_append_and_read() {
        let nats_url =
            std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
        let log = EventLog::new(&nats_url).await.unwrap();

        let ritual_id = "test-ritual";
        let run_id = uuid::Uuid::new_v4().to_string();

        // Append events
        let event1 = create_test_started_event(ritual_id, &run_id);
        let event2 = create_test_completed_event(ritual_id, &run_id);

        log.append_with_tenant(&event1, 1, None).await.unwrap();
        log.append_with_tenant(&event2, 2, None).await.unwrap();

        // Read back
        let events = log.read_run(ritual_id, &run_id).await.unwrap();

        assert_eq!(events.len(), 2);
        matches!(events[0], RitualEvent::Started { .. });
        matches!(events[1], RitualEvent::Completed { .. });
    }

    #[tokio::test]
    #[ignore] // Requires NATS to be running
    async fn test_idempotency() {
        let nats_url =
            std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
        let log = EventLog::new(&nats_url).await.unwrap();

        let ritual_id = "test-ritual";
        let run_id = uuid::Uuid::new_v4().to_string();

        let event = create_test_started_event(ritual_id, &run_id);

        // Publish same event twice with same sequence
        log.append_with_tenant(&event, 1, None).await.unwrap();
        log.append_with_tenant(&event, 1, None).await.unwrap(); // Should be deduplicated

        // Read back - should only have one event
        let events = log.read_run(ritual_id, &run_id).await.unwrap();
        assert_eq!(events.len(), 1);
    }
}
