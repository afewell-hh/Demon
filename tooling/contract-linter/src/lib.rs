//! Contract schema linter for detecting breaking changes
//!
//! Compares two versions of a JSON Schema contract and detects breaking changes
//! such as removed fields, type changes, or constraint tightening.

use anyhow::{Context, Result};
use semver::Version;
use serde_json::Value;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LintError {
    #[error("Breaking change detected: {0}")]
    BreakingChange(String),

    #[error("Version bump required: {current} -> {proposed} (breaking changes detected but version not incremented properly)")]
    VersionBumpRequired { current: String, proposed: String },

    #[error("Invalid semver version: {0}")]
    InvalidVersion(String),
}

/// Result of linting a contract schema change
#[derive(Debug, Clone)]
pub struct LintResult {
    pub breaking_changes: Vec<String>,
    pub version_check_passed: bool,
    pub current_version: Option<String>,
    pub proposed_version: Option<String>,
}

impl LintResult {
    pub fn is_ok(&self) -> bool {
        self.breaking_changes.is_empty() || self.version_check_passed
    }

    pub fn has_breaking_changes(&self) -> bool {
        !self.breaking_changes.is_empty()
    }
}

/// Compare two JSON Schema objects and detect breaking changes
pub fn lint_schema_change(
    current_schema: &Value,
    proposed_schema: &Value,
    current_version: Option<&str>,
    proposed_version: Option<&str>,
) -> Result<LintResult> {
    let mut breaking_changes = Vec::new();

    // Detect breaking changes in schema structure
    detect_breaking_changes(
        current_schema,
        proposed_schema,
        "root",
        &mut breaking_changes,
    );

    // Check if version bump is appropriate for breaking changes
    let version_check_passed = if !breaking_changes.is_empty() {
        match (current_version, proposed_version) {
            (Some(curr), Some(prop)) => validate_version_bump(curr, prop)?,
            _ => false, // No version info, can't validate
        }
    } else {
        true // No breaking changes, version bump is optional
    };

    Ok(LintResult {
        breaking_changes,
        version_check_passed,
        current_version: current_version.map(String::from),
        proposed_version: proposed_version.map(String::from),
    })
}

/// Detect breaking changes between two schema values
fn detect_breaking_changes(
    current: &Value,
    proposed: &Value,
    path: &str,
    changes: &mut Vec<String>,
) {
    match (current, proposed) {
        (Value::Object(curr_obj), Value::Object(prop_obj)) => {
            // Check for removed properties
            let current_props = get_properties(curr_obj);
            let proposed_props = get_properties(prop_obj);

            for key in current_props.difference(&proposed_props) {
                changes.push(format!("Removed required property '{}' at {}", key, path));
            }

            // Check for type changes in common properties
            for key in current_props.intersection(&proposed_props) {
                let curr_val = curr_obj
                    .get("properties")
                    .and_then(|p| p.as_object())
                    .and_then(|p| p.get(key.as_str()));
                let prop_val = prop_obj
                    .get("properties")
                    .and_then(|p| p.as_object())
                    .and_then(|p| p.get(key.as_str()));

                if let (Some(curr_val), Some(prop_val)) = (curr_val, prop_val) {
                    let new_path = format!("{}.{}", path, key);
                    detect_breaking_changes(curr_val, prop_val, &new_path, changes);
                }
            }

            // Check for type changes in schema
            if let (Some(curr_type), Some(prop_type)) = (curr_obj.get("type"), prop_obj.get("type"))
            {
                if curr_type != prop_type {
                    changes.push(format!(
                        "Type changed from {:?} to {:?} at {}",
                        curr_type, prop_type, path
                    ));
                }

                // When type is "array", recursively check items schema for breaking changes
                if curr_type == "array" && prop_type == "array" {
                    if let (Some(curr_items), Some(prop_items)) =
                        (curr_obj.get("items"), prop_obj.get("items"))
                    {
                        detect_breaking_changes(
                            curr_items,
                            prop_items,
                            &format!("{}[items]", path),
                            changes,
                        );
                    }
                }
            }

            // Check for stricter constraints
            check_constraint_changes(curr_obj, prop_obj, path, changes);
        }
        (Value::Array(curr_arr), Value::Array(prop_arr)) => {
            // For arrays, check if items schema changed
            if let (Some(curr_items), Some(prop_items)) = (curr_arr.first(), prop_arr.first()) {
                detect_breaking_changes(curr_items, prop_items, &format!("{}[]", path), changes);
            }
        }
        _ => {
            // Primitive type mismatch
            if current != proposed && !is_compatible_change(current, proposed) {
                changes.push(format!(
                    "Incompatible value change at {}: {:?} -> {:?}",
                    path, current, proposed
                ));
            }
        }
    }
}

/// Extract property names from a JSON Schema object
fn get_properties(obj: &serde_json::Map<String, Value>) -> HashSet<String> {
    obj.get("properties")
        .and_then(|v| v.as_object())
        .map(|props| props.keys().cloned().collect())
        .unwrap_or_default()
}

/// Check for constraint tightening (breaking changes)
fn check_constraint_changes(
    current: &serde_json::Map<String, Value>,
    proposed: &serde_json::Map<String, Value>,
    path: &str,
    changes: &mut Vec<String>,
) {
    // Check minLength increase
    if let (Some(curr_min), Some(prop_min)) = (
        current.get("minLength").and_then(|v| v.as_u64()),
        proposed.get("minLength").and_then(|v| v.as_u64()),
    ) {
        if prop_min > curr_min {
            changes.push(format!(
                "Increased minLength from {} to {} at {}",
                curr_min, prop_min, path
            ));
        }
    }

    // Check maxLength decrease
    if let (Some(curr_max), Some(prop_max)) = (
        current.get("maxLength").and_then(|v| v.as_u64()),
        proposed.get("maxLength").and_then(|v| v.as_u64()),
    ) {
        if prop_max < curr_max {
            changes.push(format!(
                "Decreased maxLength from {} to {} at {}",
                curr_max, prop_max, path
            ));
        }
    }

    // Check required fields added
    if let (Some(curr_req), Some(prop_req)) = (
        current.get("required").and_then(|v| v.as_array()),
        proposed.get("required").and_then(|v| v.as_array()),
    ) {
        let curr_set: HashSet<_> = curr_req.iter().collect();
        let prop_set: HashSet<_> = prop_req.iter().collect();

        for new_req in prop_set.difference(&curr_set) {
            changes.push(format!(
                "New required field added: {:?} at {}",
                new_req, path
            ));
        }
    }
}

/// Check if a value change is compatible (non-breaking)
fn is_compatible_change(current: &Value, proposed: &Value) -> bool {
    // Allow string description/title changes
    matches!((current, proposed), (Value::String(_), Value::String(_)))
}

/// Validate that version bump is appropriate for breaking changes
fn validate_version_bump(current: &str, proposed: &str) -> Result<bool> {
    let curr_ver =
        Version::parse(current).context(format!("Invalid current version: {}", current))?;
    let prop_ver =
        Version::parse(proposed).context(format!("Invalid proposed version: {}", proposed))?;

    // Breaking changes require major version bump (or minor if major is 0)
    let valid_bump = if curr_ver.major == 0 {
        // In 0.x, breaking changes can bump minor
        prop_ver.minor > curr_ver.minor || prop_ver.major > curr_ver.major
    } else {
        // In 1.x+, breaking changes require major bump
        prop_ver.major > curr_ver.major
    };

    Ok(valid_bump)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_no_changes() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });

        let result = lint_schema_change(&schema, &schema, Some("1.0.0"), Some("1.0.1")).unwrap();

        assert!(result.is_ok());
        assert!(result.breaking_changes.is_empty());
    }

    #[test]
    fn test_removed_property() {
        let current = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "number"}
            }
        });

        let proposed = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });

        let result = lint_schema_change(&current, &proposed, Some("1.0.0"), Some("1.0.1")).unwrap();

        assert!(result.has_breaking_changes());
        assert!(!result.version_check_passed);
        assert_eq!(result.breaking_changes.len(), 1);
        assert!(result.breaking_changes[0].contains("Removed"));
    }

    #[test]
    fn test_type_change() {
        let current = json!({
            "type": "object",
            "properties": {
                "age": {"type": "number"}
            }
        });

        let proposed = json!({
            "type": "object",
            "properties": {
                "age": {"type": "string"}
            }
        });

        let result = lint_schema_change(&current, &proposed, Some("1.0.0"), Some("2.0.0")).unwrap();

        assert!(result.has_breaking_changes());
        assert!(result.version_check_passed); // Major bump is valid
    }

    #[test]
    fn test_valid_major_bump() {
        let current = json!({"type": "object", "properties": {"name": {"type": "string"}}});
        let proposed = json!({"type": "object", "properties": {}});

        let result = lint_schema_change(&current, &proposed, Some("1.0.0"), Some("2.0.0")).unwrap();

        assert!(result.has_breaking_changes());
        assert!(result.version_check_passed);
    }

    #[test]
    fn test_invalid_minor_bump_for_breaking_change() {
        let current = json!({"type": "object", "properties": {"name": {"type": "string"}}});
        let proposed = json!({"type": "object", "properties": {}});

        let result = lint_schema_change(&current, &proposed, Some("1.0.0"), Some("1.1.0")).unwrap();

        assert!(result.has_breaking_changes());
        assert!(!result.version_check_passed);
    }

    #[test]
    fn test_zero_version_allows_minor_bump() {
        let current = json!({"type": "object", "properties": {"name": {"type": "string"}}});
        let proposed = json!({"type": "object", "properties": {}});

        let result = lint_schema_change(&current, &proposed, Some("0.1.0"), Some("0.2.0")).unwrap();

        assert!(result.has_breaking_changes());
        assert!(result.version_check_passed); // 0.x allows minor bump for breaking changes
    }

    #[test]
    fn test_array_item_type_change_detected() {
        let current = json!({
            "type": "array",
            "items": {"type": "string"}
        });

        let proposed = json!({
            "type": "array",
            "items": {"type": "number"}
        });

        let result = lint_schema_change(&current, &proposed, Some("1.0.0"), Some("2.0.0")).unwrap();

        assert!(result.has_breaking_changes());
        assert_eq!(result.breaking_changes.len(), 1);
        assert!(result.breaking_changes[0].contains("Type changed"));
        assert!(result.breaking_changes[0].contains("root[items]"));
        assert!(result.version_check_passed); // Major bump is valid
    }

    #[test]
    fn test_array_item_property_removal_detected() {
        let current = json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "name": {"type": "string"}
                }
            }
        });

        let proposed = json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "id": {"type": "string"}
                }
            }
        });

        let result = lint_schema_change(&current, &proposed, Some("1.0.0"), Some("2.0.0")).unwrap();

        assert!(result.has_breaking_changes());
        assert_eq!(result.breaking_changes.len(), 1);
        assert!(result.breaking_changes[0].contains("Removed"));
        assert!(result.breaking_changes[0].contains("name"));
        assert!(result.version_check_passed); // Major bump is valid
    }
}
