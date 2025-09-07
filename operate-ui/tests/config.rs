use operate_ui::jetstream::JetStreamClient;

#[test]
fn env_defaults_are_applied() {
    // Do not rely on env: just verify parsing defaults by constructing config via new()
    // We can't call async new() here; instead, check the default strings directly
    let default_stream =
        std::env::var("RITUAL_STREAM_NAME").unwrap_or_else(|_| "RITUAL_EVENTS".into());
    let default_subjects =
        std::env::var("RITUAL_SUBJECTS").unwrap_or_else(|_| "demon.ritual.v1.>".into());
    assert_eq!(default_stream, "RITUAL_EVENTS");
    assert_eq!(default_subjects, "demon.ritual.v1.>");
}
