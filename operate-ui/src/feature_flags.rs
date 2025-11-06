//! Feature flag support for Operate UI
//!
//! Feature flags can be controlled via:
//! - `OPERATE_UI_FLAGS` environment variable (comma-separated list)
//! - URL query parameter `?flags=feature1,feature2`

use std::collections::HashSet;
use std::sync::OnceLock;

static ENABLED_FLAGS: OnceLock<HashSet<String>> = OnceLock::new();

/// Initialize feature flags from environment variable
pub fn init_feature_flags() {
    let flags_str = std::env::var("OPERATE_UI_FLAGS").unwrap_or_default();
    let flags: HashSet<String> = flags_str
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let _ = ENABLED_FLAGS.set(flags);
}

/// Check if a feature flag is enabled
pub fn is_enabled(flag: &str) -> bool {
    ENABLED_FLAGS
        .get_or_init(|| {
            init_feature_flags();
            ENABLED_FLAGS.get().cloned().unwrap_or_default()
        })
        .contains(&flag.to_lowercase())
}

/// Check if feature flag is enabled via query parameter
pub fn is_enabled_via_query(flag: &str, query_flags: Option<&str>) -> bool {
    if is_enabled(flag) {
        return true;
    }

    // Check query parameter
    if let Some(query) = query_flags {
        query
            .split(',')
            .any(|f| f.trim().eq_ignore_ascii_case(flag))
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_enabled_via_query() {
        assert!(is_enabled_via_query(
            "test-feature",
            Some("test-feature,other")
        ));
        assert!(is_enabled_via_query("test-feature", Some("TEST-FEATURE")));
        assert!(!is_enabled_via_query("test-feature", Some("other")));
        assert!(!is_enabled_via_query("test-feature", None));
    }
}
