//! JWT authentication middleware with scope validation

use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,         // Subject (user identifier)
    pub exp: usize,          // Expiration time (Unix timestamp)
    pub iat: Option<usize>,  // Issued at (Unix timestamp)
    pub scopes: Vec<String>, // Permission scopes
}

/// JWT configuration loaded from environment
#[derive(Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub algorithm: Algorithm,
}

impl JwtConfig {
    /// Load JWT configuration from environment variables
    ///
    /// # Panics
    ///
    /// Panics if `JWT_SECRET` environment variable is not set, as this is required
    /// for production security.
    pub fn from_env() -> Self {
        let secret = std::env::var("JWT_SECRET")
            .expect("JWT_SECRET environment variable must be set. Set it to a secure random string (minimum 32 characters recommended).");

        let algorithm = std::env::var("JWT_ALGORITHM")
            .ok()
            .and_then(|a| match a.as_str() {
                "HS256" => Some(Algorithm::HS256),
                "HS384" => Some(Algorithm::HS384),
                "HS512" => Some(Algorithm::HS512),
                _ => None,
            })
            .unwrap_or(Algorithm::HS256);

        Self { secret, algorithm }
    }

    /// Create from explicit secret (for testing)
    pub fn new(secret: String, algorithm: Algorithm) -> Self {
        Self { secret, algorithm }
    }
}

/// Extension to attach validated claims to the request
#[derive(Clone, Debug)]
pub struct AuthClaims(pub Claims);

/// JWT middleware that validates tokens and extracts claims
pub async fn jwt_middleware(
    state: axum::extract::State<crate::AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let config = &state.jwt_config;

    // Extract Authorization header
    let auth_header = request.headers().get("Authorization").cloned();

    match auth_header {
        Some(header_value) => match header_value.to_str() {
            Ok(auth_str) => {
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    match verify_jwt(token, config) {
                        Ok(claims) => {
                            debug!(
                                "JWT token validated for subject: {}, scopes: {:?}",
                                claims.sub, claims.scopes
                            );
                            // Attach claims to request extensions
                            request.extensions_mut().insert(AuthClaims(claims));
                            next.run(request).await
                        }
                        Err(e) => {
                            warn!("JWT validation failed: {}", e);
                            unauthorized_response(format!("Invalid token: {}", e))
                        }
                    }
                } else {
                    warn!("Authorization header not in Bearer format");
                    unauthorized_response("Authorization header must use Bearer scheme".to_string())
                }
            }
            Err(e) => {
                warn!("Failed to parse Authorization header: {}", e);
                unauthorized_response("Invalid Authorization header".to_string())
            }
        },
        None => {
            debug!("No Authorization header present - returning 401");
            unauthorized_response("Missing Authorization header".to_string())
        }
    }
}

/// Verify JWT token and extract claims
fn verify_jwt(token: &str, config: &JwtConfig) -> Result<Claims, String> {
    let mut validation = Validation::new(config.algorithm);
    validation.validate_exp = true;

    let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());

    let token_data = decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|e| format!("Token decode error: {}", e))?;

    Ok(token_data.claims)
}

/// Create a 401 Unauthorized response
fn unauthorized_response(message: String) -> Response {
    (StatusCode::UNAUTHORIZED, message).into_response()
}

/// Check if claims contain a specific scope
pub fn has_scope(claims: &Claims, required_scope: &str) -> bool {
    claims.scopes.iter().any(|s| s == required_scope)
}

/// Extract claims from request extensions
pub fn extract_claims(request: &Request<Body>) -> Option<Claims> {
    request
        .extensions()
        .get::<AuthClaims>()
        .map(|ac| ac.0.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    fn create_test_token(scopes: Vec<String>, secret: &str) -> String {
        let claims = Claims {
            sub: "test-user".to_string(),
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
            iat: Some(chrono::Utc::now().timestamp() as usize),
            scopes,
        };

        let header = Header::new(Algorithm::HS256);
        encode(
            &header,
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    #[test]
    fn test_verify_valid_jwt() {
        let secret = "test-secret";
        let config = JwtConfig::new(secret.to_string(), Algorithm::HS256);
        let token = create_test_token(vec!["contracts:read".to_string()], secret);

        let result = verify_jwt(&token, &config);
        assert!(result.is_ok());

        let claims = result.unwrap();
        assert_eq!(claims.sub, "test-user");
        assert!(claims.scopes.contains(&"contracts:read".to_string()));
    }

    #[test]
    fn test_verify_invalid_secret() {
        let secret = "test-secret";
        let wrong_secret = "wrong-secret";
        let config = JwtConfig::new(wrong_secret.to_string(), Algorithm::HS256);
        let token = create_test_token(vec!["contracts:read".to_string()], secret);

        let result = verify_jwt(&token, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_has_scope() {
        let claims = Claims {
            sub: "user".to_string(),
            exp: 9999999999,
            iat: None,
            scopes: vec!["contracts:write".to_string(), "contracts:read".to_string()],
        };

        assert!(has_scope(&claims, "contracts:write"));
        assert!(has_scope(&claims, "contracts:read"));
        assert!(!has_scope(&claims, "contracts:delete"));
    }

    #[test]
    fn test_expired_token() {
        let secret = "test-secret";
        let config = JwtConfig::new(secret.to_string(), Algorithm::HS256);

        let expired_claims = Claims {
            sub: "test-user".to_string(),
            exp: (chrono::Utc::now() - chrono::Duration::hours(1)).timestamp() as usize,
            iat: Some(chrono::Utc::now().timestamp() as usize),
            scopes: vec!["contracts:read".to_string()],
        };

        let header = Header::new(Algorithm::HS256);
        let token = encode(
            &header,
            &expired_claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();

        let result = verify_jwt(&token, &config);
        assert!(result.is_err());
        // JWT library may use "ExpiredSignature" or similar error message
        let err_msg = result.unwrap_err().to_lowercase();
        assert!(err_msg.contains("exp") || err_msg.contains("token"));
    }

    #[test]
    #[should_panic(expected = "JWT_SECRET environment variable must be set")]
    fn given_missing_jwt_secret_when_from_env_called_then_panics() {
        // Ensure JWT_SECRET is not set for this test
        std::env::remove_var("JWT_SECRET");

        // This should panic with a clear error message
        let _config = JwtConfig::from_env();
    }
}
