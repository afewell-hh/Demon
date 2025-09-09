use anyhow::Result;
use async_nats::jetstream::consumer::DeliverPolicy;
use async_nats::jetstream::{self};
use futures_util::StreamExt;
use std::time::Duration;

fn nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string())
}

async fn ensure_stream(js: &jetstream::Context) -> Result<()> {
    let desired = std::env::var("RITUAL_STREAM_NAME").unwrap_or_else(|_| "RITUAL_EVENTS".into());
    let _ = js
        .get_or_create_stream(async_nats::jetstream::stream::Config {
            name: desired,
            subjects: vec!["demon.ritual.v1.>".to_string()],
            ..Default::default()
        })
        .await?;
    Ok(())
}

async fn read_events_for_run(
    js: &jetstream::Context,
    ritual_id: &str,
    run_id: &str,
) -> Result<Vec<serde_json::Value>> {
    let subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);
    let stream = if let Ok(s) = js.get_stream("RITUAL_EVENTS").await {
        s
    } else {
        js.get_stream("DEMON_RITUAL_EVENTS").await?
    };
    let consumer = stream
        .create_consumer(async_nats::jetstream::consumer::pull::Config {
            filter_subject: subject,
            deliver_policy: DeliverPolicy::All,
            ack_policy: async_nats::jetstream::consumer::AckPolicy::None,
            ..Default::default()
        })
        .await?;
    let mut out = Vec::new();
    let mut msgs = consumer
        .batch()
        .max_messages(10_000)
        .expires(Duration::from_secs(2))
        .messages()
        .await?;
    while let Some(m) = msgs.next().await {
        let m = match m {
            Ok(x) => x,
            Err(e) => return Err(anyhow::anyhow!(e.to_string())),
        };
        out.push(serde_json::from_slice(&m.message.payload)?);
    }
    Ok(out)
}

async fn publish_granted(
    js: &jetstream::Context,
    ritual: &str,
    run: &str,
    gate: &str,
) -> Result<()> {
    let subject = format!("demon.ritual.v1.{}.{}.events", ritual, run);
    let now = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "event": "approval.granted:v1",
        "ts": now,
        "tenantId": "default",
        "runId": run,
        "ritualId": ritual,
        "gateId": gate,
        "approver": "ops@example.com",
        "note": "ok"
    });
    let mut hdrs = async_nats::HeaderMap::new();
    hdrs.insert(
        "Nats-Msg-Id",
        format!("{}:approval:{}:granted", run, gate).as_str(),
    );
    js.publish_with_headers(subject, hdrs, serde_json::to_vec(&payload)?.into())
        .await?
        .await?;
    Ok(())
}

#[tokio::test]
#[ignore]
async fn expires_after_ttl_if_no_terminal() -> Result<()> {
    std::env::set_var("APPROVAL_TTL_SECONDS", "2");
    std::env::set_var("RITUAL_STREAM_NAME", "RITUAL_EVENTS");

    let client = async_nats::connect(nats_url()).await?;
    let js = jetstream::new(client);
    ensure_stream(&js).await?;

    let ritual = "ttl-ritual";
    let run = format!("ttl-run-{}", uuid::Uuid::new_v4());
    let gate = "g-expire";

    // Emit requested (schedules timer.scheduled)
    engine::rituals::approvals::await_gate(&run, ritual, gate, "dev", "promote").await?;

    // Assert no terminal yet
    let events = read_events_for_run(&js, ritual, &run).await?;
    let terminals = events
        .iter()
        .filter(|e| {
            e.get("event").and_then(|v| v.as_str()) == Some("approval.granted:v1")
                || e.get("event").and_then(|v| v.as_str()) == Some("approval.denied:v1")
        })
        .count();
    assert_eq!(terminals, 0);

    // Simulate timer wheel firing after TTL by invoking the engine expiry helper
    tokio::time::sleep(Duration::from_millis(2300)).await;
    let emitted = engine::rituals::approvals::process_expiry_if_pending(&run, ritual, gate).await?;
    assert!(emitted, "auto-deny should be emitted when pending");

    // Read back and verify exactly one denied with reason: expired
    let events = read_events_for_run(&js, ritual, &run).await?;
    let denied: Vec<_> = events
        .iter()
        .filter(|e| e.get("event").and_then(|v| v.as_str()) == Some("approval.denied:v1"))
        .collect();
    assert_eq!(denied.len(), 1);
    assert_eq!(denied[0]["reason"], "expired");
    assert_eq!(denied[0]["approver"], "system");
    Ok(())
}

#[tokio::test]
#[ignore]
async fn grant_preempts_expiry_integration() -> Result<()> {
    std::env::set_var("APPROVAL_TTL_SECONDS", "2");
    std::env::set_var("RITUAL_STREAM_NAME", "RITUAL_EVENTS");

    let client = async_nats::connect(nats_url()).await?;
    let js = jetstream::new(client);
    ensure_stream(&js).await?;

    let ritual = "ttl-ritual";
    let run = format!("ttl-run-{}", uuid::Uuid::new_v4());
    let gate = "g-grant";

    engine::rituals::approvals::await_gate(&run, ritual, gate, "dev", "promote").await?;
    // Grant before TTL elapses
    publish_granted(&js, ritual, &run, gate).await?;

    // Sleep past TTL and attempt auto-expiry; should be a no-op
    tokio::time::sleep(Duration::from_millis(2300)).await;
    let emitted = engine::rituals::approvals::process_expiry_if_pending(&run, ritual, gate).await?;
    assert!(!emitted, "expiry should be a no-op when already granted");

    // Verify there is no auto-deny
    let events = read_events_for_run(&js, ritual, &run).await?;
    assert_eq!(
        events
            .iter()
            .filter(|e| e.get("event").and_then(|v| v.as_str()) == Some("approval.denied:v1"))
            .count(),
        0
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| e.get("event").and_then(|v| v.as_str()) == Some("approval.granted:v1"))
            .count(),
        1
    );
    Ok(())
}
