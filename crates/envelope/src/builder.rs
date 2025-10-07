use crate::envelope::*;
use chrono::Utc;
use std::collections::HashMap;

pub struct ResultEnvelopeBuilder<T> {
    result: Option<OperationResult<T>>,
    diagnostics: Vec<Diagnostic>,
    suggestions: Vec<Suggestion>,
    metrics: Option<Metrics>,
    provenance: Option<Provenance>,
}

impl<T> Default for ResultEnvelopeBuilder<T> {
    fn default() -> Self {
        Self {
            result: None,
            diagnostics: Vec::new(),
            suggestions: Vec::new(),
            metrics: None,
            provenance: None,
        }
    }
}

impl<T> ResultEnvelopeBuilder<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn result(mut self, result: OperationResult<T>) -> Self {
        self.result = Some(result);
        self
    }

    pub fn success(mut self, data: T) -> Self {
        self.result = Some(OperationResult::success(data));
        self
    }

    pub fn error(mut self, message: impl Into<String>) -> Self {
        self.result = Some(OperationResult::error(message));
        self
    }

    pub fn error_with_code(mut self, message: impl Into<String>, code: impl Into<String>) -> Self {
        self.result = Some(OperationResult::error_with_code(message, code));
        self
    }

    pub fn add_diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }

    pub fn add_info(self, message: impl Into<String>) -> Self {
        self.add_diagnostic(Diagnostic::info(message))
    }

    pub fn add_warning(self, message: impl Into<String>) -> Self {
        self.add_diagnostic(Diagnostic::warning(message))
    }

    pub fn add_error(self, message: impl Into<String>) -> Self {
        self.add_diagnostic(Diagnostic::error(message))
    }

    pub fn add_debug(self, message: impl Into<String>) -> Self {
        self.add_diagnostic(Diagnostic::debug(message))
    }

    pub fn add_fatal(self, message: impl Into<String>) -> Self {
        self.add_diagnostic(Diagnostic::fatal(message))
    }

    pub fn diagnostics(mut self, diagnostics: Vec<Diagnostic>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    pub fn add_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    pub fn suggestions(mut self, suggestions: Vec<Suggestion>) -> Self {
        self.suggestions = suggestions;
        self
    }

    pub fn metrics(mut self, metrics: Metrics) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub fn provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    pub fn with_timing<F, R>(self, f: F) -> (Self, R)
    where
        F: FnOnce() -> R,
    {
        let start = std::time::Instant::now();
        let result = f();
        let duration = start.elapsed();

        let metrics = Metrics {
            duration: Some(DurationMetrics {
                total_ms: Some(duration.as_millis() as f64),
                phases: HashMap::new(),
            }),
            resources: None,
            counters: HashMap::new(),
            runtime: None,
            counts: HashMap::new(),
            custom: None,
        };

        (self.metrics(metrics), result)
    }

    pub fn with_source_info(
        self,
        system: impl Into<String>,
        version: Option<impl Into<String>>,
        instance: Option<impl Into<String>>,
    ) -> Self {
        let source_info = SourceInfo {
            system: system.into(),
            version: version.map(Into::into),
            instance: instance.map(Into::into),
        };

        let provenance = Provenance {
            source: Some(source_info),
            timestamp: Some(Utc::now()),
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            chain: Vec::new(),
        };

        self.provenance(provenance)
    }

    pub fn with_trace_info(
        mut self,
        trace_id: impl Into<String>,
        span_id: impl Into<String>,
        parent_span_id: Option<impl Into<String>>,
    ) -> Self {
        let mut provenance = self.provenance.unwrap_or_default();
        provenance.trace_id = Some(trace_id.into());
        provenance.span_id = Some(span_id.into());
        provenance.parent_span_id = parent_span_id.map(Into::into);
        self.provenance = Some(provenance);
        self
    }

    pub fn build(self) -> Result<ResultEnvelope<T>, BuildError> {
        let result = self.result.ok_or(BuildError::MissingResult)?;

        Ok(ResultEnvelope {
            result,
            diagnostics: self.diagnostics,
            suggestions: self.suggestions,
            metrics: self.metrics,
            provenance: self.provenance,
        })
    }
}

impl<T> ResultEnvelope<T> {
    pub fn builder() -> ResultEnvelopeBuilder<T> {
        ResultEnvelopeBuilder::new()
    }
}

impl Default for Provenance {
    fn default() -> Self {
        Self {
            source: None,
            timestamp: Some(Utc::now()),
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            chain: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("Result is required to build an envelope")]
    MissingResult,
}

pub struct SuggestionBuilder {
    suggestion_type: SuggestionType,
    description: String,
    patch: Option<Vec<JsonPatchOperation>>,
    priority: Option<SuggestionPriority>,
    rationale: Option<String>,
}

impl SuggestionBuilder {
    pub fn new(suggestion_type: SuggestionType, description: impl Into<String>) -> Self {
        Self {
            suggestion_type,
            description: description.into(),
            patch: None,
            priority: None,
            rationale: None,
        }
    }

    pub fn action(description: impl Into<String>) -> Self {
        Self::new(SuggestionType::Action, description)
    }

    pub fn configuration(description: impl Into<String>) -> Self {
        Self::new(SuggestionType::Configuration, description)
    }

    pub fn optimization(description: impl Into<String>) -> Self {
        Self::new(SuggestionType::Optimization, description)
    }

    pub fn modification(description: impl Into<String>) -> Self {
        Self::new(SuggestionType::Modification, description)
    }

    pub fn with_patch(mut self, patch: Vec<JsonPatchOperation>) -> Self {
        self.patch = Some(patch);
        self
    }

    pub fn with_priority(mut self, priority: SuggestionPriority) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn with_rationale(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = Some(rationale.into());
        self
    }

    pub fn build(self) -> Suggestion {
        Suggestion {
            suggestion_type: self.suggestion_type,
            description: self.description,
            patch: self.patch,
            priority: self.priority,
            rationale: self.rationale,
        }
    }
}

impl Suggestion {
    pub fn builder(
        suggestion_type: SuggestionType,
        description: impl Into<String>,
    ) -> SuggestionBuilder {
        SuggestionBuilder::new(suggestion_type, description)
    }

    pub fn action(description: impl Into<String>) -> SuggestionBuilder {
        SuggestionBuilder::action(description)
    }

    pub fn configuration(description: impl Into<String>) -> SuggestionBuilder {
        SuggestionBuilder::configuration(description)
    }

    pub fn optimization(description: impl Into<String>) -> SuggestionBuilder {
        SuggestionBuilder::optimization(description)
    }

    pub fn modification(description: impl Into<String>) -> SuggestionBuilder {
        SuggestionBuilder::modification(description)
    }
}

impl JsonPatchOperation {
    pub fn add(path: impl Into<String>, value: serde_json::Value) -> Self {
        Self {
            op: JsonPatchOp::Add,
            path: path.into(),
            value: Some(value),
            from: None,
        }
    }

    pub fn remove(path: impl Into<String>) -> Self {
        Self {
            op: JsonPatchOp::Remove,
            path: path.into(),
            value: None,
            from: None,
        }
    }

    pub fn replace(path: impl Into<String>, value: serde_json::Value) -> Self {
        Self {
            op: JsonPatchOp::Replace,
            path: path.into(),
            value: Some(value),
            from: None,
        }
    }

    pub fn move_op(path: impl Into<String>, from: impl Into<String>) -> Self {
        Self {
            op: JsonPatchOp::Move,
            path: path.into(),
            value: None,
            from: Some(from.into()),
        }
    }

    pub fn copy(path: impl Into<String>, from: impl Into<String>) -> Self {
        Self {
            op: JsonPatchOp::Copy,
            path: path.into(),
            value: None,
            from: Some(from.into()),
        }
    }

    pub fn test(path: impl Into<String>, value: serde_json::Value) -> Self {
        Self {
            op: JsonPatchOp::Test,
            path: path.into(),
            value: Some(value),
            from: None,
        }
    }
}
