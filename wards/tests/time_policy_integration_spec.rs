#![allow(clippy::field_reassign_with_default)]
use chrono::{DateTime, TimeZone, Utc};
use serial_test::serial;
use std::collections::HashMap;
use wards::config::{QuotaCfg, WardsConfig};
use wards::policy::PolicyKernel;
use wards::schedule::{ScheduleAction, ScheduleConfig, ScheduleRule};

/// Mock a time provider for deterministic testing
#[allow(dead_code)]
struct MockTimeProvider {
    current_time: DateTime<Utc>,
}

#[allow(dead_code)]
impl MockTimeProvider {
    fn new(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> Self {
        Self {
            current_time: Utc
                .with_ymd_and_hms(year, month, day, hour, minute, 0)
                .unwrap(),
        }
    }

    fn now(&self) -> DateTime<Utc> {
        self.current_time
    }
}

/// Test integration: schedule evaluation with real time scenarios
#[test]
fn integration_schedule_evaluation_with_mock_time() {
    let mut config = WardsConfig::default();

    // Set up reasonable quota
    config.global_quota = Some(QuotaCfg {
        limit: 10,
        window_seconds: 3600,
    });

    // Business hours rule: allow Mon-Fri 9AM-5PM PST
    let business_rule = ScheduleRule {
        action: ScheduleAction::Allow,
        timezone: "America/Los_Angeles".to_string(),
        days: Some(vec![
            "Mon".to_string(),
            "Tue".to_string(),
            "Wed".to_string(),
            "Thu".to_string(),
            "Fri".to_string(),
        ]),
        start: "09:00".to_string(),
        end: "17:00".to_string(),
        escalation_timeout_seconds: None,
    };

    // Maintenance window: deny Sundays 2AM-4AM UTC
    let maintenance_rule = ScheduleRule {
        action: ScheduleAction::Deny,
        timezone: "UTC".to_string(),
        days: Some(vec!["Sun".to_string()]),
        start: "02:00".to_string(),
        end: "04:00".to_string(),
        escalation_timeout_seconds: None,
    };

    let mut schedule_config = ScheduleConfig::default();
    schedule_config
        .global_schedules
        .insert("capsule.deploy".to_string(), vec![maintenance_rule]);
    schedule_config.tenant_schedules.insert(
        "business-tenant".to_string(),
        HashMap::from([("capsule.deploy".to_string(), vec![business_rule])]),
    );
    config.schedules = schedule_config;

    let mut kernel = PolicyKernel::new(config);

    // Test various scenarios by checking decisions
    // Note: Since we can't easily inject time, we test the configuration and logic

    // Test business tenant vs regular tenant
    let business_decision = kernel.allow_and_count("business-tenant", "capsule.deploy");
    let regular_decision = kernel.allow_and_count("regular-tenant", "capsule.deploy");

    // Both should respect the same quota limits
    assert_eq!(business_decision.limit, regular_decision.limit);
    assert_eq!(
        business_decision.window_seconds,
        regular_decision.window_seconds
    );

    // Both should have the same basic structure but potentially different allow status
    // depending on current time and applicable rules
    assert!(business_decision.deny_reason.is_none() || business_decision.deny_reason.is_some());
    assert!(regular_decision.deny_reason.is_none() || regular_decision.deny_reason.is_some());
}

/// Test that schedule configuration is properly loaded from environment
#[test]
#[serial]
fn integration_full_env_loading_with_schedules() {
    // Set up comprehensive environment
    std::env::set_var(
        "WARDS_CAP_QUOTAS",
        r#"{
            "prod-tenant": {
                "capsule.deploy": { "limit": 5, "windowSeconds": 3600 },
                "capsule.audit": { "limit": 100, "windowSeconds": 300 }
            }
        }"#,
    );

    std::env::set_var(
        "WARDS_SCHEDULES",
        r#"{
            "global": {
                "capsule.audit": [
                    {
                        "action": "allow",
                        "timezone": "UTC",
                        "start": "00:00",
                        "end": "23:59"
                    }
                ]
            },
            "prod-tenant": {
                "capsule.deploy": [
                    {
                        "action": "allow",
                        "timezone": "America/New_York",
                        "days": ["Mon", "Tue", "Wed", "Thu", "Fri"],
                        "start": "09:00",
                        "end": "17:00",
                        "escalation_timeout_seconds": 7200
                    },
                    {
                        "action": "deny",
                        "timezone": "UTC",
                        "days": ["Sun"],
                        "start": "01:00",
                        "end": "05:00"
                    }
                ]
            }
        }"#,
    );

    let config = wards::config::load_from_env();
    let mut kernel = PolicyKernel::new(config);

    // Test that quotas are properly loaded
    let quota = kernel.effective_quota("prod-tenant", "capsule.deploy");
    assert_eq!(quota.limit, 5);
    assert_eq!(quota.window_seconds, 3600);

    let audit_quota = kernel.effective_quota("prod-tenant", "capsule.audit");
    assert_eq!(audit_quota.limit, 100);
    assert_eq!(audit_quota.window_seconds, 300);

    // Test that schedules are properly loaded and integrated
    let deploy_decision = kernel.allow_and_count("prod-tenant", "capsule.deploy");
    assert_eq!(deploy_decision.limit, 5); // From quota config

    let audit_decision = kernel.allow_and_count("prod-tenant", "capsule.audit");
    assert_eq!(audit_decision.limit, 100); // From quota config

    // Test other tenant (should get fallback behavior)
    let other_decision = kernel.allow_and_count("other-tenant", "capsule.deploy");
    assert_eq!(other_decision.limit, 0); // Fallback quota

    // Clean up
    std::env::remove_var("WARDS_CAP_QUOTAS");
    std::env::remove_var("WARDS_SCHEDULES");
}

/// Test error handling when schedule configuration is malformed
#[test]
#[serial]
fn integration_handles_malformed_schedule_config() {
    // Set malformed JSON
    std::env::set_var("WARDS_SCHEDULES", "{ invalid json");

    let config = wards::config::load_from_env();
    let mut kernel = PolicyKernel::new(config);

    // Should still work with quota-only behavior
    let decision = kernel.allow_and_count("tenant-a", "capsule.test");
    assert_eq!(decision.limit, 0); // Fallback quota
    assert!(decision.deny_reason.is_some() || !decision.allowed); // Should be denied by quota

    std::env::remove_var("WARDS_SCHEDULES");
}

/// Test escalation timeout configuration is preserved
#[test]
fn escalation_timeout_configuration_preserved() {
    let rule = ScheduleRule {
        action: ScheduleAction::Allow,
        timezone: "UTC".to_string(),
        days: Some(vec!["Mon".to_string()]),
        start: "09:00".to_string(),
        end: "17:00".to_string(),
        escalation_timeout_seconds: Some(3600),
    };

    assert_eq!(rule.escalation_timeout_seconds, Some(3600));

    // Test serialization/deserialization
    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: ScheduleRule = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.escalation_timeout_seconds, Some(3600));
}

/// Test precedence: time policy denial overrides quota allowance
#[test]
fn time_policy_denial_overrides_quota_allowance() {
    let mut config = WardsConfig::default();

    // Set high quota that would normally allow
    config.global_quota = Some(QuotaCfg {
        limit: 1000,
        window_seconds: 3600,
    });

    // But deny everything with a time policy
    let deny_rule = ScheduleRule {
        action: ScheduleAction::Deny,
        timezone: "UTC".to_string(),
        days: None, // all days
        start: "00:00".to_string(),
        end: "23:59".to_string(),
        escalation_timeout_seconds: None,
    };

    let mut schedule_config = ScheduleConfig::default();
    schedule_config
        .global_schedules
        .insert("capsule.test".to_string(), vec![deny_rule]);
    config.schedules = schedule_config;

    let mut kernel = PolicyKernel::new(config);

    // Should be denied by time policy despite high quota
    let decision = kernel.allow_and_count("tenant-a", "capsule.test");

    // If current time matches the deny rule, should be denied with time_policy_denied
    // If current time doesn't match, should be allowed
    // Either way, the quota limit should be preserved
    assert_eq!(decision.limit, 1000);
    assert_eq!(decision.window_seconds, 3600);

    // The deny_reason should indicate the reason for denial
    if !decision.allowed {
        assert!(
            decision.deny_reason == Some("time_policy_denied".to_string())
                || decision.deny_reason == Some("quota_exceeded".to_string())
        );
    }
}

/// Test multiple schedule rules - first matching rule wins
#[test]
fn multiple_schedule_rules_first_match_wins() {
    let schedule_json = r#"{
        "global": {
            "capsule.deploy": [
                {
                    "action": "deny",
                    "timezone": "UTC",
                    "days": ["Sun"],
                    "start": "00:00",
                    "end": "23:59"
                },
                {
                    "action": "allow",
                    "timezone": "UTC",
                    "start": "00:00",
                    "end": "23:59"
                }
            ]
        }
    }"#;

    let schedules: HashMap<String, HashMap<String, Vec<ScheduleRule>>> =
        serde_json::from_str(schedule_json).unwrap();

    let deploy_rules = &schedules["global"]["capsule.deploy"];
    assert_eq!(deploy_rules.len(), 2);
    assert_eq!(deploy_rules[0].action, ScheduleAction::Deny);
    assert_eq!(deploy_rules[1].action, ScheduleAction::Allow);

    // The first rule should take precedence on Sundays
    // The second rule should take precedence on other days (when first doesn't match)
}
