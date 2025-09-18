use capsules_echo::echo;

#[test]
fn echo_capsule_returns_valid_envelope() {
    let message = "Hello from test".to_string();
    let envelope = echo(message.clone());

    // Test basic envelope structure
    assert!(envelope.result.is_success());
    if let envelope::OperationResult::Success { data, .. } = &envelope.result {
        assert_eq!(data.echoed_message, message);
        assert_eq!(data.character_count, message.chars().count());
    } else {
        panic!("Expected success result");
    }

    // Test that timestamp is recent (within last minute)
    let now = chrono::Utc::now();
    if let envelope::OperationResult::Success { data, .. } = &envelope.result {
        let time_diff = now.signed_duration_since(data.timestamp);
        assert!(time_diff.num_seconds() < 60, "Timestamp should be recent");
    }

    // Test diagnostics are present
    assert!(
        !envelope.diagnostics.is_empty(),
        "Should have at least one diagnostic"
    );

    // Test provenance is set
    assert!(envelope.provenance.is_some(), "Should have provenance");
    let provenance = envelope.provenance.as_ref().unwrap();
    if let Some(source) = &provenance.source {
        assert_eq!(source.system, "echo-capsule");
        assert_eq!(source.version, Some("0.0.1".to_string()));
    } else {
        panic!("Expected source info in provenance");
    }

    // Test metrics are present
    assert!(envelope.metrics.is_some(), "Should have metrics");
    let metrics = envelope.metrics.as_ref().unwrap();
    assert!(metrics.duration.is_some(), "Should have duration metric");
    assert!(
        metrics.counters.contains_key("characterCount"),
        "Should have character count metric"
    );
}

#[test]
fn echo_capsule_envelope_validates_against_schema() {
    let envelope = echo("Test validation".to_string());

    // This should pass schema validation
    envelope
        .validate()
        .expect("Echo envelope should validate against schema");
}

#[test]
fn echo_capsule_handles_edge_cases() {
    // Test empty string
    let empty_envelope = echo("".to_string());
    assert!(empty_envelope.result.is_success());
    if let envelope::OperationResult::Success { data, .. } = &empty_envelope.result {
        assert_eq!(data.character_count, 0);
    } else {
        panic!("Expected success result");
    }

    // Should have warning about empty message
    let warnings: Vec<_> = empty_envelope
        .diagnostics
        .iter()
        .filter(|d| matches!(d.level, envelope::DiagnosticLevel::Warning))
        .collect();
    assert!(!warnings.is_empty(), "Should warn about empty message");

    // Test very long string
    let long_message = "x".repeat(150);
    let long_envelope = echo(long_message.clone());
    assert!(long_envelope.result.is_success());
    if let envelope::OperationResult::Success { data, .. } = &long_envelope.result {
        assert_eq!(data.character_count, 150);
    } else {
        panic!("Expected success result");
    }

    // Should have warning about long message
    let warnings: Vec<_> = long_envelope
        .diagnostics
        .iter()
        .filter(|d| matches!(d.level, envelope::DiagnosticLevel::Warning))
        .collect();
    assert!(!warnings.is_empty(), "Should warn about long message");

    // Test whitespace trimming suggestion
    let whitespace_message = "  hello  ".to_string();
    let whitespace_envelope = echo(whitespace_message);
    assert!(
        !whitespace_envelope.suggestions.is_empty(),
        "Should suggest trimming whitespace"
    );
}

#[test]
fn echo_capsule_metrics_are_accurate() {
    let message = "test message for metrics".to_string();
    let envelope = echo(message.clone());

    let metrics = envelope.metrics.as_ref().unwrap();

    // Check character count metric
    let char_count = metrics.counters.get("characterCount").unwrap();
    assert_eq!(*char_count, message.chars().count() as i64);

    // Check duration metric exists and is reasonable (should be very small)
    let duration = metrics.duration.as_ref().unwrap().total_ms.unwrap();
    assert!(duration >= 0.0, "Duration should be non-negative");
    assert!(
        duration < 1000.0,
        "Duration should be less than 1 second for simple echo"
    );
}
