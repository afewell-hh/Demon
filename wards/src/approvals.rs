use std::collections::HashMap;

/// Resolution state for a single approval gate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateState {
    Requested,
    Granted,
    Denied,
}

/// In-memory approvals store keyed by (runId, gateId)
#[derive(Default)]
pub struct Approvals {
    states: HashMap<(String, String), GateState>,
}

impl Approvals {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Request an approval; returns true only on the first request per (runId, gateId)
    pub fn request(
        &mut self,
        run_id: &str,
        gate_id: &str,
        _requester: &str,
        _reason: &str,
    ) -> bool {
        let key = (run_id.to_string(), gate_id.to_string());
        match self.states.get(&key) {
            None => {
                self.states.insert(key, GateState::Requested);
                true
            }
            Some(_) => false, // duplicate requests are no-ops
        }
    }

    /// Grant an approval; first-writer-wins. Returns true only if transitioning from Requested.
    pub fn grant(
        &mut self,
        run_id: &str,
        gate_id: &str,
        _approver: &str,
        _note: Option<&str>,
    ) -> bool {
        let key = (run_id.to_string(), gate_id.to_string());
        match self.states.get(&key) {
            Some(GateState::Requested) => {
                self.states.insert(key, GateState::Granted);
                true
            }
            _ => false, // already resolved or never requested
        }
    }

    /// Deny an approval; first-writer-wins. Returns true only if transitioning from Requested.
    pub fn deny(&mut self, run_id: &str, gate_id: &str, _approver: &str, _reason: &str) -> bool {
        let key = (run_id.to_string(), gate_id.to_string());
        match self.states.get(&key) {
            Some(GateState::Requested) => {
                self.states.insert(key, GateState::Denied);
                true
            }
            _ => false,
        }
    }

    /// Current state, if any, for a gate under a run
    pub fn state(&self, run_id: &str, gate_id: &str) -> Option<GateState> {
        self.states
            .get(&(run_id.to_string(), gate_id.to_string()))
            .cloned()
    }
}
