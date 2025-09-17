use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for approval escalation chains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationConfig {
    /// Per-tenant escalation rules
    pub tenants: HashMap<String, TenantEscalationRules>,
}

/// Escalation rules for a specific tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantEscalationRules {
    /// Gate-specific escalation chains (e.g., "ritual.deploy" -> chain)
    pub gates: HashMap<String, EscalationChain>,
}

/// A complete escalation chain for a gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationChain {
    /// Ordered list of escalation levels
    pub levels: Vec<EscalationLevel>,
}

/// A single level in an escalation chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationLevel {
    /// Level number (1-based for display)
    pub level: u32,
    /// Required roles for this level
    pub roles: Vec<String>,
    /// Timeout in seconds before escalating to next level (0 = no timeout)
    #[serde(rename = "timeoutSeconds")]
    pub timeout_seconds: u64,
    /// Whether this level allows emergency override
    #[serde(default)]
    #[serde(rename = "emergencyOverride")]
    pub emergency_override: bool,
    /// Optional notification hooks (placeholder for future implementation)
    #[serde(default)]
    pub notifications: Vec<String>,
}

/// Current state of an approval request in the escalation chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationState {
    /// Current level (1-based)
    pub current_level: u32,
    /// Total levels in the chain
    pub total_levels: u32,
    /// Timestamp when current level started
    pub level_started_at: chrono::DateTime<chrono::Utc>,
    /// Next escalation time (if any)
    pub next_escalation_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether this request was emergency overridden
    pub emergency_override: bool,
    /// History of escalations
    pub escalation_history: Vec<EscalationHistoryEntry>,
}

/// Entry in the escalation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationHistoryEntry {
    /// Level that was escalated from
    pub from_level: u32,
    /// Level that was escalated to
    pub to_level: u32,
    /// Timestamp of escalation
    pub escalated_at: chrono::DateTime<chrono::Utc>,
    /// Reason for escalation (timeout, manual, etc.)
    pub reason: String,
}

impl EscalationConfig {
    /// Load escalation configuration from environment variable
    pub fn from_env() -> Result<Option<Self>> {
        match std::env::var("APPROVAL_ESCALATION_RULES") {
            Ok(json_str) => {
                let config: EscalationConfig = serde_json::from_str(&json_str)?;
                Ok(Some(config))
            }
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(e) => Err(anyhow::anyhow!(
                "Failed to read APPROVAL_ESCALATION_RULES: {}",
                e
            )),
        }
    }

    /// Get escalation chain for a specific tenant and gate
    pub fn get_chain(&self, tenant: &str, gate_id: &str) -> Option<&EscalationChain> {
        self.tenants
            .get(tenant)
            .and_then(|rules| rules.gates.get(gate_id))
    }
}

impl EscalationChain {
    /// Get the first level of the chain
    pub fn first_level(&self) -> Option<&EscalationLevel> {
        self.levels.first()
    }

    /// Get a specific level by number (1-based)
    pub fn get_level(&self, level: u32) -> Option<&EscalationLevel> {
        self.levels.iter().find(|l| l.level == level)
    }

    /// Get the next level after the given level
    pub fn next_level(&self, current_level: u32) -> Option<&EscalationLevel> {
        self.levels.iter().find(|l| l.level == current_level + 1)
    }

    /// Check if this is the final level
    pub fn is_final_level(&self, level: u32) -> bool {
        self.levels.iter().map(|l| l.level).max().unwrap_or(0) == level
    }

    /// Validate that the chain is well-formed
    pub fn validate(&self) -> Result<()> {
        if self.levels.is_empty() {
            return Err(anyhow::anyhow!("Escalation chain cannot be empty"));
        }

        // Check that levels are consecutive starting from 1
        let mut expected_level = 1;
        for level in &self.levels {
            if level.level != expected_level {
                return Err(anyhow::anyhow!(
                    "Escalation levels must be consecutive starting from 1, found level {} but expected {}",
                    level.level,
                    expected_level
                ));
            }
            expected_level += 1;

            if level.roles.is_empty() {
                return Err(anyhow::anyhow!(
                    "Level {} must have at least one role",
                    level.level
                ));
            }
        }

        Ok(())
    }
}

impl EscalationState {
    /// Create initial escalation state for a new approval request
    pub fn new(chain: &EscalationChain) -> Result<Self> {
        chain.validate()?;

        let first_level = chain
            .first_level()
            .ok_or_else(|| anyhow::anyhow!("Chain has no levels"))?;

        let now = chrono::Utc::now();
        let next_escalation_at = if first_level.timeout_seconds > 0 {
            Some(now + chrono::Duration::seconds(first_level.timeout_seconds as i64))
        } else {
            None
        };

        Ok(Self {
            current_level: 1,
            total_levels: chain.levels.len() as u32,
            level_started_at: now,
            next_escalation_at,
            emergency_override: false,
            escalation_history: Vec::new(),
        })
    }

    /// Escalate to the next level
    pub fn escalate(&mut self, chain: &EscalationChain, reason: String) -> Result<bool> {
        if let Some(next_level) = chain.next_level(self.current_level) {
            // Record the escalation
            self.escalation_history.push(EscalationHistoryEntry {
                from_level: self.current_level,
                to_level: next_level.level,
                escalated_at: chrono::Utc::now(),
                reason,
            });

            // Update state
            self.current_level = next_level.level;
            self.level_started_at = chrono::Utc::now();

            // Set next escalation time
            self.next_escalation_at = if next_level.timeout_seconds > 0 {
                Some(
                    self.level_started_at
                        + chrono::Duration::seconds(next_level.timeout_seconds as i64),
                )
            } else {
                None
            };

            Ok(true) // Escalated
        } else {
            Ok(false) // No more levels to escalate to
        }
    }

    /// Check if the current level has timed out
    pub fn is_timed_out(&self) -> bool {
        self.next_escalation_at
            .map(|timeout| chrono::Utc::now() > timeout)
            .unwrap_or(false)
    }

    /// Mark as emergency override
    pub fn mark_emergency_override(&mut self) {
        self.emergency_override = true;
        self.next_escalation_at = None; // Stop escalation timers
    }

    /// Check if current level allows emergency override
    pub fn can_emergency_override(&self, chain: &EscalationChain) -> bool {
        chain
            .get_level(self.current_level)
            .map(|level| level.emergency_override)
            .unwrap_or(false)
    }
}

/// Check if an approver is allowed for a specific role
pub fn approver_allowed_for_role(approver: &str, _role: &str) -> bool {
    // For now, use the existing APPROVER_ALLOWLIST
    // In future, this could be extended to support role-based mapping
    let allowlist = std::env::var("APPROVER_ALLOWLIST").unwrap_or_default();
    if allowlist.is_empty() {
        return false;
    }

    // TODO: Implement role-specific allowlists
    // For MVP, any allowed approver can approve at any level
    allowlist
        .split(',')
        .map(|s| s.trim())
        .any(|allowed| !allowed.is_empty() && allowed.eq_ignore_ascii_case(approver))
}

/// Check if an approver is allowed for a specific escalation level
pub fn approver_allowed_for_level(approver: &str, level: &EscalationLevel) -> bool {
    // For MVP, check against any role in the level
    // In future, this could be more sophisticated role mapping
    level
        .roles
        .iter()
        .any(|role| approver_allowed_for_role(approver, role))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escalation_config_parsing() {
        let json = r#"
        {
            "tenants": {
                "tenant-a": {
                    "gates": {
                        "ritual.deploy": {
                            "levels": [
                                {
                                    "level": 1,
                                    "roles": ["team-lead"],
                                    "timeoutSeconds": 7200
                                },
                                {
                                    "level": 2,
                                    "roles": ["manager"],
                                    "timeoutSeconds": 86400
                                },
                                {
                                    "level": 3,
                                    "roles": ["director"],
                                    "timeoutSeconds": 0,
                                    "emergencyOverride": true
                                }
                            ]
                        }
                    }
                }
            }
        }
        "#;

        let config: EscalationConfig = serde_json::from_str(json).unwrap();
        let chain = config.get_chain("tenant-a", "ritual.deploy").unwrap();

        assert_eq!(chain.levels.len(), 3);
        assert_eq!(chain.first_level().unwrap().level, 1);
        assert!(chain.get_level(3).unwrap().emergency_override);

        chain.validate().unwrap();
    }

    #[test]
    fn test_escalation_state_creation() {
        let chain = EscalationChain {
            levels: vec![
                EscalationLevel {
                    level: 1,
                    roles: vec!["team-lead".to_string()],
                    timeout_seconds: 3600,
                    emergency_override: false,
                    notifications: vec![],
                },
                EscalationLevel {
                    level: 2,
                    roles: vec!["manager".to_string()],
                    timeout_seconds: 0,
                    emergency_override: true,
                    notifications: vec![],
                },
            ],
        };

        let state = EscalationState::new(&chain).unwrap();
        assert_eq!(state.current_level, 1);
        assert_eq!(state.total_levels, 2);
        assert!(state.next_escalation_at.is_some());
        assert!(!state.emergency_override);
    }

    #[test]
    fn test_escalation_state_escalate() {
        let chain = EscalationChain {
            levels: vec![
                EscalationLevel {
                    level: 1,
                    roles: vec!["team-lead".to_string()],
                    timeout_seconds: 3600,
                    emergency_override: false,
                    notifications: vec![],
                },
                EscalationLevel {
                    level: 2,
                    roles: vec!["manager".to_string()],
                    timeout_seconds: 0,
                    emergency_override: true,
                    notifications: vec![],
                },
            ],
        };

        let mut state = EscalationState::new(&chain).unwrap();

        // Escalate from level 1 to 2
        let escalated = state.escalate(&chain, "timeout".to_string()).unwrap();
        assert!(escalated);
        assert_eq!(state.current_level, 2);
        assert_eq!(state.escalation_history.len(), 1);
        assert!(state.next_escalation_at.is_none()); // Level 2 has no timeout

        // Try to escalate beyond final level
        let escalated = state.escalate(&chain, "test".to_string()).unwrap();
        assert!(!escalated);
        assert_eq!(state.current_level, 2);
    }

    #[test]
    fn test_chain_validation() {
        // Valid chain
        let valid_chain = EscalationChain {
            levels: vec![EscalationLevel {
                level: 1,
                roles: vec!["role1".to_string()],
                timeout_seconds: 0,
                emergency_override: false,
                notifications: vec![],
            }],
        };
        valid_chain.validate().unwrap();

        // Empty chain
        let empty_chain = EscalationChain { levels: vec![] };
        assert!(empty_chain.validate().is_err());

        // Non-consecutive levels
        let bad_chain = EscalationChain {
            levels: vec![
                EscalationLevel {
                    level: 1,
                    roles: vec!["role1".to_string()],
                    timeout_seconds: 0,
                    emergency_override: false,
                    notifications: vec![],
                },
                EscalationLevel {
                    level: 3, // Should be 2
                    roles: vec!["role2".to_string()],
                    timeout_seconds: 0,
                    emergency_override: false,
                    notifications: vec![],
                },
            ],
        };
        assert!(bad_chain.validate().is_err());

        // Level with no roles
        let no_roles_chain = EscalationChain {
            levels: vec![EscalationLevel {
                level: 1,
                roles: vec![], // Empty roles
                timeout_seconds: 0,
                emergency_override: false,
                notifications: vec![],
            }],
        };
        assert!(no_roles_chain.validate().is_err());
    }
}
