//! Minimal ritual interpreter for Milestone 0 (single task with end=true)

pub mod approvals;
pub mod escalation;
pub mod guards;
pub mod log;
pub mod state;
pub mod timers;
pub mod worker;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{json, Value::Null as null};
use tracing::{info, warn};
use uuid::Uuid;
use wards::{config::load_from_env, policy::PolicyKernel};

#[derive(Debug, Deserialize, Clone)]
pub struct FunctionRef {
    #[serde(rename = "refName")]
    pub ref_name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Clone)]
pub struct Action {
    #[serde(rename = "functionRef")]
    pub function_ref: FunctionRef,
}

#[derive(Debug, Deserialize, Clone)]
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
        }
    }

    /// Execute a minimal ritual: only a single `task` with `end: true` is supported.
    pub async fn run_from_file(&mut self, path: &str) -> Result<()> {
        let spec = Self::load_spec(path)?;
        let _ = self.run_spec_internal(spec, true).await?;
        Ok(())
    }

    /// Execute a minimal ritual and return the result envelope without printing to stdout.
    /// This method is similar to run_from_file but returns the ritual completion event
    /// instead of printing it, allowing the caller to save it or process it further.
    pub async fn run_from_file_with_result(&mut self, path: &str) -> Result<serde_json::Value> {
        let spec = Self::load_spec(path)?;
        self.run_spec_internal(spec, false).await
    }

    /// Execute a ritual specification that has already been loaded from disk and return
    /// the completion envelope. This is used by higher-level services (e.g. runtime HTTP API)
    /// that hydrate specs from installed App Packs before invoking the engine.
    pub async fn run_spec_with_result(&mut self, spec: RitualSpec) -> Result<serde_json::Value> {
        self.run_spec_internal(spec, false).await
    }

    fn load_spec(path: &str) -> Result<RitualSpec> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading ritual spec: {path}"))?;
        let spec = serde_yaml::from_str(&text).with_context(|| "parsing ritual yaml")?;
        Ok(spec)
    }

    async fn run_spec_internal(
        &mut self,
        spec: RitualSpec,
        emit_completion_stdout: bool,
    ) -> Result<serde_json::Value> {
        let ritual_id = spec.id.clone();
        let run_id = Uuid::new_v4().to_string();
        info!(ritual = %ritual_id, %run_id, "ritual.start");

        let state = spec
            .states
            .first()
            .cloned()
            .context("Milestone 0 expects exactly one state")?;

        match state {
            State::Task { action, end, .. } => {
                let tenant_id = "default"; // TODO: Extract from ritual spec or context
                let capability = action.function_ref.ref_name.clone();

                if let Some(ref mut kernel) = self.policy_kernel {
                    let decision = kernel.allow_and_count(tenant_id, &capability);

                    let policy_event = json!({
                        "event": "policy.decision:v1",
                        "ritualId": ritual_id,
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
                        warn!(
                            ritual = %ritual_id,
                            %run_id,
                            %tenant_id,
                            capability = %capability,
                            "ritual denied due to quota limits"
                        );
                        let evt = json!({
                          "event": "ritual.completed:v1",
                          "ritualId": ritual_id,
                          "runId": run_id,
                          "ts": chrono::Utc::now().to_rfc3339(),
                          "outputs": null,
                          "reason": "policy_denied"
                        });
                        if emit_completion_stdout {
                            println!("{}", serde_json::to_string_pretty(&evt)?);
                        }
                        info!(ritual = %ritual_id, %run_id, "ritual.end");
                        return Ok(evt);
                    }

                    info!(
                        ritual = %ritual_id,
                        %run_id,
                        %tenant_id,
                        capability = %capability,
                        limit = decision.limit,
                        remaining = decision.remaining,
                        "policy decision: allowed"
                    );
                }

                let out = self
                    .router
                    .dispatch(
                        &action.function_ref.ref_name,
                        &action.function_ref.arguments,
                        &run_id,
                        &ritual_id,
                    )
                    .await?;
                if !end {
                    warn!(
                        "Milestone 0 only supports single task with end=true; treating as terminal"
                    );
                }
                let evt = json!({
                  "event": "ritual.completed:v1",
                  "ritualId": ritual_id,
                  "runId": run_id,
                  "ts": chrono::Utc::now().to_rfc3339(),
                  "outputs": out
                });
                if emit_completion_stdout {
                    println!("{}", serde_json::to_string_pretty(&evt)?);
                }
                info!(ritual = %ritual_id, %run_id, "ritual.end");
                Ok(evt)
            }
        }
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
