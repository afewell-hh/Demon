//! Metrics stub for scale hint handler
//!
//! NOTE: Full Prometheus implementation pending resolution of metrics crate version conflict
//! between metrics 0.21 (direct dependency) and 0.22 (transitive via metrics-exporter-prometheus).
//! Current implementation uses structured logging. Follow-up PR will add full Prometheus support.

use anyhow::Result;
use tracing::info;

/// Metrics collector for scale hint handler
#[derive(Clone)]
pub struct Metrics;

impl Metrics {
    /// Initialize metrics (currently logs-based)
    pub fn init(port: u16) -> Result<()> {
        info!("Metrics initialized (log-based) on port {}", port);
        Ok(())
    }

    /// Record a scale recommendation
    pub fn record_recommendation(&self, recommendation: &str, tenant_id: &str) {
        info!(
            recommendation = %recommendation,
            tenant_id = %tenant_id,
            "Recorded scale recommendation"
        );
    }

    /// Record autoscale API call result
    pub fn record_autoscale_call(&self, success: bool, tenant_id: &str) {
        info!(
            success = success,
            tenant_id = %tenant_id,
            "Recorded autoscale call"
        );
    }

    /// Record throttled event
    pub fn record_throttled(&self, tenant_id: &str) {
        info!(tenant_id = %tenant_id, "Recorded throttled event");
    }

    /// Record processing error
    pub fn record_error(&self, error_type: &str, tenant_id: &str) {
        info!(
            error_type = %error_type,
            tenant_id = %tenant_id,
            "Recorded error"
        );
    }

    /// Update metrics gauges from event
    pub fn update_gauges(
        &self,
        queue_lag: u64,
        p95_latency_ms: f64,
        error_rate: f64,
        tenant_id: &str,
    ) {
        info!(
            queue_lag = queue_lag,
            p95_latency_ms = p95_latency_ms,
            error_rate = error_rate,
            tenant_id = %tenant_id,
            "Updated metrics gauges"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics;
        metrics.record_recommendation("scale_up", "test-tenant");
        metrics.record_autoscale_call(true, "test-tenant");
        metrics.record_throttled("test-tenant");
        metrics.record_error("deserialization", "test-tenant");
        metrics.update_gauges(100, 250.5, 0.05, "test-tenant");
    }
}
