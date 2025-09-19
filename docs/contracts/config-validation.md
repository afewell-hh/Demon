# Configuration Validation

This document describes the configuration validation system for capsules in the Demon runtime.

## Overview

The configuration validation system ensures that capsules receive valid configuration before execution. It validates configuration files against JSON Schema definitions and emits policy decisions to track validation results.

## Schema Location

Configuration schemas are stored in the `contracts/config/` directory with the naming convention:
```
contracts/config/{capsule-name}-config.v1.json
```

For example, the echo capsule schema is located at:
```
contracts/config/echo-config.v1.json
```

The contracts directory location is determined by:
1. The `CONTRACTS_DIR` environment variable (if set and directory exists)
2. Searching up the directory tree from the current working directory for a `contracts/` folder

## Configuration File Location

Configuration files are loaded from the directory specified by the `CONFIG_DIR` environment variable, or `.demon/config/` by default. Files follow the naming convention:
```
{CONFIG_DIR}/{capsule-name}.json
```

For example:
```
.demon/config/echo.json
```

## Schema Format

Configuration schemas use JSON Schema Draft 7. Here's an example schema for the echo capsule:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "echo-config.v1.json",
  "title": "Echo Capsule Configuration",
  "description": "Configuration schema for the echo capsule",
  "type": "object",
  "properties": {
    "messagePrefix": {
      "type": "string",
      "description": "Prefix to add to echoed messages",
      "default": ""
    },
    "enableTrim": {
      "type": "boolean",
      "description": "Whether to trim whitespace from messages",
      "default": true
    },
    "maxMessageLength": {
      "type": "integer",
      "description": "Maximum length of messages to process",
      "minimum": 1,
      "maximum": 10000,
      "default": 1000
    },
    "outputFormat": {
      "type": "string",
      "description": "Format for output messages",
      "enum": ["plain", "json", "structured"],
      "default": "plain"
    }
  },
  "required": ["messagePrefix", "enableTrim"],
  "additionalProperties": false
}
```

## Runtime Behavior

When a capsule is invoked through the runtime router:

1. **Configuration Loading**:
   - If a configuration file exists, it is loaded from the expected location
   - If no configuration file exists, default values are extracted from the schema
2. **Schema Validation**: The configuration (loaded or default) is validated against the capsule's schema
3. **Policy Decision**: A `policy.decision:v1` event is emitted with the validation result
4. **Execution Control**:
   - If validation succeeds, the capsule is executed with the validated configuration
   - If validation fails, execution is denied and an error is returned

### Default Configuration Behavior

When a configuration file is missing, the system automatically generates a default configuration using the `default` values specified in the schema properties. This allows capsules to run without requiring explicit configuration files, using sensible defaults.

**Requirements for default behavior:**
- The schema must exist for the capsule
- Required fields must have `default` values in the schema
- The generated defaults must pass schema validation

**Example:** If no `.demon/config/echo.json` file exists, the system will generate:
```json
{
  "messagePrefix": "",
  "enableTrim": true,
  "maxMessageLength": 1000,
  "outputFormat": "plain"
}
```

This default configuration is validated against the schema before use.

### Policy Decision Events

The runtime emits policy decisions for configuration validation:

**Allowed (valid configuration):**
```json
{
  "event": "policy.decision:v1",
  "ts": "2024-01-15T10:30:00Z",
  "tenantId": "default",
  "runId": "run-123",
  "ritualId": "ritual-456",
  "capability": "echo",
  "decision": {
    "allowed": true
  },
  "validation": {
    "type": "config",
    "schema": "echo-config.v1.json"
  }
}
```

**Denied (invalid configuration):**
```json
{
  "event": "policy.decision:v1",
  "ts": "2024-01-15T10:30:00Z",
  "tenantId": "default",
  "runId": "run-123",
  "ritualId": "ritual-456",
  "capability": "echo",
  "decision": {
    "allowed": false,
    "reason": "config_validation_failed",
    "details": "Path /enableTrim: Expected boolean, found string (schema: /properties/enableTrim/type)"
  },
  "validation": {
    "type": "config",
    "schema": "echo-config.v1.json"
  }
}
```

## CLI Validation

The `demonctl` CLI provides commands for validating configuration files:

**Note:** Configuration files are optional for `demonctl run` commands. If no configuration file exists, the runtime will use schema defaults. However, the `validate-config` command requires an explicit file to validate.

### Validate from File

```bash
# Auto-detect capsule from filename
demonctl contracts validate-config echo_config.json

# Specify capsule explicitly
demonctl contracts validate-config myconfig.json --schema echo
```

### Validate from stdin

```bash
# Must specify schema when reading from stdin
cat config.json | demonctl contracts validate-config --stdin --schema echo
```

### Example Output

**Valid configuration:**
```
✓ Valid config for capsule: echo
```

**Invalid configuration:**
```
✗ Invalid config for capsule 'echo':
  Path /enableTrim: Expected boolean, found string
    Schema: /properties/enableTrim/type
  Path : Missing required property 'messagePrefix'
    Schema: /required
```

## Error Types

The configuration validation system provides detailed error information:

- **SchemaNotFound**: The schema file for the specified capsule doesn't exist
- **ConfigFileNotFound**: The configuration file doesn't exist (only for explicit file validation via CLI)
- **SchemaCompilationFailed**: The schema file contains invalid JSON Schema syntax
- **ValidationFailed**: The configuration doesn't match the schema requirements
- **JsonParsingFailed**: The configuration file contains invalid JSON
- **IoError**: File system error (permissions, disk full, etc.)

**Note:** For runtime execution, missing configuration files do not generate `ConfigFileNotFound` errors. Instead, the system attempts to use schema defaults. `ConfigFileNotFound` errors only occur when explicitly validating a specific file path using the CLI.

## Integration Examples

### Runtime Integration

```rust
use config_loader::ConfigManager;
use runtime::link::router::Router;

// Create router with config validation
let router = Router::new();

// Dispatch will automatically validate config
let result = router.dispatch("echo", &args, "run-id", "ritual-id").await?;
```

### Direct Validation

```rust
use config_loader::ConfigManager;

let manager = ConfigManager::new();

// Validate a config file
manager.validate_config_file("echo", Path::new("config.json"))?;

// Validate a config value
let config_value = serde_json::json!({
    "messagePrefix": "Test: ",
    "enableTrim": true
});
manager.validate_config_value("echo", &config_value)?;
```

## Best Practices

1. **Schema Design**:
   - Use descriptive titles and descriptions
   - Specify appropriate constraints (minimum, maximum, enum values)
   - Mark required fields explicitly
   - Set `additionalProperties: false` to prevent unexpected fields

2. **Error Handling**:
   - Always handle `ValidationFailed` errors gracefully
   - Provide clear error messages to users
   - Log detailed validation errors for debugging

3. **Configuration Management**:
   - Use environment variables for deployment-specific settings
   - Validate configuration files in CI/CD pipelines
   - Document configuration options clearly

4. **Testing**:
   - Test both valid and invalid configurations
   - Include edge cases (empty values, boundary conditions)
   - Verify error messages are helpful and accurate