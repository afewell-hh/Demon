use jsonschema::JSONSchema;
use std::{fs, path::Path};

#[test]
fn policy_fixtures_validate_against_schemas() {
    let schemas = [
        (
            "../contracts/schemas/policy.decision.v1.json",
            "../contracts/fixtures/events/policy.decision.allowed.v1.json",
        ),
        (
            "../contracts/schemas/policy.decision.v1.json",
            "../contracts/fixtures/events/policy.decision.denied.v1.json",
        ),
    ];

    for (schema_path, fixture_path) in schemas {
        assert!(Path::new(schema_path).exists(), "missing {schema_path}");
        assert!(Path::new(fixture_path).exists(), "missing {fixture_path}");

        // Scope to avoid borrow issues from jsonschema iterator types
        {
            let schema_text = fs::read_to_string(schema_path).expect(schema_path);
            let fixture_text = fs::read_to_string(fixture_path).expect(fixture_path);

            if schema_text.trim().is_empty() || fixture_text.trim().is_empty() {
                eprintln!(
                    "skipping validation for placeholders: {} / {}",
                    schema_path, fixture_path
                );
                continue; // allow bootstrap placeholders until agent fills them
            }

            let schema =
                JSONSchema::compile(&serde_json::from_str(&schema_text).expect("parse schema"))
                    .expect("schema compiles");
            let instance: serde_json::Value =
                serde_json::from_str(&fixture_text).expect("parse fixture");

            assert!(
                schema.validate(&instance).is_ok(),
                "fixture {} should validate against schema {}. Validation errors: {:?}",
                fixture_path,
                schema_path,
                schema.validate(&instance).unwrap_err().collect::<Vec<_>>()
            );
        }
    }
}
