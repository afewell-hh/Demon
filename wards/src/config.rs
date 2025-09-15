use std::collections::HashMap;
use std::fmt::Write as _;

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
pub struct QuotaCfg {
    pub limit: u32,
    #[serde(rename = "windowSeconds")]
    pub window_seconds: u64,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct CapQuotas(pub HashMap<String, HashMap<String, QuotaCfg>>);

#[derive(Debug, Clone, Default)]
pub struct WardsConfig {
    pub caps: HashMap<String, Vec<String>>, // tenant -> capabilities
    pub quotas: HashMap<String, QuotaCfg>,  // tenant -> quota
    pub cap_quotas: HashMap<String, HashMap<String, QuotaCfg>>, // tenant -> capability -> quota
    pub global_cap_quotas: HashMap<String, QuotaCfg>, // capability -> quota (applies to all tenants)
    pub global_quota: Option<QuotaCfg>,
}

pub fn load_from_env() -> WardsConfig {
    let caps = std::env::var("WARDS_CAPS")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let quotas = std::env::var("WARDS_QUOTAS")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let (cap_quotas, global_cap_quotas) = parse_cap_quotas_env();
    let global_quota = std::env::var("WARDS_GLOBAL_QUOTA")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    WardsConfig {
        caps,
        quotas,
        cap_quotas,
        global_cap_quotas,
        global_quota,
    }
}

impl WardsConfig {
    pub fn effective_quota(&self, tenant: &str, capability: &str) -> QuotaCfg {
        if let Some(q) = self.cap_quotas.get(tenant).and_then(|m| m.get(capability)) {
            return q.clone();
        }
        if let Some(q) = self.quotas.get(tenant) {
            return q.clone();
        }
        if let Some(q) = self.global_cap_quotas.get(capability) {
            return q.clone();
        }
        if let Some(q) = &self.global_quota {
            return q.clone();
        }
        QuotaCfg {
            limit: 0,
            window_seconds: 60,
        }
    }
}

fn parse_cap_quotas_env() -> (
    HashMap<String, HashMap<String, QuotaCfg>>, // tenant → cap → quota
    HashMap<String, QuotaCfg>,                  // global cap → quota
) {
    let raw = match std::env::var("WARDS_CAP_QUOTAS") {
        Ok(s) if s.trim().is_empty() => return (HashMap::new(), HashMap::new()),
        Ok(s) => s,
        Err(_) => return (HashMap::new(), HashMap::new()),
    };

    // First try JSON format for backward compatibility
    if raw.trim_start().starts_with('{') {
        match serde_json::from_str::<HashMap<String, HashMap<String, QuotaCfg>>>(&raw) {
            Ok(map) => return (map, HashMap::new()),
            Err(e) => {
                panic!("WARDS_CAP_QUOTAS JSON is malformed: {}", e);
            }
        }
    }

    // Parse compact spec: comma-separated entries
    // GLOBAL:<cap>=<limit>:<window>,TENANT:<tenant>:<cap>=<limit>:<window>
    let mut tenant_caps: HashMap<String, HashMap<String, QuotaCfg>> = HashMap::new();
    let mut global_caps: HashMap<String, QuotaCfg> = HashMap::new();

    for (idx, entry) in raw.split(',').enumerate() {
        let e = entry.trim();
        if e.is_empty() {
            continue;
        }

        // Determine scope
        let (scope, rest) = if let Some(rem) = e.strip_prefix("GLOBAL:") {
            ("GLOBAL", rem)
        } else if let Some(rem) = e.strip_prefix("TENANT:") {
            ("TENANT", rem)
        } else {
            panic!(
                "WARDS_CAP_QUOTAS entry #{} must start with GLOBAL: or TENANT:",
                idx + 1
            );
        };

        if scope == "GLOBAL" {
            // rest := <cap>=<limit>:<window>
            let mut parts = rest.split('=');
            let cap = parts
                .next()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    panic!("GLOBAL entry #{} missing capability before '='", idx + 1)
                });
            let rhs = parts.next().unwrap_or_else(|| {
                panic!(
                    "GLOBAL entry #{} missing '<limit>:<window>' after '='",
                    idx + 1
                )
            });
            if parts.next().is_some() {
                panic!("GLOBAL entry #{} has extra '=' characters", idx + 1);
            }
            let (limit, window) = parse_limit_window(rhs, idx + 1);
            global_caps.insert(
                cap.to_string(),
                QuotaCfg {
                    limit,
                    window_seconds: window,
                },
            );
        } else {
            // TENANT scope: rest := <tenant>:<cap>=<limit>:<window>
            let mut tparts = rest.splitn(2, ':');
            let tenant = tparts
                .next()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    panic!("TENANT entry #{} missing tenant id before ':'", idx + 1)
                });
            let remainder = tparts.next().unwrap_or_else(|| {
                panic!(
                    "TENANT entry #{} missing '<cap>=<limit>:<window>' after tenant",
                    idx + 1
                )
            });
            let mut cparts = remainder.split('=');
            let cap = cparts
                .next()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    panic!("TENANT entry #{} missing capability before '='", idx + 1)
                });
            let rhs = cparts.next().unwrap_or_else(|| {
                panic!(
                    "TENANT entry #{} missing '<limit>:<window>' after '='",
                    idx + 1
                )
            });
            if cparts.next().is_some() {
                panic!("TENANT entry #{} has extra '=' characters", idx + 1);
            }
            let (limit, window) = parse_limit_window(rhs, idx + 1);
            let entry = tenant_caps.entry(tenant.to_string()).or_default();
            entry.insert(
                cap.to_string(),
                QuotaCfg {
                    limit,
                    window_seconds: window,
                },
            );
        }
    }

    (tenant_caps, global_caps)
}

fn parse_limit_window(s: &str, idx: usize) -> (u32, u64) {
    let mut lw = s.split(':');
    let limit_s = lw
        .next()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| panic!("entry #{} missing <limit> before ':'", idx));
    let window_s = lw
        .next()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| panic!("entry #{} missing <window> after ':'", idx));
    if lw.next().is_some() {
        panic!(
            "entry #{} has extra ':' parts (expected '<limit>:<window>')",
            idx
        );
    }
    let limit: u32 = limit_s.parse().unwrap_or_else(|_| {
        panic!(
            "entry #{} invalid limit '{}': must be integer",
            idx, limit_s
        )
    });
    let window: u64 = window_s.parse().unwrap_or_else(|_| {
        let mut msg = String::new();
        let _ = write!(
            &mut msg,
            "entry #{} invalid windowSeconds '{}': must be integer",
            idx, window_s
        );
        panic!("{}", msg);
    });
    (limit, window)
}
