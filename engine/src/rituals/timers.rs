use chrono::{DateTime, Duration, Utc};
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::info;

// Global counter to prove cancel_by_key() was invoked (used in tests)
static CANCEL_COUNTER: AtomicU64 = AtomicU64::new(0);
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct TimerSpec {
    pub timer_id: String,
    pub ritual_id: String,
    pub run_id: String,
    pub due_at: DateTime<Utc>,
    pub delivered: bool,
}

#[derive(Default)]
pub struct TimerWheel {
    specs: Vec<TimerSpec>,
    // For testing: if set, use this instead of Utc::now()
    current_time: Option<DateTime<Utc>>,
}

impl TimerWheel {
    pub fn new() -> Self {
        Self {
            specs: Vec::new(),
            current_time: None,
        }
    }

    /// For testing: create wheel with a fixed "current time"
    pub fn new_with_time(current_time: DateTime<Utc>) -> Self {
        Self {
            specs: Vec::new(),
            current_time: Some(current_time),
        }
    }

    fn now(&self) -> DateTime<Utc> {
        if let Some(t) = self.current_time {
            t
        } else {
            Utc::now()
        }
    }

    /// Schedule a timer to fire after `delay`.
    pub fn schedule_in(&mut self, run_id: &str, ritual_id: &str, delay: Duration) -> TimerSpec {
        let now = self.now();
        let spec = TimerSpec {
            timer_id: Uuid::new_v4().to_string(),
            ritual_id: ritual_id.to_string(),
            run_id: run_id.to_string(),
            due_at: now + delay,
            delivered: false,
        };
        self.specs.push(spec.clone());
        spec
    }

    /// Schedule with a deterministic key as the timer_id (e.g., "{runId}:approval:{gateId}:expiry").
    /// If a timer with the same id exists, do not schedule a duplicate.
    pub fn schedule_with_key(
        &mut self,
        timer_id: &str,
        run_id: &str,
        ritual_id: &str,
        delay: Duration,
    ) -> TimerSpec {
        if let Some(existing) = self.specs.iter().find(|t| t.timer_id == timer_id) {
            return existing.clone();
        }
        let now = self.now();
        let spec = TimerSpec {
            timer_id: timer_id.to_string(),
            ritual_id: ritual_id.to_string(),
            run_id: run_id.to_string(),
            due_at: now + delay,
            delivered: false,
        };
        self.specs.push(spec.clone());
        spec
    }

    /// Tick evaluates timers due at or before `now`.
    pub fn tick(&mut self, now: DateTime<Utc>) -> Vec<TimerSpec> {
        self.specs
            .iter()
            .filter(|t| !t.delivered && t.due_at <= now)
            .cloned()
            .collect()
    }

    /// Mark a timer as delivered to enforce idempotency at engine layer.
    pub fn mark_fired(&mut self, timer_id: &str) {
        if let Some(t) = self.specs.iter_mut().find(|t| t.timer_id == timer_id) {
            t.delivered = true;
        }
    }

    /// Cancel by key (marks as delivered so later ticks ignore it).
    pub fn cancel_by_key(&mut self, timer_id: &str) {
        info!(%timer_id, "timer.cancel_by_key");
        CANCEL_COUNTER.fetch_add(1, Ordering::Relaxed);
        self.mark_fired(timer_id);
    }

    /// Helper to rebuild wheel from known specs (simulated persistence for tests).
    pub fn from_specs(specs: Vec<TimerSpec>) -> Self {
        Self {
            specs,
            current_time: None,
        }
    }
}

/// Testing aid: number of times `cancel_by_key` was invoked since process start.
pub fn cancel_counter() -> u64 {
    CANCEL_COUNTER.load(Ordering::Relaxed)
}

/// Testing aid: reset the cancel counter to zero.
pub fn reset_cancel_counter() {
    CANCEL_COUNTER.store(0, Ordering::Relaxed)
}
