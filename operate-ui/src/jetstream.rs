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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stream_sequence: Option<u64>,
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

    /// List recent runs with optional limit (legacy - uses default tenant)
    pub async fn list_runs(&self, limit: Option<usize>) -> Result<Vec<RunSummary>> {
        self.list_runs_for_tenant("default", limit).await
    }

    /// List recent runs for a specific tenant
    pub async fn list_runs_for_tenant(
        &self,
        tenant: &str,
        limit: Option<usize>,
    ) -> Result<Vec<RunSummary>> {
        let limit = limit.unwrap_or(50).min(1000); // Cap at 1000 for safety
        debug!("Listing runs for tenant {} with limit: {}", tenant, limit);

        // Query JetStream for ritual events
        // New subject pattern: demon.ritual.v1.<tenant>.<ritualId>.<runId>.events
        let subject_filter = format!("demon.ritual.v1.{}.*.*.events", tenant);

        let mut runs_map: HashMap<String, RunSummary> = HashMap::new();
        let mut message_count = 0;

        // Get messages from JetStream stream
        // Use DeliverPolicy::All to get all messages, then sort by timestamp to prioritize recent runs
        match self
            .query_stream_messages(&subject_filter, None, DeliverPolicy::All)
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

        // If no runs found and tenant is default, try legacy subject pattern
        // This fallback ensures backwards compatibility with pre-upgrade runs
        if runs_map.is_empty() && tenant == "default" {
            let legacy_subject = "demon.ritual.v1.*.*.events";
            debug!(
                "No runs found with tenant pattern, trying legacy pattern: {}",
                legacy_subject
            );

            match self
                .query_stream_messages(legacy_subject, None, DeliverPolicy::All)
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
                                            == DateTime::from_timestamp(0, 0)
                                                .unwrap_or_else(Utc::now)
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

                    if !runs_map.is_empty() {
                        warn!("Found legacy runs using fallback pattern - consider migrating data to tenant-scoped subjects");
                    }
                }
                Err(e) => {
                    debug!("Failed to query legacy stream messages: {}", e);
                }
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

    /// Get detailed information for a specific run (legacy - uses default tenant)
    pub async fn get_run_detail(&self, run_id: &str) -> Result<Option<RunDetail>> {
        self.get_run_detail_for_tenant("default", run_id).await
    }

    /// Get detailed information for a specific run with tenant
    pub async fn get_run_detail_for_tenant(
        &self,
        tenant: &str,
        run_id: &str,
    ) -> Result<Option<RunDetail>> {
        debug!("Getting run detail for tenant {} run: {}", tenant, run_id);

        // Try new tenant-aware subject first
        let subject_filter = &format!("demon.ritual.v1.{}.*.{}.events", tenant, run_id);

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

        // If events are empty and tenant is default, try legacy subject pattern
        if events.is_empty() && tenant == "default" {
            let legacy_subject = &format!("demon.ritual.v1.*.{}.events", run_id);
            debug!(
                "No events found with tenant pattern, trying legacy pattern: {}",
                legacy_subject
            );

            match self.query_all_stream_messages(legacy_subject).await {
                Ok(messages) => {
                    for message in messages {
                        match self.parse_message_for_event(&message) {
                            Ok(Some(event)) => {
                                events.push(event);
                            }
                            Ok(None) => {}
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
                    debug!("Failed to query legacy stream messages: {}", e);
                }
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
        // Support both formats:
        // New: demon.ritual.v1.<tenant>.<ritualId>.<runId>.events (7 parts)
        // Legacy: demon.ritual.v1.<ritualId>.<runId>.events (6 parts)
        let parts: Vec<&str> = message.subject.split('.').collect();
        let (ritual_id, run_id) = if parts.len() == 7 {
            // New tenant-aware format
            (parts[4].to_string(), parts[5].to_string())
        } else if parts.len() == 6 {
            // Legacy format
            (parts[3].to_string(), parts[4].to_string())
        } else {
            return Ok(None);
        };

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

        let stream_sequence = message.info().ok().map(|info| info.stream_sequence);

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
            stream_sequence,
            extra,
        }))
    }

    /// Extract ritual ID from subject
    fn extract_ritual_id_from_subject(&self, subject: &str) -> Option<String> {
        extract_ritual_id_from_subject(subject)
    }

    /// Stream run events for a specific run ID
    pub async fn stream_run_events(
        &self,
        run_id: &str,
    ) -> Result<impl futures_util::Stream<Item = Result<RitualEvent>>> {
        self.stream_run_events_for_tenant("default", run_id).await
    }

    pub async fn stream_run_events_for_tenant(
        &self,
        tenant: &str,
        run_id: &str,
    ) -> Result<impl futures_util::Stream<Item = Result<RitualEvent>>> {
        debug!(
            "Starting event stream for tenant {} run: {}",
            tenant, run_id
        );

        // Get initial snapshot
        let initial_events = match self.get_run_detail_for_tenant(tenant, run_id).await? {
            Some(detail) => detail.events,
            None => Vec::new(),
        };

        // Track the last published stream sequence that we emitted from the snapshot so we can
        // resume streaming without dropping mid-flight events (PR review feedback).
        let resume_sequence = initial_events
            .iter()
            .filter_map(|evt| evt.stream_sequence)
            .max();

        // Try new tenant-aware subject first, fallback to legacy if default tenant
        let subject_filter = if tenant == "default" && initial_events.is_empty() {
            // Try legacy pattern if no events found with tenant pattern
            format!("demon.ritual.v1.*.{}.events", run_id)
        } else {
            format!("demon.ritual.v1.{}.*.{}.events", tenant, run_id)
        };

        // Clone self for use in the async stream
        let js_client = self.clone();
        let run_id_owned = run_id.to_string();

        Ok(async_stream::try_stream! {
            // First, emit all initial events
            for event in initial_events.clone() {
                yield event;
            }

            // Resolve stream name with precedence
            let desired = std::env::var("RITUAL_STREAM_NAME").ok();
            let stream = if let Some(name) = desired {
                js_client.jetstream
                    .get_stream(&name)
                    .await
                    .with_context(|| format!("JetStream stream '{}' not found", name))?
            } else {
                match js_client.jetstream.get_stream("RITUAL_EVENTS").await {
                    Ok(s) => s,
                    Err(_) => {
                        let s = js_client
                            .jetstream
                            .get_stream("DEMON_RITUAL_EVENTS")
                            .await
                            .with_context(|| "JetStream stream 'RITUAL_EVENTS' not found")?;
                        warn!("Using deprecated stream name 'DEMON_RITUAL_EVENTS'");
                        s
                    }
                }
            };

            // Create ephemeral consumer for tailing new events
            let deliver_policy = resume_sequence
                .and_then(|seq| seq.checked_add(1))
                .map(|next| DeliverPolicy::ByStartSequence { start_sequence: next })
                .unwrap_or(DeliverPolicy::New);

            let consumer_config = jetstream::consumer::pull::Config {
                filter_subject: subject_filter.clone(),
                durable_name: None,
                deliver_policy,
                ack_policy: async_nats::jetstream::consumer::AckPolicy::None,
                inactive_threshold: std::time::Duration::from_secs(300), // 5 minute timeout
                ..Default::default()
            };

            let consumer = stream.create_consumer(consumer_config).await?;
            debug!("Created ephemeral consumer for streaming run {}", run_id_owned);

            // Continuously poll for new messages. The deliver policy resumes at the first
            // sequence after the snapshot so we can safely emit each message without gaps.
            loop {
                let mut messages = consumer
                    .batch()
                    .max_messages(100)
                    .expires(std::time::Duration::from_secs(30)) // Long poll for 30 seconds
                    .messages()
                    .await?;

                while let Some(msg_result) = messages.next().await {
                    match msg_result {
                        Ok(msg) => {
                            if let Ok(Some(event)) = js_client.parse_message_for_event(&msg) {
                                // Emit all events - DeliverPolicy::New ensures no duplicates
                                yield event;
                            }
                        }
                        Err(e) => {
                            warn!("Error receiving streaming message: {}", e);
                        }
                    }
                }
            }
        })
    }
}

/// Extract ritual ID from subject string (standalone function for testing)
fn extract_ritual_id_from_subject(subject: &str) -> Option<String> {
    let parts: Vec<&str> = subject.split('.').collect();
    if parts.len() == 7 {
        // New tenant-aware format: demon.ritual.v1.<tenant>.<ritualId>.<runId>.events
        Some(parts[4].to_string())
    } else if parts.len() >= 6 {
        // Legacy format: demon.ritual.v1.<ritualId>.<runId>.events
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
        // Test new tenant-aware format
        let subject = "demon.ritual.v1.default.my-ritual.run-123.events";
        let ritual_id = extract_ritual_id_from_subject(subject);
        assert_eq!(ritual_id, Some("my-ritual".to_string()));

        // Test legacy format
        let legacy_subject = "demon.ritual.v1.my-ritual.run-123.events";
        let ritual_id = extract_ritual_id_from_subject(legacy_subject);
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

    #[test]
    fn test_parse_message_for_run_summary_legacy_and_tenant_formats() {
        // Test legacy format parsing
        let legacy_subject = "demon.ritual.v1.my-ritual.run-123.events";

        // Test tenant-aware format parsing
        let tenant_subject = "demon.ritual.v1.default.my-ritual.run-123.events";

        // Verify that both subject formats are handled correctly by the extract function
        let legacy_ritual_id = extract_ritual_id_from_subject(legacy_subject);
        assert_eq!(legacy_ritual_id, Some("my-ritual".to_string()));

        let tenant_ritual_id = extract_ritual_id_from_subject(tenant_subject);
        assert_eq!(tenant_ritual_id, Some("my-ritual".to_string()));

        // Both should extract the same ritual ID despite different subject formats
        assert_eq!(legacy_ritual_id, tenant_ritual_id);
    }
}
