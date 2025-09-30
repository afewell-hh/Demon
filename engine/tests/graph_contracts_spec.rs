use jsonschema::JSONSchema;
use std::{fs, path::Path};

#[test]
fn graph_tag_fixtures_validate_against_schema() {
    let schema_path = "../contracts/schemas/events.graph.tag.updated.v1.json";
    let fixtures = [
        "../contracts/fixtures/events/graph.tag.updated.v1.json",
        "../contracts/fixtures/events/graph.tag.deleted.v1.json",
    ];

    assert!(Path::new(schema_path).exists(), "missing {}", schema_path);

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
        assert!(Path::new(fixture_path).exists(), "missing {}", fixture_path);

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

        assert!(
            schema.validate(&instance).is_ok(),
            "fixture {} should validate against schema {}. Validation errors: {:?}",
            fixture_path,
            schema_path,
            schema.validate(&instance).unwrap_err().collect::<Vec<_>>()
        );
    }
}
