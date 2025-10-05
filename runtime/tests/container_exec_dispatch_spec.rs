use runtime::link::router::Router;
use serde_json::json;
use std::fs::File;
use std::io::Write;

#[tokio::test]
async fn dispatch_container_exec_stub_returns_envelope() {
    let router = Router::new();
    let temp_dir = tempfile::tempdir().unwrap();
    let stub_path = temp_dir.path().join("stub-envelope.json");

    let envelope = envelope::ResultEnvelope::builder()
        .success(json!({"ok": true}))
        .build()
        .unwrap();
    let mut file = File::create(&stub_path).unwrap();
    file.write_all(serde_json::to_vec(&envelope).unwrap().as_slice())
        .unwrap();

    std::env::set_var("DEMON_CONTAINER_RUNTIME", "stub");
    std::env::set_var(
        "DEMON_CONTAINER_EXEC_STUB_ENVELOPE",
        stub_path.to_string_lossy().to_string(),
    );

    let args = json!({
        "imageDigest": "ghcr.io/demo/app@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "command": ["/bin/true"],
        "outputs": {"envelopePath": "/workspace/result.json"},
        "env": {},
        "capsuleName": "test-capsule"
    });

    let response = router
        .dispatch("container-exec", &args, "run-1", "ritual-1")
        .await
        .unwrap();

    std::env::remove_var("DEMON_CONTAINER_RUNTIME");
    std::env::remove_var("DEMON_CONTAINER_EXEC_STUB_ENVELOPE");

    assert!(response.get("result").is_some());
}
