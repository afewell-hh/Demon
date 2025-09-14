use anyhow::Result;
use async_nats::jetstream;
use chrono::Utc;
use futures_util::StreamExt;
use reqwest::StatusCode;
use tokio::task;

async fn start_ui() -> Result<u16> {
    // Bind ephemeral port
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let state = operate_ui::AppState::new().await;
    let app = operate_ui::create_app(state);
    task::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Ok(port)
}

async fn ensure_stream() -> Result<()> {
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = async_nats::connect(url).await?;
    let js = jetstream::new(client);
    let name = std::env::var("RITUAL_STREAM_NAME").unwrap_or_else(|_| "RITUAL_EVENTS".into());
    let _ = js
        .get_or_create_stream(jetstream::stream::Config {
            name,
            subjects: vec!["demon.ritual.v1.>".to_string()],
            ..Default::default()
        })
        .await?;
    Ok(())
}

async fn fetch_events_for_run(
    js: &jetstream::Context,
    ritual_id: &str,
    run_id: &str,
) -> Result<Vec<serde_json::Value>> {
    let stream = js
        .get_stream("RITUAL_EVENTS")
        .await
        .or(js.get_stream("DEMON_RITUAL_EVENTS").await)
        .expect("stream exists");
    let subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: subject,
            ..Default::default()
        })
        .await?;
    let mut out = Vec::new();
    let mut msgs = consumer
        .batch()
        .max_messages(10_000)
        .expires(std::time::Duration::from_secs(1))
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

async fn publish_requested(
    js: &jetstream::Context,
    ritual: &str,
    run: &str,
    gate: &str,
) -> Result<()> {
    let subject = format!("demon.ritual.v1.{}.{}.events", ritual, run);
    let payload = serde_json::json!({
        "event": "approval.requested:v1",
        "ts": Utc::now().to_rfc3339(),
        "tenantId": "tenant-a",
        "runId": run,
        "ritualId": ritual,
        "gateId": gate,
        "requester": "dev@example.com",
        "reason": "promote"
    });
    let mut hdrs = async_nats::HeaderMap::new();
    let msg_id = format!("{}:approval:{}", run, gate);
    hdrs.insert("Nats-Msg-Id", msg_id.as_str());
    js.publish_with_headers(subject, hdrs, serde_json::to_vec(&payload)?.into())
        .await?
        .await?;
    Ok(())
}

#[tokio::test]
#[ignore]
async fn grant_then_grant_is_noop_and_deny_conflicts() -> Result<()> {
    std::env::set_var("APPROVER_ALLOWLIST", "ops@example.com");
    std::env::set_var("RITUAL_STREAM_NAME", "RITUAL_EVENTS");
    ensure_stream().await?;
    let port = start_ui().await?;
    let base = format!("http://127.0.0.1:{}", port);

    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let client = async_nats::connect(url).await?;
    let js = jetstream::new(client);

    let ritual = "echo-ritual";
    let run = format!("rr-routes-{}", uuid::Uuid::new_v4());
    let gate = "gate-1";
    publish_requested(&js, ritual, &run, gate).await?;

    let http = reqwest::Client::new();
    // First grant
    let r1 = http
        .post(format!("{}/api/approvals/{}/{}/grant", base, run, gate))
        .json(&serde_json::json!({"approver":"ops@example.com","note":"ok"}))
        .send()
        .await?;
    assert_eq!(r1.status(), StatusCode::OK);

    // Duplicate grant -> 200 noop
    let r2 = http
        .post(format!("{}/api/approvals/{}/{}/grant", base, run, gate))
        .json(&serde_json::json!({"approver":"ops@example.com"}))
        .send()
        .await?;
    assert_eq!(r2.status(), StatusCode::OK);
    let body2: serde_json::Value = r2.json().await?;
    assert_eq!(body2["status"], "noop");

    // Conflicting deny -> 409
    let r3 = http
        .post(format!("{}/api/approvals/{}/{}/deny", base, run, gate))
        .json(&serde_json::json!({"approver":"ops@example.com","reason":"oops"}))
        .send()
        .await?;
    assert_eq!(r3.status(), StatusCode::CONFLICT);

    // Verify exactly one terminal event
    let events = fetch_events_for_run(&js, ritual, &run).await?;
    let terminals = events
        .iter()
        .filter(|e| {
            e.get("event").and_then(|v| v.as_str()) == Some("approval.granted:v1")
                || e.get("event").and_then(|v| v.as_str()) == Some("approval.denied:v1")
        })
        .count();
    assert_eq!(terminals, 1);
    Ok(())
}
