use anyhow::Result;
use std::env;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;
    let addr = ([0, 0, 0, 0], port).into();

    info!("Starting Demon Runtime REST API on {}", addr);

    // Start the REST API server
    runtime::server::serve(addr).await?;

    Ok(())
}
