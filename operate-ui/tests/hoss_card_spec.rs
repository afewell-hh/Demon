use operate_ui::AppState;
use tera::Context;

#[tokio::test]
async fn json_query_maps_hoss_envelope_fields() {
    let state = AppState::new().await;
    let mut tera = state.tera;

    let envelope: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/hoss_envelope.json")).unwrap();
    let outputs = &envelope["result"]["data"]["outputs"];

    let mut context = Context::new();
    context.insert("outputs", outputs);

    let artifacts = tera
        .render_str(
            "{{ outputs | json_query(query=\"artifacts_published\") }}",
            &context,
        )
        .unwrap();
    assert!(artifacts.trim().contains("12"));

    let warnings = tera
        .render_str("{{ outputs | json_query(query=\"warnings\") }}", &context)
        .unwrap();
    assert!(warnings.trim().contains("1"));

    let primary_status = tera
        .render_str(
            "{{ outputs | json_query(query=\"cards.primaryStatus\") }}",
            &context,
        )
        .unwrap();
    assert!(primary_status.contains("Promotion"));
}
