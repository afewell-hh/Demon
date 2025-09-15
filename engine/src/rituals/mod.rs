//! Minimal ritual interpreter for Milestone 0 (single task with end=true)

pub mod approvals;
pub mod guards;
pub mod log;
pub mod state;
pub mod timers;
pub mod worker;

use anyhow::{Context, Result};
use log::{EventLog, RitualEvent};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

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
    event_log: Option<EventLog>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            router: runtime::link::router::Router::new(),
            event_log: None,
        }
    }

    pub async fn with_event_log(nats_url: &str) -> Result<Self> {
        let event_log = EventLog::new(nats_url).await?;
        Ok(Self {
            router: runtime::link::router::Router::new(),
            event_log: Some(event_log),
        })
    }

    /// Execute a minimal ritual: only a single `task` with `end: true` is supported.
    pub async fn run_from_file(&self, path: &str) -> Result<()> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading ritual spec: {path}"))?;
        let spec: RitualSpec =
            serde_yaml::from_str(&text).with_context(|| "parsing ritual yaml")?;

        let run_id = Uuid::new_v4().to_string();
        info!(ritual = %spec.id, %run_id, "ritual.start");

        // Emit started event if event log is configured
        let mut sequence = 1u64;
        if let Some(event_log) = &self.event_log {
            let started_event = RitualEvent::Started {
                ritual_id: spec.id.clone(),
                run_id: run_id.clone(),
                ts: chrono::Utc::now().to_rfc3339(),
                spec: serde_json::to_value(&spec)?,
                trace_id: None,
            };
            event_log.append(&started_event, sequence).await?;
            sequence += 1;
        }

        let state = spec
            .states
            .first()
            .context("Milestone 0 expects exactly one state")?;
        match state {
            State::Task { action, end, .. } => {
                let out = self.router.dispatch(
                    &action.function_ref.ref_name,
                    &action.function_ref.arguments,
                )?;
                if !end {
                    warn!(
                        "Milestone 0 only supports single task with end=true; treating as terminal"
                    );
                }

                // Create completed event
                let completed_event = RitualEvent::Completed {
                    ritual_id: spec.id.clone(),
                    run_id: run_id.clone(),
                    ts: chrono::Utc::now().to_rfc3339(),
                    outputs: Some(out.clone()),
                    trace_id: None,
                };

                // Publish to JetStream if configured, otherwise stdout
                if let Some(event_log) = &self.event_log {
                    event_log.append(&completed_event, sequence).await?;
                    info!(ritual = %spec.id, %run_id, "ritual.completed - event persisted to JetStream");
                } else {
                    // Fallback to stdout for backward compatibility
                    let evt = json!({
                      "event": "ritual.completed:v1",
                      "ritualId": spec.id,
                      "runId": run_id,
                      "ts": chrono::Utc::now().to_rfc3339(),
                      "outputs": out
                    });
                    println!("{}", serde_json::to_string_pretty(&evt)?);
                }

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
