use engine::rituals::guards::quota_json;
use wards::policy::Decision as KernelDecision;

#[test]
fn quota_serializes_with_camel_case_window_seconds() {
    let d = KernelDecision {
        allowed: true,
        limit: 7,
        window_seconds: 42,
        remaining: 6,
    };
    let q = quota_json(&d);
    assert_eq!(q.get("limit").and_then(|v| v.as_u64()), Some(7));
    assert_eq!(q.get("windowSeconds").and_then(|v| v.as_u64()), Some(42));
    assert_eq!(q.get("remaining").and_then(|v| v.as_u64()), Some(6));
    assert!(q.get("window_seconds").is_none());
}

#[test]
fn decision_reason_omitted_when_allowed() {
    let allowed = serde_json::json!({
        "event": "policy.decision:v1",
        "decision": { "allowed": true }
    });
    assert!(allowed["decision"]["reason"].is_null());

    let denied = serde_json::json!({
        "event": "policy.decision:v1",
        "decision": { "allowed": false, "reason": "limit_exceeded" }
    });
    assert_eq!(
        denied["decision"]["reason"],
        serde_json::json!("limit_exceeded")
    );
}

