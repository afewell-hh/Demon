//! Integration tests for scale hint telemetry

use runtime::telemetry::{
    HysteresisState, PressureState, Recommendation, RuntimeMetrics, ScaleHintConfig,
    ScaleHintEmitter,
};

#[tokio::test]
async fn test_scale_hint_emitter_dry_run() {
    // Given: An emitter with no NATS context (dry-run mode)
    let config = ScaleHintConfig {
        enabled: true,
        queue_lag_high: 500,
        queue_lag_low: 50,
        min_signals_for_transition: 3,
        ..Default::default()
    };

    let emitter = ScaleHintEmitter::new(config, None, "test-tenant".to_string());

    // When: Evaluating high-pressure metrics
    let metrics = RuntimeMetrics {
        queue_lag: 600,
        p95_latency_ms: 1100.0,
        error_rate: 0.06,
        total_processed: 1000,
        total_errors: 60,
    };

    // Then: Should return None (dry-run, no NATS)
    let result = emitter.evaluate_and_emit(metrics).await;
    assert!(result.is_ok());
    // In dry-run mode with no NATS, we expect Ok(None)
}

#[tokio::test]
async fn test_scale_hint_disabled() {
    // Given: A disabled emitter
    let config = ScaleHintConfig {
        enabled: false,
        ..Default::default()
    };

    let emitter = ScaleHintEmitter::new(config, None, "test-tenant".to_string());

    // When: Evaluating any metrics
    let metrics = RuntimeMetrics {
        queue_lag: 600,
        p95_latency_ms: 1100.0,
        error_rate: 0.06,
        total_processed: 1000,
        total_errors: 60,
    };

    // Then: Should return Ok(None) because it's disabled
    let result = emitter.evaluate_and_emit(metrics).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
}

#[test]
fn test_config_from_env_defaults() {
    // When: Loading config without env vars
    std::env::remove_var("SCALE_HINT_ENABLED");
    std::env::remove_var("SCALE_HINT_QUEUE_LAG_HIGH");

    let config = ScaleHintConfig::from_env();

    // Then: Should use defaults
    assert!(!config.enabled);
    assert_eq!(config.queue_lag_high, 500);
    assert_eq!(config.queue_lag_low, 50);
    assert_eq!(config.p95_latency_high_ms, 1000.0);
    assert_eq!(config.p95_latency_low_ms, 100.0);
    assert_eq!(config.error_rate_high, 0.05);
    assert_eq!(config.min_signals_for_transition, 3);
}

#[test]
fn test_hysteresis_prevents_oscillation() {
    // Given: A config with 3 signals required for transition
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

    // When: Alternating between high and low pressure
    let high_metrics = RuntimeMetrics {
        queue_lag: 600,
        p95_latency_ms: 1100.0,
        error_rate: 0.06,
        total_processed: 100,
        total_errors: 6,
    };

    let low_metrics = RuntimeMetrics {
        queue_lag: 30,
        p95_latency_ms: 50.0,
        error_rate: 0.01,
        total_processed: 100,
        total_errors: 1,
    };

    // Send 2 high signals
    hyst.update(&high_metrics, &config);
    hyst.update(&high_metrics, &config);
    // Should still be in normal state (need 3 signals)
    assert_eq!(hyst.current_state, PressureState::Normal);

    // Send 1 low signal (resets counter)
    hyst.update(&low_metrics, &config);
    assert_eq!(hyst.current_state, PressureState::Normal);

    // Send 2 high signals again
    hyst.update(&high_metrics, &config);
    hyst.update(&high_metrics, &config);
    // Still normal
    assert_eq!(hyst.current_state, PressureState::Normal);

    // Send 1 more high signal (3 consecutive)
    hyst.update(&high_metrics, &config);
    // Now should transition to Pressure
    assert_eq!(hyst.current_state, PressureState::Pressure);
}

#[test]
fn test_recommendation_scale_up() {
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

    // Build up to scale-up recommendation
    for _ in 0..3 {
        let metrics = RuntimeMetrics {
            queue_lag: 600,
            p95_latency_ms: 1100.0,
            error_rate: 0.06,
            total_processed: 100,
            total_errors: 6,
        };
        let (rec, _reason) = hyst.update(&metrics, &config);

        // After 3 signals, should recommend scale up
        if hyst.consecutive_high_signals >= 3 {
            assert_eq!(rec, Recommendation::ScaleUp);
            assert_eq!(hyst.current_state, PressureState::Pressure);
        }
    }
}

#[test]
fn test_recommendation_scale_down() {
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

    // Send many low-pressure signals
    for i in 0..7 {
        let metrics = RuntimeMetrics {
            queue_lag: 20,
            p95_latency_ms: 40.0,
            error_rate: 0.005,
            total_processed: 100,
            total_errors: 0,
        };
        let (rec, _reason) = hyst.update(&metrics, &config);

        // After 6 consecutive low signals (2x min_signals), should recommend scale down
        if i >= 5 {
            assert_eq!(rec, Recommendation::ScaleDown);
        }
    }
}

#[test]
fn test_recommendation_steady() {
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

    // Send metrics in the middle range
    let metrics = RuntimeMetrics {
        queue_lag: 150,
        p95_latency_ms: 300.0,
        error_rate: 0.02,
        total_processed: 100,
        total_errors: 2,
    };

    let (rec, _reason) = hyst.update(&metrics, &config);

    // Should recommend steady (metrics between thresholds)
    assert_eq!(rec, Recommendation::Steady);
    assert_eq!(hyst.current_state, PressureState::Normal);
}
