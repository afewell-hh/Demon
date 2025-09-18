# Result Envelope Contract

## Overview

The Result Envelope provides a standardized format for operation results throughout the Demon platform. It wraps operation outcomes with additional context including diagnostics, suggestions, metrics, and provenance information.

## Schema

- **Location**: `contracts/envelopes/result.json`
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

## Versioning

The Result Envelope follows semantic versioning. Breaking changes will increment the schema version and require migration paths for existing consumers.

## Validation

Schema validation is enforced through:
- JSON Schema validation in tests
- Runtime validation in consuming services
- CI/CD pipeline validation for fixture updates

## Integration Points

The Result Envelope can be used by:
- Ritual execution results
- Policy decision outcomes
- Approval workflow results
- Bootstrapper operation results
- UI component data formatting