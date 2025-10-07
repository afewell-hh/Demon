use super::registry::Registry;
use super::{packs_dir, registry_path};
use anyhow::{Context, Result};
use clap::Args;
use std::fs;

#[derive(Args, Debug)]
pub struct UninstallArgs {
    /// App Pack name to uninstall
    pub name: String,
    /// Specific version to uninstall (all versions are removed if omitted)
    #[arg(long)]
    pub version: Option<String>,
    /// Retain stored bundle files on disk
    #[arg(long, action)]
    pub retain_files: bool,
}

pub fn run(args: UninstallArgs) -> Result<()> {
    let registry_path = registry_path()?;
    let mut registry = Registry::load(registry_path.clone())?;
    let removed = registry.remove(&args.name, args.version.as_deref())?;
    registry.persist()?;

    if !args.retain_files {
        let packs_root = packs_dir()?;
        for pack in &removed {
            let path = packs_root.join(&args.name).join(&pack.version);
            if path.exists() {
                fs::remove_dir_all(&path).with_context(|| {
                    format!(
                        "Failed to remove installation directory '{}'",
                        path.display()
                    )
                })?;
            }
        }

        // Clean up name directory if now empty
        let name_dir = packs_root.join(&args.name);
        if name_dir.exists() && name_dir.read_dir()?.next().is_none() {
            fs::remove_dir(name_dir)?;
        }
    }

    for pack in removed {
        println!("Uninstalled {}@{}", args.name, pack.version);
    }

    Ok(())
}
