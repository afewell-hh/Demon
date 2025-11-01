use anyhow::{Context, Result};
use config_loader::{
    ConfigError, ConfigManager, EnvFileSecretProvider, SecretProvider, SecretProviderFactory,
    ValidationError,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio::task;

/// Link-name router stub: resolves a functionRef to a capsule call.
/// Milestone 0 supports only the `echo` capsule with `{ message: String }`.
#[derive(Deserialize, Serialize, Debug)]
pub struct EchoConfig {
    #[serde(rename = "messagePrefix")]
    pub message_prefix: String,
    #[serde(rename = "enableTrim")]
    pub enable_trim: bool,
    #[serde(rename = "maxMessageLength")]
    pub max_message_length: Option<i32>,
    #[serde(rename = "outputFormat")]
    pub output_format: Option<String>,
}

impl Default for EchoConfig {
    fn default() -> Self {
        Self {
            message_prefix: String::new(),
            enable_trim: true,
            max_message_length: Some(1000),
            output_format: Some("plain".to_string()),
        }
    }
}

pub struct Router {
    config_manager: ConfigManager,
    secret_provider: Box<dyn SecretProvider>,
}

impl Router {
    pub fn new() -> Self {
        // Use factory to create provider based on environment configuration
        let secret_provider = SecretProviderFactory::create()
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to create secret provider from factory: {}. Falling back to EnvFileSecretProvider", e);
                Box::new(EnvFileSecretProvider::new())
            });

        Self {
            config_manager: ConfigManager::new(),
            secret_provider,
        }
    }

    pub fn with_config_manager(config_manager: ConfigManager) -> Self {
        // Use factory to create provider based on environment configuration
        let secret_provider = SecretProviderFactory::create()
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to create secret provider from factory: {}. Falling back to EnvFileSecretProvider", e);
                Box::new(EnvFileSecretProvider::new())
            });

        Self {
            config_manager,
            secret_provider,
        }
    }

    pub fn with_config_and_secrets<P: SecretProvider + 'static>(
        config_manager: ConfigManager,
        secret_provider: P,
    ) -> Self {
        Self {
            config_manager,
            secret_provider: Box::new(secret_provider),
        }
    }

    /// Dispatch a functionRef by name with JSON arguments and return JSON output.
    /// This function validates configuration before invoking the capsule.
    pub async fn dispatch(
        &self,
        ref_name: &str,
        args: &Value,
        run_id: &str,
        ritual_id: &str,
    ) -> Result<Value> {
        match ref_name {
            "echo" => {
                // Validate configuration first
                match self
                    .validate_and_emit_config_decision(ref_name, run_id, ritual_id)
                    .await
                {
                    Ok(_config) => {
                        // Config is valid, proceed with capsule call
                        let msg = args.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        let envelope = capsules_echo::echo(msg.to_string());
                        Ok(serde_json::to_value(envelope)?)
                    }
                    Err(e) => {
                        // Config validation failed, return error - the policy decision was already emitted
                        anyhow::bail!("Configuration validation failed: {}", e);
                    }
                }
            }
            "container-exec" => self.dispatch_container_exec(args).await,
            "graph" => self.dispatch_graph(args).await,
            other => anyhow::bail!("unknown functionRef: {other}"),
        }
    }

    /// Dispatch graph capsule operations (create, commit, tag, list-tags, get-node, neighbors, path-exists)
    async fn dispatch_graph(&self, args: &Value) -> Result<Value> {
        // Extract operation from args
        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'operation' field in graph args"))?;

        // Extract scope
        let scope = args
            .get("scope")
            .ok_or_else(|| anyhow::anyhow!("Missing 'scope' field in graph args"))?;
        let scope: capsules_graph::GraphScope =
            serde_json::from_value(scope.clone()).context("Failed to parse GraphScope")?;

        match operation {
            "create" => {
                let seed = args
                    .get("seed")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'seed' field for create operation"))?;
                let mutations: Vec<capsules_graph::Mutation> = serde_json::from_value(seed.clone())
                    .context("Failed to parse seed mutations")?;

                let envelope = capsules_graph::create(scope, mutations).await;
                Ok(serde_json::to_value(envelope)?)
            }
            "commit" => {
                let parent_ref = args
                    .get("parentRef")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let mutations_value = args.get("mutations").ok_or_else(|| {
                    anyhow::anyhow!("Missing 'mutations' field for commit operation")
                })?;
                let mutations: Vec<capsules_graph::Mutation> =
                    serde_json::from_value(mutations_value.clone())
                        .context("Failed to parse mutations")?;

                let envelope = capsules_graph::commit(scope, parent_ref, mutations).await;
                Ok(serde_json::to_value(envelope)?)
            }
            "tag" => {
                let tag = args
                    .get("tag")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'tag' field for tag operation"))?
                    .to_string();
                let commit_id = args
                    .get("commitId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'commitId' field for tag operation"))?
                    .to_string();

                let envelope = capsules_graph::tag(scope, tag, commit_id).await;
                Ok(serde_json::to_value(envelope)?)
            }
            "delete-tag" => {
                let tag = args
                    .get("tag")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'tag' field for delete-tag operation"))?
                    .to_string();

                let envelope = capsules_graph::delete_tag(scope, tag).await;
                Ok(serde_json::to_value(envelope)?)
            }
            "list-tags" => {
                let envelope = capsules_graph::list_tags(scope).await;
                Ok(serde_json::to_value(envelope)?)
            }
            "get-node" => {
                let commit_id = args
                    .get("commitId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'commitId' for get-node operation"))?
                    .to_string();
                let node_id = args
                    .get("nodeId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'nodeId' for get-node operation"))?
                    .to_string();

                let envelope = capsules_graph::get_node(scope, commit_id, node_id).await;
                Ok(serde_json::to_value(envelope)?)
            }
            "neighbors" => {
                let commit_id = args
                    .get("commitId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'commitId' for neighbors operation"))?
                    .to_string();
                let node_id = args
                    .get("nodeId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'nodeId' for neighbors operation"))?
                    .to_string();
                let depth = args
                    .get("depth")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'depth' for neighbors operation"))?
                    as u32;

                let envelope = capsules_graph::neighbors(scope, commit_id, node_id, depth).await;
                Ok(serde_json::to_value(envelope)?)
            }
            "path-exists" => {
                let commit_id = args
                    .get("commitId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'commitId' for path-exists operation"))?
                    .to_string();
                let from = args
                    .get("from")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'from' for path-exists operation"))?
                    .to_string();
                let to = args
                    .get("to")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'to' for path-exists operation"))?
                    .to_string();
                let max_depth = args
                    .get("maxDepth")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Missing 'maxDepth' for path-exists operation")
                    })? as u32;

                let envelope =
                    capsules_graph::path_exists(scope, commit_id, from, to, max_depth).await;
                Ok(serde_json::to_value(envelope)?)
            }
            other => anyhow::bail!("Unknown graph operation: {}", other),
        }
    }

    async fn validate_and_emit_config_decision(
        &self,
        capsule_name: &str,
        run_id: &str,
        ritual_id: &str,
    ) -> Result<EchoConfig, ConfigError> {
        match self
            .config_manager
            .load_with_secrets(capsule_name, self.secret_provider.as_ref())
        {
            Ok(config) => {
                // Config is valid, emit policy.decision.allowed
                if let Err(e) = self
                    .emit_policy_decision(
                        true,
                        None,
                        "config_validation_passed",
                        run_id,
                        ritual_id,
                        capsule_name,
                    )
                    .await
                {
                    tracing::warn!("Failed to emit policy decision (allowed): {}", e);
                }
                Ok(config)
            }
            Err(config_error) => {
                // Config validation or secret resolution failed, emit policy.decision.denied
                let (error_details, reason) = match &config_error {
                    ConfigError::ValidationFailed { errors } => (
                        Some(self.format_validation_errors(errors)),
                        "config_validation_failed",
                    ),
                    ConfigError::SecretResolutionFailed { error } => {
                        (Some(error.to_string()), "secret_not_found")
                    }
                    _ => (Some(config_error.to_string()), "config_validation_failed"),
                };

                if let Err(e) = self
                    .emit_policy_decision(
                        false,
                        error_details,
                        reason,
                        run_id,
                        ritual_id,
                        capsule_name,
                    )
                    .await
                {
                    tracing::warn!("Failed to emit policy decision (denied): {}", e);
                }
                Err(config_error)
            }
        }
    }

    async fn emit_policy_decision(
        &self,
        allowed: bool,
        error_details: Option<String>,
        reason: &str,
        run_id: &str,
        ritual_id: &str,
        capability: &str,
    ) -> Result<()> {
        let decision_json = if allowed {
            json!({ "allowed": true, "reason": reason })
        } else {
            json!({
                "allowed": false,
                "reason": reason,
                "details": error_details.unwrap_or_else(|| "Configuration validation failed".to_string())
            })
        };

        let payload = json!({
            "event": "policy.decision:v1",
            "ts": chrono::Utc::now().to_rfc3339(),
            "tenantId": "default", // TODO: Get actual tenant ID from context
            "runId": run_id,
            "ritualId": ritual_id,
            "capability": capability,
            "decision": decision_json,
            "validation": {
                "type": "config",
                "schema": format!("{}-config.v1.json", capability)
            }
        });

        let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
        let client = async_nats::connect(&url)
            .await
            .context("Failed to connect to NATS")?;
        let js = async_nats::jetstream::new(client.clone());

        let stream_name = std::env::var("RITUAL_STREAM_NAME").ok();
        if let Some(name) = stream_name {
            let _ = js
                .get_or_create_stream(async_nats::jetstream::stream::Config {
                    name,
                    subjects: vec!["demon.ritual.v1.>".to_string()],
                    ..Default::default()
                })
                .await?;
        } else {
            const DEFAULT: &str = "RITUAL_EVENTS";
            const DEPRECATED: &str = "DEMON_RITUAL_EVENTS";
            if js.get_stream(DEFAULT).await.is_err() {
                if js.get_stream(DEPRECATED).await.is_ok() {
                    tracing::info!(
                        "Using deprecated stream name '{}'; set RITUAL_STREAM_NAME or migrate to '{}'",
                        DEPRECATED,
                        DEFAULT
                    );
                } else {
                    let _ = js
                        .get_or_create_stream(async_nats::jetstream::stream::Config {
                            name: DEFAULT.to_string(),
                            subjects: vec!["demon.ritual.v1.>".to_string()],
                            ..Default::default()
                        })
                        .await?;
                }
            }
        }

        let subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);
        let mut headers = async_nats::HeaderMap::new();
        let uniq = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let msg_id = format!("{}:config-decision:{}:{}", run_id, capability, uniq);
        headers.insert("Nats-Msg-Id", msg_id.as_str());
        js.publish_with_headers(subject, headers, serde_json::to_vec(&payload)?.into())
            .await?
            .await?;

        Ok(())
    }

    fn format_validation_errors(&self, errors: &[ValidationError]) -> String {
        let formatted_errors: Vec<String> = errors
            .iter()
            .map(|e| {
                format!(
                    "Path {}: {} (schema: {})",
                    e.json_pointer, e.message, e.schema_path
                )
            })
            .collect();
        formatted_errors.join("; ")
    }

    async fn dispatch_container_exec(&self, args: &Value) -> Result<Value> {
        let request: ContainerExecRequest = serde_json::from_value(args.clone())
            .context("Failed to parse container-exec request")?;

        let config: capsules_container_exec::ContainerExecConfig = request.into();

        let envelope = task::spawn_blocking(move || capsules_container_exec::execute(&config))
            .await
            .context("container-exec task join error")?;

        Ok(serde_json::to_value(envelope)?)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContainerExecRequest {
    #[serde(rename = "imageDigest")]
    image_digest: String,
    command: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default, rename = "workingDir")]
    working_dir: Option<String>,
    outputs: ContainerExecOutputs,
    #[serde(default, rename = "capsuleName")]
    capsule_name: Option<String>,
    #[serde(default, rename = "workspaceDir")]
    workspace_dir: Option<String>,
    #[serde(default, rename = "artifactsDir")]
    artifacts_dir: Option<String>,
    #[serde(default, rename = "timeoutSeconds")]
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContainerExecOutputs {
    #[serde(rename = "envelopePath")]
    envelope_path: String,
}

impl From<ContainerExecRequest> for capsules_container_exec::ContainerExecConfig {
    fn from(request: ContainerExecRequest) -> Self {
        Self {
            image_digest: request.image_digest,
            command: request.command,
            env: request.env,
            working_dir: request.working_dir,
            envelope_path: request.outputs.envelope_path,
            timeout_seconds: request.timeout_seconds,
            capsule_name: request.capsule_name,
            app_pack_dir: request.workspace_dir.map(PathBuf::from),
            artifacts_dir: request.artifacts_dir.map(PathBuf::from),
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
