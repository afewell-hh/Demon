use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser)]
#[command(name = "demonctl", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a ritual from a YAML file
    Run {
        /// Path to ritual YAML
        #[arg(value_name = "FILE")]
        file: String,
        #[arg(long, default_value = "false")]
        replay: bool,
        /// Enable JetStream persistence (requires NATS to be running)
        #[arg(long)]
        jetstream: bool,
    },
    /// Print version and exit
    Version,
}

fn init_tracing() {
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .try_init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    match cli.cmd {
        Commands::Run { file, replay: _, jetstream } => {
            let engine = if jetstream {
                let nats_url = std::env::var("NATS_URL")
                    .unwrap_or_else(|_| "nats://localhost:4222".to_string());
                match engine::rituals::Engine::with_event_log(&nats_url).await {
                    Ok(e) => {
                        tracing::info!("Connected to JetStream at {}", nats_url);
                        e
                    },
                    Err(e) => {
                        eprintln!("Failed to connect to JetStream: {:?}", e);
                        eprintln!("Falling back to stdout mode");
                        engine::rituals::Engine::new()
                    }
                }
            } else {
                engine::rituals::Engine::new()
            };

            if let Err(e) = engine.run_from_file(&file).await {
                eprintln!("Error running ritual: {:?}", e);
                std::process::exit(1);
            }
        }
        Commands::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
    }
    Ok(())
}
// no-op: exercise replies guard
