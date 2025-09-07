use anyhow::{Context, Result};
use async_nats::jetstream::{self, stream::Stream, consumer::DeliverPolicy};
use chrono::{DateTime, Utc};
use futures_util::{StreamExt, stream::BoxStream};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use tracing::{debug, error, info, warn};

/// JetStream client for querying ritual events
#[derive(Clone)]
pub struct JetStreamClient {
    jetstream: jetstream::Context,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
        
        Ok(Self { jetstream })
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
        match self.query_stream_messages(subject_filter, Some(limit * 10)).await {
            Ok(mut messages) => {
                while let Some(message) = messages.next().await {
                    message_count += 1;
                    
                    match self.parse_message_for_run_summary(&message) {
                        Ok(Some(summary)) => {
                            // Merge summaries: keep earliest start time, latest status
                            let key = format!("{}:{}", summary.ritual_id, summary.run_id);
                            if let Some(mut existing) = runs_map.remove(&key) {
                                // Keep the earliest start time (unless it's the placeholder)
                                if summary.start_ts != DateTime::from_timestamp(0, 0).unwrap_or_else(|| Utc::now()) {
                                    if existing.start_ts == DateTime::from_timestamp(0, 0).unwrap_or_else(|| Utc::now()) || summary.start_ts < existing.start_ts {
                                        existing.start_ts = summary.start_ts;
                                    }
                                }
                                // Always update to the most definitive status
                                match (existing.status, summary.status) {
                                    (_, RunStatus::Completed) | (_, RunStatus::Failed) => existing.status = summary.status,
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

                    // No need to acknowledge with non-durable consumers

                    // Stop if we've processed enough messages
                    if message_count >= limit * 10 {
                        break;
                    }
                }
            }
            Err(e) => {
                error!("Failed to query stream messages: {}", e);
                return Ok(vec![]); // Return empty list on error
            }
        }

        // Sort by start time (most recent first) and limit
        let mut runs: Vec<RunSummary> = runs_map.into_values().collect();
        runs.sort_by(|a, b| b.start_ts.cmp(&a.start_ts));
        runs.truncate(limit);

        info!("Retrieved {} runs from {} messages", runs.len(), message_count);
        Ok(runs)
    }

    /// Get detailed information for a specific run
    pub async fn get_run_detail(&self, run_id: &str) -> Result<Option<RunDetail>> {
        debug!("Getting run detail for: {}", run_id);

        // Subject pattern for specific run: demon.ritual.v1.*.<runId>.events
        let subject_filter = &format!("demon.ritual.v1.*.{}.events", run_id);
        
        let mut events = Vec::new();
        let mut ritual_id: Option<String> = None;

        match self.query_stream_messages(subject_filter, None).await {
            Ok(mut messages) => {
                while let Some(message) = messages.next().await {
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
                        if let Some(extracted) = self.extract_ritual_id_from_subject(&message.subject) {
                            ritual_id = Some(extracted);
                        }
                    }

                    // No need to acknowledge with non-durable consumers
                }
            }
            Err(e) => {
                error!("Failed to query stream messages for run {}: {}", run_id, e);
                return Ok(None);
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

    /// Query stream messages with optional limit
    async fn query_stream_messages(
        &self,
        subject_filter: &str,
        limit: Option<usize>,
    ) -> Result<BoxStream<'static, async_nats::jetstream::Message>> {
        debug!("Querying messages with subject filter: {}", subject_filter);

        // Try to get or create the stream for ritual events
        let stream_name = "RITUAL_EVENTS";
        let stream = match self.jetstream.get_stream(stream_name).await {
            Ok(stream) => {
                debug!("Found existing stream: {}", stream_name);
                stream
            }
            Err(_) => {
                info!("Stream {} not found, attempting to create it", stream_name);
                
                // Create stream configuration for ritual events
                let stream_config = jetstream::stream::Config {
                    name: stream_name.to_string(),
                    subjects: vec!["demon.ritual.v1.>".to_string()],
                    max_messages: 10_000,
                    max_bytes: 100_000_000, // 100MB
                    ..Default::default()
                };

                match self.jetstream.create_stream(stream_config).await {
                    Ok(stream) => {
                        info!("Successfully created stream: {}", stream_name);
                        stream
                    }
                    Err(e) => {
                        warn!("Failed to create stream {}: {}", stream_name, e);
                        // Return empty stream if we can't create it
                        return Ok(futures_util::stream::iter(vec![]).boxed());
                    }
                }
            }
        };

        // Create a non-durable consumer for read-only queries
        // This ensures queries are idempotent and don't consume history
        let consumer_config = jetstream::consumer::pull::Config {
            filter_subject: subject_filter.to_string(),
            durable_name: None, // Non-durable for read-only operations
            deliver_policy: DeliverPolicy::All, // Get all historical messages
            ..Default::default()
        };

        match stream.create_consumer(consumer_config).await {
            Ok(consumer) => {
                debug!("Created consumer for subject filter: {}", subject_filter);
                
                // Use batch fetch with timeout to prevent hanging on empty streams
                let batch_size = limit.unwrap_or(1000).min(1000);
                let timeout = std::time::Duration::from_secs(5);
                
                match tokio::time::timeout(timeout, consumer.batch().max_messages(batch_size).expires(std::time::Duration::from_secs(2)).messages()).await {
                    Ok(Ok(messages)) => {
                        let message_stream = futures_util::stream::iter(messages);
                        Ok(message_stream.boxed())
                    }
                    Ok(Err(e)) => {
                        warn!("Failed to fetch messages: {}", e);
                        Ok(futures_util::stream::iter(vec![]).boxed())
                    }
                    Err(_) => {
                        warn!("Timeout fetching messages from JetStream");
                        Ok(futures_util::stream::iter(vec![]).boxed())
                    }
                }
            }
            Err(e) => {
                error!("Failed to create consumer: {}", e);
                // Return empty stream on error
                Ok(futures_util::stream::iter(vec![]).boxed())
            }
        }
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
            ts_str.parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now())
        } else {
            Utc::now()
        };

        // Determine status from event type and if this is a start event
        let (status, is_start_event) = if let Some(event_type) = payload.get("event").and_then(|v| v.as_str()) {
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
            DateTime::from_timestamp(0, 0).unwrap_or_else(|| Utc::now())
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
            ts_str.parse::<DateTime<Utc>>()
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