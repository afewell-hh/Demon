//! Scale Hint Handler binary - consumes scale hint events and triggers autoscale actions

use scale_hint_handler::{
    AutoscaleClient, Config, HttpAutoscaleClient, LogOnlyAutoscaleClient, Metrics,
    ScaleHintConsumer,
};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse configuration
    let config = Config::parse_config();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    info!("Starting Demon Scale Hint Handler");
    info!("Configuration:");
    info!("  NATS URL: {}", config.nats_url);
    info!("  Stream: {}", config.stream_name);
    info!("  Consumer: {}", config.consumer_name);
    info!("  Subject filter: {}", config.subject_filter());
    info!("  Dry-run: {}", config.dry_run);
    info!("  Metrics port: {}", config.metrics_port);

    // Initialize Prometheus metrics
    Metrics::init(config.metrics_port)?;

    let metrics = Metrics;

    // Create autoscale client based on configuration
    if config.has_autoscale_endpoint() {
        let endpoint = config.autoscale_endpoint.clone().unwrap();
        info!("Using HTTP autoscale client with endpoint: {}", endpoint);

        let autoscale_client = Arc::new(HttpAutoscaleClient::new(
            endpoint,
            config.autoscale_timeout_secs,
            config.max_retry_attempts,
            config.retry_backoff_ms,
        )?);

        run_consumer(config, autoscale_client, metrics).await
    } else {
        info!("Using log-only autoscale client (dry-run mode)");
        let autoscale_client = Arc::new(LogOnlyAutoscaleClient);
        run_consumer(config, autoscale_client, metrics).await
    }
}

/// Run the consumer with the specified autoscale client
async fn run_consumer<C: AutoscaleClient + 'static>(
    config: Config,
    autoscale_client: Arc<C>,
    metrics: Metrics,
) -> anyhow::Result<()> {
    let consumer = ScaleHintConsumer::new(config, autoscale_client, metrics);

    match consumer.run().await {
        Ok(()) => {
            info!("Scale hint consumer exited normally");
            Ok(())
        }
        Err(e) => {
            error!("Scale hint consumer failed: {}", e);
            Err(e)
        }
    }
}
