use jsonschema::JSONSchema;
use std::{fs, path::Path};

#[allow(dead_code)]
fn load(p: &str) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(p).expect(p)).expect(p)
}

#[test]
fn result_envelope_fixtures_validate_against_schema() {
    let schema_path = "../contracts/envelopes/result.json";
    let fixtures = [
        "../contracts/fixtures/envelopes/result_minimal.json",
        "../contracts/fixtures/envelopes/result_full.json",
        "../contracts/fixtures/envelopes/result_error.json",
        "../contracts/fixtures/envelopes/result_with_suggestions.json",
    ];

    assert!(Path::new(schema_path).exists(), "missing {schema_path}");

    let schema_text = fs::read_to_string(schema_path).expect(schema_path);
    if schema_text.trim().is_empty() {
        eprintln!(
            "skipping validation for placeholder schema: {}",
            schema_path
        );
        return;
    }

    let schema = JSONSchema::compile(&serde_json::from_str(&schema_text).expect("parse schema"))
        .expect("schema compiles");

    for fixture_path in fixtures {
        assert!(Path::new(fixture_path).exists(), "missing {fixture_path}");

        let fixture_text = fs::read_to_string(fixture_path).expect(fixture_path);
        if fixture_text.trim().is_empty() {
            eprintln!(
                "skipping validation for placeholder fixture: {}",
                fixture_path
            );
            continue;
        }

        let instance: serde_json::Value =
            serde_json::from_str(&fixture_text).expect("parse fixture");

        let result = schema.validate(&instance);
        if let Err(errors) = result {
            let error_msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
            panic!(
                "fixture {} should validate against schema. Errors:\n{}",
                fixture_path,
                error_msgs.join("\n")
            );
        }
    }
}

#[test]
fn result_envelope_schema_compiles() {
    let schema_path = "../contracts/envelopes/result.json";
    assert!(Path::new(schema_path).exists(), "missing {schema_path}");

    let schema_text = fs::read_to_string(schema_path).expect(schema_path);
    let schema_json: serde_json::Value =
        serde_json::from_str(&schema_text).expect("parse schema as JSON");

    let _compiled_schema = JSONSchema::compile(&schema_json).expect("schema compiles");
}

#[test]
fn result_envelope_minimal_structure() {
    // Test that minimal required structure passes validation
    let minimal_json = serde_json::json!({
        "result": {
            "success": true,
            "data": "test"
        }
    });

    let schema_path = "../contracts/envelopes/result.json";
    let schema_text = fs::read_to_string(schema_path).expect(schema_path);
    let schema = JSONSchema::compile(&serde_json::from_str(&schema_text).expect("parse schema"))
        .expect("schema compiles");

    assert!(
        schema.validate(&minimal_json).is_ok(),
        "minimal structure should validate"
    );
}

#[test]
fn result_envelope_rejects_invalid_diagnostics() {
    // Test that invalid diagnostic levels are rejected
    let invalid_json = serde_json::json!({
        "result": {
            "success": true,
            "data": "test"
        },
        "diagnostics": [{
            "level": "invalid_level", // Should be one of: debug, info, warning, error, fatal
            "message": "test message"
        }]
    });

    let schema_path = "../contracts/envelopes/result.json";
    let schema_text = fs::read_to_string(schema_path).expect(schema_path);
    let schema = JSONSchema::compile(&serde_json::from_str(&schema_text).expect("parse schema"))
        .expect("schema compiles");

    assert!(
        schema.validate(&invalid_json).is_err(),
        "invalid diagnostic level should be rejected"
    );
}

#[test]
fn result_envelope_rejects_invalid_json_patch() {
    // Test that invalid JSON Patch operations are rejected
    let invalid_json = serde_json::json!({
        "result": {
            "success": true,
            "data": "test"
        },
        "suggestions": [{
            "type": "modification",
            "description": "test suggestion",
            "patch": [{
                "op": "invalid_op", // Should be one of: add, remove, replace, move, copy, test
                "path": "/test"
            }]
        }]
    });

    let schema_path = "../contracts/envelopes/result.json";
    let schema_text = fs::read_to_string(schema_path).expect(schema_path);
    let schema = JSONSchema::compile(&serde_json::from_str(&schema_text).expect("parse schema"))
        .expect("schema compiles");

    assert!(
        schema.validate(&invalid_json).is_err(),
        "invalid JSON Patch operation should be rejected"
    );
}
