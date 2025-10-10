use super::manifest::{self, CosignSettings};
use super::registry::Registry;
use super::{ensure_relative_path, packs_dir, registry_path, MANIFEST_BASENAMES};
use anyhow::{anyhow, bail, ensure, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use clap::Args;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use sigstore::crypto::{CosignVerificationKey, Signature as SigstoreSignature};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
pub struct InstallArgs {
    /// Path or URI to the App Pack bundle (directory or manifest file)
    #[arg(value_name = "PACK")]
    pub pack: String,
    /// Replace an existing installation of the same name@version
    #[arg(long, action)]
    pub overwrite: bool,
}

pub fn run(args: InstallArgs) -> Result<()> {
    let resolved = resolve_pack_source(&args.pack)?;
    let manifest_raw = fs::read_to_string(&resolved.manifest_path).with_context(|| {
        format!(
            "Failed to read manifest '{}'",
            resolved.manifest_path.display()
        )
    })?;

    let manifest = manifest::parse_manifest(&manifest_raw)?;

    if let Some(settings) = manifest.cosign_settings()? {
        verify_cosign_signature(&resolved, manifest.name(), &manifest_raw, settings)?;
    }

    let packs_root = packs_dir()?;
    let install_root = packs_root.join(manifest.name()).join(manifest.version());

    if install_root.exists() {
        if args.overwrite {
            fs::remove_dir_all(&install_root).with_context(|| {
                format!(
                    "Failed to remove existing installation at '{}'",
                    install_root.display()
                )
            })?;
        } else {
            bail!(
                "App Pack '{}@{}' is already installed. Use --overwrite to replace it.",
                manifest.name(),
                manifest.version()
            );
        }
    }

    fs::create_dir_all(&install_root).with_context(|| {
        format!(
            "Failed to create installation directory '{}'",
            install_root.display()
        )
    })?;

    // Copy the entire App Pack directory to the install location so capsules/
    // scripts and other runtime assets are available under /workspace.
    // This preserves directory structure and best-effort file permissions.
    // We still verify referenced contracts exist to surface clear errors.

    fn copy_tree(src_root: &Path, dest_root: &Path) -> Result<()> {
        for entry in walkdir::WalkDir::new(src_root).follow_links(false) {
            let entry = entry?;
            let rel = entry.path().strip_prefix(src_root).unwrap();
            if rel.as_os_str().is_empty() {
                continue;
            }
            // Skip VCS metadata
            if rel.components().any(|c| c.as_os_str() == ".git") {
                continue;
            }
            let dest_path = dest_root.join(rel);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&dest_path).with_context(|| {
                    format!("Failed to create directory '{}'", dest_path.display())
                })?;
            } else if entry.file_type().is_file() {
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!("Failed to create directory '{}'", parent.display())
                    })?;
                }
                std::fs::copy(entry.path(), &dest_path).with_context(|| {
                    format!(
                        "Failed to copy '{}' to '{}'",
                        entry.path().display(),
                        dest_path.display()
                    )
                })?;
                // Best effort: carry over executable bit when present
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = std::fs::metadata(entry.path()) {
                        let mode = meta.permissions().mode();
                        let _ = std::fs::set_permissions(
                            &dest_path,
                            std::fs::Permissions::from_mode(mode),
                        );
                    }
                }
            }
        }
        Ok(())
    }

    copy_tree(&resolved.root, &install_root)?;

    // Verify contracts referenced by the manifest are present in the install.
    for contract in &manifest.contracts {
        let relative = Path::new(&contract.path);
        ensure_relative_path(relative)?;
        let destination = install_root.join(relative);
        ensure!(
            destination.exists(),
            "Contract '{}' referenced by App Pack '{}' not found at '{}' after install",
            contract.id,
            manifest.name(),
            destination.display()
        );
    }

    // Resolve stored manifest path (post-copy) for registry entry
    let dest_manifest = MANIFEST_BASENAMES
        .iter()
        .map(|b| install_root.join(b))
        .find(|p| p.exists())
        .unwrap_or_else(|| install_root.join("app-pack.yaml"));

    let registry_path = registry_path()?;
    let mut registry = Registry::load(registry_path.clone())?;

    let manifest_store_path = fs::canonicalize(&dest_manifest).with_context(|| {
        format!(
            "Failed to canonicalize stored manifest path '{}'",
            dest_manifest.display()
        )
    })?;

    registry.register_install(
        &manifest,
        manifest_store_path,
        resolved.source_display(),
        args.overwrite,
    )?;
    registry.persist()?;

    println!(
        "Installed App Pack {}@{}",
        manifest.name(),
        manifest.version()
    );

    Ok(())
}

struct ResolvedPack {
    root: PathBuf,
    manifest_path: PathBuf,
    source: String,
}

impl ResolvedPack {
    fn root(&self) -> &Path {
        &self.root
    }

    fn source_display(&self) -> String {
        self.source.clone()
    }
}

fn verify_cosign_signature(
    resolved: &ResolvedPack,
    manifest_name: &str,
    manifest_raw: &str,
    settings: &CosignSettings,
) -> Result<()> {
    let signature_rel = settings
        .signature_path()
        .ok_or_else(|| anyhow!("signing.cosign.signaturePath must be provided"))?;
    let public_key_rel = settings
        .public_key_path()
        .ok_or_else(|| anyhow!("signing.cosign.publicKeyPath must be provided"))?;
    let hash = settings
        .public_key_hash()
        .ok_or_else(|| anyhow!("signing.cosign.publicKeyHash must be provided"))?;

    ensure_relative_path(Path::new(signature_rel))?;
    ensure_relative_path(Path::new(public_key_rel))?;

    let signature_path = resolved.root().join(signature_rel);
    let public_key_path = resolved.root().join(public_key_rel);

    ensure!(
        signature_path.exists(),
        "Cosign signature not found at {}",
        signature_rel
    );
    ensure!(
        public_key_path.exists(),
        "Cosign public key not found at {}",
        public_key_rel
    );

    let key_pem = fs::read_to_string(&public_key_path).with_context(|| {
        format!(
            "Failed to read public key at '{}'",
            public_key_path.display()
        )
    })?;

    let expected_hash = hash.value();
    let computed_hash = match hash.algorithm().to_ascii_lowercase().as_str() {
        "sha256" => hex::encode(Sha256::digest(key_pem.as_bytes())),
        other => bail!("Unsupported public key hash algorithm '{}'.", other),
    };

    ensure!(
        expected_hash.eq_ignore_ascii_case(&computed_hash),
        "Public key hash mismatch for {}",
        public_key_rel
    );

    let parsed_signature = load_cosign_signature(&signature_path)?;

    if let (Some(algo), Some(expected_digest)) = (
        parsed_signature.hash_algorithm.as_deref(),
        parsed_signature.hash_value.as_deref(),
    ) {
        let manifest_digest = compute_digest(algo, manifest_raw.as_bytes())?;
        ensure!(
            expected_digest.eq_ignore_ascii_case(&manifest_digest),
            "Cosign bundle hash mismatch for {}",
            signature_rel
        );
    }

    let verification_key = CosignVerificationKey::try_from_pem(key_pem.as_bytes())
        .context("Cosign public key must be a supported key type")?;

    verification_key
        .verify_signature(
            SigstoreSignature::Base64Encoded(parsed_signature.signature_b64.as_bytes()),
            manifest_raw.as_bytes(),
        )
        .with_context(|| format!("Cosign signature verification failed for '{manifest_name}'"))?;

    Ok(())
}

fn compute_digest(algorithm: &str, data: &[u8]) -> Result<String> {
    match algorithm.to_ascii_lowercase().as_str() {
        "sha256" => Ok(hex::encode(Sha256::digest(data))),
        other => bail!("Unsupported cosign payload hash algorithm '{}'.", other),
    }
}

struct ParsedSignature {
    signature_b64: String,
    hash_algorithm: Option<String>,
    hash_value: Option<String>,
}

fn load_cosign_signature(path: &Path) -> Result<ParsedSignature> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("Failed to read signature at '{}'", path.display()))?;
    let trimmed = contents.trim();

    if trimmed.is_empty() {
        bail!("Cosign signature file at '{}' is empty", path.display());
    }

    if let Ok(json) = serde_json::from_str::<JsonValue>(trimmed) {
        let (signature_b64, hash_algorithm, hash_value) =
            extract_bundle_signature(&json).context("Failed to interpret cosign bundle")?;
        Ok(ParsedSignature {
            signature_b64,
            hash_algorithm,
            hash_value,
        })
    } else {
        Ok(ParsedSignature {
            signature_b64: trimmed.to_string(),
            hash_algorithm: None,
            hash_value: None,
        })
    }
}

fn extract_bundle_signature(json: &JsonValue) -> Result<(String, Option<String>, Option<String>)> {
    let payload = json
        .get("payload")
        .or_else(|| json.get("Payload"))
        .ok_or_else(|| anyhow!("Cosign bundle missing payload section"))?;

    let body_b64 = payload
        .get("body")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow!("Cosign bundle payload missing body"))?;

    let body_bytes = BASE64
        .decode(body_b64.trim())
        .context("Failed to decode cosign bundle payload body as base64")?;

    let body_json: JsonValue = serde_json::from_slice(&body_bytes)
        .context("Failed to parse cosign bundle payload body as JSON")?;

    let spec = body_json
        .get("spec")
        .ok_or_else(|| anyhow!("Cosign bundle payload missing spec section"))?;

    let signature_b64 = spec
        .get("signature")
        .and_then(|sig| sig.get("content"))
        .and_then(JsonValue::as_str)
        .or_else(|| json.get("signature").and_then(JsonValue::as_str))
        .ok_or_else(|| anyhow!("Cosign bundle missing signature content"))?;

    let hash = spec.get("data").and_then(|data| data.get("hash"));

    let hash_algorithm = hash
        .and_then(|h| h.get("algorithm"))
        .and_then(JsonValue::as_str)
        .map(|s| s.to_string());

    let hash_value = hash
        .and_then(|h| h.get("value"))
        .and_then(JsonValue::as_str)
        .map(|s| s.to_string());

    Ok((signature_b64.trim().to_string(), hash_algorithm, hash_value))
}

fn resolve_pack_source(input: &str) -> Result<ResolvedPack> {
    if input.starts_with("http://") || input.starts_with("https://") || input.starts_with("oci://")
    {
        bail!("Remote App Pack URIs are not yet supported");
    }

    let path = Path::new(input);
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("Failed to resolve path '{}'", path.display()))?;

    if canonical.is_file() {
        resolve_from_file(canonical, input.to_string())
    } else if canonical.is_dir() {
        resolve_from_directory(canonical, input.to_string())
    } else {
        bail!(
            "App Pack source '{}' is neither a file nor a directory",
            input
        );
    }
}

fn resolve_from_file(path: PathBuf, source: String) -> Result<ResolvedPack> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if ext == "yaml" || ext == "yml" {
        let root = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ok(ResolvedPack {
            root,
            manifest_path: path,
            source,
        })
    } else {
        bail!(
            "Unsupported App Pack file '{}'. Expected a .yaml/.yml manifest",
            path.display()
        );
    }
}

fn resolve_from_directory(dir: PathBuf, source: String) -> Result<ResolvedPack> {
    for candidate in MANIFEST_BASENAMES {
        let candidate_path = dir.join(candidate);
        if candidate_path.exists() {
            return Ok(ResolvedPack {
                root: dir,
                manifest_path: candidate_path,
                source,
            });
        }
    }

    bail!(
        "No manifest found in '{}'. Expected one of: {}",
        dir.display(),
        MANIFEST_BASENAMES.join(", ")
    );
}
