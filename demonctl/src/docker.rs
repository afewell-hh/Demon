use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use zip::ZipArchive;

use crate::github::{GithubActionsClient, WorkflowRun};

const DIGEST_ARTIFACT_NAME: &str = "docker-image-digests";
const MANIFEST_FILE_NAME: &str = "docker-image-digests.json";

pub const REQUIRED_COMPONENTS: [(&str, &str); 3] = [
    ("operate-ui", "OPERATE_UI_IMAGE_TAG"),
    ("runtime", "RUNTIME_IMAGE_TAG"),
    ("engine", "ENGINE_IMAGE_TAG"),
];

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManifestEntry {
    pub repository: String,
    pub digest: String,
    pub image: String,
    #[serde(rename = "gitShaTag")]
    pub git_sha_tag: Option<String>,
}

pub type Manifest = HashMap<String, ManifestEntry>;

pub struct FetchManifestResult {
    pub manifest: Manifest,
    pub workflow_run: WorkflowRun,
}

pub struct DockerDigestClient {
    github: GithubActionsClient,
}

impl DockerDigestClient {
    pub fn new(token: String) -> Result<Self> {
        Self::new_with_overrides(token, None, None)
    }

    pub fn new_with_overrides(
        token: String,
        repo_override: Option<&str>,
        api_url_override: Option<&str>,
    ) -> Result<Self> {
        let github = GithubActionsClient::new(token, repo_override, api_url_override)?;
        Ok(Self { github })
    }

    pub async fn fetch_manifest(
        &self,
        workflow: &str,
        branch: &str,
    ) -> Result<FetchManifestResult> {
        let workflow_run = self.github.latest_successful_run(workflow, branch).await?;
        let artifact = self
            .github
            .find_artifact(workflow_run.id, DIGEST_ARTIFACT_NAME)
            .await?;
        let archive_bytes = self.github.download_artifact(artifact.id).await?;
        let manifest = extract_manifest(&archive_bytes)?;
        validate_manifest(&manifest)?;

        Ok(FetchManifestResult {
            manifest,
            workflow_run,
        })
    }
}

fn extract_manifest(bytes: &[u8]) -> Result<Manifest> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut archive = ZipArchive::new(cursor).context("Failed to open artifact zip archive")?;
    let mut file = archive
        .by_name(MANIFEST_FILE_NAME)
        .context("docker-image-digests.json not found in artifact")?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .context("Failed to read digest manifest from artifact")?;

    let manifest: Manifest = serde_json::from_str(&contents)
        .context("Failed to parse docker image digest manifest JSON")?;

    Ok(manifest)
}

fn validate_manifest(manifest: &Manifest) -> Result<()> {
    for (component, _) in REQUIRED_COMPONENTS {
        let entry = manifest
            .get(component)
            .with_context(|| format!("Manifest is missing required component '{}'.", component))?;

        if !entry.digest.starts_with("sha256:") {
            anyhow::bail!(
                "Manifest entry for '{}' does not contain a sha256 digest (found '{}')",
                component,
                entry.digest
            );
        }

        if !entry.image.contains("@sha256:") {
            anyhow::bail!(
                "Manifest entry for '{}' does not reference the image by digest (found '{}')",
                component,
                entry.image
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_manifest_checks_required_components() {
        let mut manifest = Manifest::new();
        manifest.insert(
            "operate-ui".to_string(),
            ManifestEntry {
                repository: "ghcr.io/acme/operate-ui".to_string(),
                digest: "sha256:abc".to_string(),
                image: "ghcr.io/acme/operate-ui@sha256:abc".to_string(),
                git_sha_tag: Some("sha-123".to_string()),
            },
        );
        manifest.insert(
            "runtime".to_string(),
            ManifestEntry {
                repository: "ghcr.io/acme/runtime".to_string(),
                digest: "sha256:def".to_string(),
                image: "ghcr.io/acme/runtime@sha256:def".to_string(),
                git_sha_tag: Some("sha-123".to_string()),
            },
        );
        manifest.insert(
            "engine".to_string(),
            ManifestEntry {
                repository: "ghcr.io/acme/engine".to_string(),
                digest: "sha256:ghi".to_string(),
                image: "ghcr.io/acme/engine@sha256:ghi".to_string(),
                git_sha_tag: Some("sha-123".to_string()),
            },
        );

        validate_manifest(&manifest).unwrap();
    }

    #[test]
    fn validate_manifest_rejects_missing_component() {
        let manifest = Manifest::new();
        let err = validate_manifest(&manifest).unwrap_err();
        assert!(err.to_string().contains("operate-ui"));
    }

    #[test]
    fn validate_manifest_rejects_non_digest_image_tag() {
        let mut manifest = Manifest::new();
        manifest.insert(
            "operate-ui".to_string(),
            ManifestEntry {
                repository: "ghcr.io/acme/operate-ui".to_string(),
                digest: "sha256:abc".to_string(),
                image: "ghcr.io/acme/operate-ui:main".to_string(),
                git_sha_tag: Some("sha-123".to_string()),
            },
        );
        manifest.insert(
            "runtime".to_string(),
            ManifestEntry {
                repository: "ghcr.io/acme/runtime".to_string(),
                digest: "sha256:def".to_string(),
                image: "ghcr.io/acme/runtime@sha256:def".to_string(),
                git_sha_tag: Some("sha-123".to_string()),
            },
        );
        manifest.insert(
            "engine".to_string(),
            ManifestEntry {
                repository: "ghcr.io/acme/engine".to_string(),
                digest: "sha256:ghi".to_string(),
                image: "ghcr.io/acme/engine@sha256:ghi".to_string(),
                git_sha_tag: Some("sha-123".to_string()),
            },
        );

        let err = validate_manifest(&manifest).unwrap_err();
        assert!(err
            .to_string()
            .contains("Manifest entry for 'operate-ui' does not reference the image by digest"));
    }
}
