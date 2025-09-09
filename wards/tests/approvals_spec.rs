// DoR: this file verifies the API shape for M2B.
// It intentionally references wards::approvals which does not exist yet (RED).
use wards::approvals::{Approvals, GateState};

#[test]
fn request_then_grant_transitions_once() {
    let mut a = Approvals::default();
    assert!(a.request("run-1", "gate-1", "requester", "reason")); // first time -> true
    assert_eq!(a.state("run-1", "gate-1"), Some(GateState::Requested));
    assert!(a.grant("run-1", "gate-1", "approver", Some("note")));
    assert_eq!(a.state("run-1", "gate-1"), Some(GateState::Granted));
    assert!(!a.grant("run-1", "gate-1", "approver", None)); // idempotent
}

#[test]
fn request_then_deny_transitions_once() {
    let mut a = Approvals::default();
    assert!(a.request("run-2", "gate-1", "requester", "reason"));
    assert!(a.deny("run-2", "gate-1", "approver", "because"));
    assert!(!a.grant("run-2", "gate-1", "approver", None)); // single-resolution rule
}
