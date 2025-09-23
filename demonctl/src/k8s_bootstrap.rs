use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct K8sBootstrapConfig {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: ConfigMetadata,
    pub cluster: ClusterConfig,
    pub demon: DemonConfig,
    #[serde(default)]
    pub secrets: SecretsConfig,
    #[serde(default)]
    pub addons: Vec<AddonConfig>,
    #[serde(default)]
    pub networking: NetworkingConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigMetadata {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub name: String,
    #[serde(default = "default_k3s_version")]
    pub version: String,
    #[serde(rename = "dataDir", default = "default_data_dir")]
    pub data_dir: String,
    #[serde(rename = "nodeName", default = "default_node_name")]
    pub node_name: String,
    #[serde(rename = "extraArgs", default)]
    pub extra_args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DemonConfig {
    #[serde(rename = "natsUrl", default = "default_nats_url")]
    pub nats_url: String,
    #[serde(rename = "streamName", default = "default_stream_name")]
    pub stream_name: String,
    #[serde(default = "default_subjects")]
    pub subjects: Vec<String>,
    #[serde(rename = "dedupeWindowSecs", default = "default_dedupe_window")]
    pub dedupe_window_secs: u64,
    #[serde(rename = "uiUrl", default = "default_ui_url")]
    pub ui_url: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default)]
    pub persistence: PersistenceConfig,
    #[serde(default)]
    pub bundle: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PersistenceConfig {
    #[serde(default = "default_persistence_enabled")]
    pub enabled: bool,
    #[serde(rename = "storageClass", default = "default_storage_class")]
    pub storage_class: String,
    #[serde(default = "default_storage_size")]
    pub size: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecretsConfig {
    #[serde(default = "default_secrets_provider")]
    pub provider: String,
    #[serde(default)]
    pub vault: Option<VaultConfig>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultConfig {
    pub address: String,
    pub role: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddonConfig {
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub values: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct NetworkingConfig {
    #[serde(default)]
    pub ingress: IngressConfig,
    #[serde(rename = "serviceMesh", default)]
    pub service_mesh: ServiceMeshConfig,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct IngressConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(rename = "tlsSecretName", default)]
    pub tls_secret_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ServiceMeshConfig {
    #[serde(default)]
    pub enabled: bool,
}

// Default value functions
fn default_k3s_version() -> String {
    "v1.28.2+k3s1".to_string()
}

fn default_data_dir() -> String {
    "/var/lib/rancher/k3s".to_string()
}

fn default_node_name() -> String {
    "demon-node".to_string()
}

fn default_nats_url() -> String {
    "nats://nats.demon-system.svc.cluster.local:4222".to_string()
}

fn default_stream_name() -> String {
    "RITUAL_EVENTS".to_string()
}

fn default_subjects() -> Vec<String> {
    vec!["ritual.>".to_string(), "approval.>".to_string()]
}

fn default_dedupe_window() -> u64 {
    60
}

fn default_ui_url() -> String {
    "http://operate-ui.demon-system.svc.cluster.local:3000".to_string()
}

fn default_namespace() -> String {
    "demon-system".to_string()
}

fn default_persistence_enabled() -> bool {
    true
}

fn default_storage_class() -> String {
    "local-path".to_string()
}

fn default_storage_size() -> String {
    "10Gi".to_string()
}

fn default_secrets_provider() -> String {
    "env".to_string()
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: default_persistence_enabled(),
            storage_class: default_storage_class(),
            size: default_storage_size(),
        }
    }
}

impl Default for SecretsConfig {
    fn default() -> Self {
        Self {
            provider: default_secrets_provider(),
            vault: None,
            env: None,
        }
    }
}

pub fn load_config(config_path: &str) -> Result<K8sBootstrapConfig> {
    let config_content = std::fs::read_to_string(config_path)?;
    let config: K8sBootstrapConfig = serde_yaml::from_str(&config_content)?;
    Ok(config)
}

pub fn validate_config(config: &K8sBootstrapConfig) -> Result<()> {
    if config.api_version.is_empty() {
        anyhow::bail!("apiVersion is required");
    }

    if config.kind != "BootstrapConfig" {
        anyhow::bail!("kind must be 'BootstrapConfig'");
    }

    if config.metadata.name.is_empty() {
        anyhow::bail!("metadata.name is required");
    }

    if config.cluster.name.is_empty() {
        anyhow::bail!("cluster.name is required");
    }

    if !Path::new(&config.cluster.data_dir).is_absolute() {
        anyhow::bail!("cluster.dataDir must be an absolute path");
    }

    if config.demon.nats_url.is_empty() {
        anyhow::bail!("demon.natsUrl is required");
    }

    if config.demon.stream_name.is_empty() {
        anyhow::bail!("demon.streamName is required");
    }

    if config.demon.subjects.is_empty() {
        anyhow::bail!("demon.subjects cannot be empty");
    }

    if config.demon.ui_url.is_empty() {
        anyhow::bail!("demon.uiUrl is required");
    }

    if config.demon.namespace.is_empty() {
        anyhow::bail!("demon.namespace is required");
    }

    // Validate secrets configuration
    match config.secrets.provider.as_str() {
        "vault" => {
            if config.secrets.vault.is_none() {
                anyhow::bail!("vault configuration is required when provider is 'vault'");
            }
        }
        "env" => {
            // env provider can work with empty configuration (runtime env vars)
        }
        "file" => {
            // File provider doesn't need additional config
        }
        _ => {
            anyhow::bail!("secrets.provider must be one of: vault, env, file");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn given_valid_minimal_config_when_load_then_success() {
        let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(config_yaml.as_bytes()).unwrap();

        let config = load_config(file.path().to_str().unwrap()).unwrap();
        assert_eq!(config.metadata.name, "test-cluster");
        assert_eq!(config.cluster.name, "test-cluster");
        assert_eq!(config.demon.nats_url, "nats://localhost:4222");
        assert_eq!(config.demon.stream_name, "TEST_EVENTS");
        assert_eq!(config.demon.subjects, vec!["test.>"]);
        assert_eq!(config.demon.ui_url, "http://localhost:3000");
        assert_eq!(config.demon.namespace, "test-system");
    }

    #[test]
    fn given_config_with_defaults_when_load_then_uses_defaults() {
        let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(config_yaml.as_bytes()).unwrap();

        let config = load_config(file.path().to_str().unwrap()).unwrap();

        // Check defaults are applied
        assert_eq!(config.cluster.version, "v1.28.2+k3s1");
        assert_eq!(config.cluster.data_dir, "/var/lib/rancher/k3s");
        assert_eq!(config.cluster.node_name, "demon-node");
        assert_eq!(config.demon.dedupe_window_secs, 60);
        assert_eq!(config.secrets.provider, "env");
        assert!(config.demon.persistence.enabled);
        assert_eq!(config.demon.persistence.storage_class, "local-path");
        assert_eq!(config.demon.persistence.size, "10Gi");
        assert!(!config.networking.ingress.enabled);
        assert!(!config.networking.service_mesh.enabled);
    }

    #[test]
    fn given_config_with_vault_secrets_when_load_then_success() {
        let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
secrets:
  provider: vault
  vault:
    address: "https://vault.example.com"
    role: "test-role"
    path: "secret/test"
"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(config_yaml.as_bytes()).unwrap();

        let config = load_config(file.path().to_str().unwrap()).unwrap();
        assert_eq!(config.secrets.provider, "vault");
        assert!(config.secrets.vault.is_some());
        let vault_config = config.secrets.vault.unwrap();
        assert_eq!(vault_config.address, "https://vault.example.com");
        assert_eq!(vault_config.role, "test-role");
        assert_eq!(vault_config.path, "secret/test");
    }

    #[test]
    fn given_config_with_addons_when_load_then_success() {
        let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
addons:
  - name: prometheus
    enabled: true
    values:
      retention: "7d"
      storage: "5Gi"
  - name: grafana
    enabled: false
"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(config_yaml.as_bytes()).unwrap();

        let config = load_config(file.path().to_str().unwrap()).unwrap();
        assert_eq!(config.addons.len(), 2);

        let prometheus = &config.addons[0];
        assert_eq!(prometheus.name, "prometheus");
        assert!(prometheus.enabled);
        assert_eq!(prometheus.values["retention"], "7d");
        assert_eq!(prometheus.values["storage"], "5Gi");

        let grafana = &config.addons[1];
        assert_eq!(grafana.name, "grafana");
        assert!(!grafana.enabled);
    }

    #[test]
    fn given_valid_config_when_validate_then_success() {
        let config = create_valid_test_config();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn given_config_missing_api_version_when_validate_then_error() {
        let mut config = create_valid_test_config();
        config.api_version = "".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("apiVersion is required"));
    }

    #[test]
    fn given_config_wrong_kind_when_validate_then_error() {
        let mut config = create_valid_test_config();
        config.kind = "WrongKind".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("kind must be 'BootstrapConfig'"));
    }

    #[test]
    fn given_config_missing_metadata_name_when_validate_then_error() {
        let mut config = create_valid_test_config();
        config.metadata.name = "".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("metadata.name is required"));
    }

    #[test]
    fn given_config_missing_cluster_name_when_validate_then_error() {
        let mut config = create_valid_test_config();
        config.cluster.name = "".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cluster.name is required"));
    }

    #[test]
    fn given_config_relative_data_dir_when_validate_then_error() {
        let mut config = create_valid_test_config();
        config.cluster.data_dir = "relative/path".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cluster.dataDir must be an absolute path"));
    }

    #[test]
    fn given_config_missing_nats_url_when_validate_then_error() {
        let mut config = create_valid_test_config();
        config.demon.nats_url = "".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("demon.natsUrl is required"));
    }

    #[test]
    fn given_config_empty_subjects_when_validate_then_error() {
        let mut config = create_valid_test_config();
        config.demon.subjects = vec![];

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("demon.subjects cannot be empty"));
    }

    #[test]
    fn given_vault_provider_without_config_when_validate_then_error() {
        let mut config = create_valid_test_config();
        config.secrets.provider = "vault".to_string();
        config.secrets.vault = None;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("vault configuration is required when provider is 'vault'"));
    }

    #[test]
    fn given_env_provider_without_config_when_validate_then_error() {
        let mut config = create_valid_test_config();
        config.secrets.provider = "env".to_string();
        config.secrets.env = None;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("env configuration is required when provider is 'env'"));
    }

    #[test]
    fn given_env_provider_with_empty_config_when_validate_then_error() {
        let mut config = create_valid_test_config();
        config.secrets.provider = "env".to_string();
        config.secrets.env = Some(HashMap::new());

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("env configuration is required when provider is 'env'"));
    }

    #[test]
    fn given_invalid_secrets_provider_when_validate_then_error() {
        let mut config = create_valid_test_config();
        config.secrets.provider = "invalid".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("secrets.provider must be one of: vault, env, file"));
    }

    #[test]
    fn given_file_provider_when_validate_then_success() {
        let mut config = create_valid_test_config();
        config.secrets.provider = "file".to_string();
        config.secrets.env = None;

        let result = validate_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn given_malformed_yaml_when_load_then_error() {
        let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
  invalid_yaml: [unclosed array
"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(config_yaml.as_bytes()).unwrap();

        let result = load_config(file.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn given_nonexistent_file_when_load_then_error() {
        let result = load_config("/nonexistent/config.yaml");
        assert!(result.is_err());
    }

    fn create_valid_test_config() -> K8sBootstrapConfig {
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR".to_string(), "TEST_VALUE".to_string());

        K8sBootstrapConfig {
            api_version: "demon.io/v1".to_string(),
            kind: "BootstrapConfig".to_string(),
            metadata: ConfigMetadata {
                name: "test-cluster".to_string(),
            },
            cluster: ClusterConfig {
                name: "test-cluster".to_string(),
                version: "v1.28.2+k3s1".to_string(),
                data_dir: "/var/lib/rancher/k3s".to_string(),
                node_name: "test-node".to_string(),
                extra_args: vec![],
            },
            demon: DemonConfig {
                nats_url: "nats://localhost:4222".to_string(),
                stream_name: "TEST_EVENTS".to_string(),
                subjects: vec!["test.>".to_string()],
                dedupe_window_secs: 60,
                ui_url: "http://localhost:3000".to_string(),
                namespace: "test-system".to_string(),
                persistence: PersistenceConfig::default(),
                bundle: None,
            },
            secrets: SecretsConfig {
                provider: "env".to_string(),
                vault: None,
                env: Some(env_vars),
            },
            addons: vec![],
            networking: NetworkingConfig::default(),
        }
    }
}
