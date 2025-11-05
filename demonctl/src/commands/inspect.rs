//! Inspect command - surface DAG metrics and scale hints

use anyhow::{Context, Result};
use async_nats::jetstream::{self, consumer::DeliverPolicy};
use clap::Args;
use futures_util::StreamExt;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::env;
use tabled::{settings::style::Style, Table, Tabled};

#[derive(Args, Debug)]
pub struct InspectArgs {
    /// Show graph metrics (DAG nodes, lag, latency, errors)
    #[arg(long)]
    pub graph: bool,

    /// Output machine-readable JSON
    #[arg(long)]
    pub json: bool,

    /// Tenant ID (default: from DEMON_TENANT env var or "default")
    #[arg(long, env = "DEMON_TENANT", default_value = "default")]
    pub tenant: String,

    /// NATS URL (default: from NATS_URL env var or "nats://localhost:4222")
    #[arg(long, env = "NATS_URL", default_value = "nats://localhost:4222")]
    pub nats_url: String,

    /// Queue lag threshold for WARNING status
    #[arg(long, env = "INSPECT_WARN_QUEUE_LAG", default_value = "300")]
    pub warn_queue_lag: u64,

    /// Queue lag threshold for ERROR status
    #[arg(long, env = "INSPECT_ERROR_QUEUE_LAG", default_value = "500")]
    pub error_queue_lag: u64,

    /// P95 latency threshold (ms) for WARNING status
    #[arg(long, env = "INSPECT_WARN_P95_LATENCY_MS", default_value = "500.0")]
    pub warn_p95_latency_ms: f64,

    /// P95 latency threshold (ms) for ERROR status
    #[arg(long, env = "INSPECT_ERROR_P95_LATENCY_MS", default_value = "1000.0")]
    pub error_p95_latency_ms: f64,

    /// Error rate threshold for WARNING status
    #[arg(long, env = "INSPECT_WARN_ERROR_RATE", default_value = "0.02")]
    pub warn_error_rate: f64,

    /// Error rate threshold for ERROR status
    #[arg(long, env = "INSPECT_ERROR_ERROR_RATE", default_value = "0.05")]
    pub error_error_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Ok,
    Warn,
    Error,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Ok => write!(f, "OK"),
            Status::Warn => write!(f, "WARN"),
            Status::Error => write!(f, "ERROR"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScaleHintEvent {
    event: String,
    ts: String,
    tenant_id: String,
    recommendation: String,
    metrics: Metrics,
    thresholds: Thresholds,
    hysteresis: Hysteresis,
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metrics {
    queue_lag: u64,
    p95_latency_ms: f64,
    error_rate: f64,
    total_processed: u64,
    total_errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Thresholds {
    queue_lag_high: u64,
    queue_lag_low: u64,
    p95_latency_high_ms: f64,
    p95_latency_low_ms: f64,
    error_rate_high: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Hysteresis {
    current_state: String,
    state_changed_at: Option<String>,
    consecutive_high_signals: u32,
    consecutive_low_signals: u32,
    min_signals_for_transition: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GraphMetrics {
    tenant: String,
    subject: String,
    status: Status,
    queue_lag: u64,
    p95_latency_ms: f64,
    error_rate: f64,
    total_processed: u64,
    total_errors: u64,
    last_error: Option<String>,
    recommendation: String,
    reason: String,
    timestamp: String,
}

#[derive(Debug, Tabled)]
struct GraphRow {
    #[tabled(rename = "TENANT")]
    tenant: String,
    #[tabled(rename = "STATUS")]
    status: String,
    #[tabled(rename = "QUEUE LAG")]
    queue_lag: String,
    #[tabled(rename = "P95 LATENCY")]
    p95_latency: String,
    #[tabled(rename = "ERROR RATE")]
    error_rate: String,
    #[tabled(rename = "TOTAL PROCESSED")]
    total_processed: String,
    #[tabled(rename = "RECOMMENDATION")]
    recommendation: String,
}

impl InspectArgs {
    fn classify_status(&self, metrics: &Metrics) -> Status {
        // ERROR if any metric exceeds ERROR threshold
        if metrics.queue_lag >= self.error_queue_lag
            || metrics.p95_latency_ms >= self.error_p95_latency_ms
            || metrics.error_rate >= self.error_error_rate
        {
            return Status::Error;
        }

        // WARN if any metric exceeds WARN threshold
        if metrics.queue_lag >= self.warn_queue_lag
            || metrics.p95_latency_ms >= self.warn_p95_latency_ms
            || metrics.error_rate >= self.warn_error_rate
        {
            return Status::Warn;
        }

        Status::Ok
    }

    fn should_use_color() -> bool {
        // Check NO_COLOR env var and TTY status
        if env::var("NO_COLOR").is_ok() {
            return false;
        }
        atty::is(atty::Stream::Stdout)
    }

    fn format_latency(latency_ms: f64) -> String {
        if latency_ms >= 1000.0 {
            format!("{:.2}s", latency_ms / 1000.0)
        } else {
            format!("{:.1}ms", latency_ms)
        }
    }

    fn colorize_status(status: &Status, text: &str) -> String {
        if !Self::should_use_color() {
            return text.to_string();
        }

        match status {
            Status::Ok => text.green().to_string(),
            Status::Warn => text.yellow().to_string(),
            Status::Error => text.red().to_string(),
        }
    }
}

pub async fn run(args: InspectArgs) -> Result<()> {
    if !args.graph {
        anyhow::bail!(
            "No inspection target specified. Use --graph to inspect graph metrics.\n\
             Example: demonctl inspect --graph"
        );
    }

    // Connect to NATS and fetch scale hint data
    let client = match async_nats::connect(&args.nats_url).await {
        Ok(c) => c,
        Err(e) => {
            if args.json {
                println!(
                    "{{\"error\": \"Failed to connect to NATS: {}. Verify NATS_URL is correct.\"}}",
                    e
                );
            } else {
                eprintln!("Error: Failed to connect to NATS: {}", e);
                eprintln!("\nTroubleshooting:");
                eprintln!("1. Verify NATS server is running and accessible");
                eprintln!("2. Check that NATS_URL is correct: {}", args.nats_url);
                eprintln!("3. Ensure network connectivity to NATS server");
            }
            std::process::exit(2);
        }
    };

    let jetstream = async_nats::jetstream::new(client);

    // Query the latest scale hint for the tenant
    let subject = format!("demon.scale.v1.{}.hints", args.tenant);
    let stream_name = "SCALE_HINTS";

    // Try to fetch the latest message from the stream
    let scale_hint = match fetch_latest_scale_hint(&jetstream, stream_name, &subject).await {
        Ok(hint) => hint,
        Err(e) => {
            if args.json {
                println!("{{\"error\": \"Scale hints unavailable: {}. Ensure SCALE_HINT_ENABLED=true and the runtime is emitting scale hints.\"}}", e);
                std::process::exit(2);
            }
            eprintln!("Error: Scale hints unavailable: {}", e);
            eprintln!("\nTroubleshooting:");
            eprintln!("1. Ensure the runtime is running with SCALE_HINT_ENABLED=true");
            eprintln!(
                "2. Check that NATS JetStream stream '{}' exists",
                stream_name
            );
            eprintln!("3. Verify NATS_URL is correct: {}", args.nats_url);
            std::process::exit(2);
        }
    };

    let scale_hint = match scale_hint {
        Some(hint) => hint,
        None => {
            if args.json {
                println!("{{\"error\": \"No scale hints found for tenant '{}'. Ensure the runtime is emitting scale hints.\"}}", args.tenant);
                std::process::exit(2);
            }
            eprintln!("Error: No scale hints found for tenant '{}'", args.tenant);
            eprintln!("\nTroubleshooting:");
            eprintln!("1. Ensure the runtime is running and processing workload");
            eprintln!("2. Check that scale hints are being emitted (not in steady state)");
            eprintln!("3. Verify tenant ID is correct: {}", args.tenant);
            std::process::exit(2);
        }
    };

    // Classify status based on thresholds
    let status = args.classify_status(&scale_hint.metrics);

    // Build metrics structure
    let graph_metrics = GraphMetrics {
        tenant: args.tenant.clone(),
        subject: subject.clone(),
        status: status.clone(),
        queue_lag: scale_hint.metrics.queue_lag,
        p95_latency_ms: scale_hint.metrics.p95_latency_ms,
        error_rate: scale_hint.metrics.error_rate,
        total_processed: scale_hint.metrics.total_processed,
        total_errors: scale_hint.metrics.total_errors,
        last_error: None, // Not available in scale hint events
        recommendation: scale_hint.recommendation,
        reason: scale_hint.reason,
        timestamp: scale_hint.ts,
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&graph_metrics)?);
        return Ok(());
    }

    // Table output
    let row = GraphRow {
        tenant: graph_metrics.tenant.clone(),
        status: InspectArgs::colorize_status(&status, &status.to_string()),
        queue_lag: graph_metrics.queue_lag.to_string(),
        p95_latency: InspectArgs::format_latency(graph_metrics.p95_latency_ms),
        error_rate: format!("{:.2}%", graph_metrics.error_rate * 100.0),
        total_processed: graph_metrics.total_processed.to_string(),
        recommendation: match graph_metrics.recommendation.as_str() {
            "scale_up" => {
                if InspectArgs::should_use_color() {
                    "⬆ Scale Up".yellow().to_string()
                } else {
                    "Scale Up".to_string()
                }
            }
            "scale_down" => {
                if InspectArgs::should_use_color() {
                    "⬇ Scale Down".blue().to_string()
                } else {
                    "Scale Down".to_string()
                }
            }
            "steady" => {
                if InspectArgs::should_use_color() {
                    "➡ Steady".green().to_string()
                } else {
                    "Steady".to_string()
                }
            }
            other => other.to_string(),
        },
    };

    let mut table = Table::new(vec![row]);
    table.with(Style::rounded());

    println!("\n{}", table);
    println!("\nReason: {}", graph_metrics.reason);
    println!("Last Updated: {}", graph_metrics.timestamp);
    println!(
        "\nTotal Errors: {} / {} ({:.2}%)",
        graph_metrics.total_errors,
        graph_metrics.total_processed,
        graph_metrics.error_rate * 100.0
    );

    if status == Status::Warn || status == Status::Error {
        println!(
            "\nStatus: {}",
            InspectArgs::colorize_status(&status, &status.to_string())
        );
    }

    Ok(())
}

async fn fetch_latest_scale_hint(
    jetstream: &jetstream::Context,
    stream_name: &str,
    subject_filter: &str,
) -> Result<Option<ScaleHintEvent>> {
    // Get the stream
    let stream = jetstream
        .get_stream(stream_name)
        .await
        .context(format!("Failed to get stream '{}'", stream_name))?;

    // Create ephemeral consumer to get the latest message per subject
    let consumer_config = jetstream::consumer::pull::Config {
        filter_subject: subject_filter.to_string(),
        durable_name: None,
        deliver_policy: DeliverPolicy::LastPerSubject,
        ack_policy: async_nats::jetstream::consumer::AckPolicy::None,
        inactive_threshold: std::time::Duration::from_secs(60),
        ..Default::default()
    };

    let consumer = stream
        .create_consumer(consumer_config)
        .await
        .context("Failed to create ephemeral consumer")?;

    // Fetch the latest message
    let mut messages = consumer
        .batch()
        .max_messages(1)
        .expires(std::time::Duration::from_secs(2))
        .messages()
        .await
        .context("Failed to fetch scale hint messages")?;

    if let Some(msg_result) = messages.next().await {
        let msg = msg_result
            .map_err(|e| anyhow::anyhow!("Failed to receive scale hint message: {}", e))?;
        let event: ScaleHintEvent = serde_json::from_slice(&msg.message.payload)
            .context("Failed to parse scale hint event")?;
        Ok(Some(event))
    } else {
        // No message found
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_status_ok() {
        let args = InspectArgs {
            graph: true,
            json: false,
            tenant: "test".to_string(),
            nats_url: "nats://localhost:4222".to_string(),
            warn_queue_lag: 300,
            error_queue_lag: 500,
            warn_p95_latency_ms: 500.0,
            error_p95_latency_ms: 1000.0,
            warn_error_rate: 0.02,
            error_error_rate: 0.05,
        };

        let metrics = Metrics {
            queue_lag: 100,
            p95_latency_ms: 200.0,
            error_rate: 0.01,
            total_processed: 1000,
            total_errors: 10,
        };

        assert_eq!(args.classify_status(&metrics), Status::Ok);
    }

    #[test]
    fn test_classify_status_warn() {
        let args = InspectArgs {
            graph: true,
            json: false,
            tenant: "test".to_string(),
            nats_url: "nats://localhost:4222".to_string(),
            warn_queue_lag: 300,
            error_queue_lag: 500,
            warn_p95_latency_ms: 500.0,
            error_p95_latency_ms: 1000.0,
            warn_error_rate: 0.02,
            error_error_rate: 0.05,
        };

        let metrics = Metrics {
            queue_lag: 350,
            p95_latency_ms: 200.0,
            error_rate: 0.01,
            total_processed: 1000,
            total_errors: 10,
        };

        assert_eq!(args.classify_status(&metrics), Status::Warn);
    }

    #[test]
    fn test_classify_status_error() {
        let args = InspectArgs {
            graph: true,
            json: false,
            tenant: "test".to_string(),
            nats_url: "nats://localhost:4222".to_string(),
            warn_queue_lag: 300,
            error_queue_lag: 500,
            warn_p95_latency_ms: 500.0,
            error_p95_latency_ms: 1000.0,
            warn_error_rate: 0.02,
            error_error_rate: 0.05,
        };

        let metrics = Metrics {
            queue_lag: 600,
            p95_latency_ms: 1200.0,
            error_rate: 0.06,
            total_processed: 1000,
            total_errors: 60,
        };

        assert_eq!(args.classify_status(&metrics), Status::Error);
    }

    #[test]
    fn test_format_latency() {
        assert_eq!(InspectArgs::format_latency(50.0), "50.0ms");
        assert_eq!(InspectArgs::format_latency(500.5), "500.5ms");
        assert_eq!(InspectArgs::format_latency(1500.0), "1.50s");
        assert_eq!(InspectArgs::format_latency(2345.6), "2.35s");
    }
}
