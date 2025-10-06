#![allow(dead_code)]

use anyhow::{anyhow, bail, ensure, Context, Result};
use jsonschema::JSONSchema;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

static APP_PACK_SCHEMA: Lazy<JSONSchema> = Lazy::new(|| {
    let schema_str = include_str!("../../../../contracts/schemas/app-pack.v1.schema.json");
    let schema_json: JsonValue = serde_json::from_str(schema_str)
        .expect("contracts/schemas/app-pack.v1.schema.json must be valid JSON");
    JSONSchema::compile(&schema_json).expect("app pack schema must compile")
});

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPackManifest {
    pub api_version: String,
    pub kind: String,
    pub metadata: Metadata,
    #[serde(default)]
    pub signing: Option<Signing>,
    #[serde(default)]
    pub requires: Option<Requires>,
    pub contracts: Vec<Contract>,
    pub capsules: Vec<Capsule>,
    pub rituals: Vec<Ritual>,
    #[serde(default)]
    pub ui: Option<Ui>,
}

impl AppPackManifest {
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    pub fn version(&self) -> &str {
        &self.metadata.version
    }

    pub fn requires_schema_range(&self) -> &str {
        self.requires
            .as_ref()
            .and_then(|r| r.app_pack_schema.as_deref())
            .unwrap_or(">=1.0.0 <2.0.0")
    }

    pub fn validate_semantics(&self) -> Result<()> {
        ensure!(
            self.api_version == "demon.io/v1",
            "Unsupported apiVersion: {}",
            self.api_version
        );
        ensure!(self.kind == "AppPack", "Unsupported kind: {}", self.kind);

        if self.contracts.is_empty() {
            bail!("App Pack must declare at least one contract entry");
        }

        if self.capsules.is_empty() {
            bail!("App Pack must declare at least one capsule entry");
        }

        if self.rituals.is_empty() {
            bail!("App Pack must declare at least one ritual entry");
        }

        let mut contract_ids = HashSet::new();
        for contract in &self.contracts {
            if !contract_ids.insert(contract.id.as_str()) {
                bail!("Duplicate contract id '{}'", contract.id);
            }
        }

        let mut capsule_names = HashSet::new();
        for capsule in &self.capsules {
            if !capsule_names.insert(capsule.name.as_str()) {
                bail!("Duplicate capsule name '{}'", capsule.name);
            }
        }

        let known_capsules = capsule_names.clone();

        let mut ritual_names = HashSet::new();
        for ritual in &self.rituals {
            if !ritual_names.insert(ritual.name.as_str()) {
                bail!("Duplicate ritual name '{}'", ritual.name);
            }

            if ritual.steps.is_empty() {
                bail!("Ritual '{}' must contain at least one step", ritual.name);
            }

            for step in &ritual.steps {
                if !known_capsules.contains(step.capsule.as_str()) {
                    bail!(
                        "Ritual '{}' references unknown capsule '{}'",
                        ritual.name,
                        step.capsule
                    );
                }
            }
        }

        if let Some(ui) = &self.ui {
            for card in &ui.cards {
                ensure!(
                    !card.matching.rituals.is_empty(),
                    "UI card '{}' must reference at least one ritual",
                    card.id
                );

                for ritual in &card.matching.rituals {
                    if !ritual_names.contains(ritual.as_str()) {
                        bail!(
                            "UI card '{}' references unknown ritual '{}'",
                            card.id,
                            ritual
                        );
                    }
                }
            }
        }

        if let Some(signing) = &self.signing {
            if let Some(cosign) = &signing.cosign {
                match cosign {
                    Cosign::Enabled(true) => {
                        bail!(
                            "signing.cosign must be an object with signature settings; boolean form is no longer supported"
                        );
                    }
                    Cosign::Enabled(false) => {}
                    Cosign::Settings(settings) => {
                        if settings.is_enabled() {
                            ensure!(
                                settings.signature_path.is_some(),
                                "signing.cosign.signaturePath must be provided when enabled"
                            );
                            ensure!(
                                settings.public_key_path.is_some(),
                                "signing.cosign.publicKeyPath must be provided when enabled"
                            );
                            ensure!(
                                settings.public_key_hash.is_some(),
                                "signing.cosign.publicKeyHash must be provided when enabled"
                            );

                            if let Some(sig_path) = settings.signature_path() {
                                validate_relative_path(sig_path, "signing.cosign.signaturePath")?;
                            }
                            if let Some(key_path) = settings.public_key_path() {
                                validate_relative_path(key_path, "signing.cosign.publicKeyPath")?;
                            }

                            if let Some(hash) = settings.public_key_hash() {
                                ensure!(
                                    hash.algorithm.eq_ignore_ascii_case("sha256"),
                                    "signing.cosign.publicKeyHash.algorithm must be 'sha256'"
                                );
                                ensure!(
                                    hash.value.chars().all(|c| c.is_ascii_hexdigit())
                                        && hash.value.len() == 64,
                                    "signing.cosign.publicKeyHash.value must be a 64-character hex string"
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

fn validate_relative_path(path: &str, field: &str) -> Result<()> {
    let path_ref = Path::new(path);
    ensure!(
        !path_ref.is_absolute(),
        "{} must be a relative path inside the bundle",
        field
    );
    for component in path_ref.components() {
        ensure!(
            !matches!(component, std::path::Component::ParentDir),
            "{} cannot contain '..'",
            field
        );
    }
    Ok(())
}

impl CosignSettings {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    pub fn signature_path(&self) -> Option<&str> {
        self.signature_path.as_deref()
    }

    pub fn public_key_path(&self) -> Option<&str> {
        self.public_key_path.as_deref()
    }

    pub fn public_key_hash(&self) -> Option<&HashDigest> {
        self.public_key_hash.as_ref()
    }
}

impl HashDigest {
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl AppPackManifest {
    pub fn cosign_settings(&self) -> Result<Option<&CosignSettings>> {
        let Some(signing) = &self.signing else {
            return Ok(None);
        };

        let Some(cosign) = &signing.cosign else {
            return Ok(None);
        };

        match cosign {
            Cosign::Enabled(true) => bail!(
                "signing.cosign must be an object with signature settings; boolean form is no longer supported"
            ),
            Cosign::Enabled(false) => Ok(None),
            Cosign::Settings(settings) => Ok(if settings.is_enabled() {
                Some(settings)
            } else {
                None
            }),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signing {
    #[serde(default)]
    pub cosign: Option<Cosign>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Cosign {
    Enabled(bool),
    Settings(CosignSettings),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CosignSettings {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub signature_path: Option<String>,
    #[serde(default)]
    pub public_key_path: Option<String>,
    #[serde(default)]
    pub public_key_hash: Option<HashDigest>,
    #[serde(default)]
    pub key_ref: Option<String>,
    #[serde(default)]
    pub certificate_identity: Option<String>,
    #[serde(default)]
    pub certificate_issuer: Option<String>,
    #[serde(default)]
    pub rekor_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashDigest {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Requires {
    #[serde(default)]
    pub app_pack_schema: Option<String>,
    #[serde(default)]
    pub platform_apis: Option<PlatformApis>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlatformApis {
    #[serde(default)]
    pub engine: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub operate_ui: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    pub id: String,
    pub version: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capsule {
    #[serde(rename = "type")]
    pub capsule_type: CapsuleType,
    pub name: String,
    pub image_digest: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub working_dir: Option<String>,
    pub outputs: CapsuleOutputs,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapsuleType {
    ContainerExec,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapsuleOutputs {
    pub envelope_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ritual {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub steps: Vec<RitualStep>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RitualStep {
    pub capsule: String,
    #[serde(default)]
    pub with: Option<HashMap<String, serde_yaml::Value>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ui {
    #[serde(default)]
    pub cards: Vec<UiCard>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiCard {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(rename = "match")]
    pub matching: UiCardMatch,
    #[serde(default)]
    pub fields: Option<UiCardFields>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiCardMatch {
    pub rituals: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiCardFields {
    #[serde(default)]
    pub show: Vec<String>,
    #[serde(default)]
    pub map: BTreeMap<String, String>,
}

pub fn parse_manifest(raw: &str) -> Result<AppPackManifest> {
    let yaml_value: YamlValue =
        serde_yaml::from_str(raw).context("Failed to parse App Pack manifest YAML")?;
    let json_value = serde_json::to_value(&yaml_value)
        .context("Failed to convert manifest YAML to JSON for validation")?;
    validate_against_schema(&json_value)?;
    let manifest: AppPackManifest = serde_yaml::from_value(yaml_value)
        .context("Failed to deserialize manifest into AppPackManifest")?;
    manifest.validate_semantics()?;
    Ok(manifest)
}

fn validate_against_schema(value: &JsonValue) -> Result<()> {
    if let Err(errors) = APP_PACK_SCHEMA.validate(value) {
        let mut lines = vec!["App Pack schema validation failed:".to_string()];
        for error in errors {
            lines.push(format!("  - {}", error));
        }
        let message = lines.join("\n");
        return Err(anyhow!(message));
    }
    Ok(())
}
