use assert_cmd::Command;
use httptest::{matchers::*, responders::*, Expectation, Server};
use predicates::prelude::*;
use serde_json::{json, Value};
use std::io::Write;
use tempfile::TempDir;
use zip::write::FileOptions;

fn create_manifest_zip() -> (Vec<u8>, Value) {
    let manifest = json!({
        "operate-ui": {
            "repository": "ghcr.io/acme/demon-operate-ui",
            "digest": "sha256:aaa",
            "image": "ghcr.io/acme/demon-operate-ui@sha256:aaa",
            "gitShaTag": "ghcr.io/acme/demon-operate-ui:sha-deadbeef",
        },
        "runtime": {
            "repository": "ghcr.io/acme/demon-runtime",
            "digest": "sha256:bbb",
            "image": "ghcr.io/acme/demon-runtime@sha256:bbb",
            "gitShaTag": "ghcr.io/acme/demon-runtime:sha-deadbeef",
        },
        "engine": {
            "repository": "ghcr.io/acme/demon-engine",
            "digest": "sha256:ccc",
            "image": "ghcr.io/acme/demon-engine@sha256:ccc",
            "gitShaTag": "ghcr.io/acme/demon-engine:sha-deadbeef",
        }
    });

    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    writer
        .start_file("docker-image-digests.json", FileOptions::default())
        .unwrap();
    writer.write_all(manifest.to_string().as_bytes()).unwrap();
    let cursor = writer.finish().unwrap();
    (cursor.into_inner(), manifest)
}

fn setup_server() -> (Server, Value) {
    let server = Server::run();
    let (zip_bytes, manifest) = create_manifest_zip();

    server.expect(
        Expectation::matching(all_of![
            request::method_path(
                "GET",
                "/repos/acme/demon/actions/workflows/docker-build.yml/runs",
            ),
            request::query(url_decoded(contains(("branch", "main")))),
            request::query(url_decoded(contains(("status", "success")))),
            request::query(url_decoded(contains(("per_page", "1")))),
            request::headers(contains(("authorization", "Bearer fake-token"))),
            request::headers(contains(("user-agent", "demonctl-ghcr-digests"))),
        ])
        .respond_with(json_encoded(json!({
            "workflow_runs": [
                {
                    "id": 123,
                    "run_number": 77,
                    "html_url": "https://github.com/acme/demon/actions/runs/123",
                    "display_title": "docker-build",
                }
            ]
        }))),
    );

    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/repos/acme/demon/actions/runs/123/artifacts",),
            request::headers(contains(("authorization", "Bearer fake-token"))),
        ])
        .respond_with(json_encoded(json!({
            "total_count": 1,
            "artifacts": [
                { "id": 456, "name": "docker-image-digests" }
            ]
        }))),
    );

    server.expect(
        Expectation::matching(all_of![
            request::method_path("GET", "/repos/acme/demon/actions/artifacts/456/zip",),
            request::headers(contains(("authorization", "Bearer fake-token"))),
            request::headers(contains(("accept", "application/vnd.github+json"))),
        ])
        .respond_with(
            status_code(200)
                .append_header("Content-Type", "application/zip")
                .body(zip_bytes.clone()),
        ),
    );

    (server, manifest)
}

#[test]
fn docker_digests_fetch_env_exports_and_writes_manifest() {
    let (server, manifest) = setup_server();
    let api_url = format!("http://{}/", server.addr());
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("manifest.json");

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.env("GH_TOKEN", "fake-token")
        .env("DEMONCTL_GITHUB_API_URL", api_url)
        .env("DEMONCTL_GITHUB_REPOSITORY", "acme/demon")
        .arg("docker")
        .arg("digests")
        .arg("fetch")
        .arg("--format")
        .arg("env")
        .arg("--output")
        .arg(output_path.to_string_lossy().to_string());

    cmd.assert()
        .success()
        .stdout(
            predicate::str::contains(
                "export OPERATE_UI_IMAGE_TAG=ghcr.io/acme/demon-operate-ui@sha256:aaa",
            )
            .and(predicate::str::contains(
                "export RUNTIME_IMAGE_TAG=ghcr.io/acme/demon-runtime@sha256:bbb",
            ))
            .and(predicate::str::contains(
                "export ENGINE_IMAGE_TAG=ghcr.io/acme/demon-engine@sha256:ccc",
            )),
        )
        .stderr(predicate::str::contains("Manifest written to"));

    let saved = std::fs::read_to_string(&output_path).unwrap();
    let saved_json: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved_json, manifest);
}

#[test]
fn docker_digests_fetch_requires_token() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("docker")
        .arg("digests")
        .arg("fetch")
        .env_remove("GH_TOKEN");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("GH_TOKEN"));
}
