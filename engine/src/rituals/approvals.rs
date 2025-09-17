use anyhow::Result;
use chrono::{Duration, Utc};
use futures_util::StreamExt;

/// Compute the approval expiry key for a (run, gate).
pub fn expiry_key(run_id: &str, gate_id: &str) -> String {
    format!("{}:approval:{}:expiry", run_id, gate_id)
}

/// Determine whether a terminal approval already exists for a gate in the provided events.
/// Returns Some("granted"|"denied") if terminal found, otherwise None.
pub fn terminal_for_gate(events: &[serde_json::Value], gate_id: &str) -> Option<&'static str> {
    events.iter().rev().find_map(|e| {
        let ev = e.get("event")?.as_str()?;
        let gid = e.get("gateId").and_then(|v| v.as_str());
        if gid != Some(gate_id) {
            return None;
        }
        match ev {
            "approval.granted:v1" => Some("granted"),
            "approval.denied:v1" => Some("denied"),
            _ => None,
        }
    })
}

/// If a terminal approval exists for the given gate, cancel the TTL expiry timer.
/// Returns true if a cancellation was performed.
pub fn preempt_expiry_if_terminal(
    events: &[serde_json::Value],
    run_id: &str,
    gate_id: &str,
    wheel: &mut crate::rituals::timers::TimerWheel,
) -> bool {
    if terminal_for_gate(events, gate_id).is_some() {
        let key = expiry_key(run_id, gate_id);
        wheel.cancel_by_key(&key);
        return true;
    }
    false
}

/// Parse an approvals expiry timer id of the form "{runId}:approval:{gateId}:expiry".
/// Returns Some((run_id, gate_id)) if the format matches, otherwise None.
pub fn parse_approval_expiry_timer_id(timer_id: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = timer_id.split(':').collect();
    if parts.len() == 4 && parts[1] == "approval" && parts[3] == "expiry" {
        Some((parts[0].to_string(), parts[2].to_string()))
    } else {
        None
    }
}

/// Emit approval.requested:v1 exactly once for a given (runId, gateId).
/// Subject: demon.ritual.v1.<ritualId>.<runId>.events
/// Idempotency: Nats-Msg-Id = "<runId>:approval:<gateId>"
pub async fn await_gate(
    run_id: &str,
    ritual_id: &str,
    gate_id: &str,
    requester: &str,
    reason: &str,
) -> Result<()> {
    // Back-compat wrapper: pick TTL from env and forward to the new API
    let ttl_env = std::env::var("APPROVAL_TTL_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    await_gate_with_ttl(run_id, ritual_id, gate_id, requester, reason, Some(ttl_env)).await
}

/// New API: same as `await_gate` but accepts an optional TTL override (seconds).
/// If `ttl_seconds` is 0 or None, expiry is disabled.
pub async fn await_gate_with_ttl(
    run_id: &str,
    ritual_id: &str,
    gate_id: &str,
    requester: &str,
    reason: &str,
    ttl_seconds: Option<u64>,
) -> Result<()> {
    // Best-effort emit to JetStream; actual suspension/resume handled by higher layer.
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = async_nats::connect(&url).await?;
    let js = async_nats::jetstream::new(client.clone());

    // Ensure stream exists with precedence: RITUAL_STREAM_NAME -> existing DEMON_RITUAL_EVENTS (deprecated) -> default RITUAL_EVENTS
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
        // Prefer default; fall back to deprecated if it already exists
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

    let now = chrono::Utc::now();
    let payload = serde_json::json!({
        "event": "approval.requested:v1",
        "ts": now.to_rfc3339(),
        "tenantId": "default",
        "runId": run_id,
        "ritualId": ritual_id,
        "gateId": gate_id,
        "requester": requester,
        "reason": reason,
    });
    let subject = format!("demon.ritual.v1.default.{}.{}.events", ritual_id, run_id);
    let mut headers = async_nats::HeaderMap::new();
    let msg_id = format!("{}:approval:{}", run_id, gate_id);
    headers.insert("Nats-Msg-Id", msg_id.as_str());
    js.publish_with_headers(subject, headers, serde_json::to_vec(&payload)?.into())
        .await?
        .await?;

    // TTL scheduling (optional)
    let ttl = ttl_seconds.unwrap_or(0);
    if ttl > 0 {
        let timer_id = expiry_key(run_id, gate_id);
        let scheduled_for = (now + Duration::seconds(ttl as i64)).to_rfc3339();
        let subject = format!("demon.ritual.v1.default.{}.{}.events", ritual_id, run_id);
        let timer_evt = serde_json::json!({
            "event": "timer.scheduled:v1",
            "ts": now.to_rfc3339(),
            "runId": run_id,
            "timerId": timer_id,
            "scheduledFor": scheduled_for,
        });

        let mut headers = async_nats::HeaderMap::new();
        let msg_id = format!("{}:approval:{}:expiry:scheduled", run_id, gate_id);
        headers.insert("Nats-Msg-Id", msg_id.as_str());
        let _ = js
            .publish_with_headers(subject, headers, serde_json::to_vec(&timer_evt)?.into())
            .await?
            .await?;
    }

    Ok(())
}

/// Process an expiry for a (tenant, run, ritual, gate): if no terminal exists, emit approval.denied:v1.
/// Idempotency key: "{runId}:approval:{gateId}:denied".
pub async fn process_expiry_if_pending(
    tenant: &str,
    run_id: &str,
    ritual_id: &str,
    gate_id: &str,
) -> Result<bool> {
    use async_nats::jetstream;
    // Connect and resolve stream context
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = async_nats::connect(&url).await?;
    let js = jetstream::new(client);

    // Resolve stream by scanning both likely names
    let stream = if let Ok(s) = js.get_stream("RITUAL_EVENTS").await {
        s
    } else {
        js.get_stream("DEMON_RITUAL_EVENTS").await?
    };

    // Try tenant-aware subject first, then fallback to legacy
    let tenant_subject = format!("demon.ritual.v1.{}.{}.{}.events", tenant, ritual_id, run_id);
    let legacy_subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);

    let mut events: Vec<serde_json::Value> = Vec::new();

    // Try tenant-aware format first
    let consumer_result = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: tenant_subject.clone(),
            ..Default::default()
        })
        .await;

    let consumer = match consumer_result {
        Ok(c) => c,
        Err(_) => {
            // Fallback to legacy format
            stream
                .create_consumer(jetstream::consumer::pull::Config {
                    filter_subject: legacy_subject.clone(),
                    ..Default::default()
                })
                .await?
        }
    };

    let mut batch = consumer
        .batch()
        .max_messages(10_000)
        .expires(std::time::Duration::from_secs(2))
        .messages()
        .await?;
    while let Some(m) = batch.next().await {
        let m = match m {
            Ok(x) => x,
            Err(e) => return Err(anyhow::anyhow!(e.to_string())),
        };
        events.push(serde_json::from_slice(&m.message.payload)?);
    }

    if terminal_for_gate(&events, gate_id).is_some() {
        // Already terminal; nothing to emit
        return Ok(false);
    }

    // Emit auto-deny with reason: expired
    let now = Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "event": "approval.denied:v1",
        "ts": now,
        "tenantId": tenant,
        "runId": run_id,
        "ritualId": ritual_id,
        "gateId": gate_id,
        "approver": "system",
        "reason": "expired",
    });
    let subject = format!("demon.ritual.v1.{}.{}.{}.events", tenant, ritual_id, run_id);
    let mut headers = async_nats::HeaderMap::new();
    let msg_id = format!("{}:approval:{}:denied", run_id, gate_id);
    headers.insert("Nats-Msg-Id", msg_id.as_str());
    js.publish_with_headers(subject, headers, serde_json::to_vec(&payload)?.into())
        .await?
        .await?;
    Ok(true)
}
