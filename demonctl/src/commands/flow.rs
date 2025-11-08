//! Flow export/import commands for agent-authored workflows
//!
//! This module provides CLI commands to export ritual definitions as flow manifests
//! and import/submit flow manifests to the Agent Flow API.

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Args, Subcommand};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

#[derive(Args, Debug)]
pub struct FlowArgs {
    #[command(subcommand)]
    pub cmd: FlowCommand,
}

#[derive(Subcommand, Debug)]
pub enum FlowCommand {
    /// Export a ritual as a flow manifest
    Export {
        /// Ritual ID or path to ritual YAML file
        #[arg(long, short = 'r', value_name = "RITUAL")]
        ritual: String,

        /// Output file path (JSON or YAML based on extension)
        #[arg(long, short = 'o', value_name = "FILE")]
        output: PathBuf,

        /// Optional API URL for fetching ritual metadata
        #[arg(long, env = "DEMONCTL_API_URL")]
        api_url: Option<String>,
    },

    /// Import and optionally submit a flow manifest
    Import {
        /// Path to flow manifest file (JSON or YAML)
        #[arg(long, short = 'f', value_name = "FILE")]
        file: PathBuf,

        /// Validate only, do not submit to API
        #[arg(long)]
        dry_run: bool,

        /// API URL for submission
        #[arg(
            long,
            env = "DEMONCTL_API_URL",
            default_value = "http://localhost:3000"
        )]
        api_url: String,

        /// JWT token for authentication
        #[arg(long, env = "DEMONCTL_JWT")]
        jwt: Option<String>,
    },
}

// Flow manifest structures matching contracts/schemas/flow_manifest.v1.json

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowManifest {
    pub schema_version: String,
    pub metadata: FlowMetadata,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bindings: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<FlowProvenance>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowMetadata {
    pub flow_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowNode {
    pub node_id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub config: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowProvenance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_flow_id: Option<String>,
}

// Ritual YAML structures (simplified)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RitualDefinition {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    version: String,
    states: Vec<RitualState>,
}

#[derive(Debug, Deserialize)]
struct RitualState {
    name: String,
    #[serde(rename = "type")]
    state_type: String,
    #[serde(default)]
    action: Option<Value>,
    #[serde(default)]
    end: bool,
}

// API response structures
#[derive(Debug, Deserialize)]
struct SubmitFlowResponse {
    flow_id: String,
    manifest_digest: String,
    validation_result: ValidationResult,
    submitted_at: String,
}

#[derive(Debug, Deserialize)]
struct ValidationResult {
    valid: bool,
    errors: Vec<ValidationError>,
    warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Deserialize)]
struct ValidationError {
    code: String,
    message: String,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ValidationWarning {
    code: String,
    message: String,
}

pub async fn run(args: FlowArgs) -> Result<()> {
    match args.cmd {
        FlowCommand::Export {
            ritual,
            output,
            api_url,
        } => export_flow(ritual, output, api_url).await,
        FlowCommand::Import {
            file,
            dry_run,
            api_url,
            jwt,
        } => import_flow(file, dry_run, api_url, jwt).await,
    }
}

async fn export_flow(ritual: String, output: PathBuf, _api_url: Option<String>) -> Result<()> {
    info!("Exporting ritual '{}' to flow manifest", ritual);

    // Determine if ritual is a path or ID
    let ritual_path = if ritual.ends_with(".yaml") || ritual.ends_with(".yml") {
        PathBuf::from(&ritual)
    } else {
        // Assume it's an ID and look in examples/rituals/
        PathBuf::from(format!("examples/rituals/{}.yaml", ritual))
    };

    // Read ritual YAML
    let ritual_content = fs::read_to_string(&ritual_path)
        .with_context(|| format!("Failed to read ritual file: {}", ritual_path.display()))?;

    let ritual_def: RitualDefinition =
        serde_yaml::from_str(&ritual_content).context("Failed to parse ritual YAML")?;

    debug!("Parsed ritual: {} ({})", ritual_def.name, ritual_def.id);

    // Convert ritual to flow manifest
    let manifest = convert_ritual_to_flow(&ritual_def)?;

    // Write output based on extension
    let output_content = if output.extension().and_then(|s| s.to_str()) == Some("yaml")
        || output.extension().and_then(|s| s.to_str()) == Some("yml")
    {
        serde_yaml::to_string(&manifest).context("Failed to serialize manifest as YAML")?
    } else {
        serde_json::to_string_pretty(&manifest).context("Failed to serialize manifest as JSON")?
    };

    fs::write(&output, output_content)
        .with_context(|| format!("Failed to write output file: {}", output.display()))?;

    println!("✓ Exported flow manifest to: {}", output.display());
    println!("  Flow ID: {}", manifest.metadata.flow_id);
    println!("  Nodes: {}", manifest.nodes.len());
    println!("  Edges: {}", manifest.edges.len());

    Ok(())
}

fn convert_ritual_to_flow(ritual: &RitualDefinition) -> Result<FlowManifest> {
    let flow_id = format!("flow-{}", ritual.id);
    let name = if ritual.name.is_empty() {
        ritual.id.clone()
    } else {
        ritual.name.clone()
    };

    // Convert ritual states to flow nodes
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Add start node
    nodes.push(FlowNode {
        node_id: "start".to_string(),
        node_type: "trigger".to_string(),
        config: serde_json::json!({
            "trigger_type": "manual",
            "label": "Start Workflow"
        }),
        metadata: None,
    });

    let mut prev_node_id = "start".to_string();

    for (idx, state) in ritual.states.iter().enumerate() {
        let node_id = format!("state_{}", idx);

        // Determine node type and config based on ritual state
        let (node_type, config) = match state.state_type.as_str() {
            "task" => {
                let mut cfg = serde_json::json!({
                    "state_name": state.name,
                });
                if let Some(action) = &state.action {
                    cfg["action"] = action.clone();
                }
                ("task".to_string(), cfg)
            }
            _ => (
                state.state_type.clone(),
                serde_json::json!({
                    "state_name": state.name
                }),
            ),
        };

        nodes.push(FlowNode {
            node_id: node_id.clone(),
            node_type,
            config,
            metadata: None,
        });

        // Add edge from previous node
        edges.push(FlowEdge {
            from: prev_node_id.clone(),
            to: node_id.clone(),
            condition: None,
            metadata: None,
        });

        prev_node_id = node_id.clone();

        // If this is the end state, add completion node
        if state.end {
            let complete_id = "complete".to_string();
            nodes.push(FlowNode {
                node_id: complete_id.clone(),
                node_type: "completion".to_string(),
                config: serde_json::json!({
                    "status": "success",
                    "message": "Flow completed successfully"
                }),
                metadata: None,
            });

            edges.push(FlowEdge {
                from: node_id,
                to: complete_id,
                condition: None,
                metadata: None,
            });
        }
    }

    let manifest = FlowManifest {
        schema_version: "v1".to_string(),
        metadata: FlowMetadata {
            flow_id,
            name,
            description: if ritual.description.is_empty() {
                None
            } else {
                Some(ritual.description.clone())
            },
            created_by: "demonctl-cli".to_string(),
            tags: Some(vec!["exported".to_string(), "ritual-derived".to_string()]),
        },
        nodes,
        edges,
        bindings: None,
        provenance: Some(FlowProvenance {
            agent_id: Some("demonctl".to_string()),
            generation_timestamp: Some(Utc::now().to_rfc3339()),
            source: Some("cli-export".to_string()),
            parent_flow_id: None,
        }),
    };

    Ok(manifest)
}

async fn import_flow(
    file: PathBuf,
    dry_run: bool,
    api_url: String,
    jwt: Option<String>,
) -> Result<()> {
    info!("Importing flow manifest from: {}", file.display());

    // Read manifest file
    let manifest_content = fs::read_to_string(&file)
        .with_context(|| format!("Failed to read manifest file: {}", file.display()))?;

    // Parse manifest (support both JSON and YAML)
    let manifest: FlowManifest = if file.extension().and_then(|s| s.to_str()) == Some("yaml")
        || file.extension().and_then(|s| s.to_str()) == Some("yml")
    {
        serde_yaml::from_str(&manifest_content).context("Failed to parse manifest as YAML")?
    } else {
        serde_json::from_str(&manifest_content).context("Failed to parse manifest as JSON")?
    };

    debug!(
        "Parsed manifest: flow_id={}, nodes={}",
        manifest.metadata.flow_id,
        manifest.nodes.len()
    );

    // Validate schema version
    if manifest.schema_version != "v1" {
        anyhow::bail!(
            "Unsupported schema version: {}. Expected 'v1'",
            manifest.schema_version
        );
    }

    // Validate required fields
    validate_manifest(&manifest)?;

    if dry_run {
        println!("✓ Manifest validation passed");
        println!("  Flow ID: {}", manifest.metadata.flow_id);
        println!("  Name: {}", manifest.metadata.name);
        println!("  Nodes: {}", manifest.nodes.len());
        println!("  Edges: {}", manifest.edges.len());
        println!("\n  Dry-run mode: not submitting to API");
        return Ok(());
    }

    // Submit to API
    let jwt_token = jwt.or_else(|| env::var("DEMONCTL_JWT").ok()).ok_or_else(|| {
        anyhow::anyhow!(
            "JWT token required for API submission. Set --jwt flag or DEMONCTL_JWT environment variable"
        )
    })?;

    let submit_url = format!("{}/api/flows/submit", api_url);
    debug!("Submitting to: {}", submit_url);

    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", jwt_token))
            .context("Invalid JWT token format")?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("X-Demon-API-Version", HeaderValue::from_static("v1"));

    let request_body = serde_json::json!({
        "manifest": manifest
    });

    let response = client
        .post(&submit_url)
        .headers(headers)
        .json(&request_body)
        .send()
        .await
        .context("Failed to send request to API")?;

    let status = response.status();

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        anyhow::bail!("API request failed with status {}: {}", status, error_text);
    }

    let submit_response: SubmitFlowResponse = response
        .json()
        .await
        .context("Failed to parse API response")?;

    if !submit_response.validation_result.valid {
        println!("✗ Flow validation failed:");
        for err in &submit_response.validation_result.errors {
            println!("  - [{}] {}", err.code, err.message);
            if let Some(path) = &err.path {
                println!("    Path: {}", path);
            }
        }
        anyhow::bail!("Flow manifest validation failed");
    }

    println!("✓ Flow submitted successfully");
    println!("  Flow ID: {}", submit_response.flow_id);
    println!("  Digest: {}", submit_response.manifest_digest);
    println!("  Submitted at: {}", submit_response.submitted_at);

    if !submit_response.validation_result.warnings.is_empty() {
        println!("\n  Warnings:");
        for warn in &submit_response.validation_result.warnings {
            println!("  - [{}] {}", warn.code, warn.message);
        }
    }

    Ok(())
}

fn validate_manifest(manifest: &FlowManifest) -> Result<()> {
    // Check required fields
    if manifest.metadata.flow_id.is_empty() {
        anyhow::bail!("manifest.metadata.flow_id is required");
    }

    if manifest.metadata.name.is_empty() {
        anyhow::bail!("manifest.metadata.name is required");
    }

    if manifest.metadata.created_by.is_empty() {
        anyhow::bail!("manifest.metadata.created_by is required");
    }

    if manifest.nodes.is_empty() {
        anyhow::bail!("manifest.nodes must contain at least one node");
    }

    // Validate node IDs are unique
    let mut node_ids = std::collections::HashSet::new();
    for node in &manifest.nodes {
        if !node_ids.insert(&node.node_id) {
            anyhow::bail!("Duplicate node_id found: {}", node.node_id);
        }
    }

    // Validate edges reference existing nodes
    for edge in &manifest.edges {
        if !node_ids.contains(&edge.from) {
            anyhow::bail!("Edge references non-existent node: {}", edge.from);
        }
        if !node_ids.contains(&edge.to) {
            anyhow::bail!("Edge references non-existent node: {}", edge.to);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_manifest_success() {
        let manifest = FlowManifest {
            schema_version: "v1".to_string(),
            metadata: FlowMetadata {
                flow_id: "test-001".to_string(),
                name: "Test Flow".to_string(),
                description: None,
                created_by: "test".to_string(),
                tags: None,
            },
            nodes: vec![
                FlowNode {
                    node_id: "start".to_string(),
                    node_type: "trigger".to_string(),
                    config: serde_json::json!({}),
                    metadata: None,
                },
                FlowNode {
                    node_id: "end".to_string(),
                    node_type: "completion".to_string(),
                    config: serde_json::json!({}),
                    metadata: None,
                },
            ],
            edges: vec![FlowEdge {
                from: "start".to_string(),
                to: "end".to_string(),
                condition: None,
                metadata: None,
            }],
            bindings: None,
            provenance: None,
        };

        assert!(validate_manifest(&manifest).is_ok());
    }

    #[test]
    fn test_validate_manifest_missing_flow_id() {
        let manifest = FlowManifest {
            schema_version: "v1".to_string(),
            metadata: FlowMetadata {
                flow_id: "".to_string(),
                name: "Test".to_string(),
                description: None,
                created_by: "test".to_string(),
                tags: None,
            },
            nodes: vec![],
            edges: vec![],
            bindings: None,
            provenance: None,
        };

        assert!(validate_manifest(&manifest).is_err());
    }

    #[test]
    fn test_validate_manifest_duplicate_node_ids() {
        let manifest = FlowManifest {
            schema_version: "v1".to_string(),
            metadata: FlowMetadata {
                flow_id: "test-001".to_string(),
                name: "Test".to_string(),
                description: None,
                created_by: "test".to_string(),
                tags: None,
            },
            nodes: vec![
                FlowNode {
                    node_id: "same".to_string(),
                    node_type: "trigger".to_string(),
                    config: serde_json::json!({}),
                    metadata: None,
                },
                FlowNode {
                    node_id: "same".to_string(),
                    node_type: "completion".to_string(),
                    config: serde_json::json!({}),
                    metadata: None,
                },
            ],
            edges: vec![],
            bindings: None,
            provenance: None,
        };

        assert!(validate_manifest(&manifest).is_err());
    }

    #[test]
    fn test_validate_manifest_invalid_edge_reference() {
        let manifest = FlowManifest {
            schema_version: "v1".to_string(),
            metadata: FlowMetadata {
                flow_id: "test-001".to_string(),
                name: "Test".to_string(),
                description: None,
                created_by: "test".to_string(),
                tags: None,
            },
            nodes: vec![FlowNode {
                node_id: "start".to_string(),
                node_type: "trigger".to_string(),
                config: serde_json::json!({}),
                metadata: None,
            }],
            edges: vec![FlowEdge {
                from: "start".to_string(),
                to: "nonexistent".to_string(),
                condition: None,
                metadata: None,
            }],
            bindings: None,
            provenance: None,
        };

        assert!(validate_manifest(&manifest).is_err());
    }
}
