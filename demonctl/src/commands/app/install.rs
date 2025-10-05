use super::manifest;
use super::registry::Registry;
use super::{copy_to_store, ensure_relative_path, packs_dir, registry_path, MANIFEST_BASENAMES};
use anyhow::{bail, Context, Result};
use clap::Args;
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
    /// Allow installation when signing configuration is present but verification is unavailable
    #[arg(long, action)]
    pub allow_unsigned: bool,
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

    if manifest
        .signing
        .as_ref()
        .and_then(|s| s.cosign.as_ref())
        .is_some()
        && !args.allow_unsigned
    {
        bail!(
            "Manifest declares signing.cosign; signature verification is not yet implemented. Re-run with --allow-unsigned to bypass temporarily."
        );
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

    let dest_manifest = install_root.join(
        resolved
            .manifest_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("app-pack.yaml")),
    );
    copy_to_store(&resolved.manifest_path, &dest_manifest)?;

    for contract in &manifest.contracts {
        let relative = Path::new(&contract.path);
        ensure_relative_path(relative)?;
        let source = resolved.root.join(relative);
        if !source.exists() {
            bail!(
                "Contract '{}' referenced by App Pack '{}' not found at '{}'",
                contract.id,
                manifest.name(),
                source.display()
            );
        }
        let destination = install_root.join(relative);
        copy_to_store(&source, &destination)?;
    }

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
    fn source_display(&self) -> String {
        self.source.clone()
    }
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
