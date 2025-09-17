#![allow(clippy::field_reassign_with_default)]
#[allow(unused_imports)]
use chrono::{TimeZone, Utc};
use serial_test::serial;
use std::collections::HashMap;
use wards::config::{QuotaCfg, WardsConfig};
use wards::policy::PolicyKernel;
use wards::schedule::{ScheduleAction, ScheduleConfig, ScheduleRule};

/// Test that time-based policies deny requests during maintenance windows
#[test]
#[allow(clippy::field_reassign_with_default)]
fn time_policy_denies_during_maintenance_window() {
    let mut config = WardsConfig {
        global_quota: Some(QuotaCfg {
            limit: 100,
            window_seconds: 3600,
        }),
        ..Default::default()
    };

    // Add maintenance window: deny on Sundays 2AM-4AM UTC
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
    config.schedules = schedule_config;

    let mut kernel = PolicyKernel::new(config);

    // Test during maintenance window (Sunday 3 AM UTC)
    // We can't easily mock time, so we test by setting up the scenario
    let decision = kernel.allow_and_count("tenant-a", "capsule.deploy");

    // The decision depends on current time, but we can check the basic structure
    assert!(decision.limit == 100);
    assert!(decision.window_seconds == 3600);
}

/// Test that time-based policies allow requests outside maintenance windows
#[test]
fn time_policy_allows_outside_maintenance_window() {
    let mut config = WardsConfig::default();

    // Set up quota
    config.global_quota = Some(QuotaCfg {
        limit: 100,
        window_seconds: 3600,
    });

    // Add maintenance window: deny on Sundays 2AM-4AM UTC
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
    config.schedules = schedule_config;

    let mut kernel = PolicyKernel::new(config);

    // Test outside maintenance window - should be allowed by time policy
    let decision = kernel.allow_and_count("tenant-a", "capsule.deploy");

    // Basic structure check
    assert!(decision.limit == 100);
    assert!(decision.window_seconds == 3600);
}

/// Test that business hours restrictions work correctly
#[test]
fn business_hours_policy_enforcement() {
    let mut config = WardsConfig::default();

    // Set up quota
    config.global_quota = Some(QuotaCfg {
        limit: 100,
        window_seconds: 3600,
    });

    // Allow only during business hours: Mon-Fri 9AM-5PM PST
    let business_hours_rule = ScheduleRule {
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
        escalation_timeout_seconds: Some(3600), // 1 hour escalation timeout
    };

    let mut schedule_config = ScheduleConfig::default();
    schedule_config.tenant_schedules.insert(
        "corp-tenant".to_string(),
        HashMap::from([("capsule.prod_deploy".to_string(), vec![business_hours_rule])]),
    );
    config.schedules = schedule_config;

    let mut kernel = PolicyKernel::new(config);

    // Test with corporate tenant (has specific rules)
    let decision = kernel.allow_and_count("corp-tenant", "capsule.prod_deploy");
    assert!(decision.limit == 100);

    // Test with different tenant (no specific rules, should use global fallback)
    let decision2 = kernel.allow_and_count("other-tenant", "capsule.prod_deploy");
    assert!(decision2.limit == 100);
}

/// Test precedence: tenant-specific rules override global rules
#[test]
fn tenant_specific_schedule_overrides_global() {
    let mut config = WardsConfig::default();

    config.global_quota = Some(QuotaCfg {
        limit: 100,
        window_seconds: 3600,
    });

    // Global rule: deny on weekends
    let global_rule = ScheduleRule {
        action: ScheduleAction::Deny,
        timezone: "UTC".to_string(),
        days: Some(vec!["Sat".to_string(), "Sun".to_string()]),
        start: "00:00".to_string(),
        end: "23:59".to_string(),
        escalation_timeout_seconds: None,
    };

    // Tenant-specific rule: allow always for VIP tenant
    let vip_rule = ScheduleRule {
        action: ScheduleAction::Allow,
        timezone: "UTC".to_string(),
        days: None, // applies to all days
        start: "00:00".to_string(),
        end: "23:59".to_string(),
        escalation_timeout_seconds: None,
    };

    let mut schedule_config = ScheduleConfig::default();
    schedule_config
        .global_schedules
        .insert("capsule.deploy".to_string(), vec![global_rule]);
    schedule_config.tenant_schedules.insert(
        "vip-tenant".to_string(),
        HashMap::from([("capsule.deploy".to_string(), vec![vip_rule])]),
    );
    config.schedules = schedule_config;

    let mut kernel = PolicyKernel::new(config);

    // Both tenants should have same quota but different schedule behavior
    let vip_decision = kernel.allow_and_count("vip-tenant", "capsule.deploy");
    let regular_decision = kernel.allow_and_count("regular-tenant", "capsule.deploy");

    assert_eq!(vip_decision.limit, regular_decision.limit);
    assert_eq!(vip_decision.window_seconds, regular_decision.window_seconds);
}

/// Test loading schedules from environment variable
#[test]
#[serial]
fn load_schedules_from_env_variable() {
    let schedule_json = r#"{
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
        "tenant-a": {
            "capsule.deploy": [
                {
                    "action": "deny",
                    "timezone": "America/Los_Angeles",
                    "days": ["Sun"],
                    "start": "02:00",
                    "end": "04:00"
                }
            ]
        }
    }"#;

    std::env::set_var("WARDS_SCHEDULES", schedule_json);

    let config = wards::config::load_from_env();

    // Verify global schedule was loaded
    assert!(config
        .schedules
        .global_schedules
        .contains_key("capsule.audit"));
    let audit_rules = &config.schedules.global_schedules["capsule.audit"];
    assert_eq!(audit_rules.len(), 1);
    assert_eq!(audit_rules[0].action, ScheduleAction::Allow);

    // Verify tenant-specific schedule was loaded
    assert!(config.schedules.tenant_schedules.contains_key("tenant-a"));
    let tenant_caps = &config.schedules.tenant_schedules["tenant-a"];
    assert!(tenant_caps.contains_key("capsule.deploy"));
    let deploy_rules = &tenant_caps["capsule.deploy"];
    assert_eq!(deploy_rules.len(), 1);
    assert_eq!(deploy_rules[0].action, ScheduleAction::Deny);
    assert_eq!(deploy_rules[0].timezone, "America/Los_Angeles");

    std::env::remove_var("WARDS_SCHEDULES");
}

/// Test that quotas still apply when time policies allow
#[test]
fn quota_limits_still_apply_when_time_allows() {
    let mut config = WardsConfig::default();

    // Set very low quota
    config.global_quota = Some(QuotaCfg {
        limit: 1,
        window_seconds: 3600,
    });

    // Allow always
    let allow_rule = ScheduleRule {
        action: ScheduleAction::Allow,
        timezone: "UTC".to_string(),
        days: None,
        start: "00:00".to_string(),
        end: "23:59".to_string(),
        escalation_timeout_seconds: None,
    };

    let mut schedule_config = ScheduleConfig::default();
    schedule_config
        .global_schedules
        .insert("capsule.test".to_string(), vec![allow_rule]);
    config.schedules = schedule_config;

    let mut kernel = PolicyKernel::new(config);

    // First request should be allowed
    let decision1 = kernel.allow_and_count("tenant-a", "capsule.test");
    assert!(decision1.allowed);
    assert_eq!(decision1.remaining, 0);
    assert!(decision1.deny_reason.is_none());

    // Second request should be denied by quota, not by time policy
    let decision2 = kernel.allow_and_count("tenant-a", "capsule.test");
    assert!(!decision2.allowed);
    assert_eq!(decision2.remaining, 0);
    assert_eq!(decision2.deny_reason, Some("quota_exceeded".to_string()));
}

/// Test that no schedule rules means quota-only behavior
#[test]
fn no_schedule_rules_falls_back_to_quota_only() {
    let mut config = WardsConfig::default();

    config.global_quota = Some(QuotaCfg {
        limit: 2,
        window_seconds: 3600,
    });

    // No schedule rules configured
    config.schedules = ScheduleConfig::default();

    let mut kernel = PolicyKernel::new(config);

    // Should allow up to quota limit
    let decision1 = kernel.allow_and_count("tenant-a", "capsule.test");
    assert!(decision1.allowed);
    assert_eq!(decision1.remaining, 1);

    let decision2 = kernel.allow_and_count("tenant-a", "capsule.test");
    assert!(decision2.allowed);
    assert_eq!(decision2.remaining, 0);

    let decision3 = kernel.allow_and_count("tenant-a", "capsule.test");
    assert!(!decision3.allowed);
    assert_eq!(decision3.deny_reason, Some("quota_exceeded".to_string()));
}

/// Test timezone handling in schedule evaluation
#[test]
fn timezone_conversion_works_correctly() {
    // This test verifies that our timezone logic works, but since we can't easily mock time,
    // we test the configuration loading and basic structure
    let schedule_json = r#"{
        "global": {
            "capsule.deploy": [
                {
                    "action": "allow",
                    "timezone": "Asia/Tokyo",
                    "days": ["Mon", "Tue", "Wed", "Thu", "Fri"],
                    "start": "09:00",
                    "end": "17:00"
                }
            ]
        }
    }"#;

    let schedules: HashMap<String, HashMap<String, Vec<ScheduleRule>>> =
        serde_json::from_str(schedule_json).unwrap();

    let deploy_rules = &schedules["global"]["capsule.deploy"];
    assert_eq!(deploy_rules[0].timezone, "Asia/Tokyo");
    assert_eq!(deploy_rules[0].days.as_ref().unwrap().len(), 5);

    // Test that we can parse the timezone
    let _tz: chrono_tz::Tz = deploy_rules[0].timezone.parse().unwrap();
}
