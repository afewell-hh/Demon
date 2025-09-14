#[tokio::test]
async fn templates_compile_and_render_minimal() {
    // Compile templates from crate-absolute path
    let pattern = format!("{}/templates/**/*.html", env!("CARGO_MANIFEST_DIR"));
    let mut tera = tera::Tera::new(&pattern).expect("templates should compile");
    // Register filters to mirror runtime
    let tojson = |value: &tera::Value,
                  _: &std::collections::HashMap<String, tera::Value>|
     -> tera::Result<tera::Value> {
        Ok(tera::Value::String(
            serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".into()),
        ))
    };
    tera.register_filter("json", tojson);
    tera.register_filter("tojson", tojson);

    // Render runs list with minimal context
    let mut ctx = tera::Context::new();
    ctx.insert("runs", &Vec::<serde_json::Value>::new());
    ctx.insert("error", &Option::<String>::None);
    ctx.insert("jetstream_available", &false);
<<<<<<< HEAD
    ctx.insert("stream_ready", &false);
=======
>>>>>>> origin/main
    ctx.insert("current_page", &"runs");

    let html = tera
        .render("runs_list.html", &ctx)
        .expect("runs_list.html should render");
    assert!(html.contains("Recent Runs"));

    // Render run detail with minimal context
    let mut ctx2 = tera::Context::new();
    ctx2.insert("run", &Option::<serde_json::Value>::None);
    ctx2.insert("error", &Option::<String>::None);
    ctx2.insert("jetstream_available", &false);
    ctx2.insert("run_id", &"test-run-123");
    ctx2.insert("current_page", &"runs");

    let html2 = tera
        .render("run_detail.html", &ctx2)
        .expect("run_detail.html should render");
    assert!(html2.contains("Run test-run-123"));
<<<<<<< HEAD
=======

    // Smoke: approvals expired deny label renders
    let mut ctx3 = tera::Context::new();
    ctx3.insert(
        "run",
        &serde_json::json!({
            "run_id": "run-x",
            "ritual_id": "ritual-x",
            "events": []
        }),
    );
    ctx3.insert("jetstream_available", &true);
    ctx3.insert("run_id", &"run-x");
    ctx3.insert("current_page", &"runs");
    // Fields normally provided by handler
    ctx3.insert("run_status", &"Running");
    ctx3.insert("run_status_class", &"status-running");
    ctx3.insert(
        "approvals",
        &serde_json::json!({
            "status": "Denied — expired",
            "statusClass": "status-failed",
            "gateId": "g1",
            "approver": "system",
            "reason": "expired"
        }),
    );
    let html3 = tera
        .render("run_detail.html", &ctx3)
        .expect("run_detail.html should render with approvals");
    assert!(html3.contains("Denied — expired"));
>>>>>>> origin/main
}
