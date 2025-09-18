use crate::envelope::ResultEnvelope;
use anyhow::{anyhow, Result};
use jsonschema::{Draft, JSONSchema};
use serde_json::Value;

const RESULT_ENVELOPE_SCHEMA: &str = include_str!("../../../contracts/envelopes/result.json");

pub struct EnvelopeValidator {
    schema: JSONSchema,
}

impl EnvelopeValidator {
    pub fn new() -> Result<Self> {
        let schema_value: Value = serde_json::from_str(RESULT_ENVELOPE_SCHEMA)
            .map_err(|e| anyhow!("Failed to parse envelope schema: {}", e))?;

        let schema = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema_value)
            .map_err(|e| anyhow!("Failed to compile envelope schema: {}", e))?;

        Ok(Self { schema })
    }

    pub fn validate<T>(&self, envelope: &ResultEnvelope<T>) -> Result<()>
    where
        T: serde::Serialize,
    {
        let envelope_value = serde_json::to_value(envelope)
            .map_err(|e| anyhow!("Failed to serialize envelope for validation: {}", e))?;

        let validation_result = self.schema.validate(&envelope_value);

        if let Err(errors) = validation_result {
            let error_messages: Vec<String> = errors
                .map(|error| format!("{}  at {}", error, error.instance_path))
                .collect();

            return Err(anyhow!(
                "Envelope validation failed: {}",
                error_messages.join(", ")
            ));
        }

        Ok(())
    }

    pub fn validate_json(&self, envelope_json: &Value) -> Result<()> {
        let validation_result = self.schema.validate(envelope_json);

        if let Err(errors) = validation_result {
            let error_messages: Vec<String> = errors
                .map(|error| format!("{} at {}", error, error.instance_path))
                .collect();

            return Err(anyhow!(
                "Envelope validation failed: {}",
                error_messages.join(", ")
            ));
        }

        Ok(())
    }
}

impl Default for EnvelopeValidator {
    fn default() -> Self {
        Self::new().expect("Failed to create default envelope validator")
    }
}

impl<T> ResultEnvelope<T>
where
    T: serde::Serialize,
{
    pub fn validate(&self) -> Result<()> {
        let validator = EnvelopeValidator::new()?;
        validator.validate(self)
    }

    pub fn validate_with(&self, validator: &EnvelopeValidator) -> Result<()> {
        validator.validate(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{Diagnostic, OperationResult};

    #[test]
    fn test_validator_creation() {
        let validator = EnvelopeValidator::new();
        assert!(validator.is_ok());
    }

    #[test]
    fn test_minimal_envelope_validation() {
        let envelope = ResultEnvelope {
            result: OperationResult::success("test data"),
            diagnostics: vec![],
            suggestions: vec![],
            metrics: None,
            provenance: None,
        };

        assert!(envelope.validate().is_ok());
    }

    #[test]
    fn test_envelope_with_diagnostics_validation() {
        let envelope = ResultEnvelope {
            result: OperationResult::success("test data"),
            diagnostics: vec![Diagnostic::info("Test diagnostic")],
            suggestions: vec![],
            metrics: None,
            provenance: None,
        };

        assert!(envelope.validate().is_ok());
    }

    #[test]
    fn test_error_envelope_validation() {
        let envelope = ResultEnvelope {
            result: OperationResult::<()>::error("Test error"),
            diagnostics: vec![],
            suggestions: vec![],
            metrics: None,
            provenance: None,
        };

        assert!(envelope.validate().is_ok());
    }
}
