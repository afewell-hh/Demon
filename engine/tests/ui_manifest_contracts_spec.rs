use jsonschema::JSONSchema;
use std::{fs, path::Path};

#[test]
fn ui_manifest_schema_compiles() {
    let schema_path = "../contracts/schemas/ui-manifest.v1.schema.json";
    assert!(Path::new(schema_path).exists(), "missing {schema_path}");

    let schema_text = fs::read_to_string(schema_path).expect(schema_path);
    let schema_json: serde_json::Value = serde_json::from_str(&schema_text).expect("parse schema");

    let compiled = JSONSchema::compile(&schema_json);
    assert!(
        compiled.is_ok(),
        "ui-manifest.v1 schema should compile: {:?}",
        compiled.err()
    );
}

#[test]
fn ui_manifest_fixtures_validate_against_schema() {
    let schema_path = "../contracts/schemas/ui-manifest.v1.schema.json";
    let fixtures = [
        "../contracts/fixtures/ui-manifests/result-envelope.example.v1.json",
        "../contracts/fixtures/ui-manifests/fields-table.example.v1.json",
        "../contracts/fixtures/ui-manifests/markdown-view.example.v1.json",
        "../contracts/fixtures/ui-manifests/json-viewer.example.v1.json",
        "../contracts/fixtures/ui-manifests/multi-card.example.v1.json",
    ];

    assert!(Path::new(schema_path).exists(), "missing {schema_path}");
    let schema_text = fs::read_to_string(schema_path).expect(schema_path);
    let schema = JSONSchema::compile(&serde_json::from_str(&schema_text).expect("parse schema"))
        .expect("schema compiles");

    for fixture_path in fixtures {
        assert!(Path::new(fixture_path).exists(), "missing {fixture_path}");
        let fixture_text = fs::read_to_string(fixture_path).expect(fixture_path);
        let instance: serde_json::Value =
            serde_json::from_str(&fixture_text).expect("parse fixture");

        let validation = schema.validate(&instance);
        assert!(
            validation.is_ok(),
            "fixture {} should validate against ui-manifest.v1.schema.json: {:?}",
            fixture_path,
            validation.err().map(|e| e.collect::<Vec<_>>())
        );
    }
}

#[test]
fn ui_manifest_rejects_invalid_card_kind() {
    let schema_path = "../contracts/schemas/ui-manifest.v1.schema.json";
    let schema_text = fs::read_to_string(schema_path).expect(schema_path);
    let schema = JSONSchema::compile(&serde_json::from_str(&schema_text).expect("parse schema"))
        .expect("schema compiles");

    let invalid_manifest = serde_json::json!({
        "apiVersion": "demon.io/v1",
        "kind": "UIManifest",
        "metadata": {
            "name": "test",
            "version": "1.0.0"
        },
        "cards": [{
            "id": "invalid",
            "kind": "invalid-kind",
            "match": {
                "rituals": ["test"]
            }
        }]
    });

    let validation = schema.validate(&invalid_manifest);
    assert!(
        validation.is_err(),
        "should reject invalid card kind 'invalid-kind'"
    );
}

#[test]
fn ui_manifest_rejects_missing_required_fields() {
    let schema_path = "../contracts/schemas/ui-manifest.v1.schema.json";
    let schema_text = fs::read_to_string(schema_path).expect(schema_path);
    let schema = JSONSchema::compile(&serde_json::from_str(&schema_text).expect("parse schema"))
        .expect("schema compiles");

    // Missing apiVersion
    let invalid1 = serde_json::json!({
        "kind": "UIManifest",
        "metadata": {
            "name": "test",
            "version": "1.0.0"
        },
        "cards": []
    });
    assert!(
        schema.validate(&invalid1).is_err(),
        "should reject missing apiVersion"
    );

    // Missing match.rituals
    let invalid2 = serde_json::json!({
        "apiVersion": "demon.io/v1",
        "kind": "UIManifest",
        "metadata": {
            "name": "test",
            "version": "1.0.0"
        },
        "cards": [{
            "id": "test-card",
            "kind": "json-viewer",
            "match": {}
        }]
    });
    assert!(
        schema.validate(&invalid2).is_err(),
        "should reject card without match.rituals"
    );
}

#[test]
fn ui_manifest_validates_fields_table_config() {
    let schema_path = "../contracts/schemas/ui-manifest.v1.schema.json";
    let schema_text = fs::read_to_string(schema_path).expect(schema_path);
    let schema = JSONSchema::compile(&serde_json::from_str(&schema_text).expect("parse schema"))
        .expect("schema compiles");

    // Valid fields-table with config
    let valid = serde_json::json!({
        "apiVersion": "demon.io/v1",
        "kind": "UIManifest",
        "metadata": {
            "name": "test",
            "version": "1.0.0"
        },
        "cards": [{
            "id": "test-card",
            "kind": "fields-table",
            "match": {
                "rituals": ["test"]
            },
            "config": {
                "fields": [{
                    "label": "Status",
                    "path": "result.success"
                }]
            }
        }]
    });
    assert!(
        schema.validate(&valid).is_ok(),
        "should validate fields-table with proper config"
    );
}

#[test]
fn ui_manifest_rejects_mismatched_kind_and_config() {
    let schema_path = "../contracts/schemas/ui-manifest.v1.schema.json";
    let schema_text = fs::read_to_string(schema_path).expect(schema_path);
    let schema = JSONSchema::compile(&serde_json::from_str(&schema_text).expect("parse schema"))
        .expect("schema compiles");

    // json-viewer kind with fields-table config (mismatch)
    let mismatched = serde_json::json!({
        "apiVersion": "demon.io/v1",
        "kind": "UIManifest",
        "metadata": {
            "name": "test",
            "version": "1.0.0"
        },
        "cards": [{
            "id": "test-card",
            "kind": "json-viewer",
            "match": {
                "rituals": ["test"]
            },
            "config": {
                "fields": [{
                    "label": "Status",
                    "path": "result.success"
                }]
            }
        }]
    });

    let validation = schema.validate(&mismatched);
    assert!(
        validation.is_err(),
        "should reject json-viewer kind with fields-table config: {:?}",
        validation.ok()
    );

    // result-envelope kind with markdown-view config (mismatch)
    let mismatched2 = serde_json::json!({
        "apiVersion": "demon.io/v1",
        "kind": "UIManifest",
        "metadata": {
            "name": "test",
            "version": "1.0.0"
        },
        "cards": [{
            "id": "test-card",
            "kind": "result-envelope",
            "match": {
                "rituals": ["test"]
            },
            "config": {
                "contentPath": "outputs.report"
            }
        }]
    });

    let validation2 = schema.validate(&mismatched2);
    assert!(
        validation2.is_err(),
        "should reject result-envelope kind with markdown-view config: {:?}",
        validation2.ok()
    );
}
