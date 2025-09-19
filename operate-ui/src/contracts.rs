use crate::{AppResult, AppState};
use axum::{extract::State, response::Json};
use runtime::contracts::{
    validate_envelope, validate_envelope_bulk, BulkValidationResponse, ValidateEnvelopeBulkRequest,
    ValidationResponse,
};
use serde_json::Value;
use tracing::debug;

pub async fn validate_envelope_endpoint(
    State(_state): State<AppState>,
    Json(body): Json<Value>,
) -> AppResult<Json<ValidationResponse>> {
    debug!("Received envelope validation request");
    let response = validate_envelope(&body);
    Ok(Json(response))
}

pub async fn validate_envelope_bulk_endpoint(
    State(_state): State<AppState>,
    Json(request): Json<ValidateEnvelopeBulkRequest>,
) -> AppResult<Json<BulkValidationResponse>> {
    debug!("Received bulk envelope validation request");
    let response = validate_envelope_bulk(&request);
    Ok(Json(response))
}
