use anyhow::{Context, Result};
use async_nats::jetstream::{self, consumer::DeliverPolicy};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use tracing::{debug, error, info, warn};

/// JetStream client for querying ritual events
#[derive(Clone)]
pub struct JetStreamClient {
    jetstream: jetstream::Context,
<<<<<<< HEAD
    cfg: JetStreamConfig,
}

#[derive(Clone, Debug)]
pub struct JetStreamConfig {
    pub stream_name: String,
    pub subjects: Vec<String>,
=======
>>>>>>> origin/main
}

/// Run summary information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "ritualId")]
    pub ritual_id: String,
    #[serde(rename = "startTs")]
    pub start_ts: DateTime<Utc>,
    pub status: RunStatus,
}

/// Run status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunStatus::Running => write!(f, "Running"),
            RunStatus::Completed => write!(f, "Completed"),
            RunStatus::Failed => write!(f, "Failed"),
        }
    }
}

/// Detailed run information with events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunDetail {
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "ritualId")]
    pub ritual_id: String,
    pub events: Vec<RitualEvent>,
}

/// A single ritual event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RitualEvent {
    pub ts: DateTime<Utc>,
    pub event: String,
    #[serde(rename = "stateFrom", skip_serializing_if = "Option::is_none")]
    pub state_from: Option<String>,
    #[serde(rename = "stateTo", skip_serializing_if = "Option::is_none")]
    pub state_to: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl JetStreamClient {
    /// Create a new JetStream client
    pub async fn new() -> Result<Self> {
        let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());

        info!("Connecting to NATS at {}", nats_url);

        let client = if let Ok(creds_path) = env::var("NATS_CREDS_PATH") {
            info!("Using credentials file: {}", creds_path);
            async_nats::ConnectOptions::new()
                .credentials_file(&creds_path)
                .await?
                .connect(&nats_url)
                .await?
        } else {
            warn!("No NATS credentials provided, connecting without auth");
            async_nats::connect(&nats_url).await?
        };

        let jetstream = jetstream::new(client);

<<<<<<< HEAD
        let cfg = JetStreamConfig {
            stream_name: env::var("RITUAL_STREAM_NAME").unwrap_or_else(|_| "RITUAL_EVENTS".into()),
            subjects: env::var("RITUAL_SUBJECTS")
                .unwrap_or_else(|_| "demon.ritual.v1.>".into())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        };

        Ok(Self { jetstream, cfg })
    }

    /// Ensure the configured stream exists; create if missing (idempotent).
    pub async fn ensure_stream(&self) -> Result<()> {
        match self.jetstream.get_stream(&self.cfg.stream_name).await {
            Ok(_) => Ok(()),
            Err(_) => {
                tracing::info!(
                    "Creating JetStream stream '{}' with subjects {:?}",
                    self.cfg.stream_name,
                    self.cfg.subjects
                );
                let stream_config = jetstream::stream::Config {
                    name: self.cfg.stream_name.clone(),
                    subjects: self.cfg.subjects.clone(),
                    duplicate_window: std::time::Duration::from_secs(120),
                    ..Default::default()
                };
                Ok(self
                    .jetstream
                    .create_stream(stream_config)
                    .await
                    .map(|_| ())?)
            }
        }
=======
        Ok(Self { jetstream })
>>>>>>> origin/main
    }

    /// List recent runs with optional limit
    pub async fn list_runs(&self, limit: Option<usize>) -> Result<Vec<RunSummary>> {
        let limit = limit.unwrap_or(50).min(1000); // Cap at 1000 for safety
        debug!("Listing runs with limit: {}", limit);

        // Query JetStream for ritual events
        // Subject pattern: demon.ritual.v1.<ritualId>.<runId>.events
        let subject_filter = "demon.ritual.v1.*.*.events";

        let mut runs_map: HashMap<String, RunSummary> = HashMap::new();
        let mut message_count = 0;

        // Get messages from JetStream stream
        // Use DeliverPolicy::All to get all messages, then sort by timestamp to prioritize recent runs
        match self
            .query_stream_messages(subject_filter, None, DeliverPolicy::All)
            .await
        {
            Ok(messages) => {
                for message in messages {
                    message_count += 1;

                    match self.parse_message_for_run_summary(&message) {
                        Ok(Some(summary)) => {
                            // Merge summaries: keep earliest start time, latest status
                            let key = format!("{}:{}", summary.ritual_id, summary.run_id);
                            if let Some(mut existing) = runs_map.remove(&key) {
                                // Keep the earliest start time (unless it's the placeholder)
                                if summary.start_ts
                                    != DateTime::from_timestamp(0, 0).unwrap_or_else(Utc::now)
                                    && (existing.start_ts
                                        == DateTime::from_timestamp(0, 0).unwrap_or_else(Utc::now)
                                        || summary.start_ts < existing.start_ts)
                                {
                                    existing.start_ts = summary.start_ts;
                                }
                                // Always update to the most definitive status
                                match (existing.status, summary.status) {
                                    (_, RunStatus::Completed) | (_, RunStatus::Failed) => {
                                        existing.status = summary.status
                                    }
                                    (RunStatus::Running, _) => existing.status = summary.status,
                                    _ => {} // Keep existing status
                                }
                                runs_map.insert(key, existing);
                            } else {
                                runs_map.insert(key, summary);
                            }
                        }
                        Ok(None) => {
                            // Message didn't contain useful run info
                        }
                        Err(e) => {
                            debug!("Failed to parse message for run summary: {}", e);
                        }
                    }

                    // No acknowledgment needed with AckPolicy::None

                    // Stop when we have enough unique runs (with some buffer for completeness)
                    if runs_map.len() >= limit && message_count >= limit * 2 {
                        debug!(
                            "Collected {} runs from {} messages, stopping",
                            runs_map.len(),
                            message_count
                        );
                        break;
                    }
                }
            }
            Err(e) => {
                error!("Failed to query stream messages: {}", e);
                return Err(e); // Propagate error instead of hiding as empty list
            }
        }

        // Sort by start time (most recent first) and limit
        let mut runs: Vec<RunSummary> = runs_map.into_values().collect();
        runs.sort_by(|a, b| b.start_ts.cmp(&a.start_ts));
        runs.truncate(limit);

        info!(
            "Retrieved {} runs from {} messages",
            runs.len(),
            message_count
        );
        Ok(runs)
    }

    /// Get detailed information for a specific run
    pub async fn get_run_detail(&self, run_id: &str) -> Result<Option<RunDetail>> {
        debug!("Getting run detail for: {}", run_id);

        // Subject pattern for specific run: demon.ritual.v1.*.<runId>.events
        let subject_filter = &format!("demon.ritual.v1.*.{}.events", run_id);

        let mut events = Vec::new();
        let mut ritual_id: Option<String> = None;

        // Use multiple batches to get complete event history for long-running rituals
        match self.query_all_stream_messages(subject_filter).await {
            Ok(messages) => {
                for message in messages {
                    match self.parse_message_for_event(&message) {
                        Ok(Some(event)) => {
                            events.push(event);
                        }
                        Ok(None) => {
                            // Message didn't contain useful event info
                        }
                        Err(e) => {
                            debug!("Failed to parse message for event: {}", e);
                        }
                    }

                    // Extract ritual_id from subject if we haven't found it yet
                    if ritual_id.is_none() {
                        if let Some(extracted) =
                            self.extract_ritual_id_from_subject(&message.subject)
                        {
                            ritual_id = Some(extracted);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to query stream messages for run {}: {}", run_id, e);
                return Err(e); // Propagate error instead of hiding as 404
            }
        }

        if events.is_empty() {
            return Ok(None);
        }

        // Sort events by timestamp
        events.sort_by(|a, b| a.ts.cmp(&b.ts));

        let ritual_id = ritual_id.unwrap_or_else(|| "unknown".to_string());

        Ok(Some(RunDetail {
            run_id: run_id.to_string(),
            ritual_id,
            events,
        }))
    }

    /// Query all stream messages without limit using multiple batches
    async fn query_all_stream_messages(
        &self,
        subject_filter: &str,
    ) -> Result<Vec<async_nats::jetstream::Message>> {
        debug!(
            "Querying all messages with subject filter: {}",
            subject_filter
        );

<<<<<<< HEAD
        // Get stream; caller handles missing stream
        let stream = self
            .jetstream
            .get_stream(&self.cfg.stream_name)
            .await
            .with_context(|| format!("JetStream stream '{}' not found", self.cfg.stream_name))?;
=======
        // Resolve stream with precedence: RITUAL_STREAM_NAME -> existing DEMON_RITUAL_EVENTS (deprecated) -> default RITUAL_EVENTS
        let desired = std::env::var("RITUAL_STREAM_NAME").ok();
        let stream = if let Some(name) = desired {
            self.jetstream
                .get_stream(&name)
                .await
                .with_context(|| format!("JetStream stream '{}' not found", name))?
        } else {
            match self.jetstream.get_stream("RITUAL_EVENTS").await {
                Ok(s) => s,
                Err(_) => {
                    let s = self
                        .jetstream
                        .get_stream("DEMON_RITUAL_EVENTS")
                        .await
                        .with_context(|| "JetStream stream 'RITUAL_EVENTS' not found; 'DEMON_RITUAL_EVENTS' also missing")?;
                    warn!("Using deprecated stream name 'DEMON_RITUAL_EVENTS'; set RITUAL_STREAM_NAME or migrate to 'RITUAL_EVENTS'");
                    s
                }
            }
        };
>>>>>>> origin/main

        // Create ephemeral (non-durable) consumer for truly stateless read-only queries
        // Non-durable consumers are automatically deleted after use and don't persist state
        let consumer_config = jetstream::consumer::pull::Config {
            filter_subject: subject_filter.to_string(),
            durable_name: None, // Ephemeral consumer - no state persistence
            deliver_policy: DeliverPolicy::All, // Get all historical messages
            ack_policy: async_nats::jetstream::consumer::AckPolicy::None, // No acknowledgment needed for read-only operations
            inactive_threshold: std::time::Duration::from_secs(60), // Auto-delete after 60 seconds of inactivity
            ..Default::default()
        };

        // Create ephemeral consumer (no get-or-create needed since they're temporary)
        let consumer = match stream.create_consumer(consumer_config).await {
            Ok(consumer) => {
                debug!(
                    "Created ephemeral consumer for all messages: {}",
                    subject_filter
                );
                consumer
            }
            Err(e) => {
                error!("Failed to create ephemeral consumer: {}", e);
                return Err(e.into());
            }
        };

        debug!("Using consumer for all messages: {}", subject_filter);

        const BATCH_SIZE: usize = 10_000;
        const BATCH_EXPIRES_SECS: u64 = 5; // bounded per-batch wait; documented (not hidden)

        let mut all_messages = Vec::new();
        let mut total_fetched = 0usize;

        loop {
            let mut messages = consumer
                .batch()
                .max_messages(BATCH_SIZE)
                .expires(std::time::Duration::from_secs(BATCH_EXPIRES_SECS))
                .messages()
                .await
                .context("Failed to initiate JetStream batch fetch")?;

            let mut batch_count = 0usize;
            let mut batch_messages = Vec::new();

            while let Some(msg_result) = messages.next().await {
                match msg_result {
                    Ok(msg) => {
                        batch_messages.push(msg);
                        batch_count += 1;
                    }
                    Err(e) => {
                        warn!("Error receiving message from JetStream: {}", e);
                    }
                }
            }

            if batch_count == 0 {
                debug!("No more messages available, stopping");
                break;
            }

            total_fetched += batch_count;
            all_messages.extend(batch_messages);

            if batch_count < BATCH_SIZE {
                debug!(
                    "Received {} messages (< batch size {}), stopping",
                    batch_count, BATCH_SIZE
                );
                break;
            }

            debug!(
                "Fetched batch of {} messages, total so far: {}",
                batch_count, total_fetched
            );
        }

        info!(
            "Fetched total of {} messages across all batches",
            total_fetched
        );

        // Note: Ephemeral consumers should be automatically cleaned up by JetStream
        // when the client connection is closed or after a timeout period

        Ok(all_messages)
    }

    /// Query stream messages with optional limit and delivery policy
    async fn query_stream_messages(
        &self,
        subject_filter: &str,
        limit: Option<usize>,
        deliver_policy: DeliverPolicy,
    ) -> Result<Vec<async_nats::jetstream::Message>> {
        debug!("Querying messages with subject filter: {}", subject_filter);

<<<<<<< HEAD
        // Get stream; caller handles missing stream
        let stream = self
            .jetstream
            .get_stream(&self.cfg.stream_name)
            .await
            .with_context(|| format!("JetStream stream '{}' not found", self.cfg.stream_name))?;
=======
        // Resolve stream with precedence (see above)
        let desired = std::env::var("RITUAL_STREAM_NAME").ok();
        let stream = if let Some(name) = desired {
            self.jetstream
                .get_stream(&name)
                .await
                .with_context(|| format!("JetStream stream '{}' not found", name))?
        } else {
            match self.jetstream.get_stream("RITUAL_EVENTS").await {
                Ok(s) => s,
                Err(_) => {
                    let s = self
                        .jetstream
                        .get_stream("DEMON_RITUAL_EVENTS")
                        .await
                        .with_context(|| "JetStream stream 'RITUAL_EVENTS' not found; 'DEMON_RITUAL_EVENTS' also missing")?;
                    warn!("Using deprecated stream name 'DEMON_RITUAL_EVENTS'; set RITUAL_STREAM_NAME or migrate to 'RITUAL_EVENTS'");
                    s
                }
            }
        };
>>>>>>> origin/main

        // Create ephemeral (non-durable) consumer for truly stateless read-only queries
        // Non-durable consumers are automatically deleted after use and don't persist state
        let consumer_config = jetstream::consumer::pull::Config {
            filter_subject: subject_filter.to_string(),
            durable_name: None, // Ephemeral consumer - no state persistence
            deliver_policy,     // Use provided delivery policy
            ack_policy: async_nats::jetstream::consumer::AckPolicy::None, // No acknowledgment needed for read-only operations
            inactive_threshold: std::time::Duration::from_secs(60), // Auto-delete after 60 seconds of inactivity
            ..Default::default()
        };

        // Create ephemeral consumer (no get-or-create needed since they're temporary)
        let consumer = match stream.create_consumer(consumer_config).await {
            Ok(consumer) => {
                debug!(
                    "Created ephemeral consumer for subject filter: {}",
                    subject_filter
                );
                consumer
            }
            Err(e) => {
                error!("Failed to create ephemeral consumer: {}", e);
                return Err(e.into());
            }
        };

        debug!("Using consumer for subject filter: {}", subject_filter);

        // Use batch fetch with a bounded per-batch wait (deterministic termination)
        let batch_size = limit.unwrap_or(10_000).min(10_000);
        match consumer
            .batch()
            .max_messages(batch_size)
            .expires(std::time::Duration::from_secs(5))
            .messages()
            .await
        {
            Ok(mut messages) => {
                // Collect all messages from the stream immediately
                let mut collected_messages = Vec::new();

                while let Some(msg_result) = messages.next().await {
                    match msg_result {
                        Ok(msg) => collected_messages.push(msg),
                        Err(e) => {
                            error!("Error receiving message from JetStream: {}", e);
                        }
                    }
                }

                Ok(collected_messages)
            }
            Err(e) => {
                error!("Failed to fetch messages: {}", e);
                Err(e.into())
            }
        }

        // Note: Ephemeral consumers should be automatically cleaned up by JetStream
        // when the client connection is closed or after a timeout period
    }

    /// Parse a NATS message for run summary information
    fn parse_message_for_run_summary(
        &self,
        message: &async_nats::jetstream::Message,
    ) -> Result<Option<RunSummary>> {
        let payload: serde_json::Value = serde_json::from_slice(&message.message.payload)
            .context("Failed to parse message payload as JSON")?;

        // Extract ritual and run IDs from subject
        // Subject format: demon.ritual.v1.<ritualId>.<runId>.events
        let parts: Vec<&str> = message.subject.split('.').collect();
        if parts.len() < 6 {
            return Ok(None);
        }

        let ritual_id = parts[3].to_string();
        let run_id = parts[4].to_string();

        // Try to extract timestamp and determine status
        let ts = if let Some(ts_str) = payload.get("ts").and_then(|v| v.as_str()) {
            ts_str
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now())
        } else {
            Utc::now()
        };

        // Determine status from event type and if this is a start event
        let (status, is_start_event) =
            if let Some(event_type) = payload.get("event").and_then(|v| v.as_str()) {
                match event_type {
                    "ritual.completed:v1" => (RunStatus::Completed, false),
                    "ritual.failed:v1" => (RunStatus::Failed, false),
                    "ritual.started:v1" => (RunStatus::Running, true),
                    _ => (RunStatus::Running, false),
                }
            } else {
                (RunStatus::Running, false)
            };

        // Only use this timestamp as start_ts if it's actually a start event
        // Otherwise, use a placeholder (will be overwritten by actual start time)
        let start_ts = if is_start_event {
            ts
        } else {
            DateTime::from_timestamp(0, 0).unwrap_or_else(Utc::now)
        };

        Ok(Some(RunSummary {
            run_id,
            ritual_id,
            start_ts,
            status,
        }))
    }

    /// Parse a NATS message for event information
    fn parse_message_for_event(
        &self,
        message: &async_nats::jetstream::Message,
    ) -> Result<Option<RitualEvent>> {
        let payload: serde_json::Value = serde_json::from_slice(&message.message.payload)
            .context("Failed to parse message payload as JSON")?;

        let ts = if let Some(ts_str) = payload.get("ts").and_then(|v| v.as_str()) {
            ts_str
                .parse::<DateTime<Utc>>()
                .context("Failed to parse timestamp")?
        } else {
            return Ok(None); // No timestamp, skip this event
        };

        let event = payload
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Extract state transitions if available
        let state_from = payload
            .get("stateFrom")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let state_to = payload
            .get("stateTo")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Capture extra fields
        let mut extra = HashMap::new();
        if let serde_json::Value::Object(obj) = payload {
            for (k, v) in obj {
                if !matches!(k.as_str(), "ts" | "event" | "stateFrom" | "stateTo") {
                    extra.insert(k, v);
                }
            }
        }

        Ok(Some(RitualEvent {
            ts,
            event,
            state_from,
            state_to,
            extra,
        }))
    }

    /// Extract ritual ID from subject
    fn extract_ritual_id_from_subject(&self, subject: &str) -> Option<String> {
        extract_ritual_id_from_subject(subject)
    }
}

/// Extract ritual ID from subject string (standalone function for testing)
fn extract_ritual_id_from_subject(subject: &str) -> Option<String> {
    let parts: Vec<&str> = subject.split('.').collect();
    if parts.len() >= 4 {
        Some(parts[3].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ritual_id_from_subject() {
        let subject = "demon.ritual.v1.my-ritual.run-123.events";
        let ritual_id = extract_ritual_id_from_subject(subject);
        assert_eq!(ritual_id, Some("my-ritual".to_string()));

        let invalid_subject = "demon.ritual";
        let ritual_id = extract_ritual_id_from_subject(invalid_subject);
        assert_eq!(ritual_id, None);
    }

    #[test]
    fn test_run_status_display() {
        assert_eq!(RunStatus::Running.to_string(), "Running");
        assert_eq!(RunStatus::Completed.to_string(), "Completed");
        assert_eq!(RunStatus::Failed.to_string(), "Failed");
    }
}
