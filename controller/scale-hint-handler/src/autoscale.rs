//! Autoscale client trait and implementations
//!
//! Provides pluggable autoscaling integrations. Default implementation logs
//! recommendations; HTTP implementation calls external autoscale APIs.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

/// Scale recommendation from agent.scale.hint event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    ScaleUp,
    ScaleDown,
    Steady,
}

/// Scale hint event payload (matches contract schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleHintEvent {
    pub event: String,
    pub ts: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub recommendation: Recommendation,
    pub metrics: MetricsPayload,
    pub thresholds: ThresholdsPayload,
    pub hysteresis: HysteresisPayload,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "traceId")]
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsPayload {
    #[serde(rename = "queueLag")]
    pub queue_lag: u64,
    #[serde(rename = "p95LatencyMs")]
    pub p95_latency_ms: f64,
    #[serde(rename = "errorRate")]
    pub error_rate: f64,
    #[serde(rename = "totalProcessed")]
    pub total_processed: u64,
    #[serde(rename = "totalErrors")]
    pub total_errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdsPayload {
    #[serde(rename = "queueLagHigh")]
    pub queue_lag_high: u64,
    #[serde(rename = "queueLagLow")]
    pub queue_lag_low: u64,
    #[serde(rename = "p95LatencyHighMs")]
    pub p95_latency_high_ms: f64,
    #[serde(rename = "p95LatencyLowMs")]
    pub p95_latency_low_ms: f64,
    #[serde(rename = "errorRateHigh")]
    pub error_rate_high: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HysteresisPayload {
    #[serde(rename = "currentState")]
    pub current_state: String,
    #[serde(rename = "stateChangedAt")]
    pub state_changed_at: Option<String>,
    #[serde(rename = "consecutiveHighSignals")]
    pub consecutive_high_signals: u32,
    #[serde(rename = "consecutiveLowSignals")]
    pub consecutive_low_signals: u32,
    #[serde(rename = "minSignalsForTransition")]
    pub min_signals_for_transition: u32,
}

/// Autoscale client trait - implement this to integrate with different autoscalers
#[async_trait]
pub trait AutoscaleClient: Send + Sync {
    /// Handle a scale hint event
    async fn handle_scale_hint(&self, event: &ScaleHintEvent) -> Result<()>;
}

/// Log-only autoscale client (default implementation)
pub struct LogOnlyAutoscaleClient;

#[async_trait]
impl AutoscaleClient for LogOnlyAutoscaleClient {
    async fn handle_scale_hint(&self, event: &ScaleHintEvent) -> Result<()> {
        info!(
            tenant_id = %event.tenant_id,
            recommendation = ?event.recommendation,
            queue_lag = event.metrics.queue_lag,
            p95_latency_ms = event.metrics.p95_latency_ms,
            error_rate = event.metrics.error_rate,
            reason = %event.reason,
            "Scale recommendation (log-only mode)"
        );
        Ok(())
    }
}

/// HTTP autoscale client - POSTs scale recommendations to an HTTP endpoint
pub struct HttpAutoscaleClient {
    endpoint: String,
    client: reqwest::Client,
    max_retries: u32,
    retry_backoff_ms: u64,
}

/// Payload sent to autoscale HTTP endpoint
#[derive(Debug, Serialize)]
struct AutoscaleRequest {
    tenant_id: String,
    recommendation: Recommendation,
    metrics: MetricsPayload,
    reason: String,
    timestamp: String,
    trace_id: Option<String>,
}

impl HttpAutoscaleClient {
    /// Create a new HTTP autoscale client
    pub fn new(
        endpoint: String,
        timeout_secs: u64,
        max_retries: u32,
        retry_backoff_ms: u64,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            endpoint,
            client,
            max_retries,
            retry_backoff_ms,
        })
    }
}

#[async_trait]
impl AutoscaleClient for HttpAutoscaleClient {
    async fn handle_scale_hint(&self, event: &ScaleHintEvent) -> Result<()> {
        let request = AutoscaleRequest {
            tenant_id: event.tenant_id.clone(),
            recommendation: event.recommendation,
            metrics: event.metrics.clone(),
            reason: event.reason.clone(),
            timestamp: event.ts.clone(),
            trace_id: event.trace_id.clone(),
        };

        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let backoff =
                    Duration::from_millis(self.retry_backoff_ms * (2_u64.pow(attempt - 1)));
                warn!(
                    attempt = attempt,
                    backoff_ms = backoff.as_millis(),
                    "Retrying autoscale API call after backoff"
                );
                tokio::time::sleep(backoff).await;
            }

            match self.client.post(&self.endpoint).json(&request).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        info!(
                            tenant_id = %event.tenant_id,
                            recommendation = ?event.recommendation,
                            status = %response.status(),
                            attempt = attempt + 1,
                            "Successfully called autoscale endpoint"
                        );
                        return Ok(());
                    } else {
                        let status = response.status();
                        let body = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "<unable to read body>".to_string());
                        last_error = Some(anyhow::anyhow!(
                            "Autoscale API returned error status {}: {}",
                            status,
                            body
                        ));
                    }
                }
                Err(e) => {
                    last_error = Some(anyhow::anyhow!("HTTP request failed: {}", e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Autoscale API call failed")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_log_only_client() {
        let client = LogOnlyAutoscaleClient;
        let event = create_test_event(Recommendation::ScaleUp);

        let result = client.handle_scale_hint(&event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_http_client_creation() {
        let client =
            HttpAutoscaleClient::new("http://localhost:8080/scale".to_string(), 10, 3, 1000);
        assert!(client.is_ok());
    }

    fn create_test_event(recommendation: Recommendation) -> ScaleHintEvent {
        ScaleHintEvent {
            event: "agent.scale.hint:v1".to_string(),
            ts: "2025-01-06T10:30:00Z".to_string(),
            tenant_id: "test-tenant".to_string(),
            recommendation,
            metrics: MetricsPayload {
                queue_lag: 600,
                p95_latency_ms: 1100.0,
                error_rate: 0.06,
                total_processed: 1000,
                total_errors: 60,
            },
            thresholds: ThresholdsPayload {
                queue_lag_high: 500,
                queue_lag_low: 50,
                p95_latency_high_ms: 1000.0,
                p95_latency_low_ms: 100.0,
                error_rate_high: 0.05,
            },
            hysteresis: HysteresisPayload {
                current_state: "pressure".to_string(),
                state_changed_at: Some("2025-01-06T10:29:00Z".to_string()),
                consecutive_high_signals: 3,
                consecutive_low_signals: 0,
                min_signals_for_transition: 3,
            },
            reason: "Test scale hint".to_string(),
            trace_id: None,
        }
    }
}
