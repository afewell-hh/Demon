use std::{env, fs, path::PathBuf};

#[test]
fn bundle_parses_defaults_and_env_interpolation() {
    let dir = env::temp_dir();
    let path: PathBuf = dir.join("bundle_test.yaml");
    fs::write(
        &path,
        r#"nats:
  url: "${NATS_URL:-nats://127.0.0.1:4222}"
stream:
  name: "${RITUAL_STREAM_NAME:-RITUAL_EVENTS}"
  subjects: ["${RITUAL_SUBJECTS:-demon.ritual.v1.>}"]
  duplicateWindowSeconds: ${RITUAL_DUPWIN_SECONDS:-120}
operateUi:
  baseUrl: "${OPERATE_UI_URL:-http://127.0.0.1:3000}"
seed:
  enabled: true
"#,
    )
    .unwrap();
    env::set_var("RITUAL_DUPWIN_SECONDS", "150");
    let b = bootstrapper_demonctl::bundle::load_bundle(&path).unwrap();
    assert_eq!(b.stream.duplicate_window_seconds, 150);
    assert_eq!(b.stream.subjects[0], "demon.ritual.v1.>");
}
