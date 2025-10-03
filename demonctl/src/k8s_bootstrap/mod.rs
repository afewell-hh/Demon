use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

use crate::docker;

pub mod addons;
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
    #[serde(default)]
    pub registries: Option<Vec<RegistryConfig>>,
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
    #[serde(default = "default_image_config", rename = "imageTags")]
    pub images: ImageConfig,
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

fn default_image_tag() -> String {
    "main".to_string()
}

fn default_image_config() -> ImageConfig {
    ImageConfig {
        operate_ui: default_image_tag(),
        runtime: default_image_tag(),
        engine: default_image_tag(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    #[serde(rename = "operateUi", default = "default_image_tag")]
    pub operate_ui: String,
    #[serde(default = "default_image_tag")]
    pub runtime: String,
    #[serde(default = "default_image_tag")]
    pub engine: String,
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
    #[serde(rename = "ingressClass")]
    pub ingress_class: Option<String>,
    pub annotations: Option<HashMap<String, String>>,
    pub tls: TlsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TlsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(rename = "secretName")]
    pub secret_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMeshConfig {
    pub enabled: bool,
    #[serde(default = "default_mesh_annotations")]
    pub annotations: HashMap<String, String>,
}

fn default_mesh_annotations() -> HashMap<String, String> {
    let mut annotations = HashMap::new();
    annotations.insert("sidecar.istio.io/inject".to_string(), "true".to_string());
    annotations
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    pub name: String,
    pub registry: String,
    #[serde(rename = "usernameEnv")]
    pub username_env: String,
    #[serde(rename = "passwordEnv")]
    pub password_env: String,
    #[serde(rename = "appliesTo")]
    pub applies_to: Vec<String>,
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

    validate_networking_config(&config.networking)?;

    Ok(())
}

pub fn validate_networking_config(networking: &NetworkingConfig) -> Result<()> {
    let ingress = &networking.ingress;

    if ingress.enabled {
        if let Some(hostname) = &ingress.hostname {
            if hostname.is_empty() {
                anyhow::bail!("Ingress hostname cannot be empty when specified");
            }

            // Basic hostname validation
            if !hostname.contains('.') && hostname != "localhost" {
                anyhow::bail!(
                    "Ingress hostname '{}' appears to be invalid (should contain domain)",
                    hostname
                );
            }
        }

        if ingress.tls.enabled {
            if ingress.hostname.is_none() {
                anyhow::bail!("TLS requires a hostname to be specified");
            }

            if ingress.tls.secret_name.is_none() {
                eprintln!(
                    "Warning: TLS enabled but no secret name specified - ingress will be HTTP-only"
                );
            }
        }
    }

    Ok(())
}

pub fn merge_digests_into_config(
    config: &mut K8sBootstrapConfig,
    manifest: &docker::Manifest,
) -> Result<()> {
    for (component, env_var) in docker::REQUIRED_COMPONENTS {
        let entry = manifest
            .get(component)
            .with_context(|| format!("Manifest is missing required component '{}'.", component))?;

        let image_reference = entry.image.clone();

        match component {
            "operate-ui" => config.demon.images.operate_ui = image_reference.clone(),
            "runtime" => config.demon.images.runtime = image_reference.clone(),
            "engine" => config.demon.images.engine = image_reference.clone(),
            other => anyhow::bail!("Unsupported component '{}' in manifest", other),
        }

        std::env::set_var(env_var, &image_reference);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_digests_into_config_overrides_images_and_env() {
        let mut config = K8sBootstrapConfig {
            api_version: "v1".to_string(),
            kind: "BootstrapConfig".to_string(),
            metadata: ConfigMetadata {
                name: "test".to_string(),
            },
            cluster: ClusterConfig {
                name: "test".to_string(),
                runtime: "k3s".to_string(),
                k3s: K3sConfig {
                    version: "v1".to_string(),
                    install: K3sInstallConfig {
                        channel: "stable".to_string(),
                        disable: vec![],
                    },
                    data_dir: "/var/lib/rancher/k3s".to_string(),
                    node_name: "node".to_string(),
                    extra_args: vec![],
                },
            },
            demon: DemonConfig {
                nats_url: "nats://localhost:4222".to_string(),
                namespace: "test-system".to_string(),
                stream_name: "events".to_string(),
                subjects: vec!["test.>".to_string()],
                dedupe_window_secs: 30,
                ui_url: "http://localhost:3000".to_string(),
                persistence: PersistenceConfig {
                    enabled: true,
                    storage_class: "standard".to_string(),
                    size: "10Gi".to_string(),
                },
                bundle: None,
                images: ImageConfig {
                    operate_ui: "main".to_string(),
                    runtime: "main".to_string(),
                    engine: "main".to_string(),
                },
            },
            secrets: SecretsConfig {
                provider: "env".to_string(),
                vault: None,
                env: None,
            },
            addons: vec![],
            networking: NetworkingConfig {
                ingress: IngressConfig {
                    enabled: false,
                    hostname: None,
                    ingress_class: None,
                    annotations: None,
                    tls: TlsConfig::default(),
                },
                service_mesh: ServiceMeshConfig {
                    enabled: false,
                    annotations: default_mesh_annotations(),
                },
            },
            registries: None,
        };

        std::env::remove_var("OPERATE_UI_IMAGE_TAG");
        std::env::remove_var("RUNTIME_IMAGE_TAG");
        std::env::remove_var("ENGINE_IMAGE_TAG");

        let manifest = [
            (
                "operate-ui".to_string(),
                docker::ManifestEntry {
                    repository: "ghcr.io/acme/demon-operate-ui".to_string(),
                    digest: "sha256:aaa".to_string(),
                    image: "ghcr.io/acme/demon-operate-ui@sha256:aaa".to_string(),
                    git_sha_tag: Some("ghcr.io/acme/demon-operate-ui:sha-deadbeef".to_string()),
                },
            ),
            (
                "runtime".to_string(),
                docker::ManifestEntry {
                    repository: "ghcr.io/acme/demon-runtime".to_string(),
                    digest: "sha256:bbb".to_string(),
                    image: "ghcr.io/acme/demon-runtime@sha256:bbb".to_string(),
                    git_sha_tag: Some("ghcr.io/acme/demon-runtime:sha-deadbeef".to_string()),
                },
            ),
            (
                "engine".to_string(),
                docker::ManifestEntry {
                    repository: "ghcr.io/acme/demon-engine".to_string(),
                    digest: "sha256:ccc".to_string(),
                    image: "ghcr.io/acme/demon-engine@sha256:ccc".to_string(),
                    git_sha_tag: Some("ghcr.io/acme/demon-engine:sha-deadbeef".to_string()),
                },
            ),
        ]
        .into_iter()
        .collect();

        merge_digests_into_config(&mut config, &manifest).unwrap();

        assert_eq!(
            config.demon.images.operate_ui,
            "ghcr.io/acme/demon-operate-ui@sha256:aaa"
        );
        assert_eq!(
            config.demon.images.runtime,
            "ghcr.io/acme/demon-runtime@sha256:bbb"
        );
        assert_eq!(
            config.demon.images.engine,
            "ghcr.io/acme/demon-engine@sha256:ccc"
        );

        assert_eq!(
            std::env::var("OPERATE_UI_IMAGE_TAG").unwrap(),
            "ghcr.io/acme/demon-operate-ui@sha256:aaa"
        );
        assert_eq!(
            std::env::var("RUNTIME_IMAGE_TAG").unwrap(),
            "ghcr.io/acme/demon-runtime@sha256:bbb"
        );
        assert_eq!(
            std::env::var("ENGINE_IMAGE_TAG").unwrap(),
            "ghcr.io/acme/demon-engine@sha256:ccc"
        );

        std::env::remove_var("OPERATE_UI_IMAGE_TAG");
        std::env::remove_var("RUNTIME_IMAGE_TAG");
        std::env::remove_var("ENGINE_IMAGE_TAG");
    }

    #[test]
    fn merge_digests_into_config_errors_when_component_missing() {
        let mut config = K8sBootstrapConfig {
            api_version: "v1".to_string(),
            kind: "BootstrapConfig".to_string(),
            metadata: ConfigMetadata {
                name: "test".to_string(),
            },
            cluster: ClusterConfig {
                name: "test".to_string(),
                runtime: "k3s".to_string(),
                k3s: K3sConfig {
                    version: "v1".to_string(),
                    install: K3sInstallConfig {
                        channel: "stable".to_string(),
                        disable: vec![],
                    },
                    data_dir: "/var/lib/rancher/k3s".to_string(),
                    node_name: "node".to_string(),
                    extra_args: vec![],
                },
            },
            demon: DemonConfig {
                nats_url: "nats://localhost:4222".to_string(),
                namespace: "test-system".to_string(),
                stream_name: "events".to_string(),
                subjects: vec!["test.>".to_string()],
                dedupe_window_secs: 30,
                ui_url: "http://localhost:3000".to_string(),
                persistence: PersistenceConfig {
                    enabled: true,
                    storage_class: "standard".to_string(),
                    size: "10Gi".to_string(),
                },
                bundle: None,
                images: ImageConfig {
                    operate_ui: "main".to_string(),
                    runtime: "main".to_string(),
                    engine: "main".to_string(),
                },
            },
            secrets: SecretsConfig {
                provider: "env".to_string(),
                vault: None,
                env: None,
            },
            addons: vec![],
            networking: NetworkingConfig {
                ingress: IngressConfig {
                    enabled: false,
                    hostname: None,
                    ingress_class: None,
                    annotations: None,
                    tls: TlsConfig::default(),
                },
                service_mesh: ServiceMeshConfig {
                    enabled: false,
                    annotations: default_mesh_annotations(),
                },
            },
            registries: None,
        };

        let manifest: docker::Manifest = [(
            "operate-ui".to_string(),
            docker::ManifestEntry {
                repository: "ghcr.io/acme/demon-operate-ui".to_string(),
                digest: "sha256:aaa".to_string(),
                image: "ghcr.io/acme/demon-operate-ui@sha256:aaa".to_string(),
                git_sha_tag: None,
            },
        )]
        .into_iter()
        .collect();

        let err = merge_digests_into_config(&mut config, &manifest).unwrap_err();
        assert!(err
            .to_string()
            .contains("Manifest is missing required component"));
    }
}
