use httptest::responders::status_code;
use httptest::{matchers::request, Expectation, Server};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static HTTPS_RESOLVE_MUTEX: Mutex<()> = Mutex::new(());

fn repo_path(rel: &str) -> PathBuf {
    let candidates = [
        Path::new(rel).to_path_buf(),
        Path::new("..").join(rel),
        Path::new("../..").join(rel),
        Path::new("../../..").join(rel),
    ];
    for p in candidates {
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(rel)
}

#[test]
fn libindex_resolve_and_schema_validation() {
    let idx_path = repo_path("bootstrapper/library/index.json");
    // Should load and validate via schema and resolve
    let resolved = bootstrapper_demonctl::libindex::resolve_local(
        "lib://local/preview-local-dev@0.0.1",
        &idx_path,
    )
    .unwrap();
    assert!(resolved.path.is_absolute());
    assert!(resolved.path.exists());
}

#[test]
fn libindex_bad_name_or_version_errors() {
    let idx_path = repo_path("bootstrapper/library/index.json");
    let err = bootstrapper_demonctl::libindex::resolve_local("lib://local/nope@0.0.1", &idx_path)
        .unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("bundle not found"));
}

#[test]
fn libindex_missing_file_errors() {
    // Write a temp index that points to a missing path
    let tmp = tempfile::tempdir().unwrap();
    let idx_path = tmp.path().join("index.json");
    fs::write(&idx_path, r#"{
      "provider":"local",
      "bundles":[{"name":"x","version":"1.0","path":"does/not/exist.yaml","digest":{"sha256":"deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"},"sig":{"ed25519":"AA"},"pubKeyId":"preview"}]
    }"#).unwrap();
    let err =
        bootstrapper_demonctl::libindex::resolve_local("lib://local/x@1.0", &idx_path).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("bundle file not found"));
}

#[test]
fn libindex_https_resolve_success() {
    let _guard = HTTPS_RESOLVE_MUTEX.lock().unwrap();
    // Start a test HTTP server
    let server = Server::run();

    // Use the real bundle content from local-dev.yaml
    let bundle_path = repo_path("examples/bundles/local-dev.yaml");
    let bundle_yaml = fs::read_to_string(&bundle_path).unwrap();

    // Set up expectation for bundle download
    server.expect(
        Expectation::matching(request::method_path("GET", "/bundles/local-dev.yaml"))
            .respond_with(status_code(200).body(bundle_yaml)),
    );

    // Create temp index with HTTPS provider using the real digest from library index
    let tmp = tempfile::tempdir().unwrap();
    let idx_path = tmp.path().join("index.json");
    let index_content = format!(
        r#"{{
      "provider": "https",
      "baseUrl": "{}",
      "bundles": [{{
        "name": "preview-local-dev",
        "version": "0.0.1",
        "path": "bundles/local-dev.yaml",
        "digest": {{"sha256": "f691d7f0acf56b000bea35321d5dcdfcdc56a0f2f033f49840b86e2438d59445"}},
        "sig": {{"ed25519": "azOENOcSL/BhHwi9TAZQwrCpyR4GYml9kHgJUp9wYrNSoixdog7rF6VJvDYp4JkvO2BJzppRLwDh27Ik38kfCQ"}},
        "pubKeyId": "preview"
      }}]
    }}"#,
        server.url("")
    );
    fs::write(&idx_path, index_content).unwrap();

    // Resolve the bundle
    let resolved = bootstrapper_demonctl::libindex::resolve_https(
        "lib://https/preview-local-dev@0.0.1",
        &idx_path,
    )
    .unwrap();

    assert_eq!(resolved.provider, "https");
    assert_eq!(resolved.name, "preview-local-dev");
    assert_eq!(resolved.version, "0.0.1");
    assert!(resolved.path.exists());

    // Verify content was downloaded and digest matches
    let content = fs::read_to_string(&resolved.path).unwrap();
    assert!(content.contains("nats:"));
    assert!(content.contains("operateUi:"));
}

#[test]
fn libindex_https_digest_mismatch() {
    let _guard = HTTPS_RESOLVE_MUTEX.lock().unwrap();
    // Start a test HTTP server
    let server = Server::run();

    // Use different content than what the digest expects
    let tampered_bundle_yaml = r#"
# This is completely different content that will produce a different digest
malicious:
  content: "This is not the bundle you're looking for"
  hacker: "evil@badactor.com"
"#;

    // Set up expectation for bundle download with tampered content
    server.expect(
        Expectation::matching(request::method_path("GET", "/bundles/local-dev.yaml"))
            .respond_with(status_code(200).body(tampered_bundle_yaml)),
    );

    // Create temp index with HTTPS provider using the real digest (which won't match tampered content)
    let tmp = tempfile::tempdir().unwrap();
    let idx_path = tmp.path().join("index.json");
    let index_content = format!(
        r#"{{
      "provider": "https",
      "baseUrl": "{}",
      "bundles": [{{
        "name": "preview-local-dev",
        "version": "0.0.1",
        "path": "bundles/local-dev.yaml",
        "digest": {{"sha256": "f691d7f0acf56b000bea35321d5dcdfcdc56a0f2f033f49840b86e2438d59445"}},
        "sig": {{"ed25519": "azOENOcSL/BhHwi9TAZQwrCpyR4GYml9kHgJUp9wYrNSoixdog7rF6VJvDYp4JkvO2BJzppRLwDh27Ik38kfCQ"}},
        "pubKeyId": "preview"
      }}]
    }}"#,
        server.url("")
    );
    fs::write(&idx_path, index_content).unwrap();

    // Try to resolve the bundle - should fail with digest mismatch
    let err = bootstrapper_demonctl::libindex::resolve_https(
        "lib://https/preview-local-dev@0.0.1",
        &idx_path,
    )
    .unwrap_err();

    let msg = format!("{}", err);
    assert!(msg.contains("digest mismatch"));
    assert!(msg.contains("preview-local-dev@0.0.1"));
    assert!(msg.contains("expected"));
    assert!(msg.contains("got"));
}

#[test]
fn libindex_https_http_error() {
    // Start a test HTTP server
    let server = Server::run();

    // Set up expectation for 404 error
    server.expect(
        Expectation::matching(request::method_path("GET", "/bundles/missing.yaml"))
            .respond_with(status_code(404)),
    );

    // Create temp index with HTTPS provider
    let tmp = tempfile::tempdir().unwrap();
    let idx_path = tmp.path().join("index.json");
    let index_content = format!(
        r#"{{
      "provider": "https",
      "baseUrl": "{}",
      "bundles": [{{
        "name": "missing-bundle",
        "version": "1.0.0",
        "path": "bundles/missing.yaml",
        "digest": {{"sha256": "0000000000000000000000000000000000000000000000000000000000000000"}},
        "sig": {{"ed25519": "AA"}},
        "pubKeyId": "preview"
      }}]
    }}"#,
        server.url("")
    );
    fs::write(&idx_path, index_content).unwrap();

    // Try to resolve - should fail with HTTP error
    let err = bootstrapper_demonctl::libindex::resolve_https(
        "lib://https/missing-bundle@1.0.0",
        &idx_path,
    )
    .unwrap_err();

    let msg = format!("{}", err);
    assert!(msg.contains("HTTP error 404"));
}

#[test]
fn libindex_https_missing_baseurl() {
    // Create temp index without baseUrl
    let tmp = tempfile::tempdir().unwrap();
    let idx_path = tmp.path().join("index.json");
    let index_content = r#"{
      "provider": "https",
      "bundles": [{
        "name": "test",
        "version": "1.0.0",
        "path": "test.yaml",
        "digest": {"sha256": "0000000000000000000000000000000000000000000000000000000000000000"},
        "sig": {"ed25519": "AA"},
        "pubKeyId": "preview"
      }]
    }"#;
    fs::write(&idx_path, index_content).unwrap();

    // Try to resolve - should fail due to missing baseUrl
    let err = bootstrapper_demonctl::libindex::resolve_https("lib://https/test@1.0.0", &idx_path)
        .unwrap_err();

    let msg = format!("{}", err);
    assert!(msg.contains("baseUrl") && msg.contains("required"));
}

#[test]
fn libindex_generic_resolve() {
    // Test that the generic resolve function handles both local and https
    let idx_path = repo_path("bootstrapper/library/index.json");

    // Test local resolution
    let local_resolved =
        bootstrapper_demonctl::libindex::resolve("lib://local/preview-local-dev@0.0.1", &idx_path)
            .unwrap();
    assert_eq!(local_resolved.provider, "local");

    // Test unsupported scheme
    let err = bootstrapper_demonctl::libindex::resolve("lib://unsupported/test@1.0.0", &idx_path)
        .unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("unsupported URI scheme"));
}
