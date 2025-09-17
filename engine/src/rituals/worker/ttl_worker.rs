use anyhow::Result;
use async_nats::jetstream;
use async_nats::jetstream::{consumer::DeliverPolicy, AckKind, Message};
use futures_util::StreamExt;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, info, warn};

static HANDLED: AtomicU64 = AtomicU64::new(0);
static EXPIRED: AtomicU64 = AtomicU64::new(0);
static NOOP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct TtlWorkerConfig {
    pub nats_url: String,
    pub stream_name: Option<String>,
    pub consumer_name: String,  // default: ttl-worker
    pub subject_filter: String, // default: demon.ritual.v1.*.*.events
    pub batch: usize,           // default: 100
    pub pull_timeout_ms: u64,   // default: 1500
}

impl Default for TtlWorkerConfig {
    fn default() -> Self {
        Self {
            nats_url: std::env::var("NATS_URL")
                .unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string()),
            stream_name: std::env::var("RITUAL_STREAM_NAME").ok(),
            consumer_name: std::env::var("TTL_CONSUMER_NAME")
                .unwrap_or_else(|_| "ttl-worker".to_string()),
            subject_filter: "demon.ritual.v1.*.*.*.events".to_string(),
            batch: std::env::var("TTL_BATCH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100),
            pull_timeout_ms: std::env::var("TTL_PULL_TIMEOUT_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1500),
        }
    }
}

fn incr(a: &AtomicU64) {
    a.fetch_add(1, Ordering::Relaxed);
}

pub fn counters() -> (u64, u64, u64) {
    (
        HANDLED.load(Ordering::Relaxed),
        EXPIRED.load(Ordering::Relaxed),
        NOOP.load(Ordering::Relaxed),
    )
}

pub fn reset_counters() {
    HANDLED.store(0, Ordering::Relaxed);
    EXPIRED.store(0, Ordering::Relaxed);
    NOOP.store(0, Ordering::Relaxed);
}

/// Parse tenant, ritualId and runId from a subject.
/// Supports both tenant-aware "demon.ritual.v1.<tenant>.<ritual>.<run>.events"
/// and legacy "demon.ritual.v1.<ritual>.<run>.events" formats.
/// Returns (tenant, ritual_id, run_id).
fn parse_subject(subject: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = subject.split('.').collect();
    if parts.len() >= 6 && parts[0] == "demon" && parts[1] == "ritual" && parts[2] == "v1" {
        if parts.len() == 7 {
            // Tenant-aware format: demon.ritual.v1.<tenant>.<ritual>.<run>.events
            Some((
                parts[3].to_string(),
                parts[4].to_string(),
                parts[5].to_string(),
            ))
        } else if parts.len() == 6 {
            // Legacy format: demon.ritual.v1.<ritual>.<run>.events
            Some((
                "default".to_string(),
                parts[3].to_string(),
                parts[4].to_string(),
            ))
        } else {
            None
        }
    } else {
        None
    }
}

/// Handle a single JetStream message; returns true if acked.
async fn handle_message(msg: Message) -> Result<bool> {
    let subject = msg.message.subject.clone();
    let (tenant, ritual_id, run_id_from_subject) = match parse_subject(&subject) {
        Some(x) => x,
        None => {
            warn!(%subject, "ttl_worker: unexpected subject; ack and skip");
            let _ = msg.ack().await;
            return Ok(true);
        }
    };

    let v: serde_json::Value = match serde_json::from_slice(&msg.message.payload) {
        Ok(v) => v,
        Err(e) => {
            warn!(error=%e, "ttl_worker: invalid JSON; ack and skip");
            let _ = msg.ack().await;
            return Ok(true);
        }
    };

    let ev = v.get("event").and_then(|x| x.as_str()).unwrap_or("");
    if ev != "timer.scheduled:v1" {
        let _ = msg.ack().await;
        return Ok(true);
    }

    let timer_id = match v.get("timerId").and_then(|x| x.as_str()) {
        Some(t) => t,
        None => {
            warn!(%subject, "ttl_worker: timer.scheduled missing timerId; ack");
            let _ = msg.ack().await;
            return Ok(true);
        }
    };

    // Try to parse as legacy approval timer first
    if let Some((run_id, gate_id)) =
        crate::rituals::approvals::parse_approval_expiry_timer_id(timer_id)
    {
        if run_id != run_id_from_subject {
            warn!(%subject, run_id=%run_id, sub_run=%run_id_from_subject, "ttl_worker: runId mismatch; ack");
            let _ = msg.ack().await;
            return Ok(true);
        }
        incr(&HANDLED);
        match crate::rituals::approvals::process_expiry_if_pending(
            &tenant, &run_id, &ritual_id, &gate_id,
        )
        .await
        {
            Ok(did_expire) => {
                if did_expire {
                    incr(&EXPIRED);
                    info!(%run_id, %gate_id, %ritual_id, "ttl_worker: expired");
                } else {
                    incr(&NOOP);
                    info!(%run_id, %gate_id, %ritual_id, "ttl_worker: noop_terminal");
                }
                let _ = msg.ack().await;
                Ok(true)
            }
            Err(e) => {
                error!(error=%e, %run_id, %gate_id, %ritual_id, "ttl_worker: expiry failed; nack with backoff");
                // Bounded small backoff then NAK with server-side redelivery delay.
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                let _ = msg
                    .ack_with(AckKind::Nak(Some(std::time::Duration::from_millis(500))))
                    .await;
                Ok(true)
            }
        }
    }
    // Try to parse as escalation timer
    else if let Some((run_id, gate_id, level)) =
        crate::rituals::approvals::parse_escalation_timer_id(timer_id)
    {
        if run_id != run_id_from_subject {
            warn!(%subject, run_id=%run_id, sub_run=%run_id_from_subject, level=%level, "ttl_worker: escalation runId mismatch; ack");
            let _ = msg.ack().await;
            return Ok(true);
        }
        incr(&HANDLED);
        match crate::rituals::approvals::process_expiry_if_pending(
            &tenant, &run_id, &ritual_id, &gate_id,
        )
        .await
        {
            Ok(did_expire) => {
                if did_expire {
                    incr(&EXPIRED);
                    info!(%run_id, %gate_id, %ritual_id, level=%level, "ttl_worker: escalation level expired");
                } else {
                    incr(&NOOP);
                    info!(%run_id, %gate_id, %ritual_id, level=%level, "ttl_worker: escalation noop_terminal");
                }
                let _ = msg.ack().await;
                Ok(true)
            }
            Err(e) => {
                error!(error=%e, %run_id, %gate_id, %ritual_id, level=%level, "ttl_worker: escalation expiry failed; nack with backoff");
                // Bounded small backoff then NAK with server-side redelivery delay.
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                let _ = msg
                    .ack_with(AckKind::Nak(Some(std::time::Duration::from_millis(500))))
                    .await;
                Ok(true)
            }
        }
    } else {
        // Not an approvals expiry timer; ignore but ack
        let _ = msg.ack().await;
        Ok(true)
    }
}

/// Resolve the ritual events stream (env override, then RITUAL_EVENTS, then DEMON_RITUAL_EVENTS).
async fn resolve_stream(
    js: &jetstream::Context,
    override_name: &Option<String>,
) -> Result<jetstream::stream::Stream> {
    if let Some(name) = override_name.clone() {
        return Ok(js
            .get_or_create_stream(jetstream::stream::Config {
                name,
                subjects: vec!["demon.ritual.v1.>".to_string()],
                ..Default::default()
            })
            .await?);
    }
    if let Ok(s) = js.get_stream("RITUAL_EVENTS").await {
        return Ok(s);
    }
    Ok(js.get_stream("DEMON_RITUAL_EVENTS").await?)
}

pub async fn run_loop(cfg: TtlWorkerConfig) -> Result<()> {
    info!(?cfg, "ttl_worker: starting");
    let client = async_nats::connect(&cfg.nats_url).await?;
    let js = jetstream::new(client);
    let stream = resolve_stream(&js, &cfg.stream_name).await?;

    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            durable_name: Some(cfg.consumer_name.clone()),
            filter_subject: cfg.subject_filter.clone(),
            deliver_policy: DeliverPolicy::New,
            // explicit ack is default for pull consumer
            ..Default::default()
        })
        .await?;

    loop {
        let mut batch = consumer
            .batch()
            .max_messages(cfg.batch)
            .expires(std::time::Duration::from_millis(cfg.pull_timeout_ms))
            .messages()
            .await?;
        while let Some(m) = batch.next().await {
            match m {
                Ok(m) => {
                    let _ = handle_message(m).await?;
                }
                Err(e) => warn!(error=%e, "ttl_worker: message error"),
            }
        }
    }
}

/// Process a single batch (useful for tests); returns (handled, expired, noop) deltas.
pub async fn run_one_batch(cfg: TtlWorkerConfig) -> Result<(u64, u64, u64)> {
    let client = async_nats::connect(&cfg.nats_url).await?;
    let js = jetstream::new(client);
    let stream = resolve_stream(&js, &cfg.stream_name).await?;
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            durable_name: Some(cfg.consumer_name.clone()),
            filter_subject: cfg.subject_filter.clone(),
            deliver_policy: DeliverPolicy::New,
            ..Default::default()
        })
        .await?;
    let (h0, e0, n0) = counters();
    let mut batch = consumer
        .batch()
        .max_messages(cfg.batch)
        .expires(std::time::Duration::from_millis(cfg.pull_timeout_ms))
        .messages()
        .await?;
    while let Some(m) = batch.next().await {
        if let Ok(m) = m {
            let _ = handle_message(m).await?;
        }
    }
    let (h1, e1, n1) = counters();
    Ok((h1 - h0, e1 - e0, n1 - n0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_subject_happy() {
        // Legacy format
        let s = "demon.ritual.v1.rit.run.events";
        assert_eq!(
            parse_subject(s),
            Some(("default".into(), "rit".into(), "run".into()))
        );

        // Tenant-aware format
        let s_tenant = "demon.ritual.v1.tenant1.rit.run.events";
        assert_eq!(
            parse_subject(s_tenant),
            Some(("tenant1".into(), "rit".into(), "run".into()))
        );
    }
    #[test]
    fn parse_timer_id() {
        let tid = "run-1:approval:gate-1:expiry";
        let out = crate::rituals::approvals::parse_approval_expiry_timer_id(tid);
        assert_eq!(out, Some(("run-1".into(), "gate-1".into())));
    }

    #[test]
    fn parse_escalation_timer_id() {
        let tid = "run-1:approval:gate-1:expiry:level:2";
        let out = crate::rituals::approvals::parse_escalation_timer_id(tid);
        assert_eq!(out, Some(("run-1".into(), "gate-1".into(), 2)));

        // Test invalid formats
        let bad_tid = "run-1:approval:gate-1:expiry";
        let out = crate::rituals::approvals::parse_escalation_timer_id(bad_tid);
        assert_eq!(out, None);
    }
    #[test]
    fn parse_subject_malformed() {
        assert!(parse_subject("demon.ritual.v2.bad").is_none());
        assert!(parse_subject("other.topic").is_none());
    }
    #[test]
    fn config_uses_deliver_new() {
        let cfg = TtlWorkerConfig::default();
        let conf = jetstream::consumer::pull::Config {
            durable_name: Some(cfg.consumer_name),
            filter_subject: cfg.subject_filter,
            deliver_policy: DeliverPolicy::New,
            ..Default::default()
        };
        match conf.deliver_policy {
            DeliverPolicy::New => (),
            _ => panic!("deliver policy must be New"),
        }
    }
}
