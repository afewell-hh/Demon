use chrono::Utc;
use envelope::{
    Diagnostic, DiagnosticLevel, DurationMetrics, JsonPatchOp, JsonPatchOperation, Metrics,
    ProcessingStep, Provenance, ResourceMetrics, SourceInfo, Suggestion, SuggestionPriority,
    SuggestionType,
};
use operate_ui::jetstream::{EnvelopeData, RitualEvent};
use serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn given_event_with_envelope_data_when_rendered_then_includes_diagnostics() {
    let event = RitualEvent {
        ts: Utc::now(),
        event: "capsule.result:v1".to_string(),
        state_from: None,
        state_to: None,
        stream_sequence: Some(1),
        envelope: Some(EnvelopeData {
            diagnostics: vec![
                Diagnostic {
                    level: DiagnosticLevel::Info,
                    message: "Starting processing".to_string(),
                    timestamp: Some(Utc::now()),
                    source: Some("processor".to_string()),
                    context: None,
                },
                Diagnostic {
                    level: DiagnosticLevel::Warning,
                    message: "High memory usage".to_string(),
                    timestamp: Some(Utc::now()),
                    source: Some("monitor".to_string()),
                    context: Some(json!({"memory_mb": 512})),
                },
            ],
            suggestions: vec![],
            metrics: None,
            provenance: None,
        }),
        extra: HashMap::new(),
    };

    assert!(event.envelope.is_some());
    let envelope = event.envelope.as_ref().unwrap();
    assert_eq!(envelope.diagnostics.len(), 2);
    assert_eq!(envelope.diagnostics[0].level, DiagnosticLevel::Info);
    assert_eq!(envelope.diagnostics[1].level, DiagnosticLevel::Warning);
}

#[tokio::test]
async fn given_event_with_suggestions_when_rendered_then_includes_patches() {
    let event = RitualEvent {
        ts: Utc::now(),
        event: "capsule.result:v1".to_string(),
        state_from: None,
        state_to: None,
        stream_sequence: Some(1),
        envelope: Some(EnvelopeData {
            diagnostics: vec![],
            suggestions: vec![Suggestion {
                suggestion_type: SuggestionType::Optimization,
                description: "Increase batch size".to_string(),
                patch: Some(vec![JsonPatchOperation {
                    op: JsonPatchOp::Replace,
                    path: "/config/batch_size".to_string(),
                    value: Some(json!(50)),
                    from: None,
                }]),
                priority: Some(SuggestionPriority::High),
                rationale: Some("Current batch size is suboptimal".to_string()),
            }],
            metrics: None,
            provenance: None,
        }),
        extra: HashMap::new(),
    };

    let envelope = event.envelope.as_ref().unwrap();
    assert_eq!(envelope.suggestions.len(), 1);
    let suggestion = &envelope.suggestions[0];
    assert_eq!(suggestion.suggestion_type, SuggestionType::Optimization);
    assert_eq!(suggestion.priority, Some(SuggestionPriority::High));
    assert!(suggestion.patch.is_some());
    let patch = suggestion.patch.as_ref().unwrap();
    assert_eq!(patch.len(), 1);
    assert_eq!(patch[0].op, JsonPatchOp::Replace);
}

#[tokio::test]
async fn given_event_with_metrics_when_rendered_then_includes_duration_and_resources() {
    let mut phases = HashMap::new();
    phases.insert("initialization".to_string(), 500.0);
    phases.insert("processing".to_string(), 70000.0);

    let mut counters = HashMap::new();
    counters.insert("items_processed".to_string(), 150);
    counters.insert("items_skipped".to_string(), 3);

    let event = RitualEvent {
        ts: Utc::now(),
        event: "capsule.result:v1".to_string(),
        state_from: None,
        state_to: None,
        stream_sequence: Some(1),
        envelope: Some(EnvelopeData {
            diagnostics: vec![],
            suggestions: vec![],
            metrics: Some(Metrics {
                duration: Some(DurationMetrics {
                    total_ms: Some(75000.0),
                    phases,
                }),
                resources: Some(ResourceMetrics {
                    memory_bytes: Some(536870912),
                    cpu_percent: Some(45.5),
                    io_operations: Some(1523),
                    additional: HashMap::new(),
                }),
                counters,
                custom: None,
            }),
            provenance: None,
        }),
        extra: HashMap::new(),
    };

    let envelope = event.envelope.as_ref().unwrap();
    assert!(envelope.metrics.is_some());
    let metrics = envelope.metrics.as_ref().unwrap();

    // Check duration metrics
    assert!(metrics.duration.is_some());
    let duration = metrics.duration.as_ref().unwrap();
    assert_eq!(duration.total_ms, Some(75000.0));
    assert_eq!(duration.phases.len(), 2);

    // Check resource metrics
    assert!(metrics.resources.is_some());
    let resources = metrics.resources.as_ref().unwrap();
    assert_eq!(resources.memory_bytes, Some(536870912)); // 512 MB
    assert_eq!(resources.cpu_percent, Some(45.5));

    // Check counters
    assert_eq!(metrics.counters.len(), 2);
    assert_eq!(metrics.counters.get("items_processed"), Some(&150));
}

#[tokio::test]
async fn given_event_with_provenance_when_rendered_then_includes_chain() {
    let event = RitualEvent {
        ts: Utc::now(),
        event: "capsule.result:v1".to_string(),
        state_from: None,
        state_to: None,
        stream_sequence: Some(1),
        envelope: Some(EnvelopeData {
            diagnostics: vec![],
            suggestions: vec![],
            metrics: None,
            provenance: Some(Provenance {
                source: Some(SourceInfo {
                    system: "demon-processor".to_string(),
                    version: Some("1.2.3".to_string()),
                    instance: Some("processor-west-01".to_string()),
                }),
                timestamp: Some(Utc::now()),
                trace_id: Some("4bf92f3577b34da6a3ce929d0e0e4736".to_string()),
                span_id: Some("00f067aa0ba902b7".to_string()),
                parent_span_id: None,
                chain: vec![
                    ProcessingStep {
                        step: "request_received".to_string(),
                        timestamp: Utc::now(),
                        actor: Some("api-gateway".to_string()),
                        signature: None,
                    },
                    ProcessingStep {
                        step: "processing_completed".to_string(),
                        timestamp: Utc::now(),
                        actor: Some("processor".to_string()),
                        signature: Some("SHA256:abc123...".to_string()),
                    },
                ],
            }),
        }),
        extra: HashMap::new(),
    };

    let envelope = event.envelope.as_ref().unwrap();
    assert!(envelope.provenance.is_some());
    let provenance = envelope.provenance.as_ref().unwrap();

    // Check source info
    assert!(provenance.source.is_some());
    let source = provenance.source.as_ref().unwrap();
    assert_eq!(source.system, "demon-processor");
    assert_eq!(source.version, Some("1.2.3".to_string()));

    // Check trace info
    assert!(provenance.trace_id.is_some());

    // Check processing chain
    assert_eq!(provenance.chain.len(), 2);
    assert_eq!(provenance.chain[0].step, "request_received");
    assert_eq!(provenance.chain[1].step, "processing_completed");
}

#[tokio::test]
async fn given_empty_envelope_when_parsed_then_handles_gracefully() {
    let event = RitualEvent {
        ts: Utc::now(),
        event: "capsule.result:v1".to_string(),
        state_from: None,
        state_to: None,
        stream_sequence: Some(1),
        envelope: Some(EnvelopeData {
            diagnostics: vec![],
            suggestions: vec![],
            metrics: None,
            provenance: None,
        }),
        extra: HashMap::new(),
    };

    let envelope = event.envelope.as_ref().unwrap();
    assert!(envelope.diagnostics.is_empty());
    assert!(envelope.suggestions.is_empty());
    assert!(envelope.metrics.is_none());
    assert!(envelope.provenance.is_none());
}

#[tokio::test]
async fn given_event_without_envelope_when_parsed_then_envelope_is_none() {
    let event = RitualEvent {
        ts: Utc::now(),
        event: "ritual.started:v1".to_string(),
        state_from: Some("idle".to_string()),
        state_to: Some("running".to_string()),
        stream_sequence: Some(1),
        envelope: None,
        extra: HashMap::new(),
    };

    assert!(event.envelope.is_none());
}
