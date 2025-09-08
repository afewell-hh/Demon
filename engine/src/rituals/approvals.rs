use anyhow::Result;

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
    // Best-effort emit to JetStream; actual suspension/resume handled by higher layer.
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = async_nats::connect(&url).await?;
    let js = async_nats::jetstream::new(client.clone());

    // Ensure stream exists (align with engine/log.rs)
    let _ = js
        .get_or_create_stream(async_nats::jetstream::stream::Config {
            name: "DEMON_RITUAL_EVENTS".to_string(),
            subjects: vec!["demon.ritual.v1.>".to_string()],
            ..Default::default()
        })
        .await?;

    let payload = serde_json::json!({
        "event": "approval.requested:v1",
        "ts": chrono::Utc::now().to_rfc3339(),
        "tenantId": "default",
        "runId": run_id,
        "ritualId": ritual_id,
        "gateId": gate_id,
        "requester": requester,
        "reason": reason,
    });
    let subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);
    let mut headers = async_nats::HeaderMap::new();
    let msg_id = format!("{}:approval:{}", run_id, gate_id);
    headers.insert("Nats-Msg-Id", msg_id.as_str());
    js.publish_with_headers(subject, headers, serde_json::to_vec(&payload)?.into())
        .await?
        .await?;

    Ok(())
}
