use envelope::*;
use serde_json::Value;
use std::path::Path;

const FIXTURES_DIR: &str = "../../contracts/fixtures/envelopes";

#[test]
fn given_minimal_fixture_when_deserializing_then_matches_schema() {
    let fixture_path = Path::new(FIXTURES_DIR).join("result_minimal.json");
    let fixture_content =
        std::fs::read_to_string(&fixture_path).expect("Should be able to read minimal fixture");

    let envelope: ResultEnvelope<Value> =
        serde_json::from_str(&fixture_content).expect("Should deserialize minimal fixture");

    assert!(envelope.result.is_success());
    assert!(envelope.diagnostics.is_empty());
    assert!(envelope.suggestions.is_empty());
    assert!(envelope.metrics.is_none());
    assert!(envelope.provenance.is_none());

    // Validate against schema
    assert!(envelope.validate().is_ok());
}

#[test]
fn given_full_fixture_when_deserializing_then_matches_schema() {
    let fixture_path = Path::new(FIXTURES_DIR).join("result_full.json");
    let fixture_content =
        std::fs::read_to_string(&fixture_path).expect("Should be able to read full fixture");

    let envelope: ResultEnvelope<Value> =
        serde_json::from_str(&fixture_content).expect("Should deserialize full fixture");

    assert!(envelope.result.is_success());
    assert_eq!(envelope.diagnostics.len(), 4);
    assert_eq!(envelope.suggestions.len(), 3);
    assert!(envelope.metrics.is_some());
    assert!(envelope.provenance.is_some());

    // Check diagnostics levels
    let levels: Vec<_> = envelope.diagnostics.iter().map(|d| &d.level).collect();
    assert!(levels.contains(&&DiagnosticLevel::Info));
    assert!(levels.contains(&&DiagnosticLevel::Warning));
    assert!(levels.contains(&&DiagnosticLevel::Error));
    assert!(levels.contains(&&DiagnosticLevel::Debug));

    // Check suggestions types
    let types: Vec<_> = envelope
        .suggestions
        .iter()
        .map(|s| &s.suggestion_type)
        .collect();
    assert!(types.contains(&&SuggestionType::Optimization));
    assert!(types.contains(&&SuggestionType::Configuration));
    assert!(types.contains(&&SuggestionType::Action));

    // Check metrics structure
    let metrics = envelope.metrics.as_ref().unwrap();
    assert!(metrics.duration.is_some());
    assert!(metrics.resources.is_some());
    assert!(!metrics.counters.is_empty());
    assert!(metrics.custom.is_some());

    // Check provenance structure
    let provenance = envelope.provenance.as_ref().unwrap();
    assert!(provenance.source.is_some());
    assert!(provenance.timestamp.is_some());
    assert!(provenance.trace_id.is_some());
    assert!(provenance.span_id.is_some());
    assert!(!provenance.chain.is_empty());

    // Validate against schema
    assert!(envelope.validate().is_ok());
}

#[test]
fn given_error_fixture_when_deserializing_then_matches_schema() {
    let fixture_path = Path::new(FIXTURES_DIR).join("result_error.json");
    let fixture_content =
        std::fs::read_to_string(&fixture_path).expect("Should be able to read error fixture");

    let envelope: ResultEnvelope<Value> =
        serde_json::from_str(&fixture_content).expect("Should deserialize error fixture");

    assert!(envelope.result.is_error());

    // Validate against schema
    assert!(envelope.validate().is_ok());
}

#[test]
fn given_suggestions_fixture_when_deserializing_then_matches_schema() {
    let fixture_path = Path::new(FIXTURES_DIR).join("result_with_suggestions.json");
    let fixture_content =
        std::fs::read_to_string(&fixture_path).expect("Should be able to read suggestions fixture");

    let envelope: ResultEnvelope<Value> =
        serde_json::from_str(&fixture_content).expect("Should deserialize suggestions fixture");

    assert!(!envelope.suggestions.is_empty());

    // Check for JSON Patch operations in suggestions
    let has_patch = envelope.suggestions.iter().any(|s| s.patch.is_some());
    assert!(
        has_patch,
        "At least one suggestion should have patch operations"
    );

    // Validate against schema
    assert!(envelope.validate().is_ok());
}

#[test]
fn given_fixture_when_round_trip_serializing_then_preserves_structure() {
    let fixture_path = Path::new(FIXTURES_DIR).join("result_full.json");
    let original_content =
        std::fs::read_to_string(&fixture_path).expect("Should be able to read full fixture");

    // Deserialize to our types
    let envelope: ResultEnvelope<Value> =
        serde_json::from_str(&original_content).expect("Should deserialize full fixture");

    // Serialize back to JSON
    let serialized = serde_json::to_value(&envelope).expect("Should serialize back to JSON");

    // Validate the serialized version
    let validator = EnvelopeValidator::new().expect("Should create validator");
    assert!(validator.validate_json(&serialized).is_ok());

    // Ensure key fields are preserved
    assert!(serialized["result"]["success"].as_bool().unwrap());
    assert!(serialized["diagnostics"].is_array());
    assert!(serialized["suggestions"].is_array());
    assert!(serialized["metrics"].is_object());
    assert!(serialized["provenance"].is_object());
}

#[test]
fn given_all_fixtures_when_validating_with_schema_then_all_pass() {
    let fixtures = [
        "result_minimal.json",
        "result_full.json",
        "result_error.json",
        "result_with_suggestions.json",
    ];
    let validator = EnvelopeValidator::new().expect("Should create validator");

    for fixture_name in fixtures {
        let fixture_path = Path::new(FIXTURES_DIR).join(fixture_name);
        let fixture_content = std::fs::read_to_string(&fixture_path)
            .unwrap_or_else(|_| panic!("Should be able to read fixture: {}", fixture_name));

        let fixture_json: Value = serde_json::from_str(&fixture_content)
            .unwrap_or_else(|_| panic!("Should parse JSON for fixture: {}", fixture_name));

        assert!(
            validator.validate_json(&fixture_json).is_ok(),
            "Fixture {} should validate against schema",
            fixture_name
        );
    }
}
