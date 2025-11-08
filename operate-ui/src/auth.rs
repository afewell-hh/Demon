// JWT Authentication middleware for Agent Flow API
//
// This module provides JWT token validation and scope enforcement for agent flow endpoints.
// Tokens are expected to be issued by Auth0 or a compatible JWT issuer.

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, warn};

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,              // Subject (agent/user ID)
    pub iss: Option<String>,      // Issuer
    pub aud: Option<Vec<String>>, // Audience
    pub exp: Option<usize>,       // Expiration time
    pub iat: Option<usize>,       // Issued at
    pub scope: Option<String>,    // Space-separated scopes
}

impl Claims {
    /// Check if the claims include a specific scope
    pub fn has_scope(&self, required_scope: &str) -> bool {
        self.scope
            .as_ref()
            .map(|s| s.split_whitespace().any(|scope| scope == required_scope))
            .unwrap_or(false)
    }
}

/// Extract and validate JWT from Authorization header
pub fn extract_and_validate_jwt(headers: &HeaderMap) -> Result<Claims, AuthError> {
    // Extract Bearer token from Authorization header
    let auth_header = headers
        .get("Authorization")
        .ok_or(AuthError::MissingToken)?
        .to_str()
        .map_err(|_| AuthError::InvalidToken)?;

    if !auth_header.starts_with("Bearer ") {
        return Err(AuthError::InvalidToken);
    }

    let token = &auth_header[7..]; // Skip "Bearer "

    // Get JWT secret from environment
    let secret = std::env::var("JWT_SECRET").map_err(|_| AuthError::ConfigurationError)?;

    // Decode and validate token
    let mut validation = Validation::new(Algorithm::HS256);

    // Optional: validate issuer if JWT_ISSUER is set
    if let Ok(issuer) = std::env::var("JWT_ISSUER") {
        validation.set_issuer(&[issuer]);
    }

    // Optional: validate audience if JWT_AUDIENCE is set
    if let Ok(audience) = std::env::var("JWT_AUDIENCE") {
        validation.set_audience(&[audience]);
    }

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        warn!("JWT validation failed: {}", e);
        AuthError::InvalidToken
    })?;

    Ok(token_data.claims)
}

/// Middleware to enforce JWT authentication and scope requirements
pub async fn require_scope(
    required_scope: &'static str,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
       + Clone {
    move |request: Request, next: Next| {
        let scope = required_scope;
        Box::pin(async move {
            // Extract and validate JWT
            let claims = match extract_and_validate_jwt(request.headers()) {
                Ok(c) => c,
                Err(e) => return e.into_response(),
            };

            // Check required scope
            if !claims.has_scope(scope) {
                return AuthError::InsufficientScope {
                    required: scope.to_string(),
                    provided: claims.scope.unwrap_or_default(),
                }
                .into_response();
            }

            debug!("JWT auth successful for subject: {}", claims.sub);

            // Continue processing request
            next.run(request).await
        })
    }
}

/// Authentication errors
#[derive(Debug)]
pub enum AuthError {
    MissingToken,
    InvalidToken,
    ConfigurationError,
    InsufficientScope { required: String, provided: String },
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AuthError::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "missing_token",
                "Authorization header with Bearer token is required",
            ),
            AuthError::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "invalid_token",
                "JWT token is invalid or expired",
            ),
            AuthError::ConfigurationError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_configuration_error",
                "JWT authentication is not properly configured",
            ),
            AuthError::InsufficientScope {
                ref required,
                ref provided,
            } => {
                warn!(
                    "Insufficient scope - required: {}, provided: {}",
                    required, provided
                );
                (
                    StatusCode::FORBIDDEN,
                    "insufficient_scope",
                    "Token does not have required scope",
                )
            }
        };

        (
            status,
            Json(json!({
                "code": code,
                "message": message,
            })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claims_has_scope() {
        let claims = Claims {
            sub: "test-agent".to_string(),
            iss: None,
            aud: None,
            exp: None,
            iat: None,
            scope: Some("flows:read flows:write".to_string()),
        };

        assert!(claims.has_scope("flows:read"));
        assert!(claims.has_scope("flows:write"));
        assert!(!claims.has_scope("admin:all"));
    }

    #[test]
    fn test_claims_no_scope() {
        let claims = Claims {
            sub: "test-agent".to_string(),
            iss: None,
            aud: None,
            exp: None,
            iat: None,
            scope: None,
        };

        assert!(!claims.has_scope("flows:read"));
    }
}
