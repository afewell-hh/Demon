use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Registry of installed App Packs
#[derive(Debug, Clone)]
pub struct AppPackRegistry {
    packs: HashMap<String, Vec<AppPackInfo>>,
}

#[derive(Debug, Clone, Deserialize)]
struct RegistryFile {
    #[serde(default)]
    apps: HashMap<String, Vec<InstalledPack>>,
}

#[derive(Debug, Clone, Deserialize)]
struct InstalledPack {
    #[allow(dead_code)]
    version: String,
    manifest_path: PathBuf,
}

/// Information about an installed App Pack
#[derive(Debug, Clone, Serialize)]
pub struct AppPackInfo {
    pub name: String,
    pub version: String,
    pub ui_cards: Vec<CardDefinition>,
}

/// UI card definition from App Pack manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDefinition {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "match")]
    pub match_rules: MatchRules,
    #[serde(default)]
    pub fields: Option<CardFields>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRules {
    pub rituals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardFields {
    #[serde(default)]
    pub show: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AppPackManifest {
    metadata: ManifestMetadata,
    #[serde(default)]
    ui: Option<UiManifest>,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestMetadata {
    name: String,
    version: String,
}

#[derive(Debug, Clone, Deserialize)]
struct UiManifest {
    #[serde(default)]
    cards: Vec<CardDefinition>,
}

impl AppPackRegistry {
    /// Load the App Pack registry from the default location
    pub fn load() -> Result<Self> {
        let registry_path = Self::default_registry_path()?;
        Self::load_from_path(&registry_path)
    }

    /// Load the App Pack registry from a specific path
    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                packs: HashMap::new(),
            });
        }

        let data = fs::read_to_string(path)
            .with_context(|| format!("Failed to read registry from '{}'", path.display()))?;

        let registry: RegistryFile = serde_json::from_str(&data)
            .with_context(|| format!("Failed to parse registry JSON from '{}'", path.display()))?;

        let mut packs = HashMap::new();

        for (name, installs) in registry.apps {
            let mut pack_infos = Vec::new();

            for install in installs {
                if let Ok(manifest) = Self::load_manifest(&install.manifest_path) {
                    let ui_cards = manifest.ui.map(|ui| ui.cards).unwrap_or_default();

                    pack_infos.push(AppPackInfo {
                        name: manifest.metadata.name.clone(),
                        version: manifest.metadata.version.clone(),
                        ui_cards,
                    });
                }
            }

            if !pack_infos.is_empty() {
                packs.insert(name, pack_infos);
            }
        }

        Ok(Self { packs })
    }

    /// Get all card definitions from all installed App Packs
    pub fn get_all_cards(&self) -> Vec<CardDefinition> {
        let mut cards = Vec::new();

        for pack_versions in self.packs.values() {
            for pack_info in pack_versions {
                cards.extend(pack_info.ui_cards.clone());
            }
        }

        cards
    }

    /// Get cards matching a specific ritual name
    pub fn get_cards_for_ritual(&self, ritual_name: &str) -> Vec<CardDefinition> {
        self.get_all_cards()
            .into_iter()
            .filter(|card| card.matches_ritual(ritual_name))
            .collect()
    }

    /// Load an App Pack manifest from a file
    fn load_manifest(path: &Path) -> Result<AppPackManifest> {
        let data = fs::read_to_string(path)
            .with_context(|| format!("Failed to read manifest from '{}'", path.display()))?;

        let manifest: AppPackManifest = serde_yaml::from_str(&data)
            .with_context(|| format!("Failed to parse manifest YAML from '{}'", path.display()))?;

        Ok(manifest)
    }

    /// Get the default registry path
    fn default_registry_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to determine home directory")?;
        Ok(home.join(".demon/app-packs/registry.json"))
    }
}

impl CardDefinition {
    /// Check if this card matches a given ritual name
    pub fn matches_ritual(&self, ritual_name: &str) -> bool {
        self.match_rules.rituals.contains(&ritual_name.to_string())
    }

    /// Get the list of fields to show (empty vec if not specified)
    pub fn fields_to_show(&self) -> Vec<String> {
        self.fields
            .as_ref()
            .map(|f| f.show.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_load_empty_registry() {
        let temp_dir = TempDir::new().unwrap();
        let registry_path = temp_dir.path().join("registry.json");

        let registry = AppPackRegistry::load_from_path(&registry_path).unwrap();
        assert_eq!(registry.get_all_cards().len(), 0);
    }

    #[test]
    fn test_load_registry_with_cards() {
        let temp_dir = TempDir::new().unwrap();
        let registry_path = temp_dir.path().join("registry.json");
        let pack_dir = temp_dir.path().join("packs/test/1.0.0");
        fs::create_dir_all(&pack_dir).unwrap();

        let manifest_path = pack_dir.join("app-pack.yaml");
        let manifest_content = r#"
apiVersion: demon.io/v1
kind: AppPack
metadata:
  name: test-pack
  version: 1.0.0
ui:
  cards:
    - id: test-card
      kind: result-envelope
      title: Test Card
      match:
        rituals: ["test-ritual"]
      fields:
        show: ["status"]
"#;
        fs::write(&manifest_path, manifest_content).unwrap();

        let registry_content = serde_json::json!({
            "apps": {
                "test-pack": [{
                    "version": "1.0.0",
                    "manifest_path": manifest_path.to_str().unwrap()
                }]
            }
        });
        let mut registry_file = fs::File::create(&registry_path).unwrap();
        registry_file
            .write_all(registry_content.to_string().as_bytes())
            .unwrap();

        let registry = AppPackRegistry::load_from_path(&registry_path).unwrap();
        let cards = registry.get_all_cards();

        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].id, "test-card");
        assert_eq!(cards[0].title, Some("Test Card".to_string()));
    }

    #[test]
    fn test_card_ritual_matching() {
        let card = CardDefinition {
            id: "test-card".to_string(),
            kind: "result-envelope".to_string(),
            title: None,
            description: None,
            match_rules: MatchRules {
                rituals: vec!["ritual-a".to_string(), "ritual-b".to_string()],
            },
            fields: None,
        };

        assert!(card.matches_ritual("ritual-a"));
        assert!(card.matches_ritual("ritual-b"));
        assert!(!card.matches_ritual("ritual-c"));
    }

    #[test]
    fn test_get_cards_for_ritual() {
        let temp_dir = TempDir::new().unwrap();
        let registry_path = temp_dir.path().join("registry.json");
        let pack_dir = temp_dir.path().join("packs/test/1.0.0");
        fs::create_dir_all(&pack_dir).unwrap();

        let manifest_path = pack_dir.join("app-pack.yaml");
        let manifest_content = r#"
apiVersion: demon.io/v1
kind: AppPack
metadata:
  name: test-pack
  version: 1.0.0
ui:
  cards:
    - id: card-a
      kind: result-envelope
      match:
        rituals: ["ritual-a"]
    - id: card-b
      kind: result-envelope
      match:
        rituals: ["ritual-b"]
"#;
        fs::write(&manifest_path, manifest_content).unwrap();

        let registry_content = serde_json::json!({
            "apps": {
                "test-pack": [{
                    "version": "1.0.0",
                    "manifest_path": manifest_path.to_str().unwrap()
                }]
            }
        });
        fs::write(&registry_path, registry_content.to_string()).unwrap();

        let registry = AppPackRegistry::load_from_path(&registry_path).unwrap();
        let cards_a = registry.get_cards_for_ritual("ritual-a");
        let cards_b = registry.get_cards_for_ritual("ritual-b");

        assert_eq!(cards_a.len(), 1);
        assert_eq!(cards_a[0].id, "card-a");

        assert_eq!(cards_b.len(), 1);
        assert_eq!(cards_b[0].id, "card-b");
    }
}
