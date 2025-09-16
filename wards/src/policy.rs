use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::config::{QuotaCfg, WardsConfig};

/// Build the quota counter key.
/// When TENANTING_ENABLED=1 → "{tenant}:{capability}"; otherwise → "{capability}" (global counter).
pub fn quota_key(tenant: Option<&str>, capability: &str) -> String {
    let enabled = std::env::var("TENANTING_ENABLED")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if enabled {
        format!("{}:{}", tenant.unwrap_or("default"), capability)
    } else {
        capability.to_string()
    }
}

#[derive(Debug, Clone)]
struct CounterState {
    window: Duration,
    started: Instant,
    count: u32,
}

impl CounterState {
    fn new(window_seconds: u64) -> Self {
        Self {
            window: Duration::from_secs(window_seconds),
            started: Instant::now(),
            count: 0,
        }
    }
    fn reset_if_needed(&mut self) {
        if self.started.elapsed() >= self.window {
            self.started = Instant::now();
            self.count = 0;
        }
    }
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub allowed: bool,
    pub limit: u32,
    pub window_seconds: u64,
    pub remaining: u32,
}

#[derive(Default)]
pub struct PolicyKernel {
    cfg: WardsConfig,
    counters: HashMap<String, CounterState>,
}

impl PolicyKernel {
    pub fn new(cfg: WardsConfig) -> Self {
        Self {
            cfg,
            counters: HashMap::new(),
        }
    }

    pub fn effective_quota(&self, tenant: &str, capability: &str) -> QuotaCfg {
        self.cfg.effective_quota(tenant, capability)
    }

    pub fn allow_and_count(&mut self, tenant: &str, capability: &str) -> Decision {
        let quota = self.cfg.effective_quota(tenant, capability);
        let key = quota_key(Some(tenant), capability);
        let state = self
            .counters
            .entry(key)
            .or_insert_with(|| CounterState::new(quota.window_seconds));
        if state.window != Duration::from_secs(quota.window_seconds) {
            state.window = Duration::from_secs(quota.window_seconds);
        }
        state.reset_if_needed();
        if state.count < quota.limit {
            state.count += 1;
            let remaining = quota.limit - state.count;
            Decision {
                allowed: true,
                limit: quota.limit,
                window_seconds: quota.window_seconds,
                remaining,
            }
        } else {
            Decision {
                allowed: false,
                limit: quota.limit,
                window_seconds: quota.window_seconds,
                remaining: 0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WardsConfig;
    use serial_test::serial;
    use std::collections::HashMap;

    #[test]
    #[serial]
    fn quota_key_scopes_by_tenant_when_enabled() {
        std::env::set_var("TENANTING_ENABLED", "1");
        assert_eq!(
            super::quota_key(Some("t1"), "capsule.echo"),
            "t1:capsule.echo"
        );
        std::env::remove_var("TENANTING_ENABLED");
    }

    #[test]
    #[serial]
    fn quota_key_is_global_when_disabled() {
        std::env::set_var("TENANTING_ENABLED", "0");
        assert_eq!(super::quota_key(Some("t1"), "capsule.echo"), "capsule.echo");
        std::env::remove_var("TENANTING_ENABLED");
    }

    #[test]
    #[serial]
    fn separate_counters_per_capability() {
        let mut cfg = WardsConfig::default();
        cfg.cap_quotas.insert(
            "tenant-a".into(),
            HashMap::from([
                (
                    "capsule.http".into(),
                    QuotaCfg {
                        limit: 1,
                        window_seconds: 60,
                    },
                ),
                (
                    "capsule.echo".into(),
                    QuotaCfg {
                        limit: 3,
                        window_seconds: 60,
                    },
                ),
            ]),
        );
        let mut kernel = PolicyKernel::new(cfg);

        let d1 = kernel.allow_and_count("tenant-a", "capsule.http");
        assert!(d1.allowed && d1.remaining == 0);
        let d2 = kernel.allow_and_count("tenant-a", "capsule.http");
        assert!(!d2.allowed && d2.remaining == 0);

        let mut ok = 0;
        for _ in 0..4 {
            let d = kernel.allow_and_count("tenant-a", "capsule.echo");
            if d.allowed {
                ok += 1;
            }
        }
        assert_eq!(ok, 3);
    }
}
