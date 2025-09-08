// This is an ignored integration scaffold the agent will unignore.
// It sets expectations for event emission & single-resolution behavior.
#[test]
#[ignore]
fn gate_emits_requested_then_grant_resumes_once() {
    // Arrange: start NATS (CI does), ensure ritual stream exists,
    // set APPROVER_ALLOWLIST and bring up REST.
    // Act: hit a gate -> expect approval.requested:v1 exactly once.
    // POST /api/approvals/:runId/:gateId/grant -> approval.granted:v1, engine resumes.
    // Duplicate grant/deny -> no new terminal events.
    unreachable!("agent will implement");
}

