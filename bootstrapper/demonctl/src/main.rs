use anyhow::Result;
use bootstrapper_demonctl::libindex::resolve;
use bootstrapper_demonctl::provenance::verify_provenance;
use bootstrapper_demonctl::{
    bundle::load_bundle, ensure_stream, seed_from_bundle, seed_preview_min, verify_ui_with_token,
    BootstrapConfig, Profile,
};
use clap::{ArgAction, Parser, ValueEnum};
use tracing::{info, Level};

#[derive(Parser, Debug)]
#[command(
    name = "demonctl-bootstrap",
    about = "Bootstrap Demon prerequisites (self-host v0)"
)]
struct Cli {
    /// Profile: local-dev (default) or remote-nats
    #[arg(long, value_enum, default_value_t = ProfileArg::LocalDev)]
    profile: ProfileArg,

    /// Ensure stream exists
    #[arg(long, action = ArgAction::SetTrue)]
    ensure_stream: bool,
    /// Seed minimal preview events
    #[arg(long, action = ArgAction::SetTrue)]
    seed: bool,
    /// Verify Operate UI readiness
    #[arg(long, action = ArgAction::SetTrue)]
    verify: bool,

    /// Ritual id used for seeding (default: preview)
    #[arg(long, default_value = "preview")]
    ritual_id: String,

    /// Optional bundle file (YAML)
    #[arg(long)]
    bundle: Option<String>,

    /// Optional overrides (flags > bundle > env)
    #[arg(long)]
    nats_url: Option<String>,
    #[arg(long)]
    stream_name: Option<String>,
    #[arg(long)]
    ui_base_url: Option<String>,

    /// Verify only (resolve + provenance check; no NATS/seed/verify-UI phases)
    #[arg(long, action = ArgAction::SetTrue)]
    verify_only: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ProfileArg {
    #[value(name = "local-dev")]
    LocalDev,
    #[value(name = "remote-nats")]
    RemoteNats,
}

impl From<ProfileArg> for Profile {
    fn from(p: ProfileArg) -> Self {
        match p {
            ProfileArg::LocalDev => Profile::LocalDev,
            ProfileArg::RemoteNats => Profile::RemoteNats,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let _cfg = BootstrapConfig {
        profile: cli.profile.into(),
        ..Default::default()
    };
    // Log deprecation if DEMON_RITUAL_EVENTS is set and RITUAL_STREAM_NAME is not
    if std::env::var("RITUAL_STREAM_NAME").is_err() && std::env::var("DEMON_RITUAL_EVENTS").is_ok()
    {
        tracing::warn!("[deprecation] using DEMON_RITUAL_EVENTS; set RITUAL_STREAM_NAME instead");
    }
    // Don't pass lib:// URIs to compute_effective_config - they need resolution first
    let bundle_for_config = cli
        .bundle
        .as_deref()
        .filter(|uri| !uri.starts_with("lib://"));
    let (cfg, provenance) = bootstrapper_demonctl::compute_effective_config(
        bundle_for_config.map(std::path::Path::new),
        cli.nats_url.as_deref(),
        cli.stream_name.as_deref(),
        None, // subjects - not in CLI yet
        cli.ui_base_url.as_deref(),
    )?;

    println!(
        "{}",
        serde_json::json!({
            "phase":"config",
            "effective":{
                "nats_url": cfg.nats_url,
                "stream_name": cfg.stream_name,
                "subjects": cfg.subjects,
                "dedupe": cfg.dedupe_window_secs,
                "ui_url": cfg.ui_url,
            },
            "provenance": provenance
        })
    );

    if cli.verify_only {
        if let Some(uri) = cli.bundle.as_deref() {
            if uri.starts_with("lib://") {
                let mut idx_path = std::path::PathBuf::from("bootstrapper/library/index.json");
                if !idx_path.exists() {
                    for prefix in ["..", "../..", "../../.."].iter() {
                        let p =
                            std::path::Path::new(prefix).join("bootstrapper/library/index.json");
                        if p.exists() {
                            idx_path = p;
                            break;
                        }
                    }
                }
                let resolved = tokio::task::block_in_place(|| resolve(uri, &idx_path))?;
                println!(
                    "{}",
                    serde_json::json!({
                        "phase":"resolve",
                        "uri": uri,
                        "provider": resolved.provider,
                        "name": resolved.name,
                        "version": resolved.version,
                        "path": resolved.path
                    })
                );
                let vr = verify_provenance(
                    &resolved.path,
                    &resolved.pub_key_id,
                    &resolved.digest_sha256,
                    &resolved.sig_ed25519,
                )?;
                if vr.signature_ok {
                    println!(
                        "{}",
                        serde_json::json!({
                            "phase":"verify",
                            "bundle": {"name": resolved.name, "version": resolved.version},
                            "digest": vr.digest_hex,
                            "signature": "ok",
                            "pubKeyId": resolved.pub_key_id
                        })
                    );
                } else {
                    println!(
                        "{}",
                        serde_json::json!({
                            "phase":"verify",
                            "bundle": {"name": resolved.name, "version": resolved.version},
                            "digest": vr.digest_hex,
                            "signature": "failed",
                            "reason": vr.reason.unwrap_or_else(|| "unknown".to_string()),
                            "pubKeyId": resolved.pub_key_id
                        })
                    );
                    anyhow::bail!("signature verification failed");
                }
                return Ok(());
            }
        }
        anyhow::bail!("--verify-only requires --bundle lib://... URI");
    }

    if !(cli.ensure_stream || cli.seed || cli.verify) {
        // default: run all
        run_all(&cfg, &cli.ritual_id, cli.bundle.as_deref()).await
    } else {
        run_some(&cfg, &cli).await
    }
}

async fn run_all(cfg: &BootstrapConfig, ritual: &str, bundle_uri: Option<&str>) -> Result<()> {
    let stream = ensure_stream(cfg).await?;
    info!(name=%stream.cached_info().config.name, "ensure_stream: ok");
    let client = async_nats::connect(&cfg.nats_url).await?;
    let js = async_nats::jetstream::new(client);
    if let Some(uri) = bundle_uri {
        // Resolve the URI if it's a lib:// URI
        let bundle_path = if uri.starts_with("lib://") {
            let mut idx_path = std::path::PathBuf::from("bootstrapper/library/index.json");
            if !idx_path.exists() {
                for prefix in ["..", "../..", "../../.."].iter() {
                    let p = std::path::Path::new(prefix).join("bootstrapper/library/index.json");
                    if p.exists() {
                        idx_path = p;
                        break;
                    }
                }
            }
            let resolved = tokio::task::block_in_place(|| resolve(uri, &idx_path))?;
            resolved.path
        } else {
            std::path::PathBuf::from(uri)
        };
        let b = load_bundle(&bundle_path)?;
        let b_json = serde_json::to_value(&b)?;
        seed_from_bundle(&js, &b_json, &cfg.stream_name, &cfg.ui_url).await?;
        let token = b
            .operate_ui
            .admin_token
            .or_else(|| std::env::var("ADMIN_TOKEN").ok());
        verify_ui_with_token(&cfg.ui_url, token.as_deref()).await?;
    } else {
        seed_preview_min(&js, ritual, &cfg.ui_url).await?;
        verify_ui_with_token(&cfg.ui_url, std::env::var("ADMIN_TOKEN").ok().as_deref()).await?;
    }
    info!("seed: ok");
    info!("verify: ok");
    info!("done: all checks passed");
    Ok(())
}

async fn run_some(cfg: &BootstrapConfig, cli: &Cli) -> Result<()> {
    if cli.ensure_stream {
        let stream = ensure_stream(cfg).await?;
        info!(name=%stream.cached_info().config.name, "ensure_stream: ok");
    }
    if cli.seed {
        let client = async_nats::connect(&cfg.nats_url).await?;
        let js = async_nats::jetstream::new(client);
        seed_preview_min(&js, &cli.ritual_id, &cfg.ui_url).await?;
        info!("seed: ok");
    }
    if cli.verify {
        verify_ui_with_token(&cfg.ui_url, std::env::var("ADMIN_TOKEN").ok().as_deref()).await?;
        info!("verify: ok");
    }
    info!("done");
    Ok(())
}
