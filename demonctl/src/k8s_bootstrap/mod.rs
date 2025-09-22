use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

pub mod k3s;
pub mod secrets;
pub mod templates;

pub trait CommandExecutor {
    fn execute(&self, program: &str, args: &[&str], input: Option<&str>) -> Result<CommandOutput>;
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct SystemCommandExecutor;

impl CommandExecutor for SystemCommandExecutor {
    fn execute(&self, program: &str, args: &[&str], input: Option<&str>) -> Result<CommandOutput> {
        let mut cmd = Command::new(program);
        cmd.args(args);

        if let Some(_stdin_data) = input {
            cmd.stdin(std::process::Stdio::piped());
        }

        let mut child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to execute command: {} {:?}", program, args))?;

        if let Some(stdin_data) = input {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin
                    .write_all(stdin_data.as_bytes())
                    .with_context(|| "Failed to write to stdin")?;
            }
        }

        let output = child
            .wait_with_output()
            .with_context(|| "Failed to wait for command completion")?;

        Ok(CommandOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

/// Deterministic command executor used in tests where shelling out is undesirable.
#[derive(Clone, Debug)]
pub struct SimulatedCommandExecutor {
    output: CommandOutput,
}

impl SimulatedCommandExecutor {
    pub fn success(stdout: impl Into<String>) -> Self {
        Self {
            output: CommandOutput {
                status: 0,
                stdout: stdout.into(),
                stderr: String::new(),
            },
        }
    }

    pub fn failure(stderr: impl Into<String>) -> Self {
        Self {
            output: CommandOutput {
                status: 1,
                stdout: String::new(),
                stderr: stderr.into(),
            },
        }
    }
}

impl CommandExecutor for SimulatedCommandExecutor {
    fn execute(
        &self,
        _program: &str,
        _args: &[&str],
        _input: Option<&str>,
    ) -> Result<CommandOutput> {
        Ok(self.output.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sBootstrapConfig {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: ConfigMetadata,
    pub cluster: ClusterConfig,
    pub demon: DemonConfig,
    pub secrets: SecretsConfig,
    pub addons: Vec<AddonConfig>,
    pub networking: NetworkingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub name: String,
    pub runtime: String,
    pub k3s: K3sConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K3sConfig {
    pub version: String,
    pub install: K3sInstallConfig,
    #[serde(rename = "dataDir")]
    pub data_dir: String,
    #[serde(rename = "nodeName")]
    pub node_name: String,
    #[serde(rename = "extraArgs")]
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K3sInstallConfig {
    pub channel: String,
    pub disable: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemonConfig {
    #[serde(rename = "natsUrl")]
    pub nats_url: String,
    pub namespace: String,
    #[serde(rename = "streamName")]
    pub stream_name: String,
    pub subjects: Vec<String>,
    #[serde(rename = "dedupeWindowSecs")]
    pub dedupe_window_secs: u32,
    #[serde(rename = "uiUrl")]
    pub ui_url: String,
    pub persistence: PersistenceConfig,
    pub bundle: Option<BundleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    pub enabled: bool,
    #[serde(rename = "storageClass")]
    pub storage_class: String,
    pub size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleConfig {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsConfig {
    pub provider: String,
    pub vault: Option<VaultConfig>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    #[serde(rename = "address")]
    pub address: Option<String>, // VAULT_ADDR env var fallback
    pub role: Option<String>, // for auth
    pub path: String,         // secret path in vault
    #[serde(rename = "authMethod", default = "default_auth_method")]
    pub auth_method: String,
}

fn default_auth_method() -> String {
    "token".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddonConfig {
    pub name: String,
    pub enabled: bool,
    pub config: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkingConfig {
    pub ingress: IngressConfig,
    #[serde(rename = "serviceMesh")]
    pub service_mesh: ServiceMeshConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressConfig {
    pub enabled: bool,
    pub hostname: Option<String>,
    #[serde(rename = "tlsSecretName")]
    pub tls_secret_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMeshConfig {
    pub enabled: bool,
}

pub fn load_config(config_path: &str) -> Result<K8sBootstrapConfig> {
    let config_content = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read config file: {}", config_path))?;

    let config: K8sBootstrapConfig = serde_yaml::from_str(&config_content)
        .with_context(|| "Failed to parse YAML configuration")?;

    Ok(config)
}

pub fn validate_config(config: &K8sBootstrapConfig) -> Result<()> {
    if config.cluster.runtime != "k3s" {
        anyhow::bail!("Only 'k3s' runtime is currently supported");
    }

    if config.demon.namespace.is_empty() {
        anyhow::bail!("Demon namespace cannot be empty");
    }

    if config.demon.stream_name.is_empty() {
        anyhow::bail!("Demon stream name cannot be empty");
    }

    if config.demon.subjects.is_empty() {
        anyhow::bail!("At least one subject must be specified");
    }

    Ok(())
}
