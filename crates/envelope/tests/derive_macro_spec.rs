use envelope::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, AsEnvelope, PartialEq)]
struct SimpleResult {
    id: u32,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, AsEnvelope)]
struct ComplexResult {
    data: Vec<String>,
    metadata: std::collections::HashMap<String, serde_json::Value>,
    optional_field: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, AsEnvelope)]
struct GenericResult<T> {
    value: T,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[test]
fn given_simple_struct_when_using_derive_macro_then_creates_envelope() {
    let result = SimpleResult {
        id: 42,
        name: "Test Result".to_string(),
    };

    let envelope = result.clone().into_envelope();

    assert!(envelope.result.is_success());

    // Extract the data from the success result
    if let OperationResult::Success { success: _, data } = envelope.result {
        assert_eq!(data, result);
    } else {
        panic!("Expected success result");
    }

    assert!(envelope.diagnostics.is_empty());
    assert!(envelope.suggestions.is_empty());
    assert!(envelope.metrics.is_none());
    assert!(envelope.provenance.is_none());
}

#[test]
fn given_complex_struct_when_using_derive_macro_then_creates_envelope() {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("key1".to_string(), serde_json::json!("value1"));
    metadata.insert("key2".to_string(), serde_json::json!(42));

    let result = ComplexResult {
        data: vec!["item1".to_string(), "item2".to_string()],
        metadata,
        optional_field: Some(100),
    };

    let envelope = result.into_envelope();

    assert!(envelope.result.is_success());
    assert!(envelope.validate().is_ok());
}

#[test]
fn given_generic_struct_when_using_derive_macro_then_creates_envelope() {
    let result = GenericResult {
        value: "test string".to_string(),
        timestamp: chrono::Utc::now(),
    };

    let envelope = result.into_envelope();

    assert!(envelope.result.is_success());
    assert!(envelope.validate().is_ok());

    // Test with different generic type
    let int_result = GenericResult {
        value: 42i32,
        timestamp: chrono::Utc::now(),
    };

    let int_envelope = int_result.into_envelope();
    assert!(int_envelope.result.is_success());
    assert!(int_envelope.validate().is_ok());
}

#[test]
fn given_derived_envelope_when_serializing_then_produces_valid_json() {
    let result = SimpleResult {
        id: 123,
        name: "Serialization Test".to_string(),
    };

    let envelope = result.into_envelope();
    let json = serde_json::to_string_pretty(&envelope).expect("Should serialize to JSON");

    // Verify JSON structure
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("Should parse serialized JSON");

    assert!(parsed["result"]["success"].as_bool().unwrap());
    assert_eq!(parsed["result"]["data"]["id"], 123);
    assert_eq!(parsed["result"]["data"]["name"], "Serialization Test");

    // Check that diagnostics and suggestions are either empty arrays or not present
    if let Some(diagnostics) = parsed["diagnostics"].as_array() {
        assert!(diagnostics.is_empty());
    }
    if let Some(suggestions) = parsed["suggestions"].as_array() {
        assert!(suggestions.is_empty());
    }
}

#[test]
fn given_derived_envelope_when_deserializing_then_reconstructs_correctly() {
    let original = SimpleResult {
        id: 456,
        name: "Deserialization Test".to_string(),
    };

    let envelope = original.clone().into_envelope();
    let json = serde_json::to_string(&envelope).expect("Should serialize to JSON");

    let deserialized: ResultEnvelope<SimpleResult> =
        serde_json::from_str(&json).expect("Should deserialize from JSON");

    assert!(deserialized.result.is_success());

    if let OperationResult::Success { success: _, data } = deserialized.result {
        assert_eq!(data, original);
    } else {
        panic!("Expected success result");
    }
}

#[test]
fn given_multiple_derived_types_when_creating_envelopes_then_all_validate() {
    let simple = SimpleResult {
        id: 1,
        name: "Simple".to_string(),
    };
    let complex = ComplexResult {
        data: vec!["test".to_string()],
        metadata: std::collections::HashMap::new(),
        optional_field: None,
    };
    let generic = GenericResult {
        value: vec![1, 2, 3],
        timestamp: chrono::Utc::now(),
    };

    let simple_envelope = simple.into_envelope();
    let complex_envelope = complex.into_envelope();
    let generic_envelope = generic.into_envelope();

    assert!(simple_envelope.validate().is_ok());
    assert!(complex_envelope.validate().is_ok());
    assert!(generic_envelope.validate().is_ok());
}
