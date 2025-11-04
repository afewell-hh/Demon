//! Scale hint telemetry simulation tool
//!
//! Generates sample scale hint events and can optionally publish to NATS
//!
//! Usage:
//!   cargo run -p runtime --example simulate_scale_hints
//!   NATS_URL=nats://127.0.0.1:4222 cargo run -p runtime --example simulate_scale_hints --publish

use runtime::telemetry::{RuntimeMetrics, ScaleHintConfig, ScaleHintEmitter};
use std::env;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    info!("Scale Hint Telemetry Simulation Tool");
    info!("=====================================");

    let publish = env::args().any(|arg| arg == "--publish");

    // Connect to NATS if --publish flag is set
    let js_context = if publish {
        let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
        info!("Connecting to NATS at {}", nats_url);

        match async_nats::connect(&nats_url).await {
            Ok(client) => {
                info!("Connected to NATS successfully");
                Some(async_nats::jetstream::new(client))
            }
            Err(e) => {
                warn!("Failed to connect to NATS: {}. Running in dry-run mode.", e);
                None
            }
        }
    } else {
        info!("Running in dry-run mode (use --publish to emit to NATS)");
        None
    };

    // Configure scale hints with custom test values
    let mut config = ScaleHintConfig::from_env();
    config.enabled = true;

    info!("Configuration:");
    info!("  Queue Lag High: {}", config.queue_lag_high);
    info!("  Queue Lag Low: {}", config.queue_lag_low);
    info!("  P95 Latency High: {}ms", config.p95_latency_high_ms);
    info!("  P95 Latency Low: {}ms", config.p95_latency_low_ms);
    info!("  Error Rate High: {}", config.error_rate_high);
    info!(
        "  Min Signals for Transition: {}",
        config.min_signals_for_transition
    );
    info!("");

    let emitter = ScaleHintEmitter::new(
        config.clone(),
        js_context,
        env::var("TENANT_ID").unwrap_or_else(|_| "simulation".to_string()),
    );

    // Scenario 1: Normal operation
    info!("Scenario 1: Normal operation (steady state)");
    for i in 1..=3 {
        let metrics = RuntimeMetrics {
            queue_lag: 100,
            p95_latency_ms: 250.0,
            error_rate: 0.01,
            total_processed: 1000 * i,
            total_errors: 10 * i,
        };

        info!(
            "  Iteration {}: queue_lag={}, p95={}ms, error_rate={:.3}",
            i, metrics.queue_lag, metrics.p95_latency_ms, metrics.error_rate
        );

        let result = emitter.evaluate_and_emit(metrics).await?;
        if let Some(subject) = result {
            info!("    ✓ Published to: {}", subject);
        } else {
            info!("    - Not published (dry-run or steady state)");
        }

        sleep(Duration::from_millis(500)).await;
    }
    info!("");

    // Scenario 2: Gradual pressure increase leading to scale-up
    info!("Scenario 2: Gradual pressure increase (should trigger scale-up)");
    let pressure_metrics = [
        (400, 800.0, 0.03),
        (550, 1050.0, 0.04),
        (600, 1200.0, 0.06),
        (750, 1400.0, 0.07),
    ];

    for (i, (lag, latency, err_rate)) in pressure_metrics.iter().enumerate() {
        let metrics = RuntimeMetrics {
            queue_lag: *lag,
            p95_latency_ms: *latency,
            error_rate: *err_rate,
            total_processed: 1000,
            total_errors: (*err_rate * 1000.0) as u64,
        };

        info!(
            "  Iteration {}: queue_lag={}, p95={}ms, error_rate={:.3}",
            i + 1,
            metrics.queue_lag,
            metrics.p95_latency_ms,
            metrics.error_rate
        );

        let result = emitter.evaluate_and_emit(metrics).await?;
        if let Some(subject) = result {
            info!("    ✓ Published to: {}", subject);
        } else {
            info!("    - Not published (dry-run or steady state)");
        }

        sleep(Duration::from_millis(500)).await;
    }
    info!("");

    // Scenario 3: Recovery leading to scale-down
    info!("Scenario 3: Recovery and low utilization (should trigger scale-down)");
    let recovery_metrics = [
        (200, 400.0, 0.02),
        (80, 150.0, 0.01),
        (30, 60.0, 0.005),
        (20, 40.0, 0.002),
        (15, 35.0, 0.001),
        (10, 30.0, 0.001),
        (10, 30.0, 0.001),
    ];

    for (i, (lag, latency, err_rate)) in recovery_metrics.iter().enumerate() {
        let metrics = RuntimeMetrics {
            queue_lag: *lag,
            p95_latency_ms: *latency,
            error_rate: *err_rate,
            total_processed: 1000,
            total_errors: (*err_rate * 1000.0) as u64,
        };

        info!(
            "  Iteration {}: queue_lag={}, p95={}ms, error_rate={:.3}",
            i + 1,
            metrics.queue_lag,
            metrics.p95_latency_ms,
            metrics.error_rate
        );

        let result = emitter.evaluate_and_emit(metrics).await?;
        if let Some(subject) = result {
            info!("    ✓ Published to: {}", subject);
        } else {
            info!("    - Not published (dry-run or steady state)");
        }

        sleep(Duration::from_millis(500)).await;
    }
    info!("");

    info!("Simulation complete!");
    info!("");

    if !publish {
        info!("To publish events to NATS, run:");
        info!("  NATS_URL=nats://127.0.0.1:4222 cargo run -p runtime --example simulate_scale_hints -- --publish");
    } else {
        info!("Events published to NATS. Check the stream with:");
        info!("  nats stream ls");
        info!("  nats sub 'demon.scale.v1.>'");
    }

    Ok(())
}
