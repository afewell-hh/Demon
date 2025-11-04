//! Scale hint telemetry - emit structured events based on runtime metrics
//!
//! This module implements hysteresis-based threshold logic to avoid scale oscillations
//! and emits agent.scale.hint:v1 events to NATS for consumption by scale controllers.

use anyhow::{Context, Result};
use async_nats::jetstream::publish::PublishAck;
use async_nats::jetstream::Context as JsContext;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};

/// Scale recommendation types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    ScaleUp,
    ScaleDown,
    Steady,
}

/// Pressure state with hysteresis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PressureState {
    Normal,
    Pressure,
    Overload,
}

/// Configuration for scale hint thresholds (loaded from env vars)
#[derive(Debug, Clone)]
pub struct ScaleHintConfig {
    pub queue_lag_high: u64,
    pub queue_lag_low: u64,
    pub p95_latency_high_ms: f64,
    pub p95_latency_low_ms: f64,
    pub error_rate_high: f64,
    pub min_signals_for_transition: u32,
    pub enabled: bool,
}

impl Default for ScaleHintConfig {
    fn default() -> Self {
        Self {
            queue_lag_high: 500,
            queue_lag_low: 50,
            p95_latency_high_ms: 1000.0,
            p95_latency_low_ms: 100.0,
            error_rate_high: 0.05,
            min_signals_for_transition: 3,
            enabled: false,
        }
    }
}

impl ScaleHintConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let enabled = env::var("SCALE_HINT_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        Self {
            queue_lag_high: env::var("SCALE_HINT_QUEUE_LAG_HIGH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(500),
            queue_lag_low: env::var("SCALE_HINT_QUEUE_LAG_LOW")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50),
            p95_latency_high_ms: env::var("SCALE_HINT_P95_LATENCY_HIGH_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1000.0),
            p95_latency_low_ms: env::var("SCALE_HINT_P95_LATENCY_LOW_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100.0),
            error_rate_high: env::var("SCALE_HINT_ERROR_RATE_HIGH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.05),
            min_signals_for_transition: env::var("SCALE_HINT_MIN_SIGNALS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            enabled,
        }
    }
}

/// Runtime metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMetrics {
    pub queue_lag: u64,
    pub p95_latency_ms: f64,
    pub error_rate: f64,
    pub total_processed: u64,
    pub total_errors: u64,
}

/// Hysteresis state for preventing oscillations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HysteresisState {
    pub current_state: PressureState,
    pub state_changed_at: Option<String>,
    pub consecutive_high_signals: u32,
    pub consecutive_low_signals: u32,
    pub min_signals_for_transition: u32,
}

impl HysteresisState {
    pub fn new(min_signals: u32) -> Self {
        Self {
            current_state: PressureState::Normal,
            state_changed_at: Some(Utc::now().to_rfc3339()),
            consecutive_high_signals: 0,
            consecutive_low_signals: 0,
            min_signals_for_transition: min_signals,
        }
    }

    /// Update hysteresis state based on metrics and thresholds
    pub fn update(
        &mut self,
        metrics: &RuntimeMetrics,
        config: &ScaleHintConfig,
    ) -> (Recommendation, String) {
        let is_high_pressure = metrics.queue_lag > config.queue_lag_high
            || metrics.p95_latency_ms > config.p95_latency_high_ms
            || metrics.error_rate > config.error_rate_high;

        let is_low_pressure = metrics.queue_lag < config.queue_lag_low
            && metrics.p95_latency_ms < config.p95_latency_low_ms
            && metrics.error_rate < config.error_rate_high;

        // Update signal counters
        if is_high_pressure {
            self.consecutive_high_signals += 1;
            self.consecutive_low_signals = 0;
        } else if is_low_pressure {
            self.consecutive_low_signals += 1;
            self.consecutive_high_signals = 0;
        } else {
            // In between - reset both counters
            self.consecutive_high_signals = 0;
            self.consecutive_low_signals = 0;
        }

        // State transitions with hysteresis
        let old_state = self.current_state;
        match self.current_state {
            PressureState::Normal => {
                if self.consecutive_high_signals >= self.min_signals_for_transition {
                    self.current_state = PressureState::Pressure;
                    self.state_changed_at = Some(Utc::now().to_rfc3339());
                }
            }
            PressureState::Pressure => {
                if self.consecutive_high_signals >= self.min_signals_for_transition * 2 {
                    self.current_state = PressureState::Overload;
                    self.state_changed_at = Some(Utc::now().to_rfc3339());
                } else if self.consecutive_low_signals >= self.min_signals_for_transition {
                    self.current_state = PressureState::Normal;
                    self.state_changed_at = Some(Utc::now().to_rfc3339());
                }
            }
            PressureState::Overload => {
                if self.consecutive_low_signals >= self.min_signals_for_transition {
                    self.current_state = PressureState::Pressure;
                    self.state_changed_at = Some(Utc::now().to_rfc3339());
                }
            }
        }

        // Determine recommendation and reason
        let (recommendation, reason) = self.compute_recommendation(metrics, config, old_state);

        (recommendation, reason)
    }

    fn compute_recommendation(
        &self,
        metrics: &RuntimeMetrics,
        config: &ScaleHintConfig,
        old_state: PressureState,
    ) -> (Recommendation, String) {
        match self.current_state {
            PressureState::Overload => {
                let reasons = self.build_high_pressure_reasons(metrics, config);
                (
                    Recommendation::ScaleUp,
                    format!(
                        "Overload detected ({}). Consider scaling up agents.",
                        reasons.join(", ")
                    ),
                )
            }
            PressureState::Pressure => {
                if old_state == PressureState::Normal {
                    let reasons = self.build_high_pressure_reasons(metrics, config);
                    (
                        Recommendation::ScaleUp,
                        format!(
                            "Elevated pressure ({}). Consider scaling up agents.",
                            reasons.join(", ")
                        ),
                    )
                } else {
                    (
                        Recommendation::Steady,
                        "Pressure decreasing but not yet normal; holding steady".to_string(),
                    )
                }
            }
            PressureState::Normal => {
                if self.consecutive_low_signals >= self.min_signals_for_transition * 2 {
                    (
                        Recommendation::ScaleDown,
                        format!(
                            "Low utilization for {} consecutive intervals; consider scaling down",
                            self.consecutive_low_signals
                        ),
                    )
                } else {
                    (
                        Recommendation::Steady,
                        "Metrics within normal operating range".to_string(),
                    )
                }
            }
        }
    }

    fn build_high_pressure_reasons(
        &self,
        metrics: &RuntimeMetrics,
        config: &ScaleHintConfig,
    ) -> Vec<String> {
        let mut reasons = Vec::new();
        if metrics.queue_lag > config.queue_lag_high {
            reasons.push(format!(
                "queue lag {} > {}",
                metrics.queue_lag, config.queue_lag_high
            ));
        }
        if metrics.p95_latency_ms > config.p95_latency_high_ms {
            reasons.push(format!(
                "P95 latency {:.1}ms > {:.1}ms",
                metrics.p95_latency_ms, config.p95_latency_high_ms
            ));
        }
        if metrics.error_rate > config.error_rate_high {
            reasons.push(format!(
                "error rate {:.3} > {:.3}",
                metrics.error_rate, config.error_rate_high
            ));
        }
        reasons
    }
}

/// Scale hint event payload (matches contract schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScaleHintEvent {
    event: String,
    ts: String,
    #[serde(rename = "tenantId")]
    tenant_id: String,
    recommendation: Recommendation,
    metrics: MetricsPayload,
    thresholds: ThresholdsPayload,
    hysteresis: HysteresisPayload,
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "traceId")]
    trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetricsPayload {
    #[serde(rename = "queueLag")]
    queue_lag: u64,
    #[serde(rename = "p95LatencyMs")]
    p95_latency_ms: f64,
    #[serde(rename = "errorRate")]
    error_rate: f64,
    #[serde(rename = "totalProcessed")]
    total_processed: u64,
    #[serde(rename = "totalErrors")]
    total_errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ThresholdsPayload {
    #[serde(rename = "queueLagHigh")]
    queue_lag_high: u64,
    #[serde(rename = "queueLagLow")]
    queue_lag_low: u64,
    #[serde(rename = "p95LatencyHighMs")]
    p95_latency_high_ms: f64,
    #[serde(rename = "p95LatencyLowMs")]
    p95_latency_low_ms: f64,
    #[serde(rename = "errorRateHigh")]
    error_rate_high: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HysteresisPayload {
    #[serde(rename = "currentState")]
    current_state: PressureState,
    #[serde(rename = "stateChangedAt")]
    state_changed_at: Option<String>,
    #[serde(rename = "consecutiveHighSignals")]
    consecutive_high_signals: u32,
    #[serde(rename = "consecutiveLowSignals")]
    consecutive_low_signals: u32,
    #[serde(rename = "minSignalsForTransition")]
    min_signals_for_transition: u32,
}

/// Scale hint emitter - evaluates metrics and publishes events to NATS
pub struct ScaleHintEmitter {
    config: ScaleHintConfig,
    hysteresis: Arc<Mutex<HysteresisState>>,
    js_context: Option<JsContext>,
    tenant_id: String,
}

impl ScaleHintEmitter {
    /// Create a new scale hint emitter
    pub fn new(config: ScaleHintConfig, js_context: Option<JsContext>, tenant_id: String) -> Self {
        let hysteresis = Arc::new(Mutex::new(HysteresisState::new(
            config.min_signals_for_transition,
        )));

        Self {
            config,
            hysteresis,
            js_context,
            tenant_id,
        }
    }

    /// Evaluate metrics and emit scale hint event if needed
    pub async fn evaluate_and_emit(&self, metrics: RuntimeMetrics) -> Result<Option<String>> {
        if !self.config.enabled {
            debug!("Scale hint telemetry is disabled");
            return Ok(None);
        }

        // Atomically update hysteresis and capture snapshot to avoid race conditions
        let (recommendation, reason, hyst_snapshot) = {
            let mut hyst = self
                .hysteresis
                .lock()
                .map_err(|e| anyhow::anyhow!("Hysteresis lock poisoned: {}", e))?;
            let (rec, reason) = hyst.update(&metrics, &self.config);
            let snapshot = hyst.clone();
            (rec, reason, snapshot)
        };

        // Only emit if recommendation is not steady or if we want all events
        let should_emit = recommendation != Recommendation::Steady
            || env::var("SCALE_HINT_EMIT_ALL")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(false);

        if !should_emit {
            debug!(
                recommendation = ?recommendation,
                "Skipping scale hint emission (steady state)"
            );
            return Ok(None);
        }

        let event = ScaleHintEvent {
            event: "agent.scale.hint:v1".to_string(),
            ts: Utc::now().to_rfc3339(),
            tenant_id: self.tenant_id.clone(),
            recommendation,
            metrics: MetricsPayload {
                queue_lag: metrics.queue_lag,
                p95_latency_ms: metrics.p95_latency_ms,
                error_rate: metrics.error_rate,
                total_processed: metrics.total_processed,
                total_errors: metrics.total_errors,
            },
            thresholds: ThresholdsPayload {
                queue_lag_high: self.config.queue_lag_high,
                queue_lag_low: self.config.queue_lag_low,
                p95_latency_high_ms: self.config.p95_latency_high_ms,
                p95_latency_low_ms: self.config.p95_latency_low_ms,
                error_rate_high: self.config.error_rate_high,
            },
            hysteresis: HysteresisPayload {
                current_state: hyst_snapshot.current_state,
                state_changed_at: hyst_snapshot.state_changed_at,
                consecutive_high_signals: hyst_snapshot.consecutive_high_signals,
                consecutive_low_signals: hyst_snapshot.consecutive_low_signals,
                min_signals_for_transition: hyst_snapshot.min_signals_for_transition,
            },
            reason,
            trace_id: None,
        };

        info!(
            recommendation = ?recommendation,
            queue_lag = metrics.queue_lag,
            p95_latency_ms = metrics.p95_latency_ms,
            error_rate = metrics.error_rate,
            "Emitting scale hint event"
        );

        self.publish_event(&event).await
    }

    /// Publish event to NATS JetStream
    async fn publish_event(&self, event: &ScaleHintEvent) -> Result<Option<String>> {
        let js = match &self.js_context {
            Some(ctx) => ctx,
            None => {
                warn!("No JetStream context available; scale hint not published");
                return Ok(None);
            }
        };

        let payload = serde_json::to_vec(event).context("Failed to serialize scale hint event")?;

        // Subject pattern: demon.scale.v1.<tenant>.hints
        let subject = format!("demon.scale.v1.{}.hints", self.tenant_id);

        let publish_future = js
            .publish(subject.clone(), payload.into())
            .await
            .context("Failed to publish scale hint to JetStream")?;

        let ack: PublishAck = publish_future
            .await
            .context("Failed to get publish acknowledgement")?;

        debug!(
            subject = %subject,
            stream = %ack.stream,
            sequence = ack.sequence,
            "Published scale hint event"
        );

        Ok(Some(subject))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hysteresis_state_transitions() {
        let config = ScaleHintConfig {
            queue_lag_high: 500,
            queue_lag_low: 50,
            p95_latency_high_ms: 1000.0,
            p95_latency_low_ms: 100.0,
            error_rate_high: 0.05,
            min_signals_for_transition: 3,
            enabled: true,
        };

        let mut hyst = HysteresisState::new(3);

        // Start in normal state
        assert_eq!(hyst.current_state, PressureState::Normal);

        // Send 2 high signals - should stay in normal
        for _ in 0..2 {
            let metrics = RuntimeMetrics {
                queue_lag: 600,
                p95_latency_ms: 1100.0,
                error_rate: 0.06,
                total_processed: 100,
                total_errors: 6,
            };
            hyst.update(&metrics, &config);
        }
        assert_eq!(hyst.current_state, PressureState::Normal);

        // Third high signal - should transition to pressure
        let metrics = RuntimeMetrics {
            queue_lag: 600,
            p95_latency_ms: 1100.0,
            error_rate: 0.06,
            total_processed: 100,
            total_errors: 6,
        };
        hyst.update(&metrics, &config);
        assert_eq!(hyst.current_state, PressureState::Pressure);

        // Send 6 more high signals - should transition to overload
        for _ in 0..6 {
            let metrics = RuntimeMetrics {
                queue_lag: 800,
                p95_latency_ms: 1500.0,
                error_rate: 0.08,
                total_processed: 100,
                total_errors: 8,
            };
            hyst.update(&metrics, &config);
        }
        assert_eq!(hyst.current_state, PressureState::Overload);

        // Send 3 low signals - should transition back to pressure
        for _ in 0..3 {
            let metrics = RuntimeMetrics {
                queue_lag: 30,
                p95_latency_ms: 50.0,
                error_rate: 0.01,
                total_processed: 100,
                total_errors: 1,
            };
            hyst.update(&metrics, &config);
        }
        assert_eq!(hyst.current_state, PressureState::Pressure);

        // Send 3 more low signals - should transition to normal
        for _ in 0..3 {
            let metrics = RuntimeMetrics {
                queue_lag: 20,
                p95_latency_ms: 40.0,
                error_rate: 0.005,
                total_processed: 100,
                total_errors: 0,
            };
            hyst.update(&metrics, &config);
        }
        assert_eq!(hyst.current_state, PressureState::Normal);
    }

    #[test]
    fn test_recommendation_logic() {
        let config = ScaleHintConfig {
            queue_lag_high: 500,
            queue_lag_low: 50,
            p95_latency_high_ms: 1000.0,
            p95_latency_low_ms: 100.0,
            error_rate_high: 0.05,
            min_signals_for_transition: 3,
            enabled: true,
        };

        let mut hyst = HysteresisState::new(3);

        // Trigger scale-up recommendation
        for _ in 0..3 {
            let metrics = RuntimeMetrics {
                queue_lag: 600,
                p95_latency_ms: 1100.0,
                error_rate: 0.06,
                total_processed: 100,
                total_errors: 6,
            };
            let (rec, _reason) = hyst.update(&metrics, &config);
            if hyst.consecutive_high_signals >= 3 {
                assert_eq!(rec, Recommendation::ScaleUp);
            }
        }

        // Trigger scale-down recommendation
        hyst = HysteresisState::new(3);
        for i in 0..7 {
            let metrics = RuntimeMetrics {
                queue_lag: 20,
                p95_latency_ms: 40.0,
                error_rate: 0.005,
                total_processed: 100,
                total_errors: 0,
            };
            let (rec, _reason) = hyst.update(&metrics, &config);
            if i >= 5 {
                // After 6 consecutive low signals
                assert_eq!(rec, Recommendation::ScaleDown);
            }
        }
    }

    #[test]
    fn test_config_from_env() {
        env::set_var("SCALE_HINT_ENABLED", "true");
        env::set_var("SCALE_HINT_QUEUE_LAG_HIGH", "1000");
        env::set_var("SCALE_HINT_QUEUE_LAG_LOW", "100");
        env::set_var("SCALE_HINT_P95_LATENCY_HIGH_MS", "2000.0");
        env::set_var("SCALE_HINT_P95_LATENCY_LOW_MS", "200.0");
        env::set_var("SCALE_HINT_ERROR_RATE_HIGH", "0.1");
        env::set_var("SCALE_HINT_MIN_SIGNALS", "5");

        let config = ScaleHintConfig::from_env();

        assert!(config.enabled);
        assert_eq!(config.queue_lag_high, 1000);
        assert_eq!(config.queue_lag_low, 100);
        assert_eq!(config.p95_latency_high_ms, 2000.0);
        assert_eq!(config.p95_latency_low_ms, 200.0);
        assert_eq!(config.error_rate_high, 0.1);
        assert_eq!(config.min_signals_for_transition, 5);

        // Clean up
        env::remove_var("SCALE_HINT_ENABLED");
        env::remove_var("SCALE_HINT_QUEUE_LAG_HIGH");
        env::remove_var("SCALE_HINT_QUEUE_LAG_LOW");
        env::remove_var("SCALE_HINT_P95_LATENCY_HIGH_MS");
        env::remove_var("SCALE_HINT_P95_LATENCY_LOW_MS");
        env::remove_var("SCALE_HINT_ERROR_RATE_HIGH");
        env::remove_var("SCALE_HINT_MIN_SIGNALS");
    }
}
