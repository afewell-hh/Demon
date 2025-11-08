// Agent Flow API handlers
//
// This module implements REST and NATS endpoints for agent-authored flow submission.
// Feature-flagged via OPERATE_UI_FLAGS=agent-flows.

use crate::{AppError, AppResult, AppState};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tracing::{debug, error, info};
use uuid::Uuid;

// ===== Request/Response Types =====

#[derive(Debug, Deserialize)]
pub struct ListContractsQuery {
    pub kind: Option<String>,
    pub version: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ContractMetadata {
    pub name: String,
    pub kind: String,
    pub version: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DraftFlowRequest {
    pub manifest: Value,
}

#[derive(Debug, Serialize)]
pub struct DraftFlowResponse {
    pub draft_id: String,
    pub flow_id: String,
    pub manifest_digest: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitFlowRequest {
    pub manifest: Value,
}

#[derive(Debug, Serialize)]
pub struct SubmitFlowResponse {
    pub flow_id: String,
    pub manifest_digest: String,
    pub validation_result: ValidationResult,
    pub submitted_at: String,
}

#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Serialize)]
pub struct ValidationError {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ValidationWarning {
    pub code: String,
    pub message: String,
}

// ===== Helpers =====

/// Compute SHA-256 digest of manifest
fn compute_manifest_digest(manifest: &Value) -> String {
    let manifest_str = serde_json::to_string(manifest).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(manifest_str.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// Validate flow manifest against schema
fn validate_manifest(manifest: &Value) -> ValidationResult {
    let mut errors = Vec::new();
    let warnings = Vec::new();

    // Check required fields
    if manifest.get("schema_version").is_none() {
        errors.push(ValidationError {
            code: "flow.schema_version_missing".to_string(),
            message: "schema_version field is required".to_string(),
            path: Some("schema_version".to_string()),
        });
    }

    if manifest.get("metadata").is_none() {
        errors.push(ValidationError {
            code: "flow.metadata_missing".to_string(),
            message: "metadata field is required".to_string(),
            path: Some("metadata".to_string()),
        });
    }

    if manifest.get("nodes").is_none() {
        errors.push(ValidationError {
            code: "flow.nodes_missing".to_string(),
            message: "nodes field is required".to_string(),
            path: Some("nodes".to_string()),
        });
    }

    if manifest.get("edges").is_none() {
        errors.push(ValidationError {
            code: "flow.edges_missing".to_string(),
            message: "edges field is required".to_string(),
            path: Some("edges".to_string()),
        });
    }

    // Schema version check
    if let Some(ver) = manifest.get("schema_version").and_then(|v| v.as_str()) {
        if ver != "v1" {
            errors.push(ValidationError {
                code: "flow.unsupported_schema_version".to_string(),
                message: format!("Unsupported schema_version: {}", ver),
                path: Some("schema_version".to_string()),
            });
        }
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

// ===== Route Handlers =====

/// GET /api/contracts - List contract metadata
pub async fn list_contracts_handler(
    State(_state): State<AppState>,
    Query(query): Query<ListContractsQuery>,
) -> AppResult<Json<Vec<ContractMetadata>>> {
    debug!("Listing contracts with filters: {:?}", query);

    // Check feature flag
    if !crate::feature_flags::is_enabled("agent-flows") {
        return Err(AppError {
            status_code: StatusCode::NOT_FOUND,
            message: "Agent flows feature is not enabled. Set OPERATE_UI_FLAGS=agent-flows"
                .to_string(),
        });
    }

    // Mock response for MVP
    let contracts = vec![ContractMetadata {
        name: "echo".to_string(),
        kind: "capsule".to_string(),
        version: "v1".to_string(),
        description: Some("Echo capsule contract".to_string()),
    }];

    info!("Returned {} contracts", contracts.len());
    Ok(Json(contracts))
}

/// POST /api/flows/draft - Draft a flow manifest
pub async fn draft_flow_handler(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DraftFlowRequest>,
) -> AppResult<Json<DraftFlowResponse>> {
    debug!("Drafting flow manifest");

    // Check feature flag
    if !crate::feature_flags::is_enabled("agent-flows") {
        return Err(AppError {
            status_code: StatusCode::NOT_FOUND,
            message: "Agent flows feature is not enabled".to_string(),
        });
    }

    // Compute manifest digest
    let manifest_digest = compute_manifest_digest(&req.manifest);

    // Extract flow_id from manifest
    let flow_id = req
        .manifest
        .get("metadata")
        .and_then(|m| m.get("flow_id"))
        .and_then(|id| id.as_str())
        .ok_or_else(|| AppError {
            status_code: StatusCode::BAD_REQUEST,
            message: "manifest.metadata.flow_id is required".to_string(),
        })?
        .to_string();

    // Generate draft ID
    let draft_id = format!("draft-{}", Uuid::new_v4());
    let created_at = Utc::now().to_rfc3339();

    // Check for idempotency key
    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    info!(
        "Draft created: flow_id={}, draft_id={}, idempotency_key={:?}",
        flow_id, draft_id, idempotency_key
    );

    // TODO: Emit flow.drafted:v1 event to JetStream
    // TODO: Store draft in idempotency cache

    Ok(Json(DraftFlowResponse {
        draft_id,
        flow_id,
        manifest_digest,
        created_at,
    }))
}

/// POST /api/flows/submit - Validate and submit flow
pub async fn submit_flow_handler(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SubmitFlowRequest>,
) -> AppResult<Json<SubmitFlowResponse>> {
    debug!("Submitting flow manifest");

    // Check feature flag
    if !crate::feature_flags::is_enabled("agent-flows") {
        return Err(AppError {
            status_code: StatusCode::NOT_FOUND,
            message: "Agent flows feature is not enabled".to_string(),
        });
    }

    // Validate manifest
    let validation_result = validate_manifest(&req.manifest);

    if !validation_result.valid {
        error!(
            "Flow manifest validation failed: {:?}",
            validation_result.errors
        );
        return Err(AppError {
            status_code: StatusCode::BAD_REQUEST,
            message: format!(
                "Manifest validation failed: {}",
                validation_result.errors[0].message
            ),
        });
    }

    // Compute manifest digest
    let manifest_digest = compute_manifest_digest(&req.manifest);

    // Extract flow_id
    let flow_id = req
        .manifest
        .get("metadata")
        .and_then(|m| m.get("flow_id"))
        .and_then(|id| id.as_str())
        .ok_or_else(|| AppError {
            status_code: StatusCode::BAD_REQUEST,
            message: "manifest.metadata.flow_id is required".to_string(),
        })?
        .to_string();

    let submitted_at = Utc::now().to_rfc3339();

    // Check for idempotency key
    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    info!(
        "Flow submitted: flow_id={}, digest={}, idempotency_key={:?}",
        flow_id, manifest_digest, idempotency_key
    );

    // TODO: Emit flow.submitted:v1 event to JetStream
    // TODO: Emit agent.flow.audit:v1 event
    // TODO: Check contract compatibility

    Ok(Json(SubmitFlowResponse {
        flow_id,
        manifest_digest,
        validation_result,
        submitted_at,
    }))
}
