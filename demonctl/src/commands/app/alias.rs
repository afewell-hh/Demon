use super::manifest::{self, CapsuleType};
use super::registry::Registry;
use super::registry_path;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::{Builder as TempFileBuilder, NamedTempFile, TempDir};

pub struct AliasTarget {
    pub app: String,
    pub version: Option<String>,
    pub ritual: String,
}

pub struct AliasSpec {
    temp_spec: NamedTempFile,
    _artifacts_dir: TempDir,
}

impl AliasSpec {
    pub fn spec_path(&self) -> &Path {
        self.temp_spec.path()
    }
}

pub fn parse_alias(input: &str) -> Option<AliasTarget> {
    let (app_part, ritual) = input.split_once(':')?;
    let ritual = ritual.trim();
    if ritual.is_empty() {
        return None;
    }

    let (app, version) = if let Some((name, ver)) = app_part.split_once('@') {
        let name = name.trim();
        let ver = ver.trim();
        if name.is_empty() || ver.is_empty() {
            return None;
        }
        (name.to_string(), Some(ver.to_string()))
    } else {
        let name = app_part.trim();
        if name.is_empty() {
            return None;
        }
        (name.to_string(), None)
    };

    Some(AliasTarget {
        app,
        version,
        ritual: ritual.to_string(),
    })
}

pub fn build_alias_spec(target: &AliasTarget) -> Result<AliasSpec> {
    let registry_path = registry_path()?;
    let registry = Registry::load(registry_path)?;
    let install = registry.resolve_install(&target.app, target.version.as_deref())?;

    let manifest_raw = fs::read_to_string(&install.manifest_path).with_context(|| {
        format!(
            "Failed to read manifest '{}'",
            install.manifest_path.display()
        )
    })?;

    let manifest = manifest::parse_manifest(&manifest_raw)?;

    let ritual = manifest
        .rituals
        .iter()
        .find(|r| r.name == target.ritual)
        .ok_or_else(|| {
            anyhow!(
                "Ritual '{}' not found in App Pack '{}'",
                target.ritual,
                target.app
            )
        })?;

    if ritual.steps.len() != 1 {
        bail!(
            "Ritual '{}' currently supports exactly one step (found {})",
            ritual.name,
            ritual.steps.len()
        );
    }

    let step = &ritual.steps[0];
    let capsule = manifest
        .capsules
        .iter()
        .find(|c| c.name == step.capsule)
        .ok_or_else(|| {
            anyhow!(
                "Capsule '{}' referenced by ritual '{}' not found in App Pack",
                step.capsule,
                ritual.name
            )
        })?;

    match capsule.capsule_type {
        CapsuleType::ContainerExec => {}
    }

    let pack_root = install
        .manifest_path
        .parent()
        .ok_or_else(|| anyhow!("Unable to determine App Pack root for alias"))?;
    let workspace_dir = fs::canonicalize(pack_root).with_context(|| {
        format!(
            "Failed to canonicalize App Pack directory '{}'",
            pack_root.display()
        )
    })?;

    let artifacts_dir = TempFileBuilder::new()
        .prefix("demon-artifacts-")
        .tempdir()
        .context("Failed to create artifacts directory")?;
    let artifacts_dir_path = artifacts_dir
        .path()
        .to_path_buf()
        .into_os_string()
        .into_string()
        .map_err(|_| anyhow!("Artifacts directory contains invalid UTF-8"))?;

    let mut env_map: JsonMap<String, JsonValue> = capsule
        .env
        .iter()
        .map(|(k, v)| (k.clone(), JsonValue::String(v.clone())))
        .collect();

    if let Some(with_map) = step.with.as_ref() {
        merge_with_into_env(&mut env_map, with_map)?;
    }

    let mut arguments = JsonMap::new();
    arguments.insert("imageDigest".to_string(), json!(capsule.image_digest));
    arguments.insert(
        "command".to_string(),
        JsonValue::Array(
            capsule
                .command
                .iter()
                .map(|s| JsonValue::String(s.clone()))
                .collect(),
        ),
    );
    arguments.insert("env".to_string(), JsonValue::Object(env_map));
    if let Some(wd) = &capsule.working_dir {
        arguments.insert("workingDir".to_string(), json!(wd));
    }
    arguments.insert(
        "outputs".to_string(),
        json!({"envelopePath": capsule.outputs.envelope_path.clone()}),
    );
    arguments.insert("capsuleName".to_string(), json!(capsule.name));
    arguments.insert(
        "workspaceDir".to_string(),
        workspace_dir
            .to_str()
            .ok_or_else(|| anyhow!("Workspace directory contains invalid UTF-8"))?
            .into(),
    );
    arguments.insert(
        "artifactsDir".to_string(),
        JsonValue::String(artifacts_dir_path),
    );

    let ritual_name = format!("{}:{}", target.app, ritual.name);
    let display_name = ritual
        .display_name
        .clone()
        .unwrap_or_else(|| ritual_name.clone());

    let state_name = format!("invoke-{}", step.capsule);

    let spec_json = json!({
        "id": ritual_name,
        "version": "1.0",
        "name": display_name,
        "states": [
            {
                "name": state_name,
                "type": "task",
                "action": {
                    "functionRef": {
                        "refName": "container-exec",
                        "arguments": JsonValue::Object(arguments)
                    }
                },
                "end": true
            }
        ]
    });

    let mut yaml =
        serde_yaml::to_string(&spec_json).context("Failed to serialize alias spec to YAML")?;
    if let Some(stripped) = yaml.strip_prefix("---\n") {
        yaml = stripped.to_string();
    }

    let mut temp_spec = TempFileBuilder::new()
        .prefix("demon-alias-")
        .suffix(".yaml")
        .tempfile()
        .context("Failed to create temporary alias spec")?;
    temp_spec
        .write_all(yaml.as_bytes())
        .context("Failed to write alias spec")?;
    temp_spec.flush().ok();

    Ok(AliasSpec {
        temp_spec,
        _artifacts_dir: artifacts_dir,
    })
}

fn merge_with_into_env(
    env: &mut JsonMap<String, JsonValue>,
    with_map: &HashMap<String, serde_yaml::Value>,
) -> Result<()> {
    for (key, value) in with_map {
        let rendered = match value {
            serde_yaml::Value::Null => "".to_string(),
            serde_yaml::Value::Bool(b) => b.to_string(),
            serde_yaml::Value::Number(n) => n.to_string(),
            serde_yaml::Value::String(s) => s.clone(),
            _ => {
                bail!("Non-scalar value in ritual step 'with' map is not supported yet");
            }
        };
        env.insert(key.clone(), JsonValue::String(rendered));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_alias_with_version() {
        let target = parse_alias("app@1.2.3:ritual").unwrap();
        assert_eq!(target.app, "app");
        assert_eq!(target.version.as_deref(), Some("1.2.3"));
        assert_eq!(target.ritual, "ritual");
    }

    #[test]
    fn parse_alias_without_version() {
        let target = parse_alias("app:ritual").unwrap();
        assert_eq!(target.app, "app");
        assert!(target.version.is_none());
        assert_eq!(target.ritual, "ritual");
    }

    #[test]
    fn parse_alias_invalid() {
        assert!(parse_alias("just-app").is_none());
        assert!(parse_alias(":missing").is_none());
        assert!(parse_alias("missing:").is_none());
    }
}
