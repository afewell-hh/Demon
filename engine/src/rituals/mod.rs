//! Minimal ritual interpreter for Milestone 0 (single task with end=true)

pub mod approvals;
pub mod log;
pub mod state;
pub mod timers;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct FunctionRef {
    #[serde(rename = "refName")]
    pub ref_name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct Action {
    #[serde(rename = "functionRef")]
    pub function_ref: FunctionRef,
}

#[derive(Debug, Deserialize)]
pub struct RitualSpec {
    pub id: String,
    pub version: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub states: Vec<State>,
}

#[derive(Default)]
pub struct Engine {
    router: runtime::link::router::Router,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            router: runtime::link::router::Router::new(),
        }
    }

    /// Execute a minimal ritual: only a single `task` with `end: true` is supported.
    pub fn run_from_file(&self, path: &str) -> Result<()> {
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
