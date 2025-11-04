//! NATS JetStream consumer for scale hint events

use crate::autoscale::{AutoscaleClient, ScaleHintEvent};
use crate::config::Config;
use crate::metrics::Metrics;
use anyhow::{Context, Result};
use async_nats::jetstream::{
    self,
    consumer::{AckPolicy, DeliverPolicy, PullConsumer},
    stream::Stream,
};
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Scale hint consumer - subscribes to JetStream and processes scale hint events
pub struct ScaleHintConsumer<C: AutoscaleClient> {
    config: Config,
    autoscale_client: Arc<C>,
    metrics: Metrics,
}

impl<C: AutoscaleClient> ScaleHintConsumer<C> {
    /// Create a new scale hint consumer
    pub fn new(config: Config, autoscale_client: Arc<C>, metrics: Metrics) -> Self {
        Self {
            config,
            autoscale_client,
            metrics,
        }
    }

    /// Run the consumer loop
    pub async fn run(&self) -> Result<()> {
        info!("Starting scale hint consumer");

        // Connect to NATS
        let client = self.connect_nats().await?;
        let jetstream = jetstream::new(client);

        // Get or create stream
        let stream = self.ensure_stream(&jetstream).await?;

        // Create durable consumer
        let consumer = self.create_consumer(&stream).await?;

        info!(
            consumer_name = %self.config.consumer_name,
            subject_filter = %self.config.subject_filter(),
            "Consumer created successfully, starting message processing"
        );

        // Process messages continuously
        self.process_messages(consumer).await
    }

    /// Connect to NATS server
    async fn connect_nats(&self) -> Result<async_nats::Client> {
        info!("Connecting to NATS at {}", self.config.nats_url);

        let client = if let Some(creds_path) = &self.config.nats_creds_path {
            info!("Using credentials file: {}", creds_path);
            async_nats::ConnectOptions::new()
                .credentials_file(creds_path)
                .await
                .context("Failed to load NATS credentials")?
                .connect(&self.config.nats_url)
                .await
                .context("Failed to connect to NATS with credentials")?
        } else {
            warn!("No NATS credentials provided, connecting without auth");
            async_nats::connect(&self.config.nats_url)
                .await
                .context("Failed to connect to NATS")?
        };

        info!("Successfully connected to NATS");
        Ok(client)
    }

    /// Ensure JetStream stream exists
    async fn ensure_stream(&self, jetstream: &jetstream::Context) -> Result<Stream> {
        let stream_name = &self.config.stream_name;

        match jetstream.get_stream(stream_name).await {
            Ok(stream) => {
                info!("Found existing stream: {}", stream_name);
                Ok(stream)
            }
            Err(_) => {
                info!("Stream {} not found, creating it", stream_name);
                let stream_config = jetstream::stream::Config {
                    name: stream_name.clone(),
                    subjects: vec!["demon.scale.v1.*.hints".to_string()],
                    max_age: Duration::from_secs(3600 * 24 * 7), // Retain for 7 days
                    ..Default::default()
                };

                let stream = jetstream
                    .get_or_create_stream(stream_config)
                    .await
                    .context("Failed to create JetStream stream")?;

                info!("Successfully created stream: {}", stream_name);
                Ok(stream)
            }
        }
    }

    /// Create durable JetStream consumer
    async fn create_consumer(&self, stream: &Stream) -> Result<PullConsumer> {
        let consumer_config = jetstream::consumer::pull::Config {
            durable_name: Some(self.config.consumer_name.clone()),
            filter_subject: self.config.subject_filter(),
            deliver_policy: DeliverPolicy::All,
            ack_policy: AckPolicy::Explicit, // Require explicit ack
            ack_wait: Duration::from_secs(30),
            max_deliver: 5, // Max 5 delivery attempts before moving to dead letter
            ..Default::default()
        };

        let consumer = stream
            .get_or_create_consumer(&self.config.consumer_name, consumer_config)
            .await
            .context("Failed to create consumer")?;

        Ok(consumer)
    }

    /// Process messages continuously
    async fn process_messages(&self, consumer: PullConsumer) -> Result<()> {
        const BATCH_SIZE: usize = 10;
        const BATCH_TIMEOUT_SECS: u64 = 30;

        loop {
            let mut messages = consumer
                .batch()
                .max_messages(BATCH_SIZE)
                .expires(Duration::from_secs(BATCH_TIMEOUT_SECS))
                .messages()
                .await
                .context("Failed to fetch message batch")?;

            let mut batch_count = 0;

            while let Some(msg_result) = messages.next().await {
                match msg_result {
                    Ok(msg) => {
                        batch_count += 1;
                        self.handle_message(msg).await;
                    }
                    Err(e) => {
                        error!("Error receiving message: {}", e);
                        self.metrics.record_error("receive_error", "unknown");
                    }
                }
            }

            if batch_count > 0 {
                debug!("Processed batch of {} messages", batch_count);
            }

            // Small delay between batches to prevent tight-looping
            if batch_count == 0 {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    /// Handle a single message
    async fn handle_message(&self, msg: async_nats::jetstream::Message) {
        let subject = msg.subject.clone();
        let payload = msg.payload.clone();

        debug!(
            subject = %subject,
            payload_size = payload.len(),
            "Processing scale hint message"
        );

        // Parse event
        let event: ScaleHintEvent = match serde_json::from_slice(&payload) {
            Ok(ev) => ev,
            Err(e) => {
                error!(
                    subject = %subject,
                    error = %e,
                    "Failed to deserialize scale hint event"
                );
                self.metrics.record_error("deserialization", "unknown");
                // Ack malformed messages to avoid redelivery
                if let Err(ack_err) = msg.ack().await {
                    error!("Failed to ack malformed message: {}", ack_err);
                }
                return;
            }
        };

        let tenant_id = &event.tenant_id;
        let recommendation = format!("{:?}", event.recommendation).to_lowercase();

        // Record metrics
        self.metrics
            .record_recommendation(&recommendation, tenant_id);
        self.metrics.update_gauges(
            event.metrics.queue_lag,
            event.metrics.p95_latency_ms,
            event.metrics.error_rate,
            tenant_id,
        );

        // Handle scale hint with retry
        let mut last_error = None;
        for attempt in 0..=self.config.max_retry_attempts {
            if attempt > 0 {
                let backoff =
                    Duration::from_millis(self.config.retry_backoff_ms * (2_u64.pow(attempt - 1)));
                warn!(
                    attempt = attempt,
                    backoff_ms = backoff.as_millis(),
                    tenant_id = %tenant_id,
                    "Retrying autoscale handler after backoff"
                );
                tokio::time::sleep(backoff).await;
            }

            match self.autoscale_client.handle_scale_hint(&event).await {
                Ok(()) => {
                    self.metrics.record_autoscale_call(true, tenant_id);
                    // Successfully processed, ack the message
                    if let Err(e) = msg.ack().await {
                        error!("Failed to ack message: {}", e);
                    } else {
                        debug!(
                            tenant_id = %tenant_id,
                            recommendation = %recommendation,
                            "Successfully processed and acked scale hint"
                        );
                    }
                    return;
                }
                Err(e) => {
                    warn!(
                        attempt = attempt + 1,
                        max_attempts = self.config.max_retry_attempts + 1,
                        tenant_id = %tenant_id,
                        error = %e,
                        "Autoscale handler failed"
                    );
                    last_error = Some(e);
                }
            }
        }

        // All retries exhausted
        error!(
            tenant_id = %tenant_id,
            recommendation = %recommendation,
            error = ?last_error,
            "Exhausted all retry attempts for scale hint"
        );
        self.metrics.record_autoscale_call(false, tenant_id);
        self.metrics.record_error("autoscale_exhausted", tenant_id);

        // Nak the message to requeue (JetStream will respect max_deliver)
        if let Err(e) = msg
            .ack_with(async_nats::jetstream::AckKind::Nak(None))
            .await
        {
            error!("Failed to nak message: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autoscale::LogOnlyAutoscaleClient;

    #[test]
    fn test_consumer_creation() {
        let config = Config {
            nats_url: "nats://localhost:4222".to_string(),
            nats_creds_path: None,
            stream_name: "SCALE_HINTS".to_string(),
            tenant_filter: None,
            dry_run: true,
            autoscale_endpoint: None,
            log_json: false,
            metrics_port: 9090,
            consumer_name: "test-consumer".to_string(),
            retry_backoff_ms: 1000,
            max_retry_attempts: 3,
            autoscale_timeout_secs: 10,
        };

        let client = Arc::new(LogOnlyAutoscaleClient);
        let metrics = Metrics;

        let consumer = ScaleHintConsumer::new(config, client, metrics);
        assert_eq!(consumer.config.consumer_name, "test-consumer");
    }
}
