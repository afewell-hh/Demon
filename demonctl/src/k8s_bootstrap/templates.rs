use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::k8s_bootstrap::{DemonConfig, K8sBootstrapConfig, NetworkingConfig};

pub struct TemplateRenderer {
    templates_dir: String,
}

impl TemplateRenderer {
    pub fn new(templates_dir: &str) -> Self {
        Self {
            templates_dir: templates_dir.to_string(),
        }
    }

    pub fn render_manifests(&self, config: &K8sBootstrapConfig) -> Result<String> {
        let template_context = self.build_template_context(config)?;
        let mut rendered_manifests = Vec::new();

        let mut manifest_files = vec![
            "namespace.yaml",
            "nats.yaml",
            "runtime.yaml",
            "engine.yaml",
            "operate-ui.yaml",
        ];

        // Add ingress manifest if enabled
        if config.networking.ingress.enabled {
            manifest_files.push("ingress.yaml");
        }

        for file in manifest_files {
            let template_path = Path::new(&self.templates_dir).join(file);
            let template_content = fs::read_to_string(&template_path).with_context(|| {
                format!("Failed to read template file: {}", template_path.display())
            })?;

            let rendered = self.substitute_variables(&template_content, &template_context)?;
            rendered_manifests.push(rendered);
        }

        Ok(rendered_manifests.join("\n---\n"))
    }

    fn build_template_context(
        &self,
        config: &K8sBootstrapConfig,
    ) -> Result<HashMap<String, Value>> {
        let mut context = HashMap::new();

        let demon_config = &config.demon;

        context.insert(
            "namespace".to_string(),
            Value::String(demon_config.namespace.clone()),
        );
        context.insert(
            "natsUrl".to_string(),
            Value::String(self.build_nats_url(demon_config)),
        );
        context.insert(
            "streamName".to_string(),
            Value::String(demon_config.stream_name.clone()),
        );

        let subjects_array: Vec<Value> = demon_config
            .subjects
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        context.insert("subjects".to_string(), Value::Array(subjects_array));

        context.insert(
            "dedupeWindowSecs".to_string(),
            Value::String(demon_config.dedupe_window_secs.to_string()),
        );

        let mut persistence_obj = serde_json::Map::new();
        persistence_obj.insert(
            "enabled".to_string(),
            Value::Bool(demon_config.persistence.enabled),
        );
        persistence_obj.insert(
            "storageClass".to_string(),
            Value::String(demon_config.persistence.storage_class.clone()),
        );
        persistence_obj.insert(
            "size".to_string(),
            Value::String(demon_config.persistence.size.clone()),
        );
        context.insert("persistence".to_string(), Value::Object(persistence_obj));

        // Add networking context
        let networking_context = self.build_networking_context(&config.networking)?;
        context.insert("networking".to_string(), Value::Object(networking_context));

        Ok(context)
    }

    fn build_networking_context(
        &self,
        networking: &NetworkingConfig,
    ) -> Result<serde_json::Map<String, Value>> {
        let mut networking_obj = serde_json::Map::new();

        // Build ingress context
        let mut ingress_obj = serde_json::Map::new();
        ingress_obj.insert(
            "enabled".to_string(),
            Value::Bool(networking.ingress.enabled),
        );

        if let Some(hostname) = &networking.ingress.hostname {
            ingress_obj.insert("hostname".to_string(), Value::String(hostname.clone()));
        }

        if let Some(ingress_class) = &networking.ingress.ingress_class {
            ingress_obj.insert(
                "ingressClass".to_string(),
                Value::String(ingress_class.clone()),
            );
        }

        if let Some(annotations) = &networking.ingress.annotations {
            let annotations_obj = annotations
                .iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect();
            ingress_obj.insert("annotations".to_string(), Value::Object(annotations_obj));
        }

        // Build TLS context
        let mut tls_obj = serde_json::Map::new();
        tls_obj.insert(
            "enabled".to_string(),
            Value::Bool(networking.ingress.tls.enabled),
        );
        if let Some(secret_name) = &networking.ingress.tls.secret_name {
            tls_obj.insert("secretName".to_string(), Value::String(secret_name.clone()));
        }
        ingress_obj.insert("tls".to_string(), Value::Object(tls_obj));

        networking_obj.insert("ingress".to_string(), Value::Object(ingress_obj));

        // Build service mesh context
        let mut service_mesh_obj = serde_json::Map::new();
        service_mesh_obj.insert(
            "enabled".to_string(),
            Value::Bool(networking.service_mesh.enabled),
        );

        let mesh_annotations_obj = networking
            .service_mesh
            .annotations
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        service_mesh_obj.insert(
            "annotations".to_string(),
            Value::Object(mesh_annotations_obj),
        );

        networking_obj.insert("serviceMesh".to_string(), Value::Object(service_mesh_obj));

        Ok(networking_obj)
    }

    fn build_nats_url(&self, demon_config: &DemonConfig) -> String {
        format!(
            "nats://{}.{}.svc.cluster.local:4222",
            "nats", demon_config.namespace
        )
    }

    fn substitute_variables(
        &self,
        template: &str,
        context: &HashMap<String, Value>,
    ) -> Result<String> {
        let mut result = template.to_string();

        for (key, value) in context {
            let placeholder = format!("{{{{ .{} }}}}", key);
            // Only substitute if the placeholder is actually used in the template
            if template.contains(&placeholder) {
                let replacement = self.value_to_string(value, key)?;
                result = result.replace(&placeholder, &replacement);
            }
        }

        self.handle_conditionals(&result, context)
    }

    fn value_to_string(&self, value: &Value, key: &str) -> Result<String> {
        match value {
            Value::String(s) => Ok(s.clone()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Array(arr) => {
                if key == "subjects" {
                    let strings: Result<Vec<String>, _> = arr
                        .iter()
                        .map(|v| match v {
                            Value::String(s) => Ok(s.clone()),
                            _ => Err(anyhow::anyhow!("Array element is not a string")),
                        })
                        .collect();
                    Ok(strings?.join(","))
                } else {
                    Err(anyhow::anyhow!("Unsupported array type for key: {}", key))
                }
            }
            Value::Object(_) => Err(anyhow::anyhow!(
                "Object values cannot be directly substituted"
            )),
            Value::Null => Ok("".to_string()),
        }
    }

    fn handle_conditionals(
        &self,
        template: &str,
        context: &HashMap<String, Value>,
    ) -> Result<String> {
        let mut result = template.to_string();

        // Handle persistence conditionals
        if let Some(Value::Object(persistence_obj)) = context.get("persistence") {
            let enabled = persistence_obj
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if enabled {
                result = self.process_conditional_block(&result, "persistence.enabled", true)?;
            } else {
                result = self.process_conditional_block(&result, "persistence.enabled", false)?;
            }
        }

        // Handle networking conditionals
        if let Some(Value::Object(networking_obj)) = context.get("networking") {
            // Handle ingress conditionals
            if let Some(Value::Object(ingress_obj)) = networking_obj.get("ingress") {
                let ingress_enabled = ingress_obj
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                result = self.process_conditional_block(
                    &result,
                    "networking.ingress.enabled",
                    ingress_enabled,
                )?;
            }

            // Handle service mesh conditionals
            if let Some(Value::Object(service_mesh_obj)) = networking_obj.get("serviceMesh") {
                let mesh_enabled = service_mesh_obj
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                result = self.process_conditional_block(
                    &result,
                    "networking.serviceMesh.enabled",
                    mesh_enabled,
                )?;
            }
        }

        Ok(result)
    }

    fn process_conditional_block(
        &self,
        template: &str,
        condition: &str,
        condition_value: bool,
    ) -> Result<String> {
        let if_pattern = format!("{{{{- if .{} }}}}", condition);
        let else_pattern = "{{- else }}";
        let end_pattern = "{{- end }}";

        let mut result = String::new();
        let mut lines = template.lines();

        while let Some(line) = lines.next() {
            if line.trim() == if_pattern {
                let mut if_block = Vec::new();
                let mut else_block = Vec::new();
                let mut in_else = false;

                for inner_line in lines.by_ref() {
                    if inner_line.trim() == end_pattern {
                        break;
                    } else if inner_line.trim() == else_pattern {
                        in_else = true;
                    } else if in_else {
                        else_block.push(inner_line);
                    } else {
                        if_block.push(inner_line);
                    }
                }

                let chosen_block = if condition_value {
                    if_block
                } else {
                    else_block
                };
                for block_line in chosen_block {
                    result.push_str(block_line);
                    result.push('\n');
                }
            } else {
                result.push_str(line);
                result.push('\n');
            }
        }

        Ok(result.trim_end().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k8s_bootstrap::{DemonConfig, PersistenceConfig};

    #[test]
    fn test_build_template_context() {
        let renderer = TemplateRenderer::new("test");
        let config = K8sBootstrapConfig {
            api_version: "v1".to_string(),
            kind: "K8sBootstrap".to_string(),
            metadata: crate::k8s_bootstrap::ConfigMetadata {
                name: "test".to_string(),
            },
            cluster: crate::k8s_bootstrap::ClusterConfig {
                name: "test-cluster".to_string(),
                runtime: "k3s".to_string(),
                k3s: crate::k8s_bootstrap::K3sConfig {
                    version: "v1.28.0+k3s1".to_string(),
                    install: crate::k8s_bootstrap::K3sInstallConfig {
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
                namespace: "test-ns".to_string(),
                stream_name: "test-stream".to_string(),
                subjects: vec!["subject1".to_string(), "subject2".to_string()],
                dedupe_window_secs: 30,
                ui_url: "http://localhost:3000".to_string(),
                persistence: PersistenceConfig {
                    enabled: true,
                    storage_class: "fast-ssd".to_string(),
                    size: "10Gi".to_string(),
                },
                bundle: None,
            },
            secrets: crate::k8s_bootstrap::SecretsConfig {
                provider: "env".to_string(),
                vault: None,
                env: None,
            },
            addons: vec![],
            networking: crate::k8s_bootstrap::NetworkingConfig {
                ingress: crate::k8s_bootstrap::IngressConfig {
                    enabled: false,
                    hostname: None,
                    ingress_class: None,
                    annotations: None,
                    tls: crate::k8s_bootstrap::TlsConfig {
                        enabled: false,
                        secret_name: None,
                    },
                },
                service_mesh: crate::k8s_bootstrap::ServiceMeshConfig {
                    enabled: false,
                    annotations: crate::k8s_bootstrap::default_mesh_annotations(),
                },
            },
        };

        let context = renderer.build_template_context(&config).unwrap();

        assert_eq!(
            context.get("namespace").unwrap(),
            &Value::String("test-ns".to_string())
        );
        assert_eq!(
            context.get("streamName").unwrap(),
            &Value::String("test-stream".to_string())
        );
        assert_eq!(
            context.get("natsUrl").unwrap(),
            &Value::String("nats://nats.test-ns.svc.cluster.local:4222".to_string())
        );
    }

    #[test]
    fn test_substitute_variables() {
        let renderer = TemplateRenderer::new("test");
        let mut context = HashMap::new();
        context.insert("namespace".to_string(), Value::String("demo".to_string()));
        context.insert(
            "streamName".to_string(),
            Value::String("events".to_string()),
        );

        let template = "namespace: {{ .namespace }}\nstream: {{ .streamName }}";
        let result = renderer.substitute_variables(template, &context).unwrap();

        assert!(result.contains("namespace: demo"));
        assert!(result.contains("stream: events"));
    }

    #[test]
    fn test_substitute_variables_with_numbers() {
        let renderer = TemplateRenderer::new("test");
        let mut context = HashMap::new();
        context.insert(
            "port".to_string(),
            Value::Number(serde_json::Number::from(8080)),
        );
        context.insert(
            "replicas".to_string(),
            Value::Number(serde_json::Number::from(3)),
        );

        let template = "port: {{ .port }}\nreplicas: {{ .replicas }}";
        let result = renderer.substitute_variables(template, &context).unwrap();

        assert!(result.contains("port: 8080"));
        assert!(result.contains("replicas: 3"));
    }

    #[test]
    fn test_substitute_variables_with_arrays() {
        let renderer = TemplateRenderer::new("test");
        let mut context = HashMap::new();
        let subjects = vec![
            Value::String("events.created".to_string()),
            Value::String("events.updated".to_string()),
            Value::String("events.deleted".to_string()),
        ];
        context.insert("subjects".to_string(), Value::Array(subjects));

        let template = "subjects: {{ .subjects }}";
        let result = renderer.substitute_variables(template, &context).unwrap();

        assert!(result.contains("subjects: events.created,events.updated,events.deleted"));
    }

    #[test]
    fn test_substitute_variables_missing_key() {
        let renderer = TemplateRenderer::new("test");
        let context = HashMap::new();

        let template = "value: {{ .missingKey }}";
        let result = renderer.substitute_variables(template, &context).unwrap();

        // Missing keys should remain unchanged
        assert!(result.contains("value: {{ .missingKey }}"));
    }

    #[test]
    fn test_build_nats_url() {
        let renderer = TemplateRenderer::new("test");
        let demon_config = DemonConfig {
            nats_url: "nats://localhost:4222".to_string(),
            namespace: "production".to_string(),
            stream_name: "events".to_string(),
            subjects: vec![],
            dedupe_window_secs: 30,
            ui_url: "http://localhost:3000".to_string(),
            persistence: PersistenceConfig {
                enabled: false,
                storage_class: "standard".to_string(),
                size: "1Gi".to_string(),
            },
            bundle: None,
        };

        let url = renderer.build_nats_url(&demon_config);
        assert_eq!(url, "nats://nats.production.svc.cluster.local:4222");
    }

    #[test]
    fn test_handle_conditionals_persistence_enabled() {
        let renderer = TemplateRenderer::new("test");
        let mut context = HashMap::new();

        let mut persistence_obj = serde_json::Map::new();
        persistence_obj.insert("enabled".to_string(), Value::Bool(true));
        persistence_obj.insert(
            "storageClass".to_string(),
            Value::String("fast-ssd".to_string()),
        );
        persistence_obj.insert("size".to_string(), Value::String("10Gi".to_string()));
        context.insert("persistence".to_string(), Value::Object(persistence_obj));

        let template = r#"{{- if .persistence.enabled }}
volumeClaimTemplates:
- metadata:
    name: data
  spec:
    storageClassName: fast-ssd
{{- else }}
volumes:
- name: data
  emptyDir: {}
{{- end }}"#;

        let result = renderer.handle_conditionals(template, &context).unwrap();

        assert!(result.contains("volumeClaimTemplates"));
        assert!(result.contains("storageClassName: fast-ssd"));
        assert!(!result.contains("emptyDir"));
    }

    #[test]
    fn test_handle_conditionals_persistence_disabled() {
        let renderer = TemplateRenderer::new("test");
        let mut context = HashMap::new();

        let mut persistence_obj = serde_json::Map::new();
        persistence_obj.insert("enabled".to_string(), Value::Bool(false));
        context.insert("persistence".to_string(), Value::Object(persistence_obj));

        let template = r#"{{- if .persistence.enabled }}
volumeClaimTemplates:
- metadata:
    name: data
{{- else }}
volumes:
- name: data
  emptyDir: {}
{{- end }}"#;

        let result = renderer.handle_conditionals(template, &context).unwrap();

        assert!(!result.contains("volumeClaimTemplates"));
        assert!(result.contains("emptyDir"));
    }

    #[test]
    fn test_value_to_string_unsupported_array() {
        let renderer = TemplateRenderer::new("test");
        let array_value = Value::Array(vec![Value::String("test".to_string())]);

        let result = renderer.value_to_string(&array_value, "unsupportedArray");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported array type"));
    }

    #[test]
    fn test_value_to_string_object_error() {
        let renderer = TemplateRenderer::new("test");
        let object_value = Value::Object(serde_json::Map::new());

        let result = renderer.value_to_string(&object_value, "testKey");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Object values cannot be directly substituted"));
    }

    #[test]
    fn test_value_to_string_null() {
        let renderer = TemplateRenderer::new("test");
        let null_value = Value::Null;

        let result = renderer.value_to_string(&null_value, "testKey").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_value_to_string_boolean() {
        let renderer = TemplateRenderer::new("test");

        let true_value = Value::Bool(true);
        let false_value = Value::Bool(false);

        assert_eq!(
            renderer.value_to_string(&true_value, "test").unwrap(),
            "true"
        );
        assert_eq!(
            renderer.value_to_string(&false_value, "test").unwrap(),
            "false"
        );
    }

    #[test]
    fn test_render_manifests_integration() {
        let renderer = TemplateRenderer::new("nonexistent-dir");
        let config = K8sBootstrapConfig {
            api_version: "v1".to_string(),
            kind: "K8sBootstrap".to_string(),
            metadata: crate::k8s_bootstrap::ConfigMetadata {
                name: "test-integration".to_string(),
            },
            cluster: crate::k8s_bootstrap::ClusterConfig {
                name: "test-cluster".to_string(),
                runtime: "k3s".to_string(),
                k3s: crate::k8s_bootstrap::K3sConfig {
                    version: "v1.28.0+k3s1".to_string(),
                    install: crate::k8s_bootstrap::K3sInstallConfig {
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
                namespace: "integration-test".to_string(),
                stream_name: "test-events".to_string(),
                subjects: vec!["test.events".to_string()],
                dedupe_window_secs: 60,
                ui_url: "http://localhost:3000".to_string(),
                persistence: PersistenceConfig {
                    enabled: true,
                    storage_class: "premium-ssd".to_string(),
                    size: "20Gi".to_string(),
                },
                bundle: None,
            },
            secrets: crate::k8s_bootstrap::SecretsConfig {
                provider: "env".to_string(),
                vault: None,
                env: None,
            },
            addons: vec![],
            networking: crate::k8s_bootstrap::NetworkingConfig {
                ingress: crate::k8s_bootstrap::IngressConfig {
                    enabled: false,
                    hostname: None,
                    ingress_class: None,
                    annotations: None,
                    tls: crate::k8s_bootstrap::TlsConfig {
                        enabled: false,
                        secret_name: None,
                    },
                },
                service_mesh: crate::k8s_bootstrap::ServiceMeshConfig {
                    enabled: false,
                    annotations: crate::k8s_bootstrap::default_mesh_annotations(),
                },
            },
        };

        // This should fail because templates directory doesn't exist, but we can test the context building
        let context = renderer.build_template_context(&config).unwrap();

        assert_eq!(
            context.get("namespace").unwrap(),
            &Value::String("integration-test".to_string())
        );
        assert_eq!(
            context.get("streamName").unwrap(),
            &Value::String("test-events".to_string())
        );
        assert_eq!(
            context.get("natsUrl").unwrap(),
            &Value::String("nats://nats.integration-test.svc.cluster.local:4222".to_string())
        );
        assert_eq!(
            context.get("subjects").unwrap(),
            &Value::Array(vec![Value::String("test.events".to_string())])
        );
        assert_eq!(
            context.get("dedupeWindowSecs").unwrap(),
            &Value::String("60".to_string())
        );

        // Test persistence object structure
        if let Value::Object(persistence) = context.get("persistence").unwrap() {
            assert_eq!(persistence.get("enabled").unwrap(), &Value::Bool(true));
            assert_eq!(
                persistence.get("storageClass").unwrap(),
                &Value::String("premium-ssd".to_string())
            );
            assert_eq!(
                persistence.get("size").unwrap(),
                &Value::String("20Gi".to_string())
            );
        } else {
            panic!("Persistence should be an object");
        }
    }

    #[test]
    fn test_process_conditional_block_edge_cases() {
        let renderer = TemplateRenderer::new("test");

        // Test template without conditional blocks
        let simple_template = "apiVersion: v1\nkind: Namespace";
        let result = renderer
            .process_conditional_block(simple_template, "test.condition", true)
            .unwrap();
        assert_eq!(result, simple_template);

        // Test nested conditional (should handle gracefully)
        let nested_template = r#"{{- if .persistence.enabled }}
outer: true
{{- if .nested.condition }}
nested: true
{{- end }}
{{- end }}"#;

        let result = renderer
            .process_conditional_block(nested_template, "persistence.enabled", true)
            .unwrap();
        assert!(result.contains("outer: true"));
        assert!(result.contains("{{- if .nested.condition }}"));
    }

    #[test]
    fn test_build_networking_context_ingress_disabled() {
        let renderer = TemplateRenderer::new("test");
        let networking = crate::k8s_bootstrap::NetworkingConfig {
            ingress: crate::k8s_bootstrap::IngressConfig {
                enabled: false,
                hostname: None,
                ingress_class: None,
                annotations: None,
                tls: crate::k8s_bootstrap::TlsConfig {
                    enabled: false,
                    secret_name: None,
                },
            },
            service_mesh: crate::k8s_bootstrap::ServiceMeshConfig {
                enabled: false,
                annotations: crate::k8s_bootstrap::default_mesh_annotations(),
            },
        };

        let context = renderer.build_networking_context(&networking).unwrap();

        let ingress = context.get("ingress").unwrap().as_object().unwrap();
        assert_eq!(ingress.get("enabled").unwrap(), &Value::Bool(false));
        assert!(!ingress.contains_key("hostname"));

        let tls = ingress.get("tls").unwrap().as_object().unwrap();
        assert_eq!(tls.get("enabled").unwrap(), &Value::Bool(false));

        let service_mesh = context.get("serviceMesh").unwrap().as_object().unwrap();
        assert_eq!(service_mesh.get("enabled").unwrap(), &Value::Bool(false));
    }

    #[test]
    fn test_build_networking_context_ingress_enabled_with_tls() {
        let renderer = TemplateRenderer::new("test");
        let networking = crate::k8s_bootstrap::NetworkingConfig {
            ingress: crate::k8s_bootstrap::IngressConfig {
                enabled: true,
                hostname: Some("ui.example.com".to_string()),
                ingress_class: Some("nginx".to_string()),
                annotations: None,
                tls: crate::k8s_bootstrap::TlsConfig {
                    enabled: true,
                    secret_name: Some("demon-tls".to_string()),
                },
            },
            service_mesh: crate::k8s_bootstrap::ServiceMeshConfig {
                enabled: true,
                annotations: crate::k8s_bootstrap::default_mesh_annotations(),
            },
        };

        let context = renderer.build_networking_context(&networking).unwrap();

        let ingress = context.get("ingress").unwrap().as_object().unwrap();
        assert_eq!(ingress.get("enabled").unwrap(), &Value::Bool(true));
        assert_eq!(
            ingress.get("hostname").unwrap(),
            &Value::String("ui.example.com".to_string())
        );
        assert_eq!(
            ingress.get("ingressClass").unwrap(),
            &Value::String("nginx".to_string())
        );

        let tls = ingress.get("tls").unwrap().as_object().unwrap();
        assert_eq!(tls.get("enabled").unwrap(), &Value::Bool(true));
        assert_eq!(
            tls.get("secretName").unwrap(),
            &Value::String("demon-tls".to_string())
        );

        let service_mesh = context.get("serviceMesh").unwrap().as_object().unwrap();
        assert_eq!(service_mesh.get("enabled").unwrap(), &Value::Bool(true));

        let mesh_annotations = service_mesh
            .get("annotations")
            .unwrap()
            .as_object()
            .unwrap();
        assert_eq!(
            mesh_annotations.get("sidecar.istio.io/inject").unwrap(),
            &Value::String("true".to_string())
        );
    }

    #[test]
    fn test_networking_conditionals_in_handle_conditionals() {
        let renderer = TemplateRenderer::new("test");
        let mut context = HashMap::new();

        // Build networking context for testing
        let networking = crate::k8s_bootstrap::NetworkingConfig {
            ingress: crate::k8s_bootstrap::IngressConfig {
                enabled: true,
                hostname: Some("test.example.com".to_string()),
                ingress_class: None,
                annotations: None,
                tls: crate::k8s_bootstrap::TlsConfig {
                    enabled: false,
                    secret_name: None,
                },
            },
            service_mesh: crate::k8s_bootstrap::ServiceMeshConfig {
                enabled: true,
                annotations: crate::k8s_bootstrap::default_mesh_annotations(),
            },
        };
        let networking_context = renderer.build_networking_context(&networking).unwrap();
        context.insert("networking".to_string(), Value::Object(networking_context));

        let template = r#"{{- if .networking.ingress.enabled }}
ingress: enabled
{{- else }}
ingress: disabled
{{- end }}
{{- if .networking.serviceMesh.enabled }}
mesh: enabled
{{- else }}
mesh: disabled
{{- end }}"#;

        let result = renderer.handle_conditionals(template, &context).unwrap();

        assert!(result.contains("ingress: enabled"));
        assert!(result.contains("mesh: enabled"));
        assert!(!result.contains("ingress: disabled"));
        assert!(!result.contains("mesh: disabled"));
    }
}
