pub mod install;
pub mod list;
pub mod uninstall;

pub mod alias;
mod manifest;
mod registry;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use std::path::{Path, PathBuf};

const APP_PACK_DIR: &str = "app-packs";
pub(crate) const MANIFEST_BASENAMES: [&str; 2] = ["app-pack.yaml", "app-pack.yml"];

#[derive(Subcommand, Debug)]
pub enum AppCommand {
    /// Install an App Pack bundle
    Install(install::InstallArgs),
    /// Uninstall an App Pack bundle
    Uninstall(uninstall::UninstallArgs),
    /// List installed App Packs
    List(list::ListArgs),
}

pub fn handle(cmd: AppCommand) -> Result<()> {
    match cmd {
        AppCommand::Install(args) => install::run(args),
        AppCommand::Uninstall(args) => uninstall::run(args),
        AppCommand::List(args) => list::run(args),
    }
}

pub(crate) fn resolve_store_root() -> Result<PathBuf> {
    let base = if let Ok(dir) = std::env::var("DEMON_APP_HOME") {
        PathBuf::from(dir)
    } else if let Ok(dir) = std::env::var("DEMON_HOME") {
        PathBuf::from(dir).join(APP_PACK_DIR)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".demon").join(APP_PACK_DIR)
    } else {
        bail!("Unable to determine Demon home. Set HOME or DEMON_APP_HOME.");
    };

    Ok(base)
}

pub(crate) fn ensure_relative_path(path: &Path) -> Result<()> {
    if path.is_absolute() {
        bail!("Bundle path '{}' must be relative", path.display());
    }

    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            bail!("Bundle path '{}' cannot contain '..'", path.display());
        }
    }

    Ok(())
}

pub(crate) fn registry_path() -> Result<PathBuf> {
    let root = resolve_store_root()?;
    Ok(registry::registry_path(&root))
}

pub(crate) fn packs_dir() -> Result<PathBuf> {
    Ok(resolve_store_root()?.join("packs"))
}

pub(crate) fn copy_to_store(src: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory '{}'", parent.display()))?;
    }
    std::fs::copy(src, dest)
        .with_context(|| format!("Failed to copy '{}' to '{}'", src.display(), dest.display()))?;
    Ok(())
}
