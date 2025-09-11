use anyhow::{anyhow, Context, Result};
pub mod bundle;
pub mod libindex;
pub mod provenance;
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

pub fn compute_effective_config(
    bundle_path: Option<&std::path::Path>,
    flag_nats_url: Option<&str>,
    flag_stream_name: Option<&str>,
    flag_ui_url: Option<&str>,
) -> (BootstrapConfig, serde_json::Value) {
    let mut cfg = BootstrapConfig::default();
    let mut provenance = serde_json::json!({});
    if let Some(path) = bundle_path {
        if let Ok(b) = crate::bundle::load_bundle(path) {
            if cfg.nats_url.is_empty() {
                cfg.nats_url = b.nats.url;
                provenance["nats_url"] = "bundle".into();
            }
            if cfg.stream_name.is_empty() {
                cfg.stream_name = b.stream.name;
                provenance["stream_name"] = "bundle".into();
            }
            if cfg.subjects.is_empty() {
                cfg.subjects = b.stream.subjects;
                provenance["subjects"] = "bundle".into();
            }
            if cfg.dedupe_window_secs == 0 {
                cfg.dedupe_window_secs = b.stream.duplicate_window_seconds;
                provenance["dedupe_window_secs"] = "bundle".into();
            }
            if cfg.ui_url.is_empty() {
                if let Some(u) = b.operate_ui.base_url {
                    cfg.ui_url = u;
                    provenance["ui_url"] = "bundle".into();
                }
            }
        }
    }
    if let Some(v) = flag_nats_url {
        cfg.nats_url = v.to_string();
        provenance["nats_url"] = "flag".into();
    }
    if let Some(v) = flag_stream_name {
        cfg.stream_name = v.to_string();
        provenance["stream_name"] = "flag".into();
    }
    if let Some(v) = flag_ui_url {
        cfg.ui_url = v.to_string();
        provenance["ui_url"] = "flag".into();
    }
    if provenance["nats_url"].is_null() {
        provenance["nats_url"] = "env|default".into();
    }
    if provenance["stream_name"].is_null() {
        provenance["stream_name"] = "env|default".into();
    }
    if provenance["ui_url"].is_null() {
        provenance["ui_url"] = "env|default".into();
    }
    (cfg, provenance)
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

pub async fn seed_preview_min(js: &jetstream::Context, ritual: &str, ui_url: &str) -> Result<()> {
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
    // grant via REST to exercise allow-list and first-writer-wins
    grant_via_rest(ui_url, run_b, gate_b).await?;

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
    // Admin JSON probe
    let probe: serde_json::Value = c
        .get(format!("{}/admin/templates/report", ui_url))
        .send()
        .await
        .context("failed GET /admin/templates/report")?
        .error_for_status()?
        .json()
        .await?;
    let tr = probe
        .as_object()
        .ok_or_else(|| anyhow!("invalid admin probe JSON"))?;
    if tr.get("template_ready").and_then(|v| v.as_bool()) != Some(true) {
        return Err(anyhow!("verify: admin probe template_ready!=true"));
    }
    if tr.get("has_filter_tojson").and_then(|v| v.as_bool()) != Some(true) {
        return Err(anyhow!("verify: admin probe has_filter_tojson!=true"));
    }
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
    Ok(())
}

async fn grant_via_rest(ui_url: &str, run_id: &str, gate_id: &str) -> Result<()> {
    let c = reqwest::Client::builder().build()?;
    let url = format!("{}/api/approvals/{}/{}/grant", ui_url, run_id, gate_id);
    let body = serde_json::json!({"approver":"ops@example.com","note":"bootstrap grant"});
    let resp = c.post(url).json(&body).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("grant REST failed: {}", resp.status()));
    }
    Ok(())
}

// Compatibility helpers expected by existing CLI and tests
use crate::bundle::Bundle;

pub fn build_seed_run_log(
    run_id: &str,
    ritual_id: &str,
    gate_id: &str,
    applied_req: bool,
    applied_timer: Option<bool>,
) -> serde_json::Value {
    serde_json::json!({
        "phase":"seed_run",
        "runId": run_id,
        "ritualId": ritual_id,
        "gateId": gate_id,
        "mutation_req": if applied_req {"applied"} else {"noop"},
        "mutation_timer": applied_timer.map(|b| if b {"applied"} else {"noop"}).unwrap_or("noop"),
    })
}

pub async fn seed_from_bundle(
    js: &async_nats::jetstream::Context,
    b: &Bundle,
    _ui_url: &str,
) -> Result<()> {
    if !b.seed.enabled.unwrap_or(true) {
        return Ok(());
    }
    if let Some(runs) = &b.seed.runs {
        for run in runs {
            if let Some(gates) = &run.gates {
                for g in gates {
                    // Publish request event
                    let subject =
                        format!("demon.ritual.v1.{}.{}.events", run.ritual_id, run.run_id);
                    let evt = serde_json::json!({
                        "event":"approval.requested:v1",
                        "ts": chrono::Utc::now().to_rfc3339(),
                        "tenantId":"default",
                        "runId": run.run_id,
                        "ritualId": run.ritual_id,
                        "gateId": g.gate_id,
                        "requester": g.requester,
                    });
                    let _ = publish_idem(
                        js,
                        &subject,
                        &evt,
                        &format!("{}:approval:{}", run.run_id, g.gate_id),
                    )
                    .await;
                    println!(
                        "{}",
                        build_seed_run_log(&run.run_id, &run.ritual_id, &g.gate_id, false, None)
                    );
                }
            }
        }
    }
    Ok(())
}

pub async fn verify_ui_with_token(ui_url: &str, token: Option<String>) -> Result<()> {
    let c = reqwest::Client::builder().build()?;
    let mut req = c.get(format!("{}/api/runs", ui_url));
    if let Some(t) = token.clone() {
        req = req.header("X-Admin-Token", t);
    }
    let runs: serde_json::Value = req.send().await?.error_for_status()?.json().await?;
    let len = runs.as_array().map(|a| a.len()).unwrap_or(0);
    if len < 1 {
        return Err(anyhow!("verify: /api/runs returned empty array"));
    }
    // HTML check
    let mut req2 = c.get(format!("{}/runs", ui_url));
    if let Some(t) = token {
        req2 = req2.header("X-Admin-Token", t);
    }
    let html = req2.send().await?.error_for_status()?.text().await?;
    if !html.to_lowercase().contains("<!doctype html>") {
        return Err(anyhow!("verify: /runs did not return HTML"));
    }
    Ok(())
}
