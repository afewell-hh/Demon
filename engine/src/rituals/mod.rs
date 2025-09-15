//! Minimal ritual interpreter for Milestone 0 (single task with end=true)

pub mod approvals;
pub mod guards;
pub mod log;
pub mod state;
pub mod timers;
pub mod worker;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value::Null as null};
use tracing::{info, warn};
use uuid::Uuid;
use wards::{config::load_from_env, policy::PolicyKernel};

#[derive(Debug, Deserialize, Serialize)]
pub struct FunctionRef {
    #[serde(rename = "refName")]
    pub ref_name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum State {
    #[serde(rename = "task")]
    Task {
        name: String,
        action: Action,
        #[serde(default)]
        end: bool,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Action {
    #[serde(rename = "functionRef")]
    pub function_ref: FunctionRef,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RitualSpec {
    pub id: String,
    pub version: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub states: Vec<State>,
}

pub struct Engine {
    router: runtime::link::router::Router,
    policy_kernel: Option<PolicyKernel>,
    event_log: Option<log::EventLog>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        let config = load_from_env();
        let policy_kernel = if config.cap_quotas.is_empty()
            && config.quotas.is_empty()
            && config.global_quota.is_none()
        {
            None
        } else {
            Some(PolicyKernel::new(config))
        };

        Self {
            router: runtime::link::router::Router::new(),
            policy_kernel,
            event_log: None,
        }
    }

    pub async fn with_event_log(mut self, nats_url: &str) -> Result<Self> {
        let event_log = log::EventLog::new(nats_url).await?;
        self.event_log = Some(event_log);
        Ok(self)
    }

    pub async fn run_from_file_with_tenant(
        &mut self,
        path: &str,
        tenant_id: Option<&str>,
    ) -> Result<()> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading ritual spec: {path}"))?;
        let spec: RitualSpec =
            serde_yaml::from_str(&text).with_context(|| "parsing ritual yaml")?;

        let run_id = Uuid::new_v4().to_string();
        let resolved_tenant = tenant_id.unwrap_or("default");

        info!(ritual = %spec.id, %run_id, tenant = %resolved_tenant, "ritual.start");

        // Emit Started event
        if let Some(ref event_log) = self.event_log {
            let started_event = log::RitualEvent::Started {
                ritual_id: spec.id.clone(),
                run_id: run_id.clone(),
                ts: chrono::Utc::now().to_rfc3339(),
                spec: serde_json::to_value(&spec).context("Failed to serialize spec")?,
                trace_id: None,
            };
            event_log
                .append_with_tenant(&started_event, 1, Some(resolved_tenant))
                .await?;
        }

        let state = spec
            .states
            .first()
            .context("Milestone 0 expects exactly one state")?;
        match state {
            State::Task { action, end, .. } => {
                let capability = &action.function_ref.ref_name;

                // Make policy decision if kernel is configured
                if let Some(ref mut kernel) = self.policy_kernel {
                    let decision = kernel.allow_and_count(resolved_tenant, capability);

                    // Emit policy decision event
                    let policy_event = log::RitualEvent::PolicyDecision {
                        ritual_id: spec.id.clone(),
                        run_id: run_id.clone(),
                        ts: chrono::Utc::now().to_rfc3339(),
                        tenant_id: resolved_tenant.to_string(),
                        capability: capability.clone(),
                        decision: serde_json::json!({
                            "allowed": decision.allowed,
                            "reason": if decision.allowed { serde_json::Value::Null } else { serde_json::json!("limit_exceeded") }
                        }),
                        quota: serde_json::json!({
                            "limit": decision.limit,
                            "windowSeconds": decision.window_seconds,
                            "remaining": decision.remaining
                        }),
                    };

                    if let Some(ref event_log) = self.event_log {
                        event_log
                            .append_with_tenant(&policy_event, 2, Some(resolved_tenant))
                            .await?;
                    } else {
                        println!("{}", serde_json::to_string_pretty(&policy_event)?);
                    }

                    if !decision.allowed {
                        warn!(
                            ritual = %spec.id,
                            %run_id,
                            tenant = %resolved_tenant,
                            %capability,
                            "ritual denied due to quota limits"
                        );
                        let completion_event = log::RitualEvent::Completed {
                            ritual_id: spec.id.clone(),
                            run_id: run_id.clone(),
                            ts: chrono::Utc::now().to_rfc3339(),
                            outputs: None,
                            trace_id: None,
                        };

                        if let Some(ref event_log) = self.event_log {
                            event_log
                                .append_with_tenant(&completion_event, 3, Some(resolved_tenant))
                                .await?;
                        } else {
                            println!("{}", serde_json::to_string_pretty(&completion_event)?);
                        }
                        return Ok(());
                    }

                    info!(
                        ritual = %spec.id,
                        %run_id,
                        tenant = %resolved_tenant,
                        %capability,
                        limit = decision.limit,
                        remaining = decision.remaining,
                        "policy decision: allowed"
                    );
                }

                let out = self.router.dispatch(
                    &action.function_ref.ref_name,
                    &action.function_ref.arguments,
                )?;
                if !end {
                    warn!(
                        "Milestone 0 only supports single task with end=true; treating as terminal"
                    );
                }

                // Emit completion event
                let completion_event = log::RitualEvent::Completed {
                    ritual_id: spec.id.clone(),
                    run_id: run_id.clone(),
                    ts: chrono::Utc::now().to_rfc3339(),
                    outputs: Some(out),
                    trace_id: None,
                };

                if let Some(ref event_log) = self.event_log {
                    let next_seq = if self.policy_kernel.is_some() { 4 } else { 2 };
                    event_log
                        .append_with_tenant(&completion_event, next_seq, Some(resolved_tenant))
                        .await?;
                } else {
                    println!("{}", serde_json::to_string_pretty(&completion_event)?);
                }

                info!(ritual = %spec.id, %run_id, tenant = %resolved_tenant, "ritual.end");
            }
        }

        Ok(())
    }

    /// Execute a minimal ritual: only a single `task` with `end: true` is supported.
    pub fn run_from_file(&mut self, path: &str) -> Result<()> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading ritual spec: {path}"))?;
        let spec: RitualSpec =
            serde_yaml::from_str(&text).with_context(|| "parsing ritual yaml")?;

        let run_id = Uuid::new_v4().to_string();
        info!(ritual = %spec.id, %run_id, "ritual.start");

        let state = spec
            .states
            .first()
            .context("Milestone 0 expects exactly one state")?;
        match state {
            State::Task { action, end, .. } => {
                // Check policy quota before executing action
                let tenant_id =
                    std::env::var("TENANT_DEFAULT").unwrap_or_else(|_| "default".to_string());
                let capability = &action.function_ref.ref_name;

                // Make policy decision if kernel is configured
                if let Some(ref mut kernel) = self.policy_kernel {
                    let decision = kernel.allow_and_count(&tenant_id, capability);

                    // Emit policy decision event (to stdout for now)
                    let policy_event = json!({
                        "event": "policy.decision:v1",
                        "ritualId": spec.id,
                        "runId": run_id,
                        "ts": chrono::Utc::now().to_rfc3339(),
                        "tenantId": tenant_id,
                        "capability": capability,
                        "decision": {
                            "allowed": decision.allowed,
                            "reason": if decision.allowed { null } else { json!("limit_exceeded") }
                        },
                        "quota": {
                            "limit": decision.limit,
                            "windowSeconds": decision.window_seconds,
                            "remaining": decision.remaining
                        }
                    });
                    println!("{}", serde_json::to_string_pretty(&policy_event)?);

                    if !decision.allowed {
                        // Quota exceeded - do not execute action but emit completion with policy denial
                        warn!(
                            ritual = %spec.id,
                            %run_id,
                            %tenant_id,
                            %capability,
                            "ritual denied due to quota limits"
                        );
                        let evt = json!({
                          "event": "ritual.completed:v1",
                          "ritualId": spec.id,
                          "runId": run_id,
                          "ts": chrono::Utc::now().to_rfc3339(),
                          "outputs": null,
                          "reason": "policy_denied"
                        });
                        println!("{}", serde_json::to_string_pretty(&evt)?);
                        return Ok(());
                    }

                    info!(
                        ritual = %spec.id,
                        %run_id,
                        %tenant_id,
                        %capability,
                        limit = decision.limit,
                        remaining = decision.remaining,
                        "policy decision: allowed"
                    );
                }

                let out = self.router.dispatch(
                    &action.function_ref.ref_name,
                    &action.function_ref.arguments,
                )?;
                if !end {
                    warn!(
                        "Milestone 0 only supports single task with end=true; treating as terminal"
                    );
                }
                // Emit a completion event (stdout for now; bus to be wired later)
                let evt = json!({
                  "event": "ritual.completed:v1",
                  "ritualId": spec.id,
                  "runId": run_id,
                  "ts": chrono::Utc::now().to_rfc3339(),
                  "outputs": out
                });
                println!("{}", serde_json::to_string_pretty(&evt)?);
                info!(ritual = %spec.id, %run_id, "ritual.end");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_yaml() {
        // Properly terminated raw string (r#" ... "#)
        let y = r#"id: t
version: '1.0'
states:
  - name: s
    type: task
    action:
      functionRef:
        refName: echo
        arguments:
          message: "x"
    end: true
"#;
        let spec: RitualSpec = serde_yaml::from_str(y).unwrap();
        assert_eq!(spec.states.len(), 1);
    }
}
