use anyhow::Result;
use async_nats::jetstream::{self, consumer::DeliverPolicy};
use futures_util::StreamExt;
use serde_json::Value;
use std::time::Duration;
use tokio::task;

fn nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string())
}

async fn read_events_for_run(
    js: &jetstream::Context,
    ritual_id: &str,
    run_id: &str,
) -> Result<Vec<Value>> {
    // Try new tenant-aware pattern first
    let subject = format!("demon.ritual.v1.default.{}.{}.events", ritual_id, run_id);

    // Resolve stream by scanning both likely names
    let stream = if let Ok(s) = js.get_stream("RITUAL_EVENTS").await {
        s
    } else {
        js.get_stream("DEMON_RITUAL_EVENTS").await?
    };

    // Try tenant-aware consumer first
    let consumer_result = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: subject.clone(),
            deliver_policy: DeliverPolicy::All,
            ack_policy: jetstream::consumer::AckPolicy::None,
            ..Default::default()
        })
        .await;

    let consumer = match consumer_result {
        Ok(c) => c,
        Err(_) => {
            // Fallback to legacy pattern
            let legacy_subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);
            stream
                .create_consumer(jetstream::consumer::pull::Config {
                    filter_subject: legacy_subject.clone(),
                    deliver_policy: DeliverPolicy::All,
                    ack_policy: jetstream::consumer::AckPolicy::None,
                    ..Default::default()
                })
                .await?
        }
    };

    let mut out = Vec::new();
    let mut batch = consumer
        .batch()
        .max_messages(10_000)
        .expires(Duration::from_secs(2))
        .messages()
        .await?;
    while let Some(m) = batch.next().await {
        let m = match m {
            Ok(x) => x,
            Err(e) => return Err(anyhow::anyhow!(e.to_string())),
        };
        let v: Value = serde_json::from_slice(&m.message.payload)?;
        out.push(v);
    }
    Ok(out)
}

async fn start_operate_ui() -> Result<u16> {
    // Bind to an ephemeral port
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();

    // Start app with real JetStream client
    let state = operate_ui::AppState::new().await;
    let app = operate_ui::create_app(state);
    task::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Ok(port)
}

#[tokio::test]
#[ignore]
async fn requested_once_then_grant_resumes_once() -> Result<()> {
    std::env::set_var("APPROVER_ALLOWLIST", "ops@example.com");

    // Bring up UI
    let port = start_operate_ui().await?;

    // Emit approval.requested via engine hook
    let run_id = format!("int-run-{}", uuid::Uuid::new_v4());
    let ritual_id = "int-ritual";
    let gate_id = "gate-1";
    engine::rituals::approvals::await_gate(&run_id, ritual_id, gate_id, "requester", "reason")
        .await?;

    // Verify exactly one approval.requested
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);
    let events = read_events_for_run(&js, ritual_id, &run_id).await?;
    assert_eq!(
        events
            .iter()
            .filter(|e| e["event"] == "approval.requested:v1")
            .count(),
        1
    );

    // Grant via Operate UI endpoint (twice for idempotency)
    let url = format!(
        "http://127.0.0.1:{}/api/approvals/{}/ {}/grant",
        port, run_id, gate_id
    )
    .replace(" ", "");
    let body = serde_json::json!({"approver":"ops@example.com","note":"ok"});
    let http = reqwest::Client::new();
    let r1 = http
        .post(&url)
        .header("X-Requested-With", "XMLHttpRequest")
        .json(&body)
        .send()
        .await?;
    assert!(r1.status().is_success());
    let r2 = http
        .post(&url)
        .header("X-Requested-With", "XMLHttpRequest")
        .json(&body)
        .send()
        .await?;
    assert!(r2.status().is_success());

    // Validate exactly one approval.granted and zero denied
    let events = read_events_for_run(&js, ritual_id, &run_id).await?;
    assert_eq!(
        events
            .iter()
            .filter(|e| e["event"] == "approval.granted:v1")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| e["event"] == "approval.denied:v1")
            .count(),
        0
    );
    Ok(())
}

#[tokio::test]
#[ignore]
async fn requested_once_then_deny_halts() -> Result<()> {
    std::env::set_var("APPROVER_ALLOWLIST", "ops@example.com");
    let port = start_operate_ui().await?;

    let run_id = format!("int-run-{}", uuid::Uuid::new_v4());
    let ritual_id = "int-ritual";
    let gate_id = "gate-2";
    engine::rituals::approvals::await_gate(&run_id, ritual_id, gate_id, "requester", "reason")
        .await?;

    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    // Deny twice for idempotency
    let url = format!(
        "http://127.0.0.1:{}/api/approvals/{}/{}/deny",
        port, run_id, gate_id
    );
    let body = serde_json::json!({"approver":"ops@example.com","reason":"hold"});
    let http = reqwest::Client::new();
    let r1 = http
        .post(&url)
        .header("X-Requested-With", "XMLHttpRequest")
        .json(&body)
        .send()
        .await?;
    assert!(r1.status().is_success());
    let r2 = http
        .post(&url)
        .header("X-Requested-With", "XMLHttpRequest")
        .json(&body)
        .send()
        .await?;
    assert!(r2.status().is_success());

    let events = read_events_for_run(&js, ritual_id, &run_id).await?;
    assert_eq!(
        events
            .iter()
            .filter(|e| e["event"] == "approval.denied:v1")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| e["event"] == "approval.granted:v1")
            .count(),
        0
    );
    Ok(())
}
