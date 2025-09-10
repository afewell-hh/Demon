use anyhow::{anyhow, Context, Result};
use async_nats::jetstream;
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapConfig {
    pub profile: Profile,
    pub nats_url: String,
    pub stream_name: String,
    pub subjects: Vec<String>,
    pub dedupe_window_secs: u64,
    pub ui_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    LocalDev,
    RemoteNats,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| {
            let host = std::env::var("NATS_HOST").unwrap_or_else(|_| "127.0.0.1".into());
            let port = std::env::var("NATS_PORT").unwrap_or_else(|_| "4222".into());
            format!("nats://{}:{}", host, port)
        });
        let stream_name = std::env::var("RITUAL_STREAM_NAME")
            .or_else(|_| std::env::var("DEMON_RITUAL_EVENTS"))
            .unwrap_or_else(|_| "RITUAL_EVENTS".into());
        let subjects = std::env::var("RITUAL_SUBJECTS")
            .ok()
            .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
            .unwrap_or_else(|| vec!["demon.ritual.v1.>".into()]);
        let ui_url = std::env::var("UI_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".into());
        Self {
            profile: Profile::LocalDev,
            nats_url,
            stream_name,
            subjects,
            dedupe_window_secs: 120,
            ui_url,
        }
    }
}

pub async fn ensure_stream(cfg: &BootstrapConfig) -> Result<jetstream::stream::Stream> {
    let client = async_nats::connect(&cfg.nats_url).await?;
    let js = jetstream::new(client);
    let mut sc = jetstream::stream::Config {
        name: cfg.stream_name.clone(),
        subjects: cfg.subjects.clone(),
        ..Default::default()
    };
    sc.duplicate_window = std::time::Duration::from_secs(cfg.dedupe_window_secs);
    let stream = js.get_or_create_stream(sc).await?;
    Ok(stream)
}

pub async fn seed_preview_min(js: &jetstream::Context, ritual: &str) -> Result<()> {
    let tenant = "default";
    let run_b = "bootstrap-run-b";
    let run_c = "bootstrap-run-c";
    let gate_b = "gate-b";
    let gate_c = "gate-c";
    let subject = |run: &str| format!("demon.ritual.v1.{}.{}.events", ritual, run);
    let now = || Utc::now().to_rfc3339();

    // approval.requested (B)
    let req_b = serde_json::json!({
        "event": "approval.requested:v1", "ts": now(), "tenantId": tenant,
        "runId": run_b, "ritualId": ritual, "gateId": gate_b, "requester": "dev@example.com", "reason": "promote"
    });
    publish_idem(
        js,
        &subject(run_b),
        &req_b,
        &format!("{}:approval:{}", run_b, gate_b),
    )
    .await?;
    // grant via event for seeding purposes (UI also grants via REST in other paths)
    let grant_b = serde_json::json!({
        "event": "approval.granted:v1", "ts": now(), "tenantId": tenant,
        "runId": run_b, "ritualId": ritual, "gateId": gate_b, "approver": "ops@example.com", "note": "ok"
    });
    publish_idem(
        js,
        &subject(run_b),
        &grant_b,
        &format!("{}:approval:{}:granted", run_b, gate_b),
    )
    .await?;

    // approval.requested + timer.scheduled (C)
    let req_c = serde_json::json!({
        "event": "approval.requested:v1", "ts": now(), "tenantId": tenant,
        "runId": run_c, "ritualId": ritual, "gateId": gate_c, "requester": "dev@example.com", "reason": "promote"
    });
    publish_idem(
        js,
        &subject(run_c),
        &req_c,
        &format!("{}:approval:{}", run_c, gate_c),
    )
    .await?;
    let timer_id = format!("{}:approval:{}:expiry", run_c, gate_c);
    let timer = serde_json::json!({
        "event": "timer.scheduled:v1", "ts": now(), "runId": run_c, "timerId": timer_id,
        "scheduledFor": (Utc::now() + chrono::Duration::seconds(5)).to_rfc3339()
    });
    publish_idem(
        js,
        &subject(run_c),
        &timer,
        &format!("{}:approval:{}:expiry:scheduled", run_c, gate_c),
    )
    .await?;
    Ok(())
}

async fn publish_idem(
    js: &jetstream::Context,
    subject: &str,
    value: &serde_json::Value,
    key: &str,
) -> Result<()> {
    let mut h = async_nats::HeaderMap::new();
    h.insert("Nats-Msg-Id", key);
    js.publish_with_headers(subject.to_string(), h, serde_json::to_vec(value)?.into())
        .await?
        .await?;
    Ok(())
}

pub async fn verify_ui(ui_url: &str) -> Result<()> {
    let c = reqwest::Client::builder().build()?;
    // Runs array
    let runs: serde_json::Value = c
        .get(format!("{}/api/runs", ui_url))
        .send()
        .await
        .context("failed GET /api/runs")?
        .error_for_status()?
        .json()
        .await?;
    let len = runs.as_array().map(|a| a.len()).unwrap_or(0);
    if len < 1 {
        return Err(anyhow!("verify: /api/runs returned empty array"));
    }
    // Basic HTML render smoke (since admin report endpoint may not exist yet)
    let html = c
        .get(format!("{}/runs", ui_url))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    if !html.to_lowercase().contains("<!doctype html>") {
        return Err(anyhow!("verify: /runs did not return HTML"));
    }
    Ok(())
}
