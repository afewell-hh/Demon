//! # Envelope Helper Crate
//!
//! This crate provides a helper library for creating and validating result envelopes
//! according to the Demon platform's result envelope schema. It includes strongly-typed
//! structs, builder patterns, derive macros, and schema validation.
//!
//! ## Basic Usage
//!
//! ```rust
//! use envelope::*;
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, AsEnvelope)]
//! struct MyResult {
//!     value: i32,
//!     message: String,
//! }
//!
//! // Create a simple envelope using the derive macro
//! let result = MyResult { value: 42, message: "Success".to_string() };
//! let envelope = result.into_envelope();
//!
//! // Or use the builder pattern for more control
//! let envelope = ResultEnvelope::builder()
//!     .success(MyResult { value: 100, message: "Complex operation".to_string() })
//!     .add_info("Operation started")
//!     .add_warning("Minor issue detected")
//!     .with_source_info("my-service", Some("1.0.0"), Some("instance-01"))
//!     .build()
//!     .expect("Valid envelope");
//!
//! // Validate against the schema
//! envelope.validate().expect("Should validate");
//! ```
//!
//! ## Builder Pattern
//!
//! The builder pattern provides a fluent API for constructing complex envelopes:
//!
//! ```rust
//! use envelope::*;
//! use serde_json::json;
//!
//! let suggestion = Suggestion::optimization("Increase batch size")
//!     .with_priority(SuggestionPriority::Medium)
//!     .with_rationale("Current batch size is suboptimal")
//!     .with_patch(vec![JsonPatchOperation::replace(
//!         "/config/batch_size",
//!         json!(50),
//!     )])
//!     .build();
//!
//! let envelope = ResultEnvelope::builder()
//!     .success("Operation completed")
//!     .add_diagnostic(Diagnostic::info("Processing started"))
//!     .add_suggestion(suggestion)
//!     .with_timing(|| {
//!         // Your operation here
//!         std::thread::sleep(std::time::Duration::from_millis(100));
//!         "result"
//!     })
//!     .0  // Get the builder from the timing tuple
//!     .build()
//!     .expect("Valid envelope");
//! ```
//!
//! ## Schema Validation
//!
//! All envelopes can be validated against the JSON schema:
//!
//! ```rust
//! use envelope::*;
//!
//! let envelope = ResultEnvelope::<()>::builder()
//!     .error("Something went wrong")
//!     .build()
//!     .expect("Valid envelope");
//!
//! // Validate the envelope
//! assert!(envelope.validate().is_ok());
//! ```

mod builder;
mod envelope;
mod validation;

pub use builder::*;
pub use envelope::*;
pub use validation::*;

// Re-export the derive macro
pub use envelope_derive::AsEnvelope;

pub trait AsEnvelope {
    fn into_envelope(self) -> ResultEnvelope<Self>
    where
        Self: Sized;
}
