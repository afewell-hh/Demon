//! HTTP route handlers for the Schema Registry API

use crate::{auth, kv::ContractBundle, AppError, AppResult, AppState};
use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};

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

/// Request body for publishing a contract
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PublishContractRequest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(rename = "jsonSchema")]
    pub json_schema: Option<String>,
    #[serde(rename = "witPath")]
    pub wit_path: Option<String>,
    #[serde(rename = "descriptorPath")]
    pub descriptor_path: Option<String>,
}

/// POST /registry/contracts - Publish a new contract bundle
///
/// Requires JWT with `contracts:write` scope.
/// Computes SHA-256 digest and stores bundle in KV.
pub async fn publish_contract(
    State(state): State<AppState>,
    request: Request<Body>,
) -> AppResult<(StatusCode, Json<Value>)> {
    // Extract and validate JWT claims
    let claims = auth::extract_claims(&request).ok_or_else(|| AppError {
        status_code: StatusCode::UNAUTHORIZED,
        message: "Missing authentication claims".to_string(),
    })?;

    // Verify contracts:write scope
    if !auth::has_scope(&claims, "contracts:write") {
        warn!("User {} lacks contracts:write scope", claims.sub);
        return Err(AppError {
            status_code: StatusCode::FORBIDDEN,
            message: "Insufficient permissions: contracts:write scope required".to_string(),
        });
    }

    // Extract request body with size limit to prevent DoS
    // 10 MB limit is reasonable for contract bundles (schemas + metadata)
    const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;
    let body_bytes = axum::body::to_bytes(request.into_body(), MAX_BODY_SIZE)
        .await
        .map_err(|e| {
            let err_msg = e.to_string();
            if err_msg.contains("length limit") || err_msg.contains("too large") {
                AppError {
                    status_code: StatusCode::PAYLOAD_TOO_LARGE,
                    message: format!("Request body exceeds maximum size of {} bytes", MAX_BODY_SIZE),
                }
            } else {
                AppError {
                    status_code: StatusCode::BAD_REQUEST,
                    message: format!("Failed to read request body: {}", e),
                }
            }
        })?;

    let payload: PublishContractRequest =
        serde_json::from_slice(&body_bytes).map_err(|e| AppError {
            status_code: StatusCode::BAD_REQUEST,
            message: format!("Invalid JSON payload: {}", e),
        })?;

    debug!(
        "Handling POST /registry/contracts for {} v{}",
        payload.name, payload.version
    );

    // Check for duplicate version
    if let Ok(Some(_)) = state
        .kv_client
        .get_contract(&payload.name, &payload.version)
        .await
    {
        warn!(
            "Duplicate contract version: {} v{}",
            payload.name, payload.version
        );
        return Err(AppError {
            status_code: StatusCode::CONFLICT,
            message: format!(
                "Contract {} version {} already exists",
                payload.name, payload.version
            ),
        });
    }

    // Compute SHA-256 digest of the bundle content
    let bundle_json = serde_json::to_vec(&payload).map_err(|e| AppError {
        status_code: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("Failed to serialize bundle: {}", e),
    })?;

    let mut hasher = Sha256::new();
    hasher.update(&bundle_json);
    let digest = hex::encode(hasher.finalize());

    debug!(
        "Computed SHA-256 digest for {} v{}: {}",
        payload.name, payload.version, digest
    );

    // Create bundle with timestamp and digest
    let now = chrono::Utc::now().to_rfc3339();
    let bundle = ContractBundle {
        name: payload.name.clone(),
        version: payload.version.clone(),
        description: payload.description.clone(),
        created_at: now.clone(),
        json_schema: payload.json_schema.clone(),
        wit_path: payload.wit_path.clone(),
        descriptor_path: payload.descriptor_path.clone(),
        digest: Some(digest.clone()),
    };

    // Store in KV
    state.kv_client.put_contract(&bundle).await.map_err(|e| {
        error!("Failed to store contract: {}", e);
        AppError {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to store contract: {}", e),
        }
    })?;

    info!(
        "Successfully published contract: {} v{} (digest: {})",
        payload.name, payload.version, digest
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "created",
            "name": payload.name,
            "version": payload.version,
            "digest": digest,
            "createdAt": now
        })),
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_route_patterns() {
        // Basic validation that route handler signatures are correct
        // Integration tests will verify actual behavior
    }
}
