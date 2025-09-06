use chrono::{Duration, TimeZone, Utc};

/// These tests define the expected CONTRACT of the timer wheel for M1A.
/// They are #[ignore] initially; unignore one by one as you implement (TDD).
#[test]
fn schedule_then_no_fire_before_due() {
    let t0 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    let mut wheel = engine::rituals::timers::TimerWheel::new_with_time(t0);
    let spec = wheel.schedule_in("run-1", "timer-ritual", Duration::seconds(5));
    assert_eq!(spec.timer_id.len() > 0, true);
    // 3s later: should not fire
    let fired = wheel.tick(t0 + Duration::seconds(3));
    assert!(fired.is_empty());
}

#[test]
fn fires_once_at_due_and_marks_delivered() {
    let t0 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    let mut wheel = engine::rituals::timers::TimerWheel::new_with_time(t0);
    let spec = wheel.schedule_in("run-1", "timer-ritual", Duration::seconds(5));
    // At due time
    let fired1 = wheel.tick(t0 + Duration::seconds(5));
    assert_eq!(fired1.len(), 1);
    assert_eq!(fired1[0].timer_id, spec.timer_id);
    // Re-tick at same instant simulates duplicate delivery; engine must dedupe when mark_fired is called
    wheel.mark_fired(&spec.timer_id);
    let fired2 = wheel.tick(t0 + Duration::seconds(5));
    assert!(fired2.is_empty(), "idempotent after mark_fired");
}

#[test]
fn restarting_before_due_still_fires_once_after_due() {
    let t0 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    let mut wheel = engine::rituals::timers::TimerWheel::new_with_time(t0);
    let spec = wheel.schedule_in("run-1", "timer-ritual", Duration::seconds(5));
    // "Restart": re-create the wheel and restore the spec (persistence comes in M1B).
    let mut wheel2 = engine::rituals::timers::TimerWheel::from_specs(vec![spec.clone()]);
    // 3s after start -> nothing; 5s after start -> single fire
    let fired_pre = wheel2.tick(t0 + Duration::seconds(3));
    assert!(fired_pre.is_empty());
    let fired = wheel2.tick(t0 + Duration::seconds(5));
    assert_eq!(fired.len(), 1);
}

#[test]
fn timer_ids_are_globally_unique_across_restarts() {
    let t0 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    let mut wheel = engine::rituals::timers::TimerWheel::new_with_time(t0);

    // Schedule first timer
    let spec1 = wheel.schedule_in("run-1", "ritual-1", Duration::seconds(5));

    // Fire and mark delivered (simulates delivery)
    wheel.mark_fired(&spec1.timer_id);

    // Simulate restart: rebuild wheel from specs (delivered timers filtered out)
    let remaining_specs: Vec<_> = wheel
        .tick(t0 + Duration::seconds(10))
        .into_iter()
        .filter(|s| !s.delivered)
        .collect();

    let mut wheel2 = engine::rituals::timers::TimerWheel::from_specs(remaining_specs);

    // Schedule new timer after restart
    let spec2 = wheel2.schedule_in("run-2", "ritual-2", Duration::seconds(3));

    // Timer IDs must be different to prevent confusion
    assert_ne!(
        spec1.timer_id, spec2.timer_id,
        "Timer IDs must be globally unique even after restart"
    );

    // Both should be valid UUID format
    assert!(spec1.timer_id.len() > 10, "Timer ID should be UUID-like");
    assert!(spec2.timer_id.len() > 10, "Timer ID should be UUID-like");
}
