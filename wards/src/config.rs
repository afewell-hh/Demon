use std::collections::HashMap;

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
    let cap_quotas = std::env::var("WARDS_CAP_QUOTAS")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let global_quota = std::env::var("WARDS_GLOBAL_QUOTA")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    WardsConfig {
        caps,
        quotas,
        cap_quotas,
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
        if let Some(q) = &self.global_quota {
            return q.clone();
        }
        QuotaCfg {
            limit: 0,
            window_seconds: 60,
        }
    }
}
