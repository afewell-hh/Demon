use jsonschema::JSONSchema;
use serde_json::Value;
use std::fs;

#[test]
fn fixtures_validate_against_schemas() {
    // Validate ritual.started.v1
    let schema_started = fs::read_to_string("../contracts/schemas/events.ritual.started.v1.json")
        .expect("should read started schema");
    let schema = JSONSchema::compile(&serde_json::from_str(&schema_started).unwrap())
        .expect("should compile schema");

    let fixture_started = fs::read_to_string("../contracts/fixtures/events/ritual.started.v1.json")
        .expect("should read started fixture");
    let instance: Value = serde_json::from_str(&fixture_started).expect("should parse fixture");

    assert!(
        schema.validate(&instance).is_ok(),
        "started fixture should validate"
    );

    // Validate ritual.transitioned.v1
    let schema_transitioned =
        fs::read_to_string("../contracts/schemas/events.ritual.transitioned.v1.json")
            .expect("should read transitioned schema");
    let schema = JSONSchema::compile(&serde_json::from_str(&schema_transitioned).unwrap())
        .expect("should compile schema");

    let fixture_transitioned =
        fs::read_to_string("../contracts/fixtures/events/ritual.transitioned.v1.json")
            .expect("should read transitioned fixture");
    let instance: Value =
        serde_json::from_str(&fixture_transitioned).expect("should parse fixture");

    assert!(
        schema.validate(&instance).is_ok(),
        "transitioned fixture should validate"
    );

    // Validate ritual.completed.v1
    let schema_completed =
        fs::read_to_string("../contracts/schemas/events.ritual.completed.v1.json")
            .expect("should read completed schema");
    let schema = JSONSchema::compile(&serde_json::from_str(&schema_completed).unwrap())
        .expect("should compile schema");

    let fixture_completed =
        fs::read_to_string("../contracts/fixtures/events/ritual.completed.v1.json")
            .expect("should read completed fixture");
    let instance: Value = serde_json::from_str(&fixture_completed).expect("should parse fixture");

    assert!(
        schema.validate(&instance).is_ok(),
        "completed fixture should validate"
    );
}

#[tokio::test]
#[ignore]
async fn append_read_replay_is_deterministic() {
    // TODO: Implement after log.rs and state.rs are ready
    // 1) publish started → state.transitioned → completed (with msg-id headers)
    // 2) read back for runId
    // 3) apply via state.rs and assert final status == Completed
    // 4) republish same events with identical msg-ids; assert no duplicates and same result
}

#[tokio::test]
#[ignore]
async fn idempotent_publish_prevents_duplicates() {
    // TODO: Test that publishing with same Nats-Msg-Id doesn't create duplicates
}

#[tokio::test]
#[ignore]
async fn replay_from_partial_log_works() {
    // TODO: Test replaying from a log that only has started event
}
