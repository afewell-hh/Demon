use crate::{AppResult, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, Json},
};
use runtime::bundle::BundleStatus;
use runtime::contracts::{
    validate_envelope, validate_envelope_bulk, BulkValidationResponse, ValidateEnvelopeBulkRequest,
    ValidationResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, info};

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

// ===== Contracts Browser Handlers =====

#[derive(Debug, Deserialize)]
pub struct ContractsBrowserQuery {
    pub search: Option<String>,
}

/// GET /ui/contracts - Contracts Browser HTML view
///
/// Feature-flagged UI for browsing schema registry contracts
pub async fn contracts_browser_html(
    State(state): State<AppState>,
    Query(query): Query<ContractsBrowserQuery>,
) -> AppResult<Html<String>> {
    // Check if feature flag is enabled
    if !crate::feature_flags::is_enabled("contracts-browser") {
        return Err(crate::AppError {
            status_code: StatusCode::NOT_FOUND,
            message:
                "Contracts browser feature is not enabled. Set OPERATE_UI_FLAGS=contracts-browser"
                    .to_string(),
        });
    }

    debug!("Rendering contracts browser HTML");

    let mut context = tera::Context::new();
    context.insert("current_page", &"contracts");
    context.insert("search", &query.search);
    context.insert("registry_available", &true); // TODO: check actual registry health
    context.insert(
        "contracts_browser_enabled",
        &crate::feature_flags::is_enabled("contracts-browser"),
    );
    context.insert(
        "canvas_enabled",
        &crate::feature_flags::is_enabled("canvas-ui"),
    );

    let html = state
        .tera
        .render("contracts_browser.html", &context)
        .map_err(|e| {
            error!("Failed to render contracts browser template: {}", e);
            crate::AppError::from(e)
        })?;

    Ok(Html(html))
}

/// GET /api/contracts/registry/list - Proxy to schema registry
///
/// Returns contract list from the schema registry service
pub async fn list_contracts_api(State(_state): State<AppState>) -> AppResult<Json<Value>> {
    debug!("Proxying to schema registry for contract list");

    // Check feature flag
    if !crate::feature_flags::is_enabled("contracts-browser") {
        return Err(crate::AppError {
            status_code: StatusCode::NOT_FOUND,
            message: "Contracts browser feature is not enabled".to_string(),
        });
    }

    // Get registry URL from environment
    let registry_url = std::env::var("SCHEMA_REGISTRY_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/registry/contracts", registry_url))
        .send()
        .await
        .map_err(|e| {
            error!("Failed to fetch contracts from registry: {}", e);
            crate::AppError {
                status_code: StatusCode::BAD_GATEWAY,
                message: format!("Failed to reach schema registry: {}", e),
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!("Registry returned error {}: {}", status, error_text);
        return Err(crate::AppError {
            status_code: StatusCode::BAD_GATEWAY,
            message: format!("Schema registry error: {}", error_text),
        });
    }

    let contracts: Value = response.json().await.map_err(|e| {
        error!("Failed to parse contracts response: {}", e);
        crate::AppError {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to parse registry response: {}", e),
        }
    })?;

    info!("Successfully fetched contracts from registry");
    Ok(Json(contracts))
}

/// GET /api/contracts/registry/:name/:version - Get specific contract
///
/// Returns full contract bundle including schemas and metadata
pub async fn get_contract_detail_api(
    State(_state): State<AppState>,
    Path((name, version)): Path<(String, String)>,
) -> AppResult<Json<Value>> {
    debug!("Fetching contract detail: {} v{}", name, version);

    // Check feature flag
    if !crate::feature_flags::is_enabled("contracts-browser") {
        return Err(crate::AppError {
            status_code: StatusCode::NOT_FOUND,
            message: "Contracts browser feature is not enabled".to_string(),
        });
    }

    let registry_url = std::env::var("SCHEMA_REGISTRY_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    // Encode path segments to prevent path traversal/SSRF
    let encoded_name = urlencoding::encode(&name);
    let encoded_version = urlencoding::encode(&version);

    let client = reqwest::Client::new();
    let response = client
        .get(format!(
            "{}/registry/contracts/{}/{}",
            registry_url, encoded_name, encoded_version
        ))
        .send()
        .await
        .map_err(|e| {
            error!("Failed to fetch contract {}/{}: {}", name, version, e);
            crate::AppError {
                status_code: StatusCode::BAD_GATEWAY,
                message: format!("Failed to reach schema registry: {}", e),
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(crate::AppError {
            status_code: if status == reqwest::StatusCode::NOT_FOUND {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::BAD_GATEWAY
            },
            message: format!("Contract not found: {} v{}", name, version),
        });
    }

    let contract: Value = response.json().await.map_err(|e| {
        error!("Failed to parse contract response: {}", e);
        crate::AppError {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to parse contract: {}", e),
        }
    })?;

    info!("Successfully fetched contract: {} v{}", name, version);
    Ok(Json(contract))
}
