//! Ignored by default; requires NATS dev environment.
use chrono::{Duration, Utc};
use futures_util::StreamExt;

#[tokio::test]
#[ignore]
async fn scheduled_then_expire_once() {
    std::env::set_var("APPROVAL_TTL_SECONDS", "2");
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let client = async_nats::connect(&nats_url).await.unwrap();
    let js = async_nats::jetstream::new(client.clone());

    // Ensure stream present
    let _ = js
        .get_or_create_stream(async_nats::jetstream::stream::Config {
            name: "RITUAL_EVENTS".into(),
            subjects: vec!["demon.ritual.v1.>".into()],
            ..Default::default()
        })
        .await
        .unwrap();

    // Seed approval.requested and timer.scheduled
    let run = "run-ttl-worker-1";
    let ritual = "ritual-ttl";
    let gate = "gw";
    let subject = format!("demon.ritual.v1.{}.{}.events", ritual, run);
    let now = Utc::now();
    let requested = serde_json::json!({
        "event":"approval.requested:v1","ts":now.to_rfc3339(),
        "tenantId":"default","runId":run,"ritualId":ritual,"gateId":gate,
        "requester":"dev@example.com","reason":"promote"});
    js.publish(
        subject.clone(),
        serde_json::to_vec(&requested).unwrap().into(),
    )
    .await
    .unwrap()
    .await
    .unwrap();

    let key = engine::rituals::approvals::expiry_key(run, gate);
    let scheduled = serde_json::json!({
        "event":"timer.scheduled:v1","ts":now.to_rfc3339(),
        "runId":run,"timerId":key,"scheduledFor":(now+Duration::seconds(2)).to_rfc3339()});
    js.publish(
        subject.clone(),
        serde_json::to_vec(&scheduled).unwrap().into(),
    )
    .await
    .unwrap()
    .await
    .unwrap();

    // Run one batch of worker to process messages
    let cfg = engine::rituals::worker::ttl_worker::TtlWorkerConfig::default();
    let _ = engine::rituals::worker::ttl_worker::run_one_batch(cfg)
        .await
        .unwrap();

    // Read back events and ensure exactly one denial exists
    let stream = js.get_stream("RITUAL_EVENTS").await.unwrap();
    let cons = stream
        .create_consumer(async_nats::jetstream::consumer::pull::Config {
            filter_subject: subject.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    let mut vals = Vec::new();
    let mut batch = cons
        .batch()
        .max_messages(100)
        .expires(std::time::Duration::from_secs(1))
        .messages()
        .await
        .unwrap();
    while let Some(m) = batch.next().await {
        let m = m.unwrap();
        vals.push(serde_json::from_slice::<serde_json::Value>(&m.message.payload).unwrap());
    }
    let denies: Vec<_> = vals
        .into_iter()
        .filter(|v| v.get("event").and_then(|s| s.as_str()) == Some("approval.denied:v1"))
        .collect();
    assert_eq!(denies.len(), 1, "expected exactly one expired denial");
}
