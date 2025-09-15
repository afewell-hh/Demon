use anyhow::Result;
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};

use wards::config::{load_from_env, WardsConfig};
use wards::policy::{quota_key, Decision as KernelDecision, PolicyKernel};

static KERNEL: OnceLock<Mutex<PolicyKernel>> = OnceLock::new();

fn kernel() -> &'static Mutex<PolicyKernel> {
    KERNEL.get_or_init(|| {
        let cfg: WardsConfig = load_from_env();
        tracing::info!(
            "PolicyKernel init: counters are process-local; key format 'ten:<tenant>|cap:<capability>'"
        );
        Mutex::new(PolicyKernel::new(cfg))
    })
}

pub async fn check_and_emit(
    run_id: &str,
    ritual_id: &str,
    tenant_id: &str,
    capability: &str,
) -> Result<KernelDecision> {
    let decision = kernel()
        .lock()
        .expect("kernel lock poisoned")
        .allow_and_count(tenant_id, capability);

    let key = quota_key(Some(tenant_id), capability);
    tracing::debug!(quota_key = %key, allowed = %decision.allowed, remaining = %decision.remaining, "policy decision evaluated");

    let decision_json = if decision.allowed {
        json!({ "allowed": true })
    } else {
        json!({ "allowed": false, "reason": "limit_exceeded" })
    };

    let payload = json!({
        "event": "policy.decision:v1",
        "ts": chrono::Utc::now().to_rfc3339(),
        "tenantId": tenant_id,
        "runId": run_id,
        "ritualId": ritual_id,
        "capability": capability,
        "decision": decision_json,
        "quota": quota_json(&decision)
    });

    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = async_nats::connect(&url).await?;
    let js = async_nats::jetstream::new(client.clone());

    let stream_name = std::env::var("RITUAL_STREAM_NAME").ok();
    if let Some(name) = stream_name {
        let _ = js
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name,
                subjects: vec!["demon.ritual.v1.>".to_string()],
                ..Default::default()
            })
            .await?;
    } else {
        const DEFAULT: &str = "RITUAL_EVENTS";
        const DEPRECATED: &str = "DEMON_RITUAL_EVENTS";
        if js.get_stream(DEFAULT).await.is_err() {
            if js.get_stream(DEPRECATED).await.is_ok() {
                tracing::info!(
                    "Using deprecated stream name '{}'; set RITUAL_STREAM_NAME or migrate to '{}'",
                    DEPRECATED,
                    DEFAULT
                );
            } else {
                let _ = js
                    .get_or_create_stream(async_nats::jetstream::stream::Config {
                        name: DEFAULT.to_string(),
                        subjects: vec!["demon.ritual.v1.>".to_string()],
                        ..Default::default()
                    })
                    .await?;
            }
        }
    }

    let subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);
    let mut headers = async_nats::HeaderMap::new();
    let uniq = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let msg_id = format!("{}:decision:{}:{}", run_id, capability, uniq);
    headers.insert("Nats-Msg-Id", msg_id.as_str());
    js.publish_with_headers(subject, headers, serde_json::to_vec(&payload)?.into())
        .await?
        .await?;

    Ok(decision)
}

pub fn quota_json(decision: &KernelDecision) -> Value {
    json!({
        "limit": decision.limit,
        "windowSeconds": decision.window_seconds,
        "remaining": decision.remaining,
    })
}
