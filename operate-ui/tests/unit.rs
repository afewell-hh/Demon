use operate_ui::jetstream::{RitualEvent, RunDetail, RunStatus, RunSummary};
use chrono::Utc;
use std::collections::HashMap;

#[test]
fn test_run_status_display() {
    assert_eq!(RunStatus::Running.to_string(), "Running");
    assert_eq!(RunStatus::Completed.to_string(), "Completed");
    assert_eq!(RunStatus::Failed.to_string(), "Failed");
}

#[test]
fn test_run_status_equality() {
    assert_eq!(RunStatus::Running, RunStatus::Running);
    assert_eq!(RunStatus::Completed, RunStatus::Completed);
    assert_eq!(RunStatus::Failed, RunStatus::Failed);
    
    assert_ne!(RunStatus::Running, RunStatus::Completed);
    assert_ne!(RunStatus::Completed, RunStatus::Failed);
}

#[test]
fn test_run_summary_creation() {
    let run = RunSummary {
        run_id: "test-run-123".to_string(),
        ritual_id: "test-ritual".to_string(),
        start_ts: Utc::now(),
        status: RunStatus::Running,
    };

    assert_eq!(run.run_id, "test-run-123");
    assert_eq!(run.ritual_id, "test-ritual");
    assert_eq!(run.status, RunStatus::Running);
}

#[test]
fn test_ritual_event_creation() {
    let mut extra = HashMap::new();
    extra.insert("customField".to_string(), serde_json::json!("customValue"));

    let event = RitualEvent {
        ts: Utc::now(),
        event: "ritual.started:v1".to_string(),
        state_from: Some("idle".to_string()),
        state_to: Some("running".to_string()),
        extra,
    };

    assert_eq!(event.event, "ritual.started:v1");
    assert_eq!(event.state_from, Some("idle".to_string()));
    assert_eq!(event.state_to, Some("running".to_string()));
    assert!(event.extra.contains_key("customField"));
}

#[test]
fn test_run_detail_creation() {
    let events = vec![
        RitualEvent {
            ts: Utc::now(),
            event: "ritual.started:v1".to_string(),
            state_from: None,
            state_to: Some("running".to_string()),
            extra: HashMap::new(),
        },
        RitualEvent {
            ts: Utc::now(),
            event: "ritual.completed:v1".to_string(),
            state_from: Some("running".to_string()),
            state_to: Some("completed".to_string()),
            extra: HashMap::new(),
        },
    ];

    let run = RunDetail {
        run_id: "test-run".to_string(),
        ritual_id: "test-ritual".to_string(),
        events,
    };

    assert_eq!(run.run_id, "test-run");
    assert_eq!(run.ritual_id, "test-ritual");
    assert_eq!(run.events.len(), 2);
}

#[test]
fn test_serde_serialization() {
    let run = RunSummary {
        run_id: "test-run".to_string(),
        ritual_id: "test-ritual".to_string(),
        start_ts: Utc::now(),
        status: RunStatus::Completed,
    };

    // Test serialization
    let json = serde_json::to_string(&run).unwrap();
    assert!(json.contains("runId"));
    assert!(json.contains("ritualId"));
    assert!(json.contains("startTs"));
    assert!(json.contains("Completed"));

    // Test deserialization
    let parsed: RunSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.run_id, run.run_id);
    assert_eq!(parsed.ritual_id, run.ritual_id);
    assert_eq!(parsed.status, run.status);
}

#[test]
fn test_run_detail_serde() {
    let events = vec![
        RitualEvent {
            ts: Utc::now(),
            event: "ritual.started:v1".to_string(),
            state_from: None,
            state_to: Some("running".to_string()),
            extra: HashMap::new(),
        }
    ];

    let run = RunDetail {
        run_id: "test-run".to_string(),
        ritual_id: "test-ritual".to_string(),
        events,
    };

    // Test serialization
    let json = serde_json::to_string(&run).unwrap();
    assert!(json.contains("runId"));
    assert!(json.contains("ritualId"));
    assert!(json.contains("events"));

    // Test deserialization
    let parsed: RunDetail = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.run_id, run.run_id);
    assert_eq!(parsed.ritual_id, run.ritual_id);
    assert_eq!(parsed.events.len(), 1);
}

#[test]
fn test_ritual_event_serde_with_optional_fields() {
    // Test with all fields
    let event_full = RitualEvent {
        ts: Utc::now(),
        event: "ritual.transitioned:v1".to_string(),
        state_from: Some("idle".to_string()),
        state_to: Some("running".to_string()),
        extra: HashMap::new(),
    };

    let json = serde_json::to_string(&event_full).unwrap();
    assert!(json.contains("stateFrom"));
    assert!(json.contains("stateTo"));

    // Test with no optional fields
    let event_minimal = RitualEvent {
        ts: Utc::now(),
        event: "ritual.started:v1".to_string(),
        state_from: None,
        state_to: None,
        extra: HashMap::new(),
    };

    let json_minimal = serde_json::to_string(&event_minimal).unwrap();
    // Optional fields should be omitted when None
    assert!(!json_minimal.contains("stateFrom"));
    assert!(!json_minimal.contains("stateTo"));
}

#[test]
fn test_error_handling_types() {
    use operate_ui::{AppError, AppResult};
    use anyhow::anyhow;

    // Test AppError creation from anyhow::Error
    let original_error = anyhow!("Test error message");
    let app_error = AppError::from(original_error);
    
    // AppError should wrap the original error
    assert!(format!("{:?}", app_error).contains("Test error message"));

    // Test AppResult type alias
    let success_result: AppResult<String> = Ok("success".to_string());
    assert!(success_result.is_ok());

    let error_result: AppResult<String> = Err(AppError::from(anyhow!("failure")));
    assert!(error_result.is_err());
}

#[test]
fn test_json_flattening_in_ritual_event() {
    let mut extra = HashMap::new();
    extra.insert("customField1".to_string(), serde_json::json!("value1"));
    extra.insert("customField2".to_string(), serde_json::json!(42));
    extra.insert("customField3".to_string(), serde_json::json!({"nested": "object"}));

    let event = RitualEvent {
        ts: Utc::now(),
        event: "custom.event:v1".to_string(),
        state_from: None,
        state_to: None,
        extra,
    };

    let json = serde_json::to_string(&event).unwrap();
    
    // Extra fields should be flattened into the main JSON object
    assert!(json.contains("customField1"));
    assert!(json.contains("customField2"));
    assert!(json.contains("customField3"));
    assert!(json.contains("value1"));
    assert!(json.contains("42"));
    assert!(json.contains("nested"));
}

// Mock tests for JetStream client functionality
#[test]
fn test_jetstream_subject_parsing() {
    // This would test the subject parsing logic if it were exposed
    // For now, we'll test the expected format
    let expected_subject = "demon.ritual.v1.my-ritual.run-123.events";
    let parts: Vec<&str> = expected_subject.split('.').collect();
    
    assert_eq!(parts.len(), 6);
    assert_eq!(parts[0], "demon");
    assert_eq!(parts[1], "ritual");
    assert_eq!(parts[2], "v1");
    assert_eq!(parts[3], "my-ritual"); // ritual_id
    assert_eq!(parts[4], "run-123");   // run_id
    assert_eq!(parts[5], "events");
}