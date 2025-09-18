use envelope::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestData {
    message: String,
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, AsEnvelope)]
struct TestResult {
    value: i32,
    description: String,
}

#[test]
fn given_valid_data_when_building_minimal_envelope_then_succeeds() {
    let test_data = TestData {
        message: "Operation completed successfully".to_string(),
        id: "op-12345".to_string(),
    };

    let envelope = ResultEnvelope::builder()
        .success(test_data.clone())
        .build()
        .expect("Should build successfully");

    assert!(envelope.result.is_success());
    assert!(envelope.diagnostics.is_empty());
    assert!(envelope.suggestions.is_empty());
    assert!(envelope.metrics.is_none());
    assert!(envelope.provenance.is_none());
}

#[test]
fn given_error_details_when_building_error_envelope_then_succeeds() {
    let envelope = ResultEnvelope::<()>::builder()
        .error_with_code("Processing failed", "PROCESSING_FAILED")
        .add_error("Memory allocation failed")
        .build()
        .expect("Should build successfully");

    assert!(envelope.result.is_error());
    assert_eq!(envelope.diagnostics.len(), 1);
    assert_eq!(envelope.diagnostics[0].level, DiagnosticLevel::Error);
}

#[test]
fn given_comprehensive_data_when_building_full_envelope_then_succeeds() {
    let test_data = TestData {
        message: "Complex operation completed".to_string(),
        id: "op-67890".to_string(),
    };

    let suggestion = Suggestion::optimization("Increase batch size")
        .with_priority(SuggestionPriority::Medium)
        .with_rationale("Current batch size is suboptimal")
        .with_patch(vec![JsonPatchOperation::replace(
            "/config/processing/batch_size",
            json!(50),
        )])
        .build();

    let envelope = ResultEnvelope::builder()
        .success(test_data)
        .add_info("Starting operation processing")
        .add_warning("Skipped 3 items due to validation errors")
        .add_suggestion(suggestion)
        .with_source_info("test-system", Some("1.0.0"), Some("test-01"))
        .with_trace_info("trace-123", "span-456", Some("parent-789"))
        .build()
        .expect("Should build successfully");

    assert!(envelope.result.is_success());
    assert_eq!(envelope.diagnostics.len(), 2);
    assert_eq!(envelope.suggestions.len(), 1);
    assert!(envelope.provenance.is_some());

    let provenance = envelope.provenance.unwrap();
    assert!(provenance.source.is_some());
    assert_eq!(provenance.trace_id, Some("trace-123".to_string()));
    assert_eq!(provenance.span_id, Some("span-456".to_string()));
    assert_eq!(provenance.parent_span_id, Some("parent-789".to_string()));
}

#[test]
fn given_timing_operation_when_using_with_timing_then_captures_duration() {
    let (builder, result) = ResultEnvelope::<i32>::builder().with_timing(|| {
        std::thread::sleep(std::time::Duration::from_millis(10));
        42
    });

    let envelope = builder
        .success(result)
        .build()
        .expect("Should build successfully");

    assert!(envelope.metrics.is_some());
    let metrics = envelope.metrics.unwrap();
    assert!(metrics.duration.is_some());
    let duration = metrics.duration.unwrap();
    assert!(duration.total_ms.is_some());
    assert!(duration.total_ms.unwrap() >= 10.0);
}

#[test]
fn given_custom_struct_when_using_derive_macro_then_creates_envelope() {
    let test_result = TestResult {
        value: 42,
        description: "Test result".to_string(),
    };

    let envelope = test_result.into_envelope();

    assert!(envelope.result.is_success());
    assert!(envelope.diagnostics.is_empty());
    assert!(envelope.suggestions.is_empty());
}

#[test]
fn given_json_patch_operations_when_building_suggestions_then_serializes_correctly() {
    let suggestion = Suggestion::configuration("Enable parallel processing")
        .with_patch(vec![
            JsonPatchOperation::add("/config/processing/parallel", json!(true)),
            JsonPatchOperation::replace("/config/processing/batch_size", json!(50)),
            JsonPatchOperation::remove("/config/processing/legacy_mode"),
        ])
        .build();

    let envelope = ResultEnvelope::builder()
        .success("test")
        .add_suggestion(suggestion)
        .build()
        .expect("Should build successfully");

    let json = serde_json::to_string(&envelope).expect("Should serialize");
    assert!(json.contains("\"op\":\"add\""));
    assert!(json.contains("\"op\":\"replace\""));
    assert!(json.contains("\"op\":\"remove\""));
}

#[test]
fn given_builder_without_result_when_building_then_fails() {
    let result = ResultEnvelope::<()>::builder()
        .add_info("Some diagnostic")
        .build();

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), BuildError::MissingResult));
}
