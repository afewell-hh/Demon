use crate::{AppResult, AppState};
use axum::{extract::State, response::Json};
use runtime::bundle::BundleStatus;
use runtime::contracts::{
    validate_envelope, validate_envelope_bulk, BulkValidationResponse, ValidateEnvelopeBulkRequest,
    ValidationResponse,
};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractBundleStatusResponse {
    #[serde(rename = "contractBundle")]
    pub contract_bundle: ContractBundleInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractBundleInfo {
    pub status: String,
    pub metadata: Option<serde_json::Value>,
    pub alerts: Vec<serde_json::Value>,
    #[serde(rename = "lastCheck")]
    pub last_check: String,
    #[serde(rename = "usingFallback")]
    pub using_fallback: bool,
    #[serde(rename = "cacheDir")]
    pub cache_dir: Option<String>,
    pub source: String,
}

pub async fn bundle_status_endpoint(
    State(state): State<AppState>,
) -> AppResult<Json<ContractBundleStatusResponse>> {
    debug!("Received bundle status request");

    // Check if contract bundle loading is disabled
    let skip_bundle = std::env::var("DEMON_SKIP_CONTRACT_BUNDLE")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);

    if skip_bundle {
        let response = ContractBundleStatusResponse {
            contract_bundle: ContractBundleInfo {
                status: "disabled".to_string(),
                metadata: None,
                alerts: vec![],
                last_check: chrono::Utc::now().to_rfc3339(),
                using_fallback: false,
                cache_dir: None,
                source: "disabled".to_string(),
            },
        };
        return Ok(Json(response));
    }

    // Get bundle state from the loader
    let bundle_state = state.bundle_loader.get_state().await;
    let cache_dir = state
        .bundle_loader
        .cache_dir()
        .to_string_lossy()
        .to_string();

    // Convert BundleStatus to string
    let status_str = match bundle_state.status {
        BundleStatus::Loaded => "loaded",
        BundleStatus::NotLoaded => "not_loaded",
        BundleStatus::UsingFallback => "using_fallback",
        BundleStatus::VerificationFailed => "verification_failed",
        BundleStatus::DownloadError => "download_error",
        BundleStatus::Stale => "stale",
    };

    // Convert metadata to JSON if present
    let metadata = bundle_state
        .metadata
        .map(|m| serde_json::to_value(m).unwrap_or(serde_json::Value::Null));

    // Convert alerts to JSON
    let alerts: Vec<serde_json::Value> = bundle_state
        .alerts
        .into_iter()
        .map(|alert| serde_json::to_value(alert).unwrap_or(serde_json::Value::Null))
        .collect();

    let response = ContractBundleStatusResponse {
        contract_bundle: ContractBundleInfo {
            status: status_str.to_string(),
            metadata,
            alerts,
            last_check: bundle_state.last_check,
            using_fallback: bundle_state.using_fallback,
            cache_dir: Some(cache_dir),
            source: if bundle_state.using_fallback {
                "fallback"
            } else {
                "primary"
            }
            .to_string(),
        },
    };

    Ok(Json(response))
}
