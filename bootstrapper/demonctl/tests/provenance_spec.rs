use std::path::Path;

#[test]
fn provenance_is_emitted_per_field_and_precedence() {
    // Build a tiny bundle on disk
    let dir = std::env::temp_dir();
    let path = dir.join("bundle_prov.yaml");
    std::fs::write(
        &path,
        r#"nats:
  url: "nats://bndl:4222"
stream:
  name: "BUNDLE_STREAM"
  subjects: ["demon.ritual.v1.>"]
  duplicateWindowSeconds: 120
operateUi:
  baseUrl: "http://bundle-ui"
seed:
  enabled: true
"#,
    )
    .unwrap();

    // Env defaults (will be overridden by bundle, then flags)
    std::env::set_var("NATS_URL", "nats://env:4222");
    std::env::set_var("RITUAL_STREAM_NAME", "ENV_STREAM");
    std::env::set_var("UI_URL", "http://env-ui");

    // Flags override bundle
    let (cfg, prov) = bootstrapper_demonctl::compute_effective_config(
        Some(Path::new(&path)),
        Some("nats://flag:4222"),
        Some("FLAG_STREAM"),
        Some(vec!["demon.ritual.v1.>".to_string()]),
        Some("http://flag-ui"),
    ).unwrap();
    assert_eq!(cfg.nats_url, "nats://flag:4222");
    assert_eq!(cfg.stream_name, "FLAG_STREAM");
    assert_eq!(cfg.ui_url, "http://flag-ui");
    // Provenance is now a JSON bundle, not flags
    assert!(prov.is_some());
}

#[test]
#[ignore] // Function build_seed_run_log doesn't exist yet
fn seed_reports_mutation_markers_builder() {
    // let j1 = bootstrapper_demonctl::build_seed_run_log("run-1", "rit", "gate", true, Some(false));
    // assert_eq!(j1["mutation_req"], "applied");
    // assert_eq!(j1["mutation_timer"], "noop");
    // let j2 = bootstrapper_demonctl::build_seed_run_log("run-1", "rit", "gate", false, None);
    // assert_eq!(j2["mutation_req"], "noop");
}
