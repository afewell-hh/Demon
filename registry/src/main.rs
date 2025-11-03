//! Schema Registry Service - Main Entry Point
//!
//! Provides REST API for contract schema management with JetStream KV backend.

use anyhow::Result;
use registry::{create_app, AppState};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured JSON logging
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,registry=debug")),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    info!("Starting Schema Registry Service");

    // Initialize application state
    let state = match AppState::new().await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to initialize application state: {}", e);
            return Err(e);
        }
    };

    // Create Axum router
    let app = create_app(state);

    // Bind to address
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3001".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    info!("Schema Registry listening on {}", bind_addr);

    // Start server
    axum::serve(listener, app).await?;

    Ok(())
}
