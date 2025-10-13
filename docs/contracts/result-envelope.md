# Result Envelope Contract

## Overview

The Result Envelope provides a standardized format for operation results throughout the Demon platform. It wraps operation outcomes with additional context including diagnostics, suggestions, metrics, and provenance information.

## Schema

- **JSON Schema**: `contracts/envelopes/result.json`
- **WIT Definition**: `contracts/wit/demon-envelope.wit`
- **Schema Draft**: JSON Schema Draft 7
- **ID**: `https://demon.meta/contracts/envelopes/result.json`

## Structure

### Required Fields

- `result`: The primary operation result with success/failure status

### Optional Fields

- `diagnostics`: Array of diagnostic messages with severity levels (debug, info, warning, error, fatal)
- `suggestions`: Array of suggested actions or modifications, including JSON Patch operations
- `metrics`: Performance and operational metrics (duration, resources, counters)
- `provenance`: Origin and chain of custody information with tracing support

## Usage

### Basic Success Result

```json
{
  "result": {
    "success": true,
    "data": {
      "message": "Operation completed successfully",
      "id": "op-12345"
    }
  }
}
```

### Error Result with Diagnostics

```json
{
  "result": {
    "success": false,
    "error": {
      "code": "PROCESSING_FAILED",
      "message": "Failed to process ritual due to resource constraints"
    }
  },
  "diagnostics": [
    {
      "level": "error",
      "message": "Memory allocation failed",
      "timestamp": "2025-01-15T10:00:30Z",
      "source": "runtime.allocator"
    }
  ]
}
```

### With Suggestions (JSON Patch)

The envelope supports JSON Patch operations (RFC 6902) for suggesting modifications:

```json
{
  "suggestions": [
    {
      "type": "configuration",
      "description": "Enable parallel processing",
      "patch": [
        {
          "op": "add",
          "path": "/config/processing/parallel",
          "value": true
        },
        {
          "op": "replace",
          "path": "/config/processing/batch_size",
          "value": 50
        }
      ]
    }
  ]
}
```

## Fixtures

Test fixtures are available in `contracts/fixtures/envelopes/`:

- `result_minimal.json`: Basic successful result
- `result_full.json`: Complete example with all fields populated
- `result_error.json`: Error result with diagnostics
- `result_with_suggestions.json`: Result with JSON Patch suggestions

Event fixtures (for run lifecycle) are in `contracts/fixtures/events/`:
- `ritual.completed.v1.json`: Example run completion event
- `ritual.canceled.v1.json`: Example cancellation event (no result envelope)

## Versioning

The Result Envelope follows semantic versioning. Breaking changes will increment the schema version and require migration paths for existing consumers.

## Validation

Schema validation is enforced through:
- JSON Schema validation in tests
- Runtime validation in consuming services
- CI/CD pipeline validation for fixture updates

## WIT Interface

The Result Envelope has a corresponding WebAssembly Interface Type (WIT) definition at `contracts/wit/demon-envelope.wit` that provides:

- Typed bindings for all envelope components
- Support for capsule authors to generate type-safe code
- Functions for parsing, serializing, and validating envelopes

## Registry Export

The Result Envelope schema and WIT definition are published as part of the contract registry:

```bash
# Export all contracts including the envelope
demonctl contracts bundle --include-wit --format json
```

## Integration Points

The Result Envelope can be used by:
- Ritual execution results
- Policy decision outcomes
- Approval workflow results
- Bootstrapper operation results
- UI component data formatting

## Using the Envelope Helper Crate

The `envelope` crate provides a convenient API for creating and validating result envelopes:

### Basic Usage with Derive Macro

```rust
use envelope::*;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, AsEnvelope)]
struct ProcessingResult {
    items_processed: u32,
    total_time_ms: u64,
}

let result = ProcessingResult {
    items_processed: 150,
    total_time_ms: 2500,
};

// Create an envelope automatically
let envelope = result.into_envelope();
envelope.validate().expect("Should validate against schema");
```

### Builder Pattern for Complex Envelopes

```rust
use envelope::*;
use serde_json::json;

let envelope = ResultEnvelope::builder()
    .success("Operation completed successfully")
    .add_info("Processing started")
    .add_warning("3 items skipped due to validation errors")
    .add_suggestion(
        Suggestion::optimization("Increase batch size")
            .with_priority(SuggestionPriority::Medium)
            .with_patch(vec![JsonPatchOperation::replace(
                "/config/batch_size",
                json!(50),
            )])
            .build()
    )
    .with_source_info("demon-processor", Some("1.2.3"), Some("west-01"))
    .with_trace_info("trace-123", "span-456", Some("parent-789"))
    .build()
    .expect("Valid envelope");
```

### Error Handling

```rust
use envelope::*;

let envelope = ResultEnvelope::<()>::builder()
    .error_with_code("Processing failed", "PROCESSING_ERROR")
    .add_error("Memory allocation failed")
    .add_diagnostic(
        Diagnostic::error("Out of memory")
            .with_source("allocator")
            .with_context(json!({"requested_bytes": 1048576}))
    )
    .build()
    .expect("Valid error envelope");
```
