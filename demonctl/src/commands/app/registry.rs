use crate::commands::app::manifest::AppPackManifest;
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct RegistryFile {
    #[serde(default)]
    apps: BTreeMap<String, Vec<InstalledPack>>, // key = metadata.name
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPack {
    pub version: String,
    pub manifest_path: PathBuf,
    pub installed_at: DateTime<Utc>,
    pub source: String,
    #[serde(default)]
    pub schema_range: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Registry {
    state: RegistryFile,
    path: PathBuf,
}

impl Registry {
    pub fn load(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                state: RegistryFile::default(),
                path,
            });
        }

        let data = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read registry '{}'", path.display()))?;
        let state: RegistryFile = serde_json::from_str(&data)
            .with_context(|| format!("Failed to parse registry JSON from '{}'", path.display()))?;

        Ok(Self { state, path })
    }

    pub fn list(&self) -> Vec<(String, InstalledPack)> {
        let mut rows = Vec::new();
        for (name, installs) in &self.state.apps {
            for install in installs {
                rows.push((name.clone(), install.clone()));
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.version.cmp(&b.1.version)));
        rows
    }

    pub fn register_install(
        &mut self,
        manifest: &AppPackManifest,
        manifest_path: PathBuf,
        source: String,
        overwrite: bool,
    ) -> Result<()> {
        let entry = InstalledPack {
            version: manifest.version().to_string(),
            manifest_path,
            installed_at: Utc::now(),
            source,
            schema_range: Some(manifest.requires_schema_range().to_string()),
        };

        let installs = self
            .state
            .apps
            .entry(manifest.name().to_string())
            .or_default();

        if let Some(existing_index) = installs
            .iter()
            .position(|install| install.version == entry.version)
        {
            if !overwrite {
                bail!(
                    "App Pack '{}@{}' is already installed. Use --overwrite to replace it.",
                    manifest.name(),
                    entry.version
                );
            }
            installs[existing_index] = entry;
        } else {
            installs.push(entry);
        }

        installs.sort_by(|a, b| a.version.cmp(&b.version));
        Ok(())
    }

    pub fn remove(&mut self, name: &str, version: Option<&str>) -> Result<Vec<InstalledPack>> {
        let Some(installs) = self.state.apps.get_mut(name) else {
            bail!("No App Pack named '{}' is installed", name);
        };

        let removed = if let Some(version) = version {
            let idx = installs
                .iter()
                .position(|install| install.version == version)
                .ok_or_else(|| anyhow!("App Pack '{}@{}' is not installed", name, version))?;
            vec![installs.remove(idx)]
        } else {
            installs.drain(..).collect()
        };

        if installs.is_empty() {
            self.state.apps.remove(name);
        }

        Ok(removed)
    }

    pub fn persist(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create registry directory '{}'", parent.display())
            })?;
        }

        let json = serde_json::to_string_pretty(&self.state)
            .context("Failed to serialize registry state")?;
        fs::write(&self.path, json)
            .with_context(|| format!("Failed to write registry file '{}'", self.path.display()))?;
        Ok(())
    }
}

pub fn registry_path(base_dir: &Path) -> PathBuf {
    base_dir.join("registry.json")
}
