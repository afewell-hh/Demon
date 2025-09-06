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

fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    match cli.cmd {
        Commands::Run { file, replay: _ } => {
            let engine = engine::rituals::Engine::new();
            if let Err(e) = engine.run_from_file(&file) {
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
