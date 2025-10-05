use super::registry::Registry;
use super::registry_path;
use anyhow::Result;
use clap::Args;

#[derive(Args, Debug, Default)]
pub struct ListArgs {
    /// Output machine-readable JSON
    #[arg(long, action)]
    pub json: bool,
}

pub fn run(args: ListArgs) -> Result<()> {
    let registry_path = registry_path()?;
    let registry = Registry::load(registry_path)?;
    let entries = registry.list();

    if args.json {
        let payload: Vec<_> = entries
            .iter()
            .map(|(name, pack)| {
                serde_json::json!({
                    "name": name,
                    "version": pack.version,
                    "installedAt": pack.installed_at,
                    "manifestPath": pack.manifest_path,
                    "source": pack.source,
                    "schemaRange": pack.schema_range,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    if entries.is_empty() {
        println!("No App Packs installed.");
        return Ok(());
    }

    println!(
        "{:<24} {:<12} {:<25} {}",
        "NAME", "VERSION", "INSTALLED", "SOURCE"
    );
    for (name, pack) in entries {
        println!(
            "{:<24} {:<12} {:<25} {}",
            name,
            pack.version,
            pack.installed_at.to_rfc3339(),
            pack.source
        );
    }

    Ok(())
}
