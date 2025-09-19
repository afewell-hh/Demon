use anyhow::Result;
use envelope::EnvelopeValidator;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

static ENVELOPE_VALIDATOR: Lazy<Arc<EnvelopeValidator>> = Lazy::new(|| {
    Arc::new(
        EnvelopeValidator::new()
            .expect("Failed to initialize envelope validator with compiled schema"),
    )
});

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateEnvelopeRequest {
    #[serde(flatten)]
    pub envelope: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateEnvelopeBulkRequest {
    pub envelopes: Vec<EnvelopeBulkItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvelopeBulkItem {
    pub name: String,
    pub envelope: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationResponse {
    pub valid: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BulkValidationResponse {
    pub results: Vec<BulkValidationResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BulkValidationResult {
    pub name: String,
    pub valid: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

pub fn validate_envelope(envelope: &Value) -> ValidationResponse {
    match ENVELOPE_VALIDATOR.validate_json(envelope) {
        Ok(_) => {
            info!("Envelope validation successful");
            ValidationResponse {
                valid: true,
                errors: vec![],
            }
        }
        Err(e) => {
            let error_msg = e.to_string();
            info!("Envelope validation failed: {}", error_msg);

            let errors = if error_msg.contains(" at ") {
                error_msg
                    .split(", ")
                    .map(|part| {
                        let parts: Vec<&str> = part.splitn(2, " at ").collect();
                        ValidationError {
                            message: parts.first().unwrap_or(&"Unknown error").to_string(),
                            path: parts.get(1).unwrap_or(&"").to_string(),
                            schema_path: None,
                            kind: None,
                        }
                    })
                    .collect()
            } else {
                vec![ValidationError {
                    path: String::new(),
                    message: error_msg,
                    schema_path: None,
                    kind: None,
                }]
            };

            ValidationResponse {
                valid: false,
                errors,
            }
        }
    }
}

pub fn validate_envelope_bulk(request: &ValidateEnvelopeBulkRequest) -> BulkValidationResponse {
    let results = request
        .envelopes
        .iter()
        .map(|item| {
            let validation = validate_envelope(&item.envelope);
            BulkValidationResult {
                name: item.name.clone(),
                valid: validation.valid,
                errors: validation.errors,
            }
        })
        .collect();

    BulkValidationResponse { results }
}

pub fn get_validator() -> Arc<EnvelopeValidator> {
    ENVELOPE_VALIDATOR.clone()
}

pub fn validate_for_publish(envelope: &Value) -> Result<()> {
    ENVELOPE_VALIDATOR
        .validate_json(envelope)
        .map_err(|e| anyhow::anyhow!("Envelope failed pre-publish validation: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_valid_envelope() {
        let envelope = json!({
            "result": {
                "success": true,
                "data": "test"
            }
        });

        let response = validate_envelope(&envelope);
        assert!(response.valid);
        assert!(response.errors.is_empty());
    }

    #[test]
    fn test_validate_invalid_envelope() {
        let envelope = json!({
            "invalid_field": "test"
        });

        let response = validate_envelope(&envelope);
        assert!(!response.valid);
        assert!(!response.errors.is_empty());
    }

    #[test]
    fn test_bulk_validation() {
        let request = ValidateEnvelopeBulkRequest {
            envelopes: vec![
                EnvelopeBulkItem {
                    name: "valid".to_string(),
                    envelope: json!({
                        "result": {
                            "success": true,
                            "data": "test"
                        }
                    }),
                },
                EnvelopeBulkItem {
                    name: "invalid".to_string(),
                    envelope: json!({
                        "invalid_field": "test"
                    }),
                },
            ],
        };

        let response = validate_envelope_bulk(&request);
        assert_eq!(response.results.len(), 2);
        assert!(response.results[0].valid);
        assert!(!response.results[1].valid);
    }
}
