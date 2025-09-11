use std::fs;
use std::path::{Path, PathBuf};

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
