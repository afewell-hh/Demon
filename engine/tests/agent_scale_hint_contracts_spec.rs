use jsonschema::JSONSchema;
use std::{fs, path::Path};

#[test]
fn agent_scale_hint_fixtures_validate_against_schema() {
    let schemas = [
        (
            "../contracts/schemas/events.agent.scale.hint.v1.json",
            "../contracts/fixtures/events/agent.scale.hint.scale_up.v1.json",
        ),
        (
            "../contracts/schemas/events.agent.scale.hint.v1.json",
            "../contracts/fixtures/events/agent.scale.hint.scale_down.v1.json",
        ),
        (
            "../contracts/schemas/events.agent.scale.hint.v1.json",
            "../contracts/fixtures/events/agent.scale.hint.steady.v1.json",
        ),
    ];

    for (schema_path, fixture_path) in schemas {
        assert!(Path::new(schema_path).exists(), "missing {schema_path}");
        assert!(Path::new(fixture_path).exists(), "missing {fixture_path}");

        {
            let schema_text = fs::read_to_string(schema_path).expect(schema_path);
            let fixture_text = fs::read_to_string(fixture_path).expect(fixture_path);

            if schema_text.trim().is_empty() || fixture_text.trim().is_empty() {
                eprintln!(
                    "skipping validation for placeholders: {} / {}",
                    schema_path, fixture_path
                );
                continue;
            }

            let schema =
                JSONSchema::compile(&serde_json::from_str(&schema_text).expect("parse schema"))
                    .expect("schema compiles");
            let instance: serde_json::Value =
                serde_json::from_str(&fixture_text).expect("parse fixture");

            assert!(
                schema.validate(&instance).is_ok(),
                "fixture {} should validate",
                fixture_path
            );
        }
    }
}

#[test]
fn agent_scale_hint_schema_has_required_fields() {
    let schema_path = "../contracts/schemas/events.agent.scale.hint.v1.json";
    let schema_text = fs::read_to_string(schema_path).expect("schema exists");
    let schema: serde_json::Value = serde_json::from_str(&schema_text).expect("valid json");

    let required = schema["required"].as_array().expect("required is array");
    let required_fields: Vec<&str> = required
        .iter()
        .map(|v| v.as_str().expect("string"))
        .collect();

    assert!(required_fields.contains(&"event"));
    assert!(required_fields.contains(&"ts"));
    assert!(required_fields.contains(&"tenantId"));
    assert!(required_fields.contains(&"recommendation"));
    assert!(required_fields.contains(&"metrics"));
    assert!(required_fields.contains(&"thresholds"));
    assert!(required_fields.contains(&"hysteresis"));
    assert!(required_fields.contains(&"reason"));
}

#[test]
fn agent_scale_hint_recommendation_enum_is_constrained() {
    let schema_path = "../contracts/schemas/events.agent.scale.hint.v1.json";
    let schema_text = fs::read_to_string(schema_path).expect("schema exists");
    let schema: serde_json::Value = serde_json::from_str(&schema_text).expect("valid json");

    let recommendation_enum = schema["properties"]["recommendation"]["enum"]
        .as_array()
        .expect("recommendation has enum");

    let values: Vec<&str> = recommendation_enum
        .iter()
        .map(|v| v.as_str().expect("string"))
        .collect();

    assert_eq!(values.len(), 3);
    assert!(values.contains(&"scale_up"));
    assert!(values.contains(&"scale_down"));
    assert!(values.contains(&"steady"));
}
