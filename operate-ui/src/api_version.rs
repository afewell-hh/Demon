// API Versioning support for Demon operate-ui
//
// This module provides version negotiation, header handling, and compatibility checking
// for the Demon API. All API endpoints should use these utilities to ensure consistent
// versioning behavior across the platform.

use axum::{
    body::Body,
    extract::Request,
    http::{header::HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::{debug, warn};

/// Current API version - this is the default and only supported version
pub const API_VERSION_V1: &str = "v1";

/// Header name for API version negotiation
pub const API_VERSION_HEADER: &str = "X-Demon-API-Version";

/// All currently supported API versions
pub const SUPPORTED_VERSIONS: &[&str] = &[API_VERSION_V1];

/// Check if a version string is supported
pub fn is_supported_version(version: &str) -> bool {
    SUPPORTED_VERSIONS.contains(&version)
}

/// Middleware to handle API version negotiation
///
/// This middleware:
/// 1. Checks the client's requested API version from the X-Demon-API-Version header
/// 2. Returns 406 Not Acceptable if the requested version is not supported
/// 3. Adds the API version header to all responses
///
/// If no version header is provided by the client, we assume v1 for backwards compatibility.
pub async fn version_negotiation_middleware(request: Request, next: Next) -> Response {
    let path = request.uri().path();

    // Only apply versioning to API endpoints
    if !path.starts_with("/api/") {
        return next.run(request).await;
    }

    // Check if client requested a specific version
    let requested_version = request
        .headers()
        .get(API_VERSION_HEADER)
        .and_then(|v| v.to_str().ok());

    if let Some(version) = requested_version {
        debug!("Client requested API version: {}", version);

        if !is_supported_version(version) {
            warn!(
                "Unsupported API version requested: {} (path: {})",
                version, path
            );
            return (
                StatusCode::NOT_ACCEPTABLE,
                [(API_VERSION_HEADER, API_VERSION_V1)],
                Json(json!({
                    "error": "unsupported API version",
                    "requested_version": version,
                    "supported_versions": SUPPORTED_VERSIONS,
                    "message": format!(
                        "API version '{}' is not supported. Please use one of: {}",
                        version,
                        SUPPORTED_VERSIONS.join(", ")
                    )
                })),
            )
                .into_response();
        }
    } else {
        debug!(
            "No API version header provided, assuming {}",
            API_VERSION_V1
        );
    }

    // Process the request and add version header to response
    let mut response = next.run(request).await;
    add_version_header(&mut response);
    response
}

/// Add the API version header to a response
pub fn add_version_header(response: &mut Response) {
    if let Ok(header_value) = HeaderValue::from_str(API_VERSION_V1) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-demon-api-version"), header_value);
    }
}

/// Helper to create a version-aware JSON response
pub fn versioned_json_response<T: serde::Serialize>(status: StatusCode, data: T) -> Response<Body> {
    let mut response = (status, Json(data)).into_response();
    add_version_header(&mut response);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_version() {
        assert!(is_supported_version("v1"));
        assert!(!is_supported_version("v2"));
        assert!(!is_supported_version(""));
        assert!(!is_supported_version("invalid"));
    }

    #[test]
    fn test_supported_versions_contains_v1() {
        assert!(SUPPORTED_VERSIONS.contains(&"v1"));
        assert_eq!(SUPPORTED_VERSIONS.len(), 1);
    }
}
