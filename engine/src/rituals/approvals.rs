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
