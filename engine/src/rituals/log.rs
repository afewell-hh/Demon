use anyhow::{Context, Result};
use async_nats::jetstream::{self, consumer::PullConsumer, stream::Stream};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info};

const STREAM_NAME: &str = "DEMON_RITUAL_EVENTS";
const STREAM_SUBJECTS: &str = "demon.ritual.v1.>";

#[derive(Debug, Clone)]
pub struct EventLog {
    client: async_nats::Client,
    jetstream: jetstream::Context,
    stream: Stream,
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
}

impl EventLog {
    pub async fn new(nats_url: &str) -> Result<Self> {
        let client = async_nats::connect(nats_url)
            .await
            .context("Failed to connect to NATS")?;
        
        let jetstream = jetstream::new(client.clone());
        
        // Create or get the stream
        let stream = jetstream
            .get_or_create_stream(jetstream::stream::Config {
                name: STREAM_NAME.to_string(),
                subjects: vec![STREAM_SUBJECTS.to_string()],
                retention: jetstream::stream::RetentionPolicy::Limits,
                storage: jetstream::stream::StorageType::File,
                duplicate_window: std::time::Duration::from_secs(120), // 2 minute dedup window
                ..Default::default()
            })
            .await
            .context("Failed to create/get stream")?;
        
        info!("Connected to JetStream stream: {}", STREAM_NAME);
        
        Ok(Self {
            client,
            jetstream,
            stream,
        })
    }
    
    pub async fn append(&self, event: &RitualEvent, sequence: u64) -> Result<()> {
        let (ritual_id, run_id) = match event {
            RitualEvent::Started { ritual_id, run_id, .. } |
            RitualEvent::StateTransitioned { ritual_id, run_id, .. } |
            RitualEvent::Completed { ritual_id, run_id, .. } => (ritual_id, run_id),
        };
        
        let subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);
        let msg_id = format!("{}:{}", run_id, sequence);
        
        let payload = serde_json::to_vec(event)
            .context("Failed to serialize event")?;
        
        let mut headers = async_nats::HeaderMap::new();
        headers.insert("Nats-Msg-Id", msg_id.as_str());
        
        let ack = self.jetstream
            .publish_with_headers(
                subject.clone(),
                headers,
                payload.into(),
            )
            .await
            .context("Failed to publish event")?
            .await
            .context("Failed to get ack")?;
        
        debug!("Published event with msg-id {} to {}, seq: {}", msg_id, subject, ack.sequence);
        
        Ok(())
    }
    
    pub async fn read_run(&self, ritual_id: &str, run_id: &str) -> Result<Vec<RitualEvent>> {
        let filter_subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);
        let consumer_name = format!("replay-{}", run_id);
        
        // Create ephemeral pull consumer
        let consumer: PullConsumer = self.stream
            .create_consumer(jetstream::consumer::pull::Config {
                name: Some(consumer_name.clone()),
                filter_subject: filter_subject.clone(),
                ack_policy: jetstream::consumer::AckPolicy::Explicit,
                ..Default::default()
            })
            .await
            .context("Failed to create consumer")?;
        
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
                    // Check if this is a timeout (no more messages) vs a real error
                    let error_msg = format!("{}", e);
                    if error_msg.contains("timeout") || error_msg.contains("no messages") {
                        // Timeout on empty batch - this is expected completion
                        debug!("Batch fetch timeout - no more messages available");
                        break;
                    } else {
                        // Real batch fetch error - propagate it
                        return Err(anyhow::anyhow!("Failed to fetch batch: {}", e));
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
        
        // Clean up consumer
        let _ = self.stream.delete_consumer(&consumer_name).await;
        
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
        let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
        let log = EventLog::new(&nats_url).await.unwrap();
        
        let ritual_id = "test-ritual";
        let run_id = uuid::Uuid::new_v4().to_string();
        
        // Append events
        let event1 = create_test_started_event(ritual_id, &run_id);
        let event2 = create_test_completed_event(ritual_id, &run_id);
        
        log.append(&event1, 1).await.unwrap();
        log.append(&event2, 2).await.unwrap();
        
        // Read back
        let events = log.read_run(ritual_id, &run_id).await.unwrap();
        
        assert_eq!(events.len(), 2);
        matches!(events[0], RitualEvent::Started { .. });
        matches!(events[1], RitualEvent::Completed { .. });
    }
    
    #[tokio::test]
    #[ignore] // Requires NATS to be running
    async fn test_idempotency() {
        let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
        let log = EventLog::new(&nats_url).await.unwrap();
        
        let ritual_id = "test-ritual";
        let run_id = uuid::Uuid::new_v4().to_string();
        
        let event = create_test_started_event(ritual_id, &run_id);
        
        // Publish same event twice with same sequence
        log.append(&event, 1).await.unwrap();
        log.append(&event, 1).await.unwrap(); // Should be deduplicated
        
        // Read back - should only have one event
        let events = log.read_run(ritual_id, &run_id).await.unwrap();
        assert_eq!(events.len(), 1);
    }
}