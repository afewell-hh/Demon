use anyhow::{Context, Result};
use jsonschema::{Draft, JSONSchema};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct LibraryIndex {
    pub provider: String,
    pub bundles: Vec<LibraryBundle>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LibraryBundle {
    pub name: String,
    pub version: String,
    pub path: String,
    pub digest: Digest,
    pub sig: Signature,
    #[serde(rename = "pubKeyId")]
    pub pub_key_id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Digest {
    pub sha256: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Signature {
    pub ed25519: String,
}

pub fn load_index(path: &Path) -> Result<LibraryIndex> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read index: {}", path.display()))?;
    let idx: LibraryIndex = serde_json::from_str(&text).context("parse index.json")?;
    validate_index_schema(&text)?;
    Ok(idx)
}

fn validate_index_schema(text: &str) -> Result<()> {
    let mut schema_path =
        Path::new("contracts/schemas/bootstrap.library.index.v0.json").to_path_buf();
    if !schema_path.exists() {
        for prefix in ["..", "../..", "../../.."].iter() {
            let p = Path::new(prefix).join("contracts/schemas/bootstrap.library.index.v0.json");
            if p.exists() {
                schema_path = p;
                break;
            }
        }
    }
    let schema_text = std::fs::read_to_string(&schema_path)?;
    let schema_json: Value = serde_json::from_str(&schema_text)?;
    // Extend schema lifetime for the validator
    let boxed = Box::new(schema_json);
    let leaked: &'static Value = Box::leak(boxed);
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft7)
        .compile(leaked)?;
    let doc_json: Value = serde_json::from_str(text)?;
    if let Err(errs) = compiled.validate(&doc_json) {
        let mut msg = String::from("library index schema errors:\n");
        for e in errs {
            msg.push_str(&format!("- {}\n", e));
        }
        anyhow::bail!(msg);
    }
    Ok(())
}

#[derive(Debug)]
pub struct ResolvedBundle {
    pub provider: String,
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub digest_sha256: String,
    pub sig_ed25519: String,
    pub pub_key_id: String,
}

pub fn resolve_local(uri: &str, index_path: &Path) -> Result<ResolvedBundle> {
    // uri format: lib://local/<name>@<version>
    let without = uri
        .strip_prefix("lib://local/")
        .ok_or_else(|| anyhow::anyhow!("unsupported uri: {}", uri))?;
    let mut parts = without.split('@');
    let name = parts.next().unwrap_or("");
    let version = parts.next().unwrap_or("");
    if name.is_empty() || version.is_empty() {
        anyhow::bail!("invalid bundle uri: {}", uri);
    }
    let idx = load_index(index_path)?;
    if idx.provider != "local" {
        anyhow::bail!("unsupported provider: {}", idx.provider);
    }
    let b = idx
        .bundles
        .into_iter()
        .find(|b| b.name == name && b.version == version)
        .ok_or_else(|| anyhow::anyhow!("bundle not found: {}@{}", name, version))?;
    let mut pathbuf = PathBuf::from(b.path);
    if !pathbuf.exists() {
        for prefix in [".", "..", "../..", "../../.."].iter() {
            let p = Path::new(prefix).join(&pathbuf);
            if p.exists() {
                pathbuf = p;
                break;
            }
        }
    }
    if !pathbuf.exists() {
        anyhow::bail!("bundle file not found: {}", pathbuf.display());
    }
    // Canonicalize to absolute path for logging determinism
    let pathbuf = std::fs::canonicalize(&pathbuf).unwrap_or(pathbuf);
    Ok(ResolvedBundle {
        provider: "local".into(),
        name: b.name,
        version: b.version,
        path: pathbuf,
        digest_sha256: b.digest.sha256,
        sig_ed25519: b.sig.ed25519,
        pub_key_id: b.pub_key_id,
    })
}
