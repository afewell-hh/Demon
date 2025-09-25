use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::k8s_bootstrap::{AddonConfig, K8sBootstrapConfig};

/// Trait that all add-ons must implement
pub trait AddOn: Send + Sync {
    /// Returns the unique name of the add-on
    fn name(&self) -> &str;

    /// Returns a human-readable description of what this add-on provides
    fn description(&self) -> &str;

    /// Renders the Kubernetes manifests for this add-on
    fn render_manifests(
        &self,
        addon_config: &AddonConfig,
        bootstrap_config: &K8sBootstrapConfig,
    ) -> Result<Vec<String>>;

    /// Validates the add-on configuration
    fn validate_config(&self, addon_config: &AddonConfig) -> Result<()> {
        if !addon_config.enabled {
            return Ok(());
        }

        if addon_config.name != self.name() {
            anyhow::bail!(
                "Configuration name '{}' does not match add-on name '{}'",
                addon_config.name,
                self.name()
            );
        }

        Ok(())
    }
}

/// Registry containing all available add-ons
pub struct AddOnRegistry {
    addons: HashMap<String, Box<dyn AddOn>>,
}

impl AddOnRegistry {
    /// Creates a new registry with all built-in add-ons
    pub fn new() -> Self {
        let mut registry = Self {
            addons: HashMap::new(),
        };

        // Register built-in add-ons
        registry.register(Box::new(MonitoringAddOn::new()));

        registry
    }

    /// Register a new add-on
    fn register(&mut self, addon: Box<dyn AddOn>) {
        self.addons.insert(addon.name().to_string(), addon);
    }

    /// Get an add-on by name
    pub fn get(&self, name: &str) -> Option<&dyn AddOn> {
        self.addons.get(name).map(|boxed| boxed.as_ref())
    }

    /// List all available add-ons
    #[allow(dead_code)]
    pub fn list(&self) -> Vec<&str> {
        self.addons.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for AddOnRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Monitoring add-on providing Prometheus and Grafana
pub struct MonitoringAddOn {
    templates_dir: String,
}

impl MonitoringAddOn {
    pub fn new() -> Self {
        Self {
            templates_dir: format!("{}/resources/addons/monitoring", env!("CARGO_MANIFEST_DIR")),
        }
    }

    fn build_context(
        &self,
        addon_config: &AddonConfig,
        bootstrap_config: &K8sBootstrapConfig,
    ) -> HashMap<String, Value> {
        let mut context = HashMap::new();

        // Basic configuration
        context.insert(
            "namespace".to_string(),
            Value::String(bootstrap_config.demon.namespace.clone()),
        );

        // Default configurations that can be overridden
        context.insert(
            "prometheusRetention".to_string(),
            Value::String("15d".to_string()),
        );
        context.insert(
            "prometheusStorageSize".to_string(),
            Value::String("10Gi".to_string()),
        );
        context.insert(
            "grafanaAdminPassword".to_string(),
            Value::String("admin".to_string()), // Should be overridden in production
        );

        // Override with user-provided config if available
        if let Some(config_map) = &addon_config.config {
            if let Some(retention) = config_map.get("prometheusRetention") {
                context.insert("prometheusRetention".to_string(), retention.clone());
            }
            if let Some(storage) = config_map.get("prometheusStorageSize") {
                context.insert("prometheusStorageSize".to_string(), storage.clone());
            }
            if let Some(password) = config_map.get("grafanaAdminPassword") {
                context.insert("grafanaAdminPassword".to_string(), password.clone());
            }
        }

        context
    }

    fn substitute_variables(&self, template: &str, context: &HashMap<String, Value>) -> String {
        let mut result = template.to_string();

        for (key, value) in context {
            let placeholder = format!("{{{{ .{} }}}}", key);
            if let Value::String(s) = value {
                result = result.replace(&placeholder, s);
            } else if let Value::Number(n) = value {
                result = result.replace(&placeholder, &n.to_string());
            } else if let Value::Bool(b) = value {
                result = result.replace(&placeholder, &b.to_string());
            }
        }

        result
    }
}

impl AddOn for MonitoringAddOn {
    fn name(&self) -> &str {
        "monitoring"
    }

    fn description(&self) -> &str {
        "Prometheus and Grafana for monitoring and observability"
    }

    fn render_manifests(
        &self,
        addon_config: &AddonConfig,
        bootstrap_config: &K8sBootstrapConfig,
    ) -> Result<Vec<String>> {
        if !addon_config.enabled {
            return Ok(vec![]);
        }

        let context = self.build_context(addon_config, bootstrap_config);
        let mut manifests = Vec::new();

        // Check if templates directory exists, if not use embedded templates
        let templates_dir = Path::new(&self.templates_dir);

        if templates_dir.exists() {
            // Load templates from filesystem
            let template_files = vec![
                "prometheus-configmap.yaml",
                "prometheus-deployment.yaml",
                "prometheus-service.yaml",
                "grafana-configmap.yaml",
                "grafana-deployment.yaml",
                "grafana-service.yaml",
            ];

            for file in template_files {
                let template_path = templates_dir.join(file);
                if template_path.exists() {
                    let template = fs::read_to_string(&template_path).with_context(|| {
                        format!("Failed to read template: {}", template_path.display())
                    })?;
                    let manifest = self.substitute_variables(&template, &context);
                    manifests.push(manifest);
                }
            }
        } else {
            // Use embedded templates for testing
            manifests.extend(self.generate_embedded_manifests(&context));
        }

        Ok(manifests)
    }

    fn validate_config(&self, addon_config: &AddonConfig) -> Result<()> {
        // Call parent validation first
        if !addon_config.enabled {
            return Ok(());
        }

        if addon_config.name != self.name() {
            anyhow::bail!(
                "Configuration name '{}' does not match add-on name '{}'",
                addon_config.name,
                self.name()
            );
        }

        // Validate monitoring-specific config
        if let Some(config_map) = &addon_config.config {
            // Validate storage size format if provided
            if let Some(Value::String(size)) = config_map.get("prometheusStorageSize") {
                if !size.ends_with("Gi") && !size.ends_with("Mi") && !size.ends_with("Ti") {
                    anyhow::bail!(
                        "Invalid storage size format '{}'. Must end with Gi, Mi, or Ti",
                        size
                    );
                }
            }
        }

        Ok(())
    }
}

impl MonitoringAddOn {
    /// Generate embedded manifests for testing when template files don't exist
    fn generate_embedded_manifests(&self, context: &HashMap<String, Value>) -> Vec<String> {
        let namespace = context
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("demon");

        let prometheus_retention = context
            .get("prometheusRetention")
            .and_then(|v| v.as_str())
            .unwrap_or("15d");

        let prometheus_storage = context
            .get("prometheusStorageSize")
            .and_then(|v| v.as_str())
            .unwrap_or("10Gi");

        vec![
            // Prometheus ConfigMap
            format!(
                r#"apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-config
  namespace: {}
data:
  prometheus.yml: |
    global:
      scrape_interval: 15s
      evaluation_interval: 15s
    scrape_configs:
    - job_name: 'prometheus'
      static_configs:
      - targets: ['localhost:9090']
    - job_name: 'kubernetes-pods'
      kubernetes_sd_configs:
      - role: pod
      relabel_configs:
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
        action: keep
        regex: true
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_path]
        action: replace
        target_label: __metrics_path__
        regex: (.+)
      - source_labels: [__address__, __meta_kubernetes_pod_annotation_prometheus_io_port]
        action: replace
        regex: ([^:]+)(?::\d+)?;(\d+)
        replacement: $1:$2
        target_label: __address__"#,
                namespace
            ),
            // Prometheus Deployment
            format!(
                r#"apiVersion: apps/v1
kind: Deployment
metadata:
  name: prometheus
  namespace: {}
spec:
  replicas: 1
  selector:
    matchLabels:
      app: prometheus
  template:
    metadata:
      labels:
        app: prometheus
    spec:
      containers:
      - name: prometheus
        image: prom/prometheus:v2.45.0
        args:
        - '--config.file=/etc/prometheus/prometheus.yml'
        - '--storage.tsdb.path=/prometheus'
        - '--storage.tsdb.retention.time={}'
        - '--web.console.libraries=/usr/share/prometheus/console_libraries'
        - '--web.console.templates=/usr/share/prometheus/consoles'
        ports:
        - containerPort: 9090
        volumeMounts:
        - name: prometheus-config
          mountPath: /etc/prometheus
        - name: prometheus-storage
          mountPath: /prometheus
      volumes:
      - name: prometheus-config
        configMap:
          name: prometheus-config
      - name: prometheus-storage
        persistentVolumeClaim:
          claimName: prometheus-pvc"#,
                namespace, prometheus_retention
            ),
            // Prometheus Service
            format!(
                r#"apiVersion: v1
kind: Service
metadata:
  name: prometheus
  namespace: {}
spec:
  type: ClusterIP
  ports:
  - port: 9090
    targetPort: 9090
  selector:
    app: prometheus"#,
                namespace
            ),
            // Prometheus PVC
            format!(
                r#"apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: prometheus-pvc
  namespace: {}
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: {}"#,
                namespace, prometheus_storage
            ),
            // Grafana ConfigMap
            format!(
                r#"apiVersion: v1
kind: ConfigMap
metadata:
  name: grafana-datasources
  namespace: {}
data:
  prometheus.yaml: |
    apiVersion: 1
    datasources:
    - name: Prometheus
      type: prometheus
      access: proxy
      url: http://prometheus:9090
      isDefault: true"#,
                namespace
            ),
            // Grafana Deployment
            format!(
                r#"apiVersion: apps/v1
kind: Deployment
metadata:
  name: grafana
  namespace: {}
spec:
  replicas: 1
  selector:
    matchLabels:
      app: grafana
  template:
    metadata:
      labels:
        app: grafana
    spec:
      containers:
      - name: grafana
        image: grafana/grafana:10.0.0
        ports:
        - containerPort: 3000
        env:
        - name: GF_SECURITY_ADMIN_PASSWORD
          value: admin
        volumeMounts:
        - name: grafana-datasources
          mountPath: /etc/grafana/provisioning/datasources
      volumes:
      - name: grafana-datasources
        configMap:
          name: grafana-datasources"#,
                namespace
            ),
            // Grafana Service
            format!(
                r#"apiVersion: v1
kind: Service
metadata:
  name: grafana
  namespace: {}
spec:
  type: ClusterIP
  ports:
  - port: 3000
    targetPort: 3000
  selector:
    app: grafana"#,
                namespace
            ),
        ]
    }
}

/// Process all enabled add-ons and return their manifests
pub fn process_addons(
    config: &K8sBootstrapConfig,
    dry_run: bool,
    verbose: bool,
) -> Result<Vec<String>> {
    let registry = AddOnRegistry::new();
    let mut all_manifests = Vec::new();
    let mut enabled_addons = Vec::new();

    for addon_config in &config.addons {
        if !addon_config.enabled {
            continue;
        }

        let addon = registry
            .get(&addon_config.name)
            .with_context(|| format!("Unknown add-on: {}", addon_config.name))?;

        addon.validate_config(addon_config)?;

        if verbose {
            println!(
                "  Processing add-on: {} - {}",
                addon.name(),
                addon.description()
            );
        }

        enabled_addons.push(addon.name());

        if !dry_run {
            let manifests = addon.render_manifests(addon_config, config)?;
            all_manifests.extend(manifests);
        }
    }

    if dry_run && !enabled_addons.is_empty() {
        println!("Add-ons enabled ({}):", enabled_addons.len());
        for name in enabled_addons {
            if let Some(addon) = registry.get(name) {
                println!("  - {} ({})", name, addon.description());
            }
        }
    }

    Ok(all_manifests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k8s_bootstrap::{
        default_mesh_annotations, ClusterConfig, ConfigMetadata, DemonConfig, IngressConfig,
        K3sConfig, K3sInstallConfig, NetworkingConfig, PersistenceConfig, SecretsConfig,
        ServiceMeshConfig, TlsConfig,
    };

    fn create_test_config() -> K8sBootstrapConfig {
        K8sBootstrapConfig {
            api_version: "v1".to_string(),
            kind: "K8sBootstrap".to_string(),
            metadata: ConfigMetadata {
                name: "test".to_string(),
            },
            cluster: ClusterConfig {
                name: "test-cluster".to_string(),
                runtime: "k3s".to_string(),
                k3s: K3sConfig {
                    version: "v1.28.0+k3s1".to_string(),
                    install: K3sInstallConfig {
                        channel: "stable".to_string(),
                        disable: vec![],
                    },
                    data_dir: "/var/lib/rancher/k3s".to_string(),
                    node_name: "k3s-node".to_string(),
                    extra_args: vec![],
                },
            },
            demon: DemonConfig {
                nats_url: "nats://localhost:4222".to_string(),
                namespace: "test-namespace".to_string(),
                stream_name: "test-stream".to_string(),
                subjects: vec!["test.events".to_string()],
                dedupe_window_secs: 30,
                ui_url: "http://localhost:3000".to_string(),
                persistence: PersistenceConfig {
                    enabled: false,
                    storage_class: "standard".to_string(),
                    size: "1Gi".to_string(),
                },
                bundle: None,
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
                    tls: TlsConfig {
                        enabled: false,
                        secret_name: None,
                    },
                },
                service_mesh: ServiceMeshConfig {
                    enabled: false,
                    annotations: default_mesh_annotations(),
                },
            },
            registries: None,
        }
    }

    #[test]
    fn test_addon_registry_creation() {
        let registry = AddOnRegistry::new();
        assert!(registry.get("monitoring").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_addon_registry_list() {
        let registry = AddOnRegistry::new();
        let addons = registry.list();
        assert!(addons.contains(&"monitoring"));
    }

    #[test]
    fn test_monitoring_addon_basic() {
        let addon = MonitoringAddOn::new();
        assert_eq!(addon.name(), "monitoring");
        assert!(!addon.description().is_empty());
    }

    #[test]
    fn test_monitoring_addon_disabled() {
        let addon = MonitoringAddOn::new();
        let addon_config = AddonConfig {
            name: "monitoring".to_string(),
            enabled: false,
            config: None,
        };
        let bootstrap_config = create_test_config();

        let manifests = addon
            .render_manifests(&addon_config, &bootstrap_config)
            .unwrap();
        assert!(manifests.is_empty());
    }

    #[test]
    fn test_monitoring_addon_enabled() {
        let addon = MonitoringAddOn::new();
        let addon_config = AddonConfig {
            name: "monitoring".to_string(),
            enabled: true,
            config: None,
        };
        let bootstrap_config = create_test_config();

        let manifests = addon
            .render_manifests(&addon_config, &bootstrap_config)
            .unwrap();
        assert!(!manifests.is_empty());

        // Check that manifests contain expected resources
        let combined = manifests.join("\n");
        assert!(combined.contains("kind: ConfigMap"));
        assert!(combined.contains("kind: Deployment"));
        assert!(combined.contains("kind: Service"));
        assert!(combined.contains("prometheus"));
        assert!(combined.contains("grafana"));
    }

    #[test]
    fn test_monitoring_addon_with_custom_config() {
        let addon = MonitoringAddOn::new();
        let mut config_map = HashMap::new();
        config_map.insert(
            "prometheusRetention".to_string(),
            Value::String("30d".to_string()),
        );
        config_map.insert(
            "prometheusStorageSize".to_string(),
            Value::String("50Gi".to_string()),
        );

        let addon_config = AddonConfig {
            name: "monitoring".to_string(),
            enabled: true,
            config: Some(config_map),
        };
        let bootstrap_config = create_test_config();

        let manifests = addon
            .render_manifests(&addon_config, &bootstrap_config)
            .unwrap();
        assert!(!manifests.is_empty());

        let combined = manifests.join("\n");
        assert!(combined.contains("30d"));
        assert!(combined.contains("50Gi"));
    }

    #[test]
    fn test_addon_validate_config_mismatch_name() {
        let addon = MonitoringAddOn::new();
        let addon_config = AddonConfig {
            name: "wrong-name".to_string(),
            enabled: true,
            config: None,
        };

        let result = addon.validate_config(&addon_config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("does not match add-on name"));
    }

    #[test]
    fn test_addon_validate_config_invalid_storage_size() {
        let addon = MonitoringAddOn::new();
        let mut config_map = HashMap::new();
        config_map.insert(
            "prometheusStorageSize".to_string(),
            Value::String("50GB".to_string()),
        );

        let addon_config = AddonConfig {
            name: "monitoring".to_string(),
            enabled: true,
            config: Some(config_map),
        };

        let result = addon.validate_config(&addon_config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid storage size format"));
    }

    #[test]
    fn test_process_addons_no_addons() {
        let config = create_test_config();
        let manifests = process_addons(&config, false, false).unwrap();
        assert!(manifests.is_empty());
    }

    #[test]
    fn test_process_addons_with_monitoring() {
        let mut config = create_test_config();
        config.addons = vec![AddonConfig {
            name: "monitoring".to_string(),
            enabled: true,
            config: None,
        }];

        let manifests = process_addons(&config, false, false).unwrap();
        assert!(!manifests.is_empty());
    }

    #[test]
    fn test_process_addons_unknown_addon() {
        let mut config = create_test_config();
        config.addons = vec![AddonConfig {
            name: "unknown-addon".to_string(),
            enabled: true,
            config: None,
        }];

        let result = process_addons(&config, false, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown add-on"));
    }

    #[test]
    fn test_process_addons_dry_run() {
        let mut config = create_test_config();
        config.addons = vec![AddonConfig {
            name: "monitoring".to_string(),
            enabled: true,
            config: None,
        }];

        let manifests = process_addons(&config, true, false).unwrap();
        assert!(manifests.is_empty()); // Dry-run doesn't generate manifests
    }

    #[test]
    fn test_monitoring_build_context() {
        let addon = MonitoringAddOn::new();
        let addon_config = AddonConfig {
            name: "monitoring".to_string(),
            enabled: true,
            config: None,
        };
        let bootstrap_config = create_test_config();

        let context = addon.build_context(&addon_config, &bootstrap_config);

        assert_eq!(
            context.get("namespace").unwrap(),
            &Value::String("test-namespace".to_string())
        );
        assert_eq!(
            context.get("prometheusRetention").unwrap(),
            &Value::String("15d".to_string())
        );
        assert_eq!(
            context.get("prometheusStorageSize").unwrap(),
            &Value::String("10Gi".to_string())
        );
    }

    #[test]
    fn test_monitoring_substitute_variables() {
        let addon = MonitoringAddOn::new();
        let mut context = HashMap::new();
        context.insert(
            "namespace".to_string(),
            Value::String("my-namespace".to_string()),
        );
        context.insert(
            "prometheusRetention".to_string(),
            Value::String("7d".to_string()),
        );

        let template = "namespace: {{ .namespace }}\nretention: {{ .prometheusRetention }}";
        let result = addon.substitute_variables(template, &context);

        assert!(result.contains("namespace: my-namespace"));
        assert!(result.contains("retention: 7d"));
    }
}
