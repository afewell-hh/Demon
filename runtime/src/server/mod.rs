//! REST API server for runtime services

pub mod graph;
pub mod rituals;

use axum::{routing::get, Extension, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;

/// Create the REST API application router
pub fn create_app() -> anyhow::Result<Router> {
    let service = Arc::new(rituals::RitualService::new()?);
    Ok(create_app_with_service(service))
}

pub fn create_app_with_service(service: Arc<rituals::RitualService>) -> Router {
    let rituals_router = rituals::routes();
    let graph_router = graph::routes();

    Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .nest("/api/graph", graph_router)
        .nest("/api/v1/rituals", rituals_router)
        .layer(TraceLayer::new_for_http())
        .layer(Extension(service))
}

/// Health check handler
async fn health_check() -> &'static str {
    "OK"
}

/// Readiness check handler
async fn readiness_check() -> &'static str {
    "OK"
}

/// Start the REST API server
pub async fn serve(addr: SocketAddr) -> anyhow::Result<()> {
    let app = create_app()?;

    info!("Starting REST API server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
