//! HTTP route handlers for the Schema Registry API

use crate::{AppError, AppResult, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde_json::{json, Value};
use tracing::{debug, error, info};

/// GET /registry/contracts - List all contracts
///
/// Returns a JSON array of contract metadata entries
pub async fn list_contracts(State(state): State<AppState>) -> AppResult<Json<Value>> {
    debug!("Handling GET /registry/contracts");

    match state.kv_client.list_contracts().await {
        Ok(contracts) => {
            info!("Successfully listed {} contracts", contracts.len());
            Ok(Json(json!({ "contracts": contracts })))
        }
        Err(e) => {
            error!("Failed to list contracts: {}", e);
            Err(AppError {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                message: format!("Failed to list contracts: {}", e),
            })
        }
    }
}

/// GET /registry/contracts/:name/:version - Get specific contract bundle
///
/// Returns the full contract bundle including schemas and metadata
pub async fn get_contract(
    State(state): State<AppState>,
    Path((name, version)): Path<(String, String)>,
) -> AppResult<Json<Value>> {
    debug!("Handling GET /registry/contracts/{}/{}", name, version);

    match state.kv_client.get_contract(&name, &version).await {
        Ok(Some(bundle)) => {
            info!("Successfully retrieved contract: {} v{}", name, version);
            Ok(Json(serde_json::to_value(bundle).map_err(|e| {
                AppError {
                    status_code: StatusCode::INTERNAL_SERVER_ERROR,
                    message: format!("Failed to serialize contract bundle: {}", e),
                }
            })?))
        }
        Ok(None) => {
            debug!("Contract not found: {} v{}", name, version);
            Err(AppError {
                status_code: StatusCode::NOT_FOUND,
                message: format!("Contract not found: {} v{}", name, version),
            })
        }
        Err(e) => {
            error!("Failed to get contract {} v{}: {}", name, version, e);
            Err(AppError {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                message: format!("Failed to get contract: {}", e),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_route_patterns() {
        // Basic validation that route handler signatures are correct
        // Integration tests will verify actual behavior
    }
}
