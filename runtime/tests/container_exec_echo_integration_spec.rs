#![cfg(unix)]

use capsules_echo::echo;
use runtime::link::router::Router;
use serde_json::json;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::TempDir;

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn env_guard() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

const RUNTIME_SCRIPT: &str = r#"#!/bin/sh
set -eu
mode="${TEST_RUNTIME_MODE:-success}"
host="${TEST_ENVELOPE_HOST_PATH:?missing}"
case "$mode" in
  success)
    cat "${TEST_ENVELOPE_SOURCE:?missing}" > "$host"
    echo "capsule stdout"
    echo "capsule stderr" >&2
    exit "${TEST_EXIT_CODE:-0}"
    ;;
  fail)
    echo "capsule failed" >&2
    exit "${TEST_EXIT_CODE:-1}"
    ;;
  sleep)
    sleep "${TEST_SLEEP_SECS:-5}"
    cat "${TEST_ENVELOPE_SOURCE:?missing}" > "$host"
    exit 0
    ;;
  missing)
    rm -f "$host"
    echo "capsule missing envelope" >&2
    exit "${TEST_EXIT_CODE:-0}"
    ;;
  *)
    echo "unknown mode $mode" >&2
    exit 2
    ;;
esac
"#;

struct RuntimeFixture {
    _temp: TempDir,
    script: PathBuf,
    artifacts_dir: PathBuf,
    host_envelope: PathBuf,
    stub_source: PathBuf,
}

impl RuntimeFixture {
    fn new(envelope: &serde_json::Value) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("runtime.sh");
        fs::write(&script, RUNTIME_SCRIPT).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

        let artifacts_dir = temp.path().join("artifacts");
        fs::create_dir_all(&artifacts_dir).unwrap();
        let host_envelope = artifacts_dir.join("result.json");

        let stub_source = temp.path().join("stub.json");
        fs::write(&stub_source, serde_json::to_vec(envelope).unwrap()).unwrap();

        Self {
            _temp: temp,
            script,
            artifacts_dir,
            host_envelope,
            stub_source,
        }
    }

    fn script(&self) -> &Path {
        &self.script
    }

    fn artifacts_dir(&self) -> &Path {
        &self.artifacts_dir
    }

    fn host_envelope(&self) -> &Path {
        &self.host_envelope
    }

    fn stub_source(&self) -> &Path {
        &self.stub_source
    }
}

#[tokio::test]
async fn container_exec_echo_capsule_round_trips_envelope() {
    let message = "Hello from container exec integration!";
    let envelope = echo(message.to_string());
    let envelope_json = serde_json::to_value(&envelope).expect("serialize envelope");
    let fixture = RuntimeFixture::new(&envelope_json);

    // Set up environment with guard, then drop it before await
    {
        let _guard = env_guard();
        env::set_var(
            "DEMON_CONTAINER_RUNTIME",
            fixture.script().to_string_lossy().to_string(),
        );
        env::set_var(
            "TEST_ENVELOPE_HOST_PATH",
            fixture.host_envelope().to_string_lossy().to_string(),
        );
        env::set_var(
            "TEST_ENVELOPE_SOURCE",
            fixture.stub_source().to_string_lossy().to_string(),
        );
        env::set_var("TEST_RUNTIME_MODE", "success");
        env::set_var("TEST_EXIT_CODE", "0");
    } // Guard dropped here

    let router = Router::new();
    let args = json!({
        "imageDigest": "ghcr.io/demo/app@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "command": ["/bin/run"],
        "env": {},
        "outputs": { "envelopePath": "/workspace/.artifacts/result.json" },
        "capsuleName": "echo",
        "artifactsDir": fixture.artifacts_dir().to_string_lossy(),
        "timeoutSeconds": 5
    });

    let response = router
        .dispatch("container-exec", &args, "run-echo", "ritual-echo")
        .await
        .expect("container exec dispatch");

    assert_eq!(response["result"]["success"], true);
    assert_eq!(response["result"]["data"]["echoed_message"], message);

    for key in [
        "DEMON_CONTAINER_RUNTIME",
        "TEST_ENVELOPE_HOST_PATH",
        "TEST_ENVELOPE_SOURCE",
        "TEST_RUNTIME_MODE",
        "TEST_EXIT_CODE",
    ] {
        env::remove_var(key);
    }
}
