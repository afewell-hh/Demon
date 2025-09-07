use anyhow::{Context, Result};
use async_nats::jetstream::{self, stream::Stream};
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
                            // Keep the most recent summary for each run
                            let key = format!("{}:{}", summary.ritual_id, summary.run_id);
                            if let Some(existing) = runs_map.get(&key) {
                                if summary.start_ts > existing.start_ts {
                                    runs_map.insert(key, summary);
                                }
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
    ) -> Result<impl StreamExt<Item = async_nats::jetstream::Message>> {
        // Try to get the stream - assume it exists for now
        // In a real implementation, you might want to create the stream if it doesn't exist
        
        debug!("Querying messages with subject filter: {}", subject_filter);

        // For now, return an empty stream if we can't connect
        // In a real implementation, you'd use the actual JetStream consumer API
        let messages = futures_util::stream::iter(vec![]);
        Ok(messages)
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

        // Determine status from event type
        let status = if let Some(event_type) = payload.get("event").and_then(|v| v.as_str()) {
            match event_type {
                "ritual.completed:v1" => RunStatus::Completed,
                "ritual.failed:v1" => RunStatus::Failed,
                _ => RunStatus::Running,
            }
        } else {
            RunStatus::Running
        };

        Ok(Some(RunSummary {
            run_id,
            ritual_id,
            start_ts: ts,
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
        let parts: Vec<&str> = subject.split('.').collect();
        if parts.len() >= 4 {
            Some(parts[3].to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ritual_id_from_subject() {
        let client = JetStreamClient {
            jetstream: todo!(), // This is just for testing the method
        };

        let subject = "demon.ritual.v1.my-ritual.run-123.events";
        let ritual_id = client.extract_ritual_id_from_subject(subject);
        assert_eq!(ritual_id, Some("my-ritual".to_string()));

        let invalid_subject = "demon.ritual";
        let ritual_id = client.extract_ritual_id_from_subject(invalid_subject);
        assert_eq!(ritual_id, None);
    }

    #[test]
    fn test_run_status_display() {
        assert_eq!(RunStatus::Running.to_string(), "Running");
        assert_eq!(RunStatus::Completed.to_string(), "Completed");
        assert_eq!(RunStatus::Failed.to_string(), "Failed");
    }
}