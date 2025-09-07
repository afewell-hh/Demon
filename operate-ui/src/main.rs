use operate_ui::{AppState, create_app};
use anyhow::{Context, Result};
use std::env;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "operate_ui=debug,tower_http=debug,axum::rejection=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get configuration from environment
    let port = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .context("PORT must be a valid number")?;

    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string());

    // Initialize application state
    let state = AppState::new().await;

    // Create the application
    let app = create_app(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", bind_addr, port))
        .await
        .with_context(|| format!("Failed to bind to {}:{}", bind_addr, port))?;

    info!("Server starting on http://{}:{}", bind_addr, port);

    axum::serve(listener, app)
        .await
        .context("Server failed to start")?;

    Ok(())
}