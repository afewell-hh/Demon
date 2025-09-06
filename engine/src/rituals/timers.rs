use chrono::{DateTime, Duration, Utc};
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
    current_time: Option<DateTime<Utc>>, // For testing: if set, use this instead of Utc::now()
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

    /// For M1A, a simple in-memory schedule API. Persistence comes in M1B.
    pub fn schedule_in(&mut self, run_id: &str, ritual_id: &str, delay: Duration) -> TimerSpec {
        let now = self.current_time.unwrap_or_else(|| Utc::now());
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

    /// Helper to rebuild wheel from known specs (simulated persistence for tests).
    pub fn from_specs(specs: Vec<TimerSpec>) -> Self {
        Self {
            specs,
            current_time: None,
        }
    }
}
