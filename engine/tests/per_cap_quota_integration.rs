use engine::rituals::{guards, log::EventLog};
use serde_json::Value;

fn set_env() {
    std::env::set_var(
        "WARDS_CAPS",
        r#"{"tenant-a":["capsule.http","capsule.echo"]}"#,
    );
    std::env::set_var(
        "WARDS_CAP_QUOTAS",
        r#"{"tenant-a":{"capsule.http":{"limit":1,"windowSeconds":60},"capsule.echo":{"limit":5,"windowSeconds":60}}}"#,
    );
}

#[tokio::test]
#[ignore]
async fn per_cap_quotas_are_independent_and_enforced() {
    set_env();
    let nats_url =
        std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let log = EventLog::new(&nats_url).await.expect("connect log");

    let ritual_id = "quota-test";
    let run_id = uuid::Uuid::new_v4().to_string();
    let tenant = "tenant-a";

    // HTTP: limit 1 → first allowed, remaining 0; then denied
    guards::check_and_emit(&run_id, ritual_id, tenant, "capsule.http")
        .await
        .expect("emit http #1");
    guards::check_and_emit(&run_id, ritual_id, tenant, "capsule.http")
        .await
        .expect("emit http #2");
    guards::check_and_emit(&run_id, ritual_id, tenant, "capsule.http")
        .await
        .expect("emit http #3");

    // ECHO: limit 5 → two allowed
    guards::check_and_emit(&run_id, ritual_id, tenant, "capsule.echo")
        .await
        .expect("emit echo #1");
    guards::check_and_emit(&run_id, ritual_id, tenant, "capsule.echo")
        .await
        .expect("emit echo #2");

    let events = log.read_run(ritual_id, &run_id).await.expect("read run");

    let mut http_events: Vec<Value> = Vec::new();
    let mut echo_events: Vec<Value> = Vec::new();
    for ev in events {
        let v = serde_json::to_value(&ev).unwrap();
        if v.get("event") == Some(&Value::String("policy.decision:v1".into())) {
            match v.get("capability").and_then(|c| c.as_str()) {
                Some("capsule.http") => http_events.push(v.clone()),
                Some("capsule.echo") => echo_events.push(v.clone()),
                _ => {}
            }
        }
    }

    assert!(http_events.len() >= 2, "expected multiple http decisions");
    assert!(echo_events.len() >= 2, "expected multiple echo decisions");

    let http_first = &http_events[0];
    let http_first_rem = http_first["quota"]["remaining"].as_u64().unwrap();
    assert_eq!(
        http_first_rem, 0,
        "first http remaining must be 0 (limit 1)"
    );
    let http_denied = http_events
        .iter()
        .find(|e| e["decision"]["allowed"] == Value::Bool(false))
        .expect("one http decision should be denied");
    assert_eq!(
        http_denied["decision"]["reason"],
        Value::String("limit_exceeded".into())
    );

    let echo_first = &echo_events[0];
    assert_eq!(echo_first["decision"]["allowed"], Value::Bool(true));
    let echo_first_rem = echo_first["quota"]["remaining"].as_u64().unwrap();
    assert!(
        (3..=4).contains(&echo_first_rem),
        "echo remaining should decrement from 5"
    );
}
