//! REST API server for runtime services

pub mod graph;

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing::info;

/// Create the REST API application router
pub fn create_app() -> Router {
    Router::new()
        // Health check endpoint
        .route("/health", get(health_check))
        // Mount graph API routes
        .nest("/api/graph", graph::routes())
        // Add tracing layer
        .layer(TraceLayer::new_for_http())
}

/// Health check handler
async fn health_check() -> &'static str {
    "OK"
}

/// Start the REST API server
pub async fn serve(addr: SocketAddr) -> anyhow::Result<()> {
    let app = create_app();

    info!("Starting REST API server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
