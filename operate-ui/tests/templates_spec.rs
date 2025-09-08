#[test]
fn renders_runs_list_with_empty_vm() {
    let pattern = format!("{}/templates/**/*.html", env!("CARGO_MANIFEST_DIR"));
    let mut tera = tera::Tera::new(&pattern).expect("templates should compile");
    operate_ui::templates::register_filters(&mut tera);
    let ctx = tera::Context::from_serialize(&serde_json::json!({
        "runs": [],
        "error": null,
        "jetstream_available": false,
        "current_page": "runs"
    }))
    .unwrap();
    let html = tera.render("runs_list.html", &ctx).unwrap();
    assert!(html.contains("Recent Runs"));
}

#[test]
fn renders_run_detail_with_fixture_vm() {
    let pattern = format!("{}/templates/**/*.html", env!("CARGO_MANIFEST_DIR"));
    let mut tera = tera::Tera::new(&pattern).expect("templates should compile");
    operate_ui::templates::register_filters(&mut tera);
    let ctx = tera::Context::from_serialize(&serde_json::json!({
        "run": {
            "runId": "fixture-run",
            "ritualId": "fixture-ritual",
            "events": [
                {"ts":"2025-01-01T00:00:00Z","event":"ritual.started:v1"}
            ]
        },
        "error": null,
        "jetstream_available": false,
        "run_id": "fixture-run",
        "current_page": "runs"
    }))
    .unwrap();
    let html = tera.render("run_detail.html", &ctx).unwrap();
    assert!(html.contains("fixture-run"));
}
