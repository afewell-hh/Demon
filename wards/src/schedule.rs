use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleAction {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: String, // HH:MM format
    pub end: String,   // HH:MM format
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleRule {
    pub action: ScheduleAction,
    pub timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<Vec<String>>, // ["Mon", "Tue", etc.] - if None, applies to all days
    pub start: String, // HH:MM
    pub end: String,   // HH:MM
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escalation_timeout_seconds: Option<u64>, // Future use for auto-approval
}

#[derive(Debug, Clone, Default)]
pub struct ScheduleConfig {
    pub tenant_schedules: HashMap<String, HashMap<String, Vec<ScheduleRule>>>, // tenant -> capability -> rules
    pub global_schedules: HashMap<String, Vec<ScheduleRule>>, // capability -> rules
}

impl ScheduleRule {
    /// Evaluate if this rule applies at the given time
    pub fn applies_at(&self, current_time: DateTime<Utc>) -> Result<bool, ScheduleError> {
        let tz: Tz = self
            .timezone
            .parse()
            .map_err(|_| ScheduleError::InvalidTimezone(self.timezone.clone()))?;

        let local_time = current_time.with_timezone(&tz);

        // Check day of week if specified
        if let Some(ref days) = self.days {
            let current_weekday = local_time.weekday();
            let day_matches = days.iter().any(|day| {
                parse_weekday(day)
                    .map(|wd| wd == current_weekday)
                    .unwrap_or(false)
            });
            if !day_matches {
                return Ok(false);
            }
        }

        // Check time range
        let current_minutes = local_time.hour() * 60 + local_time.minute();
        let start_minutes = parse_time_to_minutes(&self.start)?;
        let end_minutes = parse_time_to_minutes(&self.end)?;

        // Handle time ranges that cross midnight
        let in_range = if start_minutes <= end_minutes {
            current_minutes >= start_minutes && current_minutes <= end_minutes
        } else {
            current_minutes >= start_minutes || current_minutes <= end_minutes
        };

        Ok(in_range)
    }
}

#[derive(Debug, Clone)]
pub enum ScheduleError {
    InvalidTimezone(String),
    InvalidTimeFormat(String),
    InvalidWeekday(String),
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleError::InvalidTimezone(tz) => write!(f, "Invalid timezone: {}", tz),
            ScheduleError::InvalidTimeFormat(time) => {
                write!(f, "Invalid time format: {} (expected HH:MM)", time)
            }
            ScheduleError::InvalidWeekday(day) => write!(f, "Invalid weekday: {}", day),
        }
    }
}

impl std::error::Error for ScheduleError {}

/// Parse time string (HH:MM) to minutes since midnight
fn parse_time_to_minutes(time_str: &str) -> Result<u32, ScheduleError> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        return Err(ScheduleError::InvalidTimeFormat(time_str.to_string()));
    }

    let hours: u32 = parts[0]
        .parse()
        .map_err(|_| ScheduleError::InvalidTimeFormat(time_str.to_string()))?;
    let minutes: u32 = parts[1]
        .parse()
        .map_err(|_| ScheduleError::InvalidTimeFormat(time_str.to_string()))?;

    if hours >= 24 || minutes >= 60 {
        return Err(ScheduleError::InvalidTimeFormat(time_str.to_string()));
    }

    Ok(hours * 60 + minutes)
}

/// Parse weekday string to chrono::Weekday
fn parse_weekday(day: &str) -> Result<Weekday, ScheduleError> {
    match day.to_lowercase().as_str() {
        "mon" | "monday" => Ok(Weekday::Mon),
        "tue" | "tuesday" => Ok(Weekday::Tue),
        "wed" | "wednesday" => Ok(Weekday::Wed),
        "thu" | "thursday" => Ok(Weekday::Thu),
        "fri" | "friday" => Ok(Weekday::Fri),
        "sat" | "saturday" => Ok(Weekday::Sat),
        "sun" | "sunday" => Ok(Weekday::Sun),
        _ => Err(ScheduleError::InvalidWeekday(day.to_string())),
    }
}

/// Load schedule configuration from environment variables
pub fn load_schedules_from_env() -> ScheduleConfig {
    let mut config = ScheduleConfig::default();

    // Load WARDS_SCHEDULES env var
    if let Ok(schedules_json) = std::env::var("WARDS_SCHEDULES") {
        if !schedules_json.trim().is_empty() {
            if let Ok(parsed) = serde_json::from_str::<
                HashMap<String, HashMap<String, Vec<ScheduleRule>>>,
            >(&schedules_json)
            {
                // Split into tenant-specific and global schedules
                for (tenant_or_global, capabilities) in parsed {
                    if tenant_or_global == "global" {
                        config.global_schedules = capabilities;
                    } else {
                        config
                            .tenant_schedules
                            .insert(tenant_or_global, capabilities);
                    }
                }
            }
        }
    }

    config
}

impl ScheduleConfig {
    /// Get applicable schedule rules for a tenant and capability
    /// Returns rules in precedence order: tenant-specific -> global
    pub fn get_rules(&self, tenant: &str, capability: &str) -> Vec<&ScheduleRule> {
        let mut rules = Vec::new();

        // Add tenant-specific rules first (higher precedence)
        if let Some(tenant_caps) = self.tenant_schedules.get(tenant) {
            if let Some(cap_rules) = tenant_caps.get(capability) {
                rules.extend(cap_rules.iter());
            }
        }

        // Add global rules
        if let Some(global_rules) = self.global_schedules.get(capability) {
            rules.extend(global_rules.iter());
        }

        rules
    }

    /// Evaluate whether an action is allowed at the current time
    /// Returns None if no schedule rules apply (fallback to quota-only behavior)
    /// Returns Some(true) if allowed, Some(false) if denied
    pub fn evaluate_at(
        &self,
        tenant: &str,
        capability: &str,
        current_time: DateTime<Utc>,
    ) -> Result<Option<bool>, ScheduleError> {
        let rules = self.get_rules(tenant, capability);

        if rules.is_empty() {
            return Ok(None); // No schedule rules, allow quota checks to proceed
        }

        // Evaluate rules in order - first matching rule wins
        for rule in rules {
            if rule.applies_at(current_time)? {
                return Ok(Some(rule.action == ScheduleAction::Allow));
            }
        }

        // No matching rules found - default to allow
        Ok(Some(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_parse_time_to_minutes() {
        assert_eq!(parse_time_to_minutes("09:00").unwrap(), 9 * 60);
        assert_eq!(parse_time_to_minutes("17:30").unwrap(), 17 * 60 + 30);
        assert_eq!(parse_time_to_minutes("00:00").unwrap(), 0);
        assert_eq!(parse_time_to_minutes("23:59").unwrap(), 23 * 60 + 59);

        assert!(parse_time_to_minutes("24:00").is_err());
        assert!(parse_time_to_minutes("09:60").is_err());
        assert!(parse_time_to_minutes("invalid").is_err());
    }

    #[test]
    fn test_parse_weekday() {
        assert_eq!(parse_weekday("Mon").unwrap(), Weekday::Mon);
        assert_eq!(parse_weekday("monday").unwrap(), Weekday::Mon);
        assert_eq!(parse_weekday("Fri").unwrap(), Weekday::Fri);
        assert!(parse_weekday("Invalid").is_err());
    }

    #[test]
    fn test_schedule_rule_applies_at() {
        let rule = ScheduleRule {
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

        // Test during work hours on a weekday (assuming it's a Monday at 10 AM PST)
        let monday_10am_utc = Utc.with_ymd_and_hms(2024, 1, 8, 18, 0, 0).unwrap(); // Monday 10 AM PST = 6 PM UTC
        assert!(rule.applies_at(monday_10am_utc).unwrap());

        // Test outside work hours
        let monday_6am_utc = Utc.with_ymd_and_hms(2024, 1, 8, 14, 0, 0).unwrap(); // Monday 6 AM PST = 2 PM UTC
        assert!(!rule.applies_at(monday_6am_utc).unwrap());

        // Test on weekend
        let sunday_10am_utc = Utc.with_ymd_and_hms(2024, 1, 7, 18, 0, 0).unwrap(); // Sunday 10 AM PST = 6 PM UTC
        assert!(!rule.applies_at(sunday_10am_utc).unwrap());
    }

    #[test]
    fn test_schedule_config_evaluate_at() {
        let mut config = ScheduleConfig::default();

        // Add a deny rule for maintenance window
        let maintenance_rule = ScheduleRule {
            action: ScheduleAction::Deny,
            timezone: "UTC".to_string(),
            days: Some(vec!["Sun".to_string()]),
            start: "02:00".to_string(),
            end: "04:00".to_string(),
            escalation_timeout_seconds: None,
        };

        config
            .global_schedules
            .insert("capsule.deploy".to_string(), vec![maintenance_rule]);

        // Test during maintenance window
        let sunday_3am_utc = Utc.with_ymd_and_hms(2024, 1, 7, 3, 0, 0).unwrap();
        let result = config
            .evaluate_at("tenant-a", "capsule.deploy", sunday_3am_utc)
            .unwrap();
        assert_eq!(result, Some(false)); // Should be denied

        // Test outside maintenance window
        let sunday_5am_utc = Utc.with_ymd_and_hms(2024, 1, 7, 5, 0, 0).unwrap();
        let result = config
            .evaluate_at("tenant-a", "capsule.deploy", sunday_5am_utc)
            .unwrap();
        assert_eq!(result, Some(true)); // Should be allowed

        // Test capability with no rules
        let result = config
            .evaluate_at("tenant-a", "capsule.other", sunday_3am_utc)
            .unwrap();
        assert_eq!(result, None); // No rules, fallback to quota
    }
}
