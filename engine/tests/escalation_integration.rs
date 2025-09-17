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
async fn escalation_chain_timeout_escalates_to_next_level() -> Result<()> {
    std::env::set_var("APPROVER_ALLOWLIST", "ops@example.com");

    // Configure escalation chain
    let escalation_config = serde_json::json!({
        "tenants": {
            "default": {
                "gates": {
                    "escalation-gate": {
                        "levels": [
                            {
                                "level": 1,
                                "roles": ["team-lead"],
                                "timeoutSeconds": 1
                            },
                            {
                                "level": 2,
                                "roles": ["manager"],
                                "timeoutSeconds": 0,
                                "emergencyOverride": true
                            }
                        ]
                    }
                }
            }
        }
    });
    std::env::set_var("APPROVAL_ESCALATION_RULES", escalation_config.to_string());

    // Start Operate UI
    let port = start_operate_ui().await?;

    let run_id = format!("esc-run-{}", uuid::Uuid::new_v4());
    let ritual_id = "esc-ritual";
    let gate_id = "escalation-gate";

    // Request approval with escalation
    engine::rituals::approvals::await_gate(&run_id, ritual_id, gate_id, "requester", "deploy")
        .await?;

    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    // Verify initial approval.requested
    let events = read_events_for_run(&js, ritual_id, &run_id).await?;
    assert_eq!(
        events
            .iter()
            .filter(|e| e["event"] == "approval.requested:v1")
            .count(),
        1
    );

    // Manually trigger escalation
    engine::rituals::approvals::escalate_approval(
        "default", &run_id, ritual_id, gate_id, "timeout",
    )
    .await?;

    // Verify escalation event was emitted
    let events = read_events_for_run(&js, ritual_id, &run_id).await?;
    let escalated_events: Vec<_> = events
        .iter()
        .filter(|e| e["event"] == "approval.escalated:v1")
        .collect();
    assert_eq!(escalated_events.len(), 1);

    let escalated_event = escalated_events[0];
    assert_eq!(escalated_event["fromLevel"], 1);
    assert_eq!(escalated_event["toLevel"], 2);
    assert_eq!(escalated_event["reason"], "timeout");

    // Verify escalation state
    let escalation_state = &escalated_event["escalationState"];
    assert_eq!(escalation_state["current_level"], 2);
    assert_eq!(escalation_state["total_levels"], 2);
    assert_eq!(escalation_state["emergency_override"], false);

    // Grant approval at level 2
    let url = format!(
        "http://127.0.0.1:{}/api/tenants/default/approvals/{}/{}/grant",
        port, run_id, gate_id
    );
    let body = serde_json::json!({"approver":"ops@example.com","note":"approved at level 2"});
    let http = reqwest::Client::new();
    let response = http
        .post(&url)
        .header("X-Requested-With", "XMLHttpRequest")
        .json(&body)
        .send()
        .await?;
    assert!(response.status().is_success());

    // Verify approval was granted
    let events = read_events_for_run(&js, ritual_id, &run_id).await?;
    assert_eq!(
        events
            .iter()
            .filter(|e| e["event"] == "approval.granted:v1")
            .count(),
        1
    );

    Ok(())
}

#[tokio::test]
#[ignore]
async fn emergency_override_bypasses_escalation() -> Result<()> {
    std::env::set_var("APPROVER_ALLOWLIST", "ops@example.com");

    // Configure escalation chain with emergency override
    let escalation_config = serde_json::json!({
        "tenants": {
            "default": {
                "gates": {
                    "override-gate": {
                        "levels": [
                            {
                                "level": 1,
                                "roles": ["team-lead"],
                                "timeoutSeconds": 3600
                            },
                            {
                                "level": 2,
                                "roles": ["manager"],
                                "timeoutSeconds": 0,
                                "emergencyOverride": true
                            }
                        ]
                    }
                }
            }
        }
    });
    std::env::set_var("APPROVAL_ESCALATION_RULES", escalation_config.to_string());

    // Start Operate UI
    let port = start_operate_ui().await?;

    let run_id = format!("ovr-run-{}", uuid::Uuid::new_v4());
    let ritual_id = "ovr-ritual";
    let gate_id = "override-gate";

    // Request approval
    engine::rituals::approvals::await_gate(
        &run_id,
        ritual_id,
        gate_id,
        "requester",
        "emergency deploy",
    )
    .await?;

    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    // Use emergency override endpoint
    let url = format!(
        "http://127.0.0.1:{}/api/tenants/default/approvals/{}/{}/override",
        port, run_id, gate_id
    );
    let body = serde_json::json!({"approver":"ops@example.com","note":"emergency security patch"});
    let http = reqwest::Client::new();
    let response = http
        .post(&url)
        .header("X-Requested-With", "XMLHttpRequest")
        .json(&body)
        .send()
        .await?;
    assert!(response.status().is_success());

    // Verify override event was emitted
    let events = read_events_for_run(&js, ritual_id, &run_id).await?;
    let override_events: Vec<_> = events
        .iter()
        .filter(|e| e["event"] == "approval.override:v1")
        .collect();
    assert_eq!(override_events.len(), 1);

    let override_event = override_events[0];
    assert_eq!(override_event["approver"], "ops@example.com");
    assert_eq!(override_event["overrideLevel"], 1);
    assert_eq!(override_event["note"], "emergency security patch");

    // Verify escalation state shows emergency override
    let escalation_state = &override_event["escalationState"];
    assert_eq!(escalation_state["emergency_override"], true);

    // Verify no escalation events occurred
    assert_eq!(
        events
            .iter()
            .filter(|e| e["event"] == "approval.escalated:v1")
            .count(),
        0
    );

    Ok(())
}

#[tokio::test]
#[ignore]
async fn escalation_config_validation() -> Result<()> {
    use engine::rituals::escalation::{EscalationChain, EscalationConfig, EscalationLevel};

    // Test valid config
    let valid_config = r#"
    {
        "tenants": {
            "test-tenant": {
                "gates": {
                    "test-gate": {
                        "levels": [
                            {
                                "level": 1,
                                "roles": ["team-lead"],
                                "timeoutSeconds": 3600
                            },
                            {
                                "level": 2,
                                "roles": ["manager"],
                                "timeoutSeconds": 0,
                                "emergencyOverride": true
                            }
                        ]
                    }
                }
            }
        }
    }
    "#;

    let config: EscalationConfig = serde_json::from_str(valid_config)?;
    let chain = config.get_chain("test-tenant", "test-gate").unwrap();
    chain.validate()?;

    assert_eq!(chain.levels.len(), 2);
    assert_eq!(chain.first_level().unwrap().level, 1);
    assert!(chain.is_final_level(2));
    assert!(!chain.is_final_level(1));

    // Test invalid config - empty levels
    let invalid_chain = EscalationChain { levels: vec![] };
    assert!(invalid_chain.validate().is_err());

    // Test invalid config - non-consecutive levels
    let invalid_chain = EscalationChain {
        levels: vec![
            EscalationLevel {
                level: 1,
                roles: vec!["role1".to_string()],
                timeout_seconds: 0,
                emergency_override: false,
                notifications: vec![],
            },
            EscalationLevel {
                level: 3, // Should be 2
                roles: vec!["role2".to_string()],
                timeout_seconds: 0,
                emergency_override: false,
                notifications: vec![],
            },
        ],
    };
    assert!(invalid_chain.validate().is_err());

    Ok(())
}

#[tokio::test]
#[ignore]
async fn escalation_state_management() -> Result<()> {
    use engine::rituals::escalation::{EscalationChain, EscalationLevel, EscalationState};

    let chain = EscalationChain {
        levels: vec![
            EscalationLevel {
                level: 1,
                roles: vec!["team-lead".to_string()],
                timeout_seconds: 3600,
                emergency_override: false,
                notifications: vec![],
            },
            EscalationLevel {
                level: 2,
                roles: vec!["manager".to_string()],
                timeout_seconds: 0,
                emergency_override: true,
                notifications: vec![],
            },
        ],
    };

    // Create initial state
    let mut state = EscalationState::new(&chain)?;
    assert_eq!(state.current_level, 1);
    assert_eq!(state.total_levels, 2);
    assert!(!state.emergency_override);
    assert!(state.next_escalation_at.is_some());

    // Test escalation
    let escalated = state.escalate(&chain, "timeout".to_string())?;
    assert!(escalated);
    assert_eq!(state.current_level, 2);
    assert_eq!(state.escalation_history.len(), 1);
    assert!(state.next_escalation_at.is_none()); // Level 2 has no timeout

    // Try to escalate beyond final level
    let escalated = state.escalate(&chain, "test".to_string())?;
    assert!(!escalated);
    assert_eq!(state.current_level, 2);

    // Test emergency override
    assert!(state.can_emergency_override(&chain));
    state.mark_emergency_override();
    assert!(state.emergency_override);
    assert!(state.next_escalation_at.is_none());

    Ok(())
}

#[tokio::test]
#[ignore]
async fn no_escalation_config_uses_traditional_approval() -> Result<()> {
    std::env::set_var("APPROVER_ALLOWLIST", "ops@example.com");
    // Remove escalation config
    std::env::remove_var("APPROVAL_ESCALATION_RULES");

    let port = start_operate_ui().await?;

    let run_id = format!("trad-run-{}", uuid::Uuid::new_v4());
    let ritual_id = "trad-ritual";
    let gate_id = "traditional-gate";

    // Request approval
    engine::rituals::approvals::await_gate(
        &run_id,
        ritual_id,
        gate_id,
        "requester",
        "normal deploy",
    )
    .await?;

    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    // Grant approval normally
    let url = format!(
        "http://127.0.0.1:{}/api/approvals/{}/{}/grant",
        port, run_id, gate_id
    );
    let body = serde_json::json!({"approver":"ops@example.com","note":"normal approval"});
    let http = reqwest::Client::new();
    let response = http
        .post(&url)
        .header("X-Requested-With", "XMLHttpRequest")
        .json(&body)
        .send()
        .await?;
    assert!(response.status().is_success());

    // Verify normal grant without escalation
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
            .filter(|e| e["event"] == "approval.escalated:v1")
            .count(),
        0
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| e["event"] == "approval.override:v1")
            .count(),
        0
    );

    Ok(())
}
