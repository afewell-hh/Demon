use chrono::Utc;
use envelope::{
    AsEnvelope, Diagnostic, DiagnosticLevel, DurationMetrics, JsonPatchOperation, Metrics,
    ResultEnvelope, Suggestion, SuggestionPriority,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result structure for echo capsule operations
#[derive(Serialize, Deserialize, AsEnvelope, Debug)]
pub struct EchoResult {
    pub echoed_message: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub character_count: usize,
}

/// Minimal echo capsule. In later milestones this will be a WASM component
/// with capability-scoped interfaces. For M0 we keep it as a native lib.
pub fn echo(message: String) -> ResultEnvelope<EchoResult> {
    let start = std::time::Instant::now();
    let timestamp = Utc::now();

    println!("{message}");

    let character_count = message.chars().count();
    let result = EchoResult {
        echoed_message: message.clone(),
        timestamp,
        character_count,
    };

    let mut builder = ResultEnvelope::builder()
        .success(result)
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Info,
            format!(
                "Echo operation completed at {}",
                timestamp.format("%Y-%m-%d %H:%M:%S UTC")
            ),
        ))
        .with_source_info("echo-capsule", Some("0.0.1"), None::<String>);

    // Add diagnostics based on input content
    if message.is_empty() {
        builder = builder.add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Warning,
            "Input message was empty".to_string(),
        ));
    }

    if character_count > 100 {
        builder = builder.add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Warning,
            format!("Message is quite long ({} characters)", character_count),
        ));
    }

    // Add suggestions when relevant
    if message.trim() != message {
        let suggestion = Suggestion::optimization("Consider trimming whitespace from input")
            .with_priority(SuggestionPriority::Low)
            .with_rationale("Leading/trailing whitespace detected")
            .with_patch(vec![JsonPatchOperation::replace(
                "/message",
                serde_json::Value::String(message.trim().to_string()),
            )])
            .build();

        builder = builder.add_suggestion(suggestion);
    }

    let duration = start.elapsed();
    let mut counters = HashMap::new();
    counters.insert("characterCount".to_string(), character_count as i64);

    let metrics = Metrics {
        duration: Some(DurationMetrics {
            total_ms: Some(duration.as_secs_f64() * 1000.0),
            phases: HashMap::new(),
        }),
        resources: None,
        counters,
        custom: None,
    };

    builder = builder.metrics(metrics);

    builder.build().expect("Valid envelope")
}

#[cfg(test)]
mod tests {

    #[test]
    fn echoes_simple_message() {
        let msg = "hi".to_string();
        let envelope = super::echo(msg.clone());

        assert!(envelope.result.is_success());
        if let envelope::OperationResult::Success { data, .. } = &envelope.result {
            assert_eq!(data.echoed_message, msg);
            assert_eq!(data.character_count, 2);
        } else {
            panic!("Expected success result");
        }
        assert!(!envelope.diagnostics.is_empty());
        assert!(envelope.suggestions.is_empty());
    }

    #[test]
    fn echoes_empty_message_with_warning() {
        let msg = "".to_string();
        let envelope = super::echo(msg);

        assert!(envelope.result.is_success());
        if let envelope::OperationResult::Success { data, .. } = &envelope.result {
            assert_eq!(data.echoed_message, "");
            assert_eq!(data.character_count, 0);
        } else {
            panic!("Expected success result");
        }

        // Should have warning about empty message
        let warnings: Vec<_> = envelope
            .diagnostics
            .iter()
            .filter(|d| matches!(d.level, envelope::DiagnosticLevel::Warning))
            .collect();
        assert!(!warnings.is_empty());
    }

    #[test]
    fn echoes_long_message_with_warning() {
        let msg = "a".repeat(150);
        let envelope = super::echo(msg.clone());

        assert!(envelope.result.is_success());
        if let envelope::OperationResult::Success { data, .. } = &envelope.result {
            assert_eq!(data.echoed_message, msg);
            assert_eq!(data.character_count, 150);
        } else {
            panic!("Expected success result");
        }

        // Should have warning about long message
        let warnings: Vec<_> = envelope
            .diagnostics
            .iter()
            .filter(|d| matches!(d.level, envelope::DiagnosticLevel::Warning))
            .collect();
        assert!(!warnings.is_empty());
    }

    #[test]
    fn echoes_message_with_whitespace_suggestion() {
        let msg = "  hello world  ".to_string();
        let envelope = super::echo(msg.clone());

        assert!(envelope.result.is_success());
        if let envelope::OperationResult::Success { data, .. } = &envelope.result {
            assert_eq!(data.echoed_message, msg);
        } else {
            panic!("Expected success result");
        }
        assert!(!envelope.suggestions.is_empty());

        // Should suggest trimming whitespace
        let optimization_suggestions: Vec<_> = envelope
            .suggestions
            .iter()
            .filter(|s| matches!(s.suggestion_type, envelope::SuggestionType::Optimization))
            .collect();
        assert!(!optimization_suggestions.is_empty());
    }

    #[test]
    fn envelope_validates() {
        let msg = "test message".to_string();
        let envelope = super::echo(msg);

        // Should pass schema validation
        envelope
            .validate()
            .expect("Envelope should validate against schema");
    }
}
// guard: third no-op
