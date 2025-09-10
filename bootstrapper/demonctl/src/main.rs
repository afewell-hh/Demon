use anyhow::Result;
use bootstrapper_demonctl::{ensure_stream, seed_preview_min, verify_ui, BootstrapConfig, Profile};
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
    let cfg = BootstrapConfig {
        profile: cli.profile.into(),
        ..Default::default()
    };
    // Log deprecation if DEMON_RITUAL_EVENTS is set and RITUAL_STREAM_NAME is not
    if std::env::var("RITUAL_STREAM_NAME").is_err() && std::env::var("DEMON_RITUAL_EVENTS").is_ok()
    {
        tracing::warn!("[deprecation] using DEMON_RITUAL_EVENTS; set RITUAL_STREAM_NAME instead");
    }
    info!(?cfg, "bootstrap: effective_config");

    if !(cli.ensure_stream || cli.seed || cli.verify) {
        // default: run all
        run_all(&cfg, &cli.ritual_id).await
    } else {
        run_some(&cfg, &cli).await
    }
}

async fn run_all(cfg: &BootstrapConfig, ritual: &str) -> Result<()> {
    let stream = ensure_stream(cfg).await?;
    info!(name=%stream.cached_info().config.name, "ensure_stream: ok");
    let client = async_nats::connect(&cfg.nats_url).await?;
    let js = async_nats::jetstream::new(client);
    seed_preview_min(&js, ritual, &cfg.ui_url).await?;
    info!("seed: ok");
    verify_ui(&cfg.ui_url).await?;
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
        verify_ui(&cfg.ui_url).await?;
        info!("verify: ok");
    }
    info!("done");
    Ok(())
}
