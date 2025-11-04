//! Configuration for the scale hint handler service

use clap::Parser;

/// Configuration for scale hint handler
#[derive(Debug, Clone, Parser)]
#[command(name = "demon-scale-hint-handler")]
#[command(about = "Consumes agent.scale.hint events and triggers autoscale actions")]
pub struct Config {
    /// NATS server URL
    #[arg(long, env, default_value = "nats://localhost:4222")]
    pub nats_url: String,

    /// Path to NATS credentials file
    #[arg(long, env)]
    pub nats_creds_path: Option<String>,

    /// JetStream stream name
    #[arg(long, env, default_value = "SCALE_HINTS")]
    pub stream_name: String,

    /// Tenant ID filter (if specified, only consume events for this tenant)
    #[arg(long, env)]
    pub tenant_filter: Option<String>,

    /// Dry-run mode (log only, no actual autoscale calls)
    #[arg(long, env, default_value = "true")]
    pub dry_run: bool,

    /// Autoscale endpoint URL (HTTP POST endpoint for scale actions)
    #[arg(long, env)]
    pub autoscale_endpoint: Option<String>,

    /// Output logs in JSON format
    #[arg(long, env, default_value = "false")]
    pub log_json: bool,

    /// Prometheus metrics port
    #[arg(long, env, default_value = "9090")]
    pub metrics_port: u16,

    /// Consumer name (for durable JetStream consumer)
    #[arg(long, env, default_value = "scale-hint-handler")]
    pub consumer_name: String,

    /// Backoff retry delay in milliseconds
    #[arg(long, env, default_value = "1000")]
    pub retry_backoff_ms: u64,

    /// Maximum retry attempts for failed autoscale calls
    #[arg(long, env, default_value = "3")]
    pub max_retry_attempts: u32,

    /// Autoscale API timeout in seconds
    #[arg(long, env, default_value = "10")]
    pub autoscale_timeout_secs: u64,
}

impl Config {
    /// Parse configuration from command-line args and environment variables
    pub fn parse_config() -> Self {
        Config::parse()
    }

    /// Get the subject filter for JetStream consumer
    pub fn subject_filter(&self) -> String {
        if let Some(tenant) = &self.tenant_filter {
            format!("demon.scale.v1.{}.hints", tenant)
        } else {
            "demon.scale.v1.*.hints".to_string()
        }
    }

    /// Check if autoscale endpoint is configured
    pub fn has_autoscale_endpoint(&self) -> bool {
        self.autoscale_endpoint.is_some() && !self.dry_run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subject_filter_all_tenants() {
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

        assert_eq!(config.subject_filter(), "demon.scale.v1.*.hints");
    }

    #[test]
    fn test_subject_filter_specific_tenant() {
        let config = Config {
            nats_url: "nats://localhost:4222".to_string(),
            nats_creds_path: None,
            stream_name: "SCALE_HINTS".to_string(),
            tenant_filter: Some("production".to_string()),
            dry_run: true,
            autoscale_endpoint: None,
            log_json: false,
            metrics_port: 9090,
            consumer_name: "test-consumer".to_string(),
            retry_backoff_ms: 1000,
            max_retry_attempts: 3,
            autoscale_timeout_secs: 10,
        };

        assert_eq!(config.subject_filter(), "demon.scale.v1.production.hints");
    }

    #[test]
    fn test_has_autoscale_endpoint() {
        let mut config = Config {
            nats_url: "nats://localhost:4222".to_string(),
            nats_creds_path: None,
            stream_name: "SCALE_HINTS".to_string(),
            tenant_filter: None,
            dry_run: true,
            autoscale_endpoint: Some("http://autoscaler:8080/scale".to_string()),
            log_json: false,
            metrics_port: 9090,
            consumer_name: "test-consumer".to_string(),
            retry_backoff_ms: 1000,
            max_retry_attempts: 3,
            autoscale_timeout_secs: 10,
        };

        // Dry-run mode disables autoscale
        assert!(!config.has_autoscale_endpoint());

        // Non-dry-run with endpoint
        config.dry_run = false;
        assert!(config.has_autoscale_endpoint());

        // Non-dry-run without endpoint
        config.autoscale_endpoint = None;
        assert!(!config.has_autoscale_endpoint());
    }
}
