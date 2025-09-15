use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    pub enabled: bool,
    pub default_tenant: String,
    pub allowlist: Option<HashSet<String>>,
}

impl Default for TenantConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_tenant: "default".to_string(),
            allowlist: None,
        }
    }
}

impl TenantConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("TENANTING_ENABLED")
            .unwrap_or_else(|_| "0".to_string())
            .parse::<u8>()
            .unwrap_or(0)
            == 1;

        let default_tenant =
            std::env::var("TENANT_DEFAULT").unwrap_or_else(|_| "default".to_string());

        let allowlist = std::env::var("TENANT_ALLOWLIST").ok().and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.split(',').map(|s| s.trim().to_string()).collect())
            }
        });

        debug!(
            "Tenant configuration - enabled: {}, default: {}, allowlist: {:?}",
            enabled, default_tenant, allowlist
        );

        Self {
            enabled,
            default_tenant,
            allowlist,
        }
    }

    pub fn validate_tenant(&self, tenant: &str) -> bool {
        if !self.enabled {
            return true;
        }

        if let Some(ref allowlist) = self.allowlist {
            if allowlist.contains(tenant) {
                return true;
            }
            warn!("Tenant '{}' not in allowlist: {:?}", tenant, allowlist);
            false
        } else {
            true
        }
    }

    pub fn resolve_tenant(&self, requested: Option<&str>) -> String {
        if !self.enabled {
            return self.default_tenant.clone();
        }

        match requested {
            Some(tenant) => {
                if self.validate_tenant(tenant) {
                    tenant.to_string()
                } else {
                    self.default_tenant.clone()
                }
            }
            None => self.default_tenant.clone(),
        }
    }

    pub fn get_subject_pattern(
        &self,
        tenant: &str,
        ritual_id: Option<&str>,
        run_id: Option<&str>,
    ) -> String {
        if !self.enabled {
            match (ritual_id, run_id) {
                (Some(ritual), Some(run)) => format!("demon.ritual.v1.{}.{}.events", ritual, run),
                (Some(ritual), None) => format!("demon.ritual.v1.{}.*.events", ritual),
                (None, None) => "demon.ritual.v1.*.*.events".to_string(),
                _ => "demon.ritual.v1.*.*.events".to_string(),
            }
        } else {
            match (ritual_id, run_id) {
                (Some(ritual), Some(run)) => {
                    format!("demon.ritual.v1.{}.{}.{}.events", tenant, ritual, run)
                }
                (Some(ritual), None) => format!("demon.ritual.v1.{}.{}.*.events", tenant, ritual),
                (None, None) => format!("demon.ritual.v1.{}.*.*events", tenant),
                _ => format!("demon.ritual.v1.{}.*.*events", tenant),
            }
        }
    }

    pub fn extract_tenant_from_subject(&self, subject: &str) -> Option<String> {
        if !self.enabled {
            return Some(self.default_tenant.clone());
        }

        let parts: Vec<&str> = subject.split('.').collect();
        if parts.len() >= 4 && parts[0] == "demon" && parts[1] == "ritual" && parts[2] == "v1" {
            Some(parts[3].to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_config_default() {
        let config = TenantConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.default_tenant, "default");
        assert!(config.allowlist.is_none());
    }

    #[test]
    fn test_resolve_tenant_disabled() {
        let config = TenantConfig {
            enabled: false,
            default_tenant: "default".to_string(),
            allowlist: Some(
                ["tenant-a", "tenant-b"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
        };

        assert_eq!(config.resolve_tenant(Some("tenant-a")), "default");
        assert_eq!(config.resolve_tenant(Some("unknown")), "default");
        assert_eq!(config.resolve_tenant(None), "default");
    }

    #[test]
    fn test_resolve_tenant_enabled_with_allowlist() {
        let config = TenantConfig {
            enabled: true,
            default_tenant: "default".to_string(),
            allowlist: Some(
                ["default", "tenant-a", "tenant-b"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
        };

        assert_eq!(config.resolve_tenant(Some("tenant-a")), "tenant-a");
        assert_eq!(config.resolve_tenant(Some("unknown")), "default");
        assert_eq!(config.resolve_tenant(None), "default");
    }

    #[test]
    fn test_resolve_tenant_enabled_no_allowlist() {
        let config = TenantConfig {
            enabled: true,
            default_tenant: "default".to_string(),
            allowlist: None,
        };

        assert_eq!(config.resolve_tenant(Some("any-tenant")), "any-tenant");
        assert_eq!(config.resolve_tenant(None), "default");
    }

    #[test]
    fn test_subject_pattern_disabled() {
        let config = TenantConfig {
            enabled: false,
            ..Default::default()
        };

        assert_eq!(
            config.get_subject_pattern("tenant-a", Some("ritual1"), Some("run1")),
            "demon.ritual.v1.ritual1.run1.events"
        );
        assert_eq!(
            config.get_subject_pattern("tenant-a", None, None),
            "demon.ritual.v1.*.*.events"
        );
    }

    #[test]
    fn test_subject_pattern_enabled() {
        let config = TenantConfig {
            enabled: true,
            ..Default::default()
        };

        assert_eq!(
            config.get_subject_pattern("tenant-a", Some("ritual1"), Some("run1")),
            "demon.ritual.v1.tenant-a.ritual1.run1.events"
        );
        assert_eq!(
            config.get_subject_pattern("tenant-a", None, None),
            "demon.ritual.v1.tenant-a.*.*events"
        );
    }

    #[test]
    fn test_extract_tenant_from_subject() {
        let config = TenantConfig {
            enabled: true,
            ..Default::default()
        };

        assert_eq!(
            config.extract_tenant_from_subject("demon.ritual.v1.tenant-a.ritual1.run1.events"),
            Some("tenant-a".to_string())
        );

        let disabled_config = TenantConfig {
            enabled: false,
            default_tenant: "default".to_string(),
            ..Default::default()
        };

        assert_eq!(
            disabled_config.extract_tenant_from_subject("demon.ritual.v1.ritual1.run1.events"),
            Some("default".to_string())
        );
    }
}
