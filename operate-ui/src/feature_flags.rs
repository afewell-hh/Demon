//! Feature flag support for Operate UI
//!
//! Feature flags are controlled via the `OPERATE_UI_FLAGS` environment variable
//! (comma-separated list, e.g., `OPERATE_UI_FLAGS=contracts-browser,other-feature`)

use std::collections::HashSet;
use std::sync::OnceLock;

static ENABLED_FLAGS: OnceLock<HashSet<String>> = OnceLock::new();

/// Initialize feature flags from environment variable
///
/// This is called automatically on first access to feature flags
fn init_feature_flags() -> HashSet<String> {
    let flags_str = std::env::var("OPERATE_UI_FLAGS").unwrap_or_default();
    flags_str
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Check if a feature flag is enabled
///
/// Feature flags are loaded from the `OPERATE_UI_FLAGS` environment variable
/// on first access and cached for the lifetime of the process.
pub fn is_enabled(flag: &str) -> bool {
    ENABLED_FLAGS
        .get_or_init(init_feature_flags)
        .contains(&flag.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_enabled_with_empty_env() {
        // With no environment variable set, no flags should be enabled
        assert!(!is_enabled("test-feature"));
        assert!(!is_enabled("other-feature"));
    }

    #[test]
    fn test_flag_matching_is_case_insensitive() {
        // This test assumes OPERATE_UI_FLAGS may be set in the environment
        // The lowercase normalization ensures case-insensitive matching
        let test_flag = "TEST-FLAG";
        assert_eq!(
            is_enabled(test_flag),
            is_enabled(test_flag.to_lowercase().as_str())
        );
    }
}
