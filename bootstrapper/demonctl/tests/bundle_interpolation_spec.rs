use std::{env, fs, path::PathBuf, sync::Mutex};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn bundle_parses_defaults_and_env_interpolation() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let prev_subjects = env::var("RITUAL_SUBJECTS").ok();
    let prev_dupwin = env::var("RITUAL_DUPWIN_SECONDS").ok();

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
    // Ensure default subjects are used by clearing any inherited env override
    env::remove_var("RITUAL_SUBJECTS");
    let b = bootstrapper_demonctl::bundle::load_bundle(&path).unwrap();
    assert_eq!(b.stream.duplicate_window_seconds, 150);
    assert_eq!(b.stream.subjects[0], "demon.ritual.v1.>");

    match prev_subjects {
        Some(v) => env::set_var("RITUAL_SUBJECTS", v),
        None => env::remove_var("RITUAL_SUBJECTS"),
    }
    match prev_dupwin {
        Some(v) => env::set_var("RITUAL_DUPWIN_SECONDS", v),
        None => env::remove_var("RITUAL_DUPWIN_SECONDS"),
    }
}

#[test]
fn precedence_flags_override_bundle() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let dir = env::temp_dir();
    let path: PathBuf = dir.join("precedence_test.yaml");
    fs::write(
        &path,
        r#"nats:
  url: "nats://bundle.example.com:4222"
stream:
  name: "BUNDLE_STREAM"
  subjects: ["bundle.>"]
operateUi:
  baseUrl: "http://bundle.example.com:3000"
seed:
  enabled: false
"#,
    )
    .unwrap();

    // Test that command line flags override bundle values
    let (cfg, _) = bootstrapper_demonctl::compute_effective_config(
        Some(&path),
        Some("nats://flag.example.com:4222"), // should override bundle
        Some("FLAG_STREAM"),                  // should override bundle
        Some(vec!["flag.>".to_string()]),     // should override bundle
        Some("http://flag.example.com:3000"), // should override bundle
    )
    .unwrap();

    assert_eq!(cfg.nats_url, "nats://flag.example.com:4222");
    assert_eq!(cfg.stream_name, "FLAG_STREAM");
    assert_eq!(cfg.subjects, vec!["flag.>"]);
    assert_eq!(cfg.ui_url, "http://flag.example.com:3000");
}

#[test]
fn precedence_bundle_overrides_env() {
    let _guard = ENV_MUTEX.lock().unwrap();
    // Set environment variables
    env::set_var("NATS_URL", "nats://env.example.com:4222");
    env::set_var("RITUAL_STREAM_NAME", "ENV_STREAM");
    env::set_var("RITUAL_SUBJECTS", "env.>");
    env::set_var("UI_URL", "http://env.example.com:3000");

    let dir = env::temp_dir();
    let path: PathBuf = dir.join("precedence_bundle_test.yaml");
    fs::write(
        &path,
        r#"nats:
  url: "nats://bundle.example.com:4222"
stream:
  name: "BUNDLE_STREAM"
  subjects: ["bundle.>"]
operateUi:
  baseUrl: "${TEST_UI_URL:-http://bundle.example.com:3000}"
seed:
  enabled: false
"#,
    )
    .unwrap();

    // Set the environment variable to ensure the bundle value is used
    env::set_var("TEST_UI_URL", "http://bundle.example.com:3000");

    // Test that bundle values override environment variables
    let (cfg, _provenance) = bootstrapper_demonctl::compute_effective_config(
        Some(&path),
        None, // no flag overrides
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(cfg.nats_url, "nats://bundle.example.com:4222");
    assert_eq!(cfg.stream_name, "BUNDLE_STREAM");
    assert_eq!(cfg.subjects, vec!["bundle.>"]);
    assert_eq!(cfg.ui_url, "http://bundle.example.com:3000");

    // Clean up env vars
    env::remove_var("NATS_URL");
    env::remove_var("RITUAL_STREAM_NAME");
    env::remove_var("RITUAL_SUBJECTS");
    env::remove_var("UI_URL");
    env::remove_var("TEST_UI_URL");
}

#[test]
fn precedence_env_as_fallback() {
    let _guard = ENV_MUTEX.lock().unwrap();
    // Set environment variables
    env::set_var("NATS_URL", "nats://env.example.com:4222");
    env::set_var("RITUAL_STREAM_NAME", "ENV_STREAM");
    env::set_var("RITUAL_SUBJECTS", "env.>");
    env::set_var("UI_URL", "http://env.example.com:3000");

    // Test that environment variables are used when no bundle is provided
    let (cfg, _) = bootstrapper_demonctl::compute_effective_config(
        None, // no bundle
        None, // no flag overrides
        None, None, None,
    )
    .unwrap();

    assert_eq!(cfg.nats_url, "nats://env.example.com:4222");
    assert_eq!(cfg.stream_name, "ENV_STREAM");
    assert_eq!(cfg.subjects, vec!["env.>"]);
    assert_eq!(cfg.ui_url, "http://env.example.com:3000");

    // Clean up env vars
    env::remove_var("NATS_URL");
    env::remove_var("RITUAL_STREAM_NAME");
    env::remove_var("RITUAL_SUBJECTS");
    env::remove_var("UI_URL");
}

#[test]
fn profile_default_bundle_resolution() {
    let local_dev_bundle = bootstrapper_demonctl::get_default_bundle_for_profile(
        &bootstrapper_demonctl::Profile::LocalDev,
    );
    let remote_nats_bundle = bootstrapper_demonctl::get_default_bundle_for_profile(
        &bootstrapper_demonctl::Profile::RemoteNats,
    );

    // The function should resolve to a working path, regardless of exact directory structure
    assert!(local_dev_bundle.is_some());
    assert!(remote_nats_bundle.is_some());

    let local_path = local_dev_bundle.unwrap();
    let remote_path = remote_nats_bundle.unwrap();

    assert!(local_path.ends_with("local-dev.yaml"));
    assert!(remote_path.ends_with("remote-nats.yaml"));

    // Verify the files actually exist
    assert!(std::path::Path::new(&local_path).exists());
    assert!(std::path::Path::new(&remote_path).exists());
}
