use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use semver::Version;
use serde::Deserialize;

use super::models::RitualInvocationRequest;

#[derive(Clone)]
pub struct AppPackRegistry {
    #[allow(dead_code)]
    root: PathBuf,
    registry_path: PathBuf,
}

impl AppPackRegistry {
    pub fn new() -> Result<Self> {
        let root = resolve_store_root()?;
        let registry_path = root.join("registry.json");
        Ok(Self {
            root,
            registry_path,
        })
    }

    pub fn with_root(root: PathBuf) -> Self {
        let registry_path = root.join("registry.json");
        Self {
            root,
            registry_path,
        }
    }

    pub fn resolve_invocation(
        &self,
        ritual_name: &str,
        request: &RitualInvocationRequest,
    ) -> Result<ResolvedInvocation> {
        let registry = self.load_registry()?;
        let installs = registry
            .apps
            .get(&request.app)
            .ok_or_else(|| anyhow!("App Pack '{}' is not installed", request.app))?;

        let install = if let Some(version) = request.version.as_deref() {
            installs
                .iter()
                .find(|entry| entry.version == version)
                .ok_or_else(|| anyhow!("App Pack '{}@{}' is not installed", request.app, version))?
        } else {
            select_latest(installs).ok_or_else(|| {
                anyhow!("App Pack '{}' has no recorded installations", request.app)
            })?
        };

        let manifest = load_manifest(&install.manifest_path)?;
        let ritual = manifest
            .rituals
            .iter()
            .find(|ritual| ritual.name == ritual_name)
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "Ritual '{}' is not defined in App Pack {}@{}",
                    ritual_name,
                    manifest.metadata.name,
                    manifest.metadata.version
                )
            })?;

        ensure_single_step(&ritual)?;

        Ok(ResolvedInvocation {
            manifest,
            ritual,
            manifest_path: install.manifest_path.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedInvocation {
    pub manifest: AppPackManifest,
    pub ritual: RitualEntry,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppPackManifest {
    pub metadata: ManifestMetadata,
    pub capsules: Vec<CapsuleEntry>,
    pub rituals: Vec<RitualEntry>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManifestMetadata {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CapsuleEntry {
    #[serde(rename_all = "camelCase")]
    ContainerExec {
        name: String,
        #[serde(rename = "imageDigest")]
        image_digest: String,
        command: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
        #[serde(default, rename = "workingDir")]
        working_dir: Option<String>,
        outputs: CapsuleOutputs,
    },
    #[serde(other)]
    Unsupported,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CapsuleOutputs {
    #[serde(rename = "envelopePath")]
    pub envelope_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RitualEntry {
    pub name: String,
    #[serde(default)]
    pub steps: Vec<RitualStep>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RitualStep {
    pub capsule: String,
    #[serde(default)]
    pub with: serde_json::Value,
}

fn resolve_store_root() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("DEMON_APP_HOME") {
        return Ok(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::var("DEMON_HOME") {
        return Ok(PathBuf::from(dir).join("app-packs"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home).join(".demon").join("app-packs"));
    }
    bail!("Unable to determine App Pack store root. Set DEMON_APP_HOME or HOME");
}

fn load_manifest(path: &Path) -> Result<AppPackManifest> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading manifest at {}", path.display()))?;
    let manifest = serde_yaml::from_str(&raw).with_context(|| "parsing App Pack manifest")?;
    Ok(manifest)
}

fn ensure_single_step(ritual: &RitualEntry) -> Result<()> {
    if ritual.steps.len() != 1 {
        bail!(
            "Ritual '{}' must contain exactly one step for Milestone 0",
            ritual.name
        );
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct RegistryFile {
    #[serde(default)]
    apps: BTreeMap<String, Vec<RegistryInstall>>, // metadata.name -> installations
}

#[derive(Debug, Deserialize)]
struct RegistryInstall {
    version: String,
    #[serde(rename = "manifest_path")]
    manifest_path: PathBuf,
}

impl AppPackRegistry {
    fn load_registry(&self) -> Result<RegistryFile> {
        if !self.registry_path.exists() {
            bail!(
                "App Pack registry not found at {}. Install packs via demonctl app install",
                self.registry_path.display()
            );
        }

        let raw = fs::read_to_string(&self.registry_path)
            .with_context(|| format!("reading registry {}", self.registry_path.display()))?;
        let registry = serde_json::from_str(&raw).with_context(|| "parsing registry JSON")?;
        Ok(registry)
    }
}

fn select_latest(installs: &[RegistryInstall]) -> Option<&RegistryInstall> {
    let mut best: Option<(&RegistryInstall, Version)> = None;
    for install in installs {
        if let Ok(ver) = Version::parse(&install.version) {
            match best {
                Some((_, ref current)) if ver <= *current => {}
                _ => best = Some((install, ver)),
            }
        } else if best.is_none() {
            best = Some((install, Version::new(0, 0, 0)));
        }
    }
    best.map(|(install, _)| install)
}
