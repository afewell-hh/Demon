//! Contract linter CLI tool
//!
//! Detects breaking changes between contract schema versions and validates
//! semantic versioning compliance.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use contract_linter::lint_schema_change;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "contract-linter")]
#[command(about = "Lint contract schemas for breaking changes", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compare two schema versions and detect breaking changes
    Compare {
        /// Path to current (existing) schema JSON file
        #[arg(long)]
        current: PathBuf,

        /// Path to proposed (new) schema JSON file
        #[arg(long)]
        proposed: PathBuf,

        /// Current version string (semver)
        #[arg(long)]
        current_version: Option<String>,

        /// Proposed version string (semver)
        #[arg(long)]
        proposed_version: Option<String>,

        /// Fail with non-zero exit code if breaking changes detected
        #[arg(long, default_value_t = true)]
        strict: bool,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compare {
            current,
            proposed,
            current_version,
            proposed_version,
            strict,
        } => {
            let result = compare_schemas(
                &current,
                &proposed,
                current_version.as_deref(),
                proposed_version.as_deref(),
            )?;

            // Print results
            println!("Contract Schema Linter Results");
            println!("===============================");
            println!();

            if let Some(curr_ver) = &result.current_version {
                println!("Current version:  {}", curr_ver);
            }
            if let Some(prop_ver) = &result.proposed_version {
                println!("Proposed version: {}", prop_ver);
            }
            println!();

            if result.breaking_changes.is_empty() {
                println!("✓ No breaking changes detected");
                println!();
                process::exit(0);
            } else {
                println!("⚠ Breaking changes detected:");
                println!();
                for (i, change) in result.breaking_changes.iter().enumerate() {
                    println!("  {}. {}", i + 1, change);
                }
                println!();

                if result.version_check_passed {
                    println!("✓ Version bump is appropriate for breaking changes");
                    println!();
                    process::exit(0);
                } else {
                    println!("✗ Version bump required");
                    println!();
                    println!(
                        "Breaking changes detected, but version was not bumped appropriately."
                    );
                    println!();
                    println!("Guidelines:");
                    println!("  - For versions 0.x.y: bump minor version (0.1.0 -> 0.2.0)");
                    println!("  - For versions 1.x.y+: bump major version (1.0.0 -> 2.0.0)");
                    println!();

                    if strict {
                        process::exit(1);
                    } else {
                        process::exit(0);
                    }
                }
            }
        }
    }
}

fn compare_schemas(
    current_path: &PathBuf,
    proposed_path: &PathBuf,
    current_version: Option<&str>,
    proposed_version: Option<&str>,
) -> Result<contract_linter::LintResult> {
    // Read schema files
    let current_contents = fs::read_to_string(current_path)
        .with_context(|| format!("Failed to read current schema: {:?}", current_path))?;

    let proposed_contents = fs::read_to_string(proposed_path)
        .with_context(|| format!("Failed to read proposed schema: {:?}", proposed_path))?;

    // Parse JSON
    let current_schema: Value = serde_json::from_str(&current_contents)
        .with_context(|| format!("Failed to parse current schema JSON: {:?}", current_path))?;

    let proposed_schema: Value = serde_json::from_str(&proposed_contents)
        .with_context(|| format!("Failed to parse proposed schema JSON: {:?}", proposed_path))?;

    // Run linter
    lint_schema_change(
        &current_schema,
        &proposed_schema,
        current_version,
        proposed_version,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_compare_identical_schemas() {
        let schema = r#"{"type": "object", "properties": {"name": {"type": "string"}}}"#;

        let mut current_file = NamedTempFile::new().unwrap();
        let mut proposed_file = NamedTempFile::new().unwrap();

        current_file.write_all(schema.as_bytes()).unwrap();
        proposed_file.write_all(schema.as_bytes()).unwrap();

        let result = compare_schemas(
            &current_file.path().to_path_buf(),
            &proposed_file.path().to_path_buf(),
            Some("1.0.0"),
            Some("1.0.1"),
        )
        .unwrap();

        assert!(result.is_ok());
        assert!(result.breaking_changes.is_empty());
    }

    #[test]
    fn test_compare_breaking_change() {
        let current = r#"{"type": "object", "properties": {"name": {"type": "string"}, "age": {"type": "number"}}}"#;
        let proposed = r#"{"type": "object", "properties": {"name": {"type": "string"}}}"#;

        let mut current_file = NamedTempFile::new().unwrap();
        let mut proposed_file = NamedTempFile::new().unwrap();

        current_file.write_all(current.as_bytes()).unwrap();
        proposed_file.write_all(proposed.as_bytes()).unwrap();

        let result = compare_schemas(
            &current_file.path().to_path_buf(),
            &proposed_file.path().to_path_buf(),
            Some("1.0.0"),
            Some("1.1.0"),
        )
        .unwrap();

        assert!(result.has_breaking_changes());
        assert!(!result.version_check_passed);
    }
}
