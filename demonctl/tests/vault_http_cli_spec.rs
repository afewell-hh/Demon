use assert_cmd::Command;
use httptest::{matchers::*, responders::*, Expectation, Server};
use predicates::prelude::*;
use serde_json::json;
use std::env;

#[test]
fn test_demonctl_secrets_set_vault_http() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    server.expect(
        Expectation::matching(all_of![
            request::method_path("POST", "/v1/secret/data/test/key"),
            request::headers(contains(("x-vault-token", "test-token"))),
            request::body(json_decoded(eq(json!({
                "data": {
                    "key": "test-value"
                }
            })))),
        ])
        .respond_with(status_code(200)),
    );

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.env("VAULT_ADDR", &vault_addr)
        .env("VAULT_TOKEN", "test-token")
        .env("CONFIG_SECRETS_PROVIDER", "vault")
        .arg("secrets")
        .arg("set")
        .arg("test/key")
        .arg("test-value")
        .arg("--provider")
        .arg("vault");

    cmd.assert().success().stdout(predicate::str::contains(
        "Secret test/key set successfully in Vault (HTTP)",
    ));
}

#[test]
fn test_demonctl_secrets_get_vault_http() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/v1/secret/data/test/key"),
            request::headers(contains(("x-vault-token", "test-token"))),
        ])
        .respond_with(json_encoded(json!({
            "data": {
                "data": {
                    "key": "secret-value"
                }
            }
        }))),
    );

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.env("VAULT_ADDR", &vault_addr)
        .env("VAULT_TOKEN", "test-token")
        .env("CONFIG_SECRETS_PROVIDER", "vault")
        .arg("secrets")
        .arg("get")
        .arg("test/key")
        .arg("--provider")
        .arg("vault");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("test/key: sec***"));
}

#[test]
fn test_demonctl_secrets_get_vault_http_raw() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/v1/secret/data/test/key"),
            request::headers(contains(("x-vault-token", "test-token"))),
        ])
        .respond_with(json_encoded(json!({
            "data": {
                "data": {
                    "key": "secret-value-raw"
                }
            }
        }))),
    );

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.env("VAULT_ADDR", &vault_addr)
        .env("VAULT_TOKEN", "test-token")
        .env("CONFIG_SECRETS_PROVIDER", "vault")
        .arg("secrets")
        .arg("get")
        .arg("test/key")
        .arg("--raw")
        .arg("--provider")
        .arg("vault");

    cmd.assert().success().stdout(
        predicate::str::contains("secret-value-raw").and(predicate::str::contains("***").not()),
    );
}

#[test]
fn test_demonctl_secrets_delete_vault_http() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    server.expect(
        Expectation::matching(all_of![
            request::method_path("DELETE", "/v1/secret/metadata/test/key"),
            request::headers(contains(("x-vault-token", "test-token"))),
        ])
        .respond_with(status_code(204)),
    );

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.env("VAULT_ADDR", &vault_addr)
        .env("VAULT_TOKEN", "test-token")
        .env("CONFIG_SECRETS_PROVIDER", "vault")
        .arg("secrets")
        .arg("delete")
        .arg("test/key")
        .arg("--provider")
        .arg("vault");

    cmd.assert().success().stdout(predicate::str::contains(
        "Secret test/key deleted from Vault (HTTP)",
    ));
}

#[test]
fn test_demonctl_secrets_vault_http_missing_token() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    // Ensure VAULT_TOKEN is not set
    env::remove_var("VAULT_TOKEN");

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.env("VAULT_ADDR", &vault_addr)
        .env("CONFIG_SECRETS_PROVIDER", "vault")
        .arg("secrets")
        .arg("get")
        .arg("test/key")
        .arg("--provider")
        .arg("vault");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("VAULT_TOKEN"));
}

#[test]
fn test_demonctl_secrets_vault_http_auth_failure() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/v1/secret/data/test/key"),
            request::headers(contains(("x-vault-token", "invalid-token"))),
        ])
        .respond_with(status_code(403)),
    );

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.env("VAULT_ADDR", &vault_addr)
        .env("VAULT_TOKEN", "invalid-token")
        .env("CONFIG_SECRETS_PROVIDER", "vault")
        .arg("secrets")
        .arg("get")
        .arg("test/key")
        .arg("--provider")
        .arg("vault");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Failed"));
}

#[test]
fn test_demonctl_secrets_list_vault_http_not_supported() {
    let server = Server::run();
    let vault_addr = format!("http://{}", server.addr());

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.env("VAULT_ADDR", &vault_addr)
        .env("VAULT_TOKEN", "test-token")
        .env("CONFIG_SECRETS_PROVIDER", "vault")
        .arg("secrets")
        .arg("list")
        .arg("--provider")
        .arg("vault");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not yet supported"));
}

#[test]
fn test_demonctl_secrets_vault_stub_fallback() {
    // When VAULT_ADDR starts with file://, it should use the stub provider
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.env("VAULT_ADDR", "file://vault_stub")
        .env("CONFIG_SECRETS_PROVIDER", "vault")
        .arg("secrets")
        .arg("set")
        .arg("test/key")
        .arg("stub-value")
        .arg("--provider")
        .arg("vault");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("vault stub"));
}
