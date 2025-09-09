use chrono::{Duration, Utc};

#[test]
fn expires_after_ttl_if_no_terminal() {
    // Arrange: schedule expiry via TimerWheel helper and craft events with only requested
    let t0 = Utc::now();
    let run = "run-ttl-1";
    let ritual = "ritual-ttl";
    let gate = "g-1";

    let mut wheel = engine::rituals::timers::TimerWheel::new_with_time(t0);
    let key = engine::rituals::approvals::expiry_key(run, gate);
    let spec = wheel.schedule_with_key(&key, run, ritual, Duration::seconds(5));
    assert_eq!(spec.timer_id, key);

    // Nothing due before TTL
    assert!(wheel.tick(t0 + Duration::seconds(3)).is_empty());
    // At TTL, the timer would fire
    let fired = wheel.tick(t0 + Duration::seconds(5));
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].timer_id, key);

    // Given only a requested event, terminal_for_gate should be None => would emit deny(expired)
    let events = vec![serde_json::json!({
        "event": "approval.requested:v1",
        "ts": t0.to_rfc3339(),
        "tenantId": "default",
        "runId": run,
        "ritualId": ritual,
        "gateId": gate,
        "requester": "dev@example.com",
        "reason": "promote"
    })];
    let term = engine::rituals::approvals::terminal_for_gate(&events, gate);
    assert!(term.is_none());
}

#[test]
fn grant_preempts_expiry() {
    let run = "run-ttl-2";
    let ritual = "ritual-ttl";
    let gate = "g-2";
    let t = Utc::now();
    let events = vec![
        serde_json::json!({
            "event": "approval.requested:v1",
            "ts": t.to_rfc3339(),
            "tenantId": "default",
            "runId": run,
            "ritualId": ritual,
            "gateId": gate,
            "requester": "dev@example.com",
            "reason": "promote"
        }),
        serde_json::json!({
            "event": "approval.granted:v1",
            "ts": (t + Duration::seconds(1)).to_rfc3339(),
            "tenantId": "default",
            "runId": run,
            "ritualId": ritual,
            "gateId": gate,
            "approver": "ops@example.com",
            "note": "ok"
        }),
    ];
    let term = engine::rituals::approvals::terminal_for_gate(&events, gate);
    assert_eq!(term, Some("granted"));
}

#[test]
fn replay_does_not_duplicate() {
    let run = "run-ttl-3";
    let ritual = "ritual-ttl";
    let gate = "g-3";
    let t = Utc::now();
    let mut events = vec![serde_json::json!({
        "event": "approval.requested:v1",
        "ts": t.to_rfc3339(),
        "tenantId": "default",
        "runId": run,
        "ritualId": ritual,
        "gateId": gate,
        "requester": "dev@example.com",
        "reason": "promote"
    })];
    // First evaluation: no terminal
    assert!(engine::rituals::approvals::terminal_for_gate(&events, gate).is_none());

    // Suppose an auto-deny was emitted once (idempotent key ensures once)
    events.push(serde_json::json!({
        "event": "approval.denied:v1",
        "ts": (t + Duration::seconds(5)).to_rfc3339(),
        "tenantId": "default",
        "runId": run,
        "ritualId": ritual,
        "gateId": gate,
        "approver": "system",
        "reason": "expired",
        "note": "TTL exceeded"
    }));
    // Replay after restart should see terminal and not produce duplicates
    assert_eq!(
        engine::rituals::approvals::terminal_for_gate(&events, gate),
        Some("denied")
    );
}

#[test]
fn cancel_prevents_fire_without_sleep() {
    let t0 = Utc::now();
    let run = "run-ttl-4";
    let ritual = "ritual-ttl";
    let gate = "g-4";
    let key = engine::rituals::approvals::expiry_key(run, gate);

    let mut wheel = engine::rituals::timers::TimerWheel::new_with_time(t0);
    let _ = wheel.schedule_with_key(&key, run, ritual, Duration::seconds(2));

    // Before due
    assert!(wheel.tick(t0 + Duration::seconds(1)).is_empty());

    // Cancel and ensure even after due time it won’t fire
    wheel.cancel_by_key(&key);
    assert!(wheel.tick(t0 + Duration::seconds(3)).is_empty());
}

#[test]
fn terminal_preempts_and_cancels_counter() {
    // Arrange
    let t0 = Utc::now();
    let run = "run-ttl-5";
    let ritual = "ritual-ttl";
    let gate = "g-5";
    engine::rituals::timers::reset_cancel_counter();

    // Schedule expiry at +5s with deterministic key
    let key = engine::rituals::approvals::expiry_key(run, gate);
    let mut wheel = engine::rituals::timers::TimerWheel::new_with_time(t0);
    let _ = wheel.schedule_with_key(&key, run, ritual, Duration::seconds(5));

    // Events contain a grant at +1s (preempts TTL)
    let events = vec![
        serde_json::json!({
            "event": "approval.requested:v1",
            "ts": t0.to_rfc3339(),
            "tenantId": "default",
            "runId": run,
            "ritualId": ritual,
            "gateId": gate,
            "requester": "dev@example.com",
            "reason": "promote"
        }),
        serde_json::json!({
            "event": "approval.granted:v1",
            "ts": (t0 + Duration::seconds(1)).to_rfc3339(),
            "tenantId": "default",
            "runId": run,
            "ritualId": ritual,
            "gateId": gate,
            "approver": "ops@example.com",
            "note": "ok"
        }),
    ];

    // Act: preempt and cancel
    let did_cancel =
        engine::rituals::approvals::preempt_expiry_if_terminal(&events, run, gate, &mut wheel);

    // Assert: cancellation logged via counter and no fire even after TTL
    assert!(did_cancel, "expected preemption to cancel expiry timer");
    assert_eq!(
        engine::rituals::timers::cancel_counter(),
        1,
        "cancel_by_key should be called once"
    );
    assert!(wheel.tick(t0 + Duration::seconds(6)).is_empty());
}
