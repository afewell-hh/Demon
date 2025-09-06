use crate::rituals::log::RitualEvent;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RitualStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RitualState {
    pub ritual_id: String,
    pub run_id: String,
    pub status: RitualStatus,
    pub current_state: Option<String>,
    pub spec: Option<Value>,
    pub outputs: Option<Value>,
    pub trace_id: Option<String>,
    pub event_count: u64,
}

impl RitualState {
    pub fn new(ritual_id: String, run_id: String) -> Self {
        Self {
            ritual_id,
            run_id,
            status: RitualStatus::Running,
            current_state: None,
            spec: None,
            outputs: None,
            trace_id: None,
            event_count: 0,
        }
    }

    pub fn apply_event(&mut self, event: &RitualEvent) -> Result<()> {
        match event {
            RitualEvent::Started {
                ritual_id,
                run_id,
                spec,
                trace_id,
                ..
            } => {
                debug!("Applying Started event for {}/{}", ritual_id, run_id);
                self.ritual_id = ritual_id.clone();
                self.run_id = run_id.clone();
                self.status = RitualStatus::Running;
                self.spec = Some(spec.clone());
                self.trace_id = trace_id.clone();

                // Extract initial state from spec
                if let Some(initial) = spec.get("initial").and_then(|v| v.as_str()) {
                    self.current_state = Some(initial.to_string());
                }

                self.event_count += 1;
            }

            RitualEvent::StateTransitioned {
                to_state, trace_id, ..
            } => {
                debug!("Applying StateTransitioned event to state {}", to_state);
                self.current_state = Some(to_state.clone());
                if trace_id.is_some() {
                    self.trace_id = trace_id.clone();
                }
                self.event_count += 1;
            }

            RitualEvent::Completed {
                outputs, trace_id, ..
            } => {
                debug!("Applying Completed event");
                self.status = RitualStatus::Completed;
                self.outputs = outputs.clone();
                if trace_id.is_some() {
                    self.trace_id = trace_id.clone();
                }
                self.event_count += 1;
            }
        }

        Ok(())
    }

    pub fn replay(events: &[RitualEvent]) -> Result<Self> {
        if events.is_empty() {
            anyhow::bail!("Cannot replay empty event list");
        }

        // Get ritual_id and run_id from first event
        let (ritual_id, run_id) = match &events[0] {
            RitualEvent::Started {
                ritual_id, run_id, ..
            } => (ritual_id.clone(), run_id.clone()),
            _ => anyhow::bail!("First event must be Started event"),
        };

        let mut state = Self::new(ritual_id, run_id);

        for event in events {
            state.apply_event(event)?;
        }

        debug!(
            "Replayed {} events, final status: {:?}",
            events.len(),
            state.status
        );

        Ok(state)
    }
}

#[derive(Default)]
pub struct StateStore {
    states: HashMap<String, RitualState>,
}

impl StateStore {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    pub fn get(&self, run_id: &str) -> Option<&RitualState> {
        self.states.get(run_id)
    }

    pub fn insert(&mut self, state: RitualState) {
        self.states.insert(state.run_id.clone(), state);
    }

    pub fn update_with_event(&mut self, event: &RitualEvent) -> Result<()> {
        let run_id = match event {
            RitualEvent::Started { run_id, .. }
            | RitualEvent::StateTransitioned { run_id, .. }
            | RitualEvent::Completed { run_id, .. } => run_id,
        };

        let state = self.states.entry(run_id.clone()).or_insert_with(|| {
            let ritual_id = match event {
                RitualEvent::Started { ritual_id, .. }
                | RitualEvent::StateTransitioned { ritual_id, .. }
                | RitualEvent::Completed { ritual_id, .. } => ritual_id.clone(),
            };
            RitualState::new(ritual_id, run_id.clone())
        });

        state.apply_event(event)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_events() -> Vec<RitualEvent> {
        let ritual_id = "test-ritual";
        let run_id = "test-run-123";
        let ts = Utc::now().to_rfc3339();

        vec![
            RitualEvent::Started {
                ritual_id: ritual_id.to_string(),
                run_id: run_id.to_string(),
                ts: ts.clone(),
                spec: serde_json::json!({
                    "id": ritual_id,
                    "initial": "start",
                    "states": {
                        "start": { "type": "action", "action": "echo" },
                        "end": { "type": "end" }
                    }
                }),
                trace_id: Some("trace-123".to_string()),
            },
            RitualEvent::StateTransitioned {
                ritual_id: ritual_id.to_string(),
                run_id: run_id.to_string(),
                ts: ts.clone(),
                from_state: "start".to_string(),
                to_state: "end".to_string(),
                trace_id: Some("trace-123".to_string()),
            },
            RitualEvent::Completed {
                ritual_id: ritual_id.to_string(),
                run_id: run_id.to_string(),
                ts,
                outputs: Some(serde_json::json!({ "result": "success" })),
                trace_id: Some("trace-123".to_string()),
            },
        ]
    }

    #[test]
    fn test_replay_produces_completed_state() {
        let events = create_test_events();
        let state = RitualState::replay(&events).unwrap();

        assert_eq!(state.status, RitualStatus::Completed);
        assert_eq!(state.current_state, Some("end".to_string()));
        assert_eq!(state.event_count, 3);
        assert!(state.outputs.is_some());
    }

    #[test]
    fn test_replay_determinism() {
        let events = create_test_events();

        let state1 = RitualState::replay(&events).unwrap();
        let state2 = RitualState::replay(&events).unwrap();

        assert_eq!(state1.status, state2.status);
        assert_eq!(state1.current_state, state2.current_state);
        assert_eq!(state1.outputs, state2.outputs);
        assert_eq!(state1.event_count, state2.event_count);
    }

    #[test]
    fn test_partial_replay() {
        let events = create_test_events();

        // Replay only first event
        let state = RitualState::replay(&events[..1]).unwrap();
        assert_eq!(state.status, RitualStatus::Running);
        assert_eq!(state.current_state, Some("start".to_string()));
        assert_eq!(state.event_count, 1);

        // Replay first two events
        let state = RitualState::replay(&events[..2]).unwrap();
        assert_eq!(state.status, RitualStatus::Running);
        assert_eq!(state.current_state, Some("end".to_string()));
        assert_eq!(state.event_count, 2);
    }
}
