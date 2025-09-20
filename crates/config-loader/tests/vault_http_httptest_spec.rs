use config_loader::{SecretError, SecretProvider, VaultHttpSecretProvider};
use httptest::{matchers::*, responders::*, Expectation, Server};
use serde_json::json;

#[test]
fn test_vault_http_resolve_secret_success() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    // Set up expectation for successful secret retrieval
    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/v1/secret/data/demo/api-key"),
            request::headers(contains(("x-vault-token", "test-token"))),
        ])
        .respond_with(json_encoded(json!({
            "data": {
                "data": {
                    "api-key": "secret-value-123"
                }
            }
        }))),
    );

    let provider =
        VaultHttpSecretProvider::new(Some(vault_addr), Some("test-token".to_string()), None)
            .unwrap();

    let result = provider.resolve("demo", "api-key");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "secret-value-123");
}

#[test]
fn test_vault_http_resolve_secret_not_found() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/v1/secret/data/demo/missing"),
            request::headers(contains(("x-vault-token", "test-token"))),
        ])
        .respond_with(status_code(404)),
    );

    let provider =
        VaultHttpSecretProvider::new(Some(vault_addr), Some("test-token".to_string()), None)
            .unwrap();

    let result = provider.resolve("demo", "missing");
    assert!(result.is_err());
    assert!(matches!(result, Err(SecretError::SecretNotFound { .. })));
}

#[test]
fn test_vault_http_resolve_with_namespace() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/v1/secret/data/demo/key"),
            request::headers(contains(("x-vault-token", "test-token"))),
            request::headers(contains(("x-vault-namespace", "my-namespace"))),
        ])
        .respond_with(json_encoded(json!({
            "data": {
                "data": {
                    "key": "namespaced-value"
                }
            }
        }))),
    );

    let provider = VaultHttpSecretProvider::new(
        Some(vault_addr),
        Some("test-token".to_string()),
        Some("my-namespace".to_string()),
    )
    .unwrap();

    let result = provider.resolve("demo", "key");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "namespaced-value");
}

#[test]
fn test_vault_http_put_secret_success() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    server.expect(
        Expectation::matching(all_of![
            request::method_path("POST", "/v1/secret/data/demo/new-key"),
            request::headers(contains(("x-vault-token", "test-token"))),
            request::body(json_decoded(eq(json!({
                "data": {
                    "new-key": "new-value"
                }
            })))),
        ])
        .respond_with(status_code(200)),
    );

    let provider =
        VaultHttpSecretProvider::new(Some(vault_addr), Some("test-token".to_string()), None)
            .unwrap();

    let result = provider.put("demo", "new-key", "new-value");
    assert!(result.is_ok());
}

#[test]
fn test_vault_http_delete_secret_success() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    server.expect(
        Expectation::matching(all_of![
            request::method_path("DELETE", "/v1/secret/metadata/demo/old-key"),
            request::headers(contains(("x-vault-token", "test-token"))),
        ])
        .respond_with(status_code(204)),
    );

    let provider =
        VaultHttpSecretProvider::new(Some(vault_addr), Some("test-token".to_string()), None)
            .unwrap();

    let result = provider.delete("demo", "old-key");
    assert!(result.is_ok());
}

#[test]
fn test_vault_http_auth_failure_no_retry() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    // Expect only one request - auth failures should not retry
    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/v1/secret/data/demo/key"),
            request::headers(contains(("x-vault-token", "bad-token"))),
        ])
        .times(1)
        .respond_with(status_code(403)),
    );

    let provider =
        VaultHttpSecretProvider::new(Some(vault_addr), Some("bad-token".to_string()), None)
            .unwrap();

    let result = provider.resolve("demo", "key");
    assert!(result.is_err());
}

#[test]
#[ignore] // TODO: Fix retry test - httptest expectations not working correctly
fn test_vault_http_retry_on_server_error() {
    std::env::set_var("VAULT_RETRY_ATTEMPTS", "3");

    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    // First two attempts fail with 500, third succeeds
    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/v1/secret/data/demo/key"),
            request::headers(contains(("x-vault-token", "test-token"))),
        ])
        .times(2)
        .respond_with(status_code(500)),
    );

    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/v1/secret/data/demo/key"),
            request::headers(contains(("x-vault-token", "test-token"))),
        ])
        .times(1)
        .respond_with(json_encoded(json!({
            "data": {
                "data": {
                    "key": "retry-success"
                }
            }
        }))),
    );

    let provider =
        VaultHttpSecretProvider::new(Some(vault_addr), Some("test-token".to_string()), None)
            .unwrap();

    let result = provider.resolve("demo", "key");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "retry-success");

    std::env::remove_var("VAULT_RETRY_ATTEMPTS");
}

#[test]
fn test_vault_http_all_retries_exhausted() {
    std::env::set_var("VAULT_RETRY_ATTEMPTS", "3");

    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    // All attempts fail with 500
    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/v1/secret/data/demo/key"),
            request::headers(contains(("x-vault-token", "test-token"))),
        ])
        .times(3)
        .respond_with(status_code(500)),
    );

    let provider =
        VaultHttpSecretProvider::new(Some(vault_addr), Some("test-token".to_string()), None)
            .unwrap();

    let result = provider.resolve("demo", "key");
    assert!(result.is_err());

    std::env::remove_var("VAULT_RETRY_ATTEMPTS");
}
