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

## Secret Resolution

The configuration system supports resolving secret values using `secret://` URI references. This allows sensitive data to be stored separately from configuration files and resolved at runtime.

### Secret URI Format

Secret references use the format: `secret://scope/key`

- **scope**: A namespace for grouping related secrets (e.g., "database", "api", "app")
- **key**: The specific secret identifier within the scope

**Example:**
```json
{
  "messagePrefix": "secret://app/prefix",
  "enableTrim": true,
  "databaseUrl": "secret://database/connection_string"
}
```

### Secret Providers

The secret resolution system supports pluggable providers, allowing you to integrate with different secret management systems. The provider is selected using the `CONFIG_SECRETS_PROVIDER` environment variable.

#### Available Providers

1. **EnvFile Provider** (`envfile` - default):
   - Resolves secrets from environment variables and JSON files
   - Environment variables: `SECRET_<SCOPE>_<KEY>` (uppercase with underscores)
   - Default file location: `.demon/secrets.json`
   - Custom location via `CONFIG_SECRETS_FILE` environment variable

2. **Vault Stub Provider** (`vault`):
   - Simulates Vault-like secret storage for development and testing
   - Supports file-based and HTTP modes
   - Configured via `VAULT_ADDR` and optional `VAULT_TOKEN` environment variables

#### Provider Configuration

**Using EnvFile Provider (default):**
```bash
# Provider is envfile by default, no configuration needed
# Or explicitly set:
export CONFIG_SECRETS_PROVIDER=envfile
export CONFIG_SECRETS_FILE=.demon/secrets.json  # optional
```

**Using Vault Stub Provider:**
```bash
export CONFIG_SECRETS_PROVIDER=vault
export VAULT_ADDR=file://vault_stub  # or http://vault-server:8200
export VAULT_TOKEN=your-token  # optional for stub mode
```

#### EnvFile Provider Sources

Secrets are resolved using the following priority order:

1. **Environment Variables**: `SECRET_<SCOPE>_<KEY>` (uppercase with underscores)
   - `secret://app/prefix` → `SECRET_APP_PREFIX`
   - `secret://database/password` → `SECRET_DATABASE_PASSWORD`

2. **Secrets File**: JSON file containing nested secret values
   - Default location: `.demon/secrets.json`
   - Custom location via `CONFIG_SECRETS_FILE` environment variable

**Example secrets file:**
```json
{
  "app": {
    "prefix": "Secret Prefix: ",
    "api_key": "sk-abcd1234"
  },
  "database": {
    "password": "super_secret_password",
    "connection_string": "postgresql://user:pass@host:5432/db"
  }
}
```

#### Vault Stub Provider

The Vault stub provider supports two modes:

**File Mode** (default):
```bash
export VAULT_ADDR=file://vault_stub
# Stores secrets in ~/.demon/vault_stub/ as JSON files
```

**HTTP Mode**:
```bash
export VAULT_ADDR=http://127.0.0.1:8200
export VAULT_TOKEN=vault-token
# Makes HTTP requests to Vault-compatible endpoints (minimal implementation)
```

### Secret Resolution Behavior

1. **Resolution Timing**: Secrets are resolved after loading configuration but before schema validation
2. **Error Handling**: Missing secrets cause configuration validation to fail with `SecretResolutionFailed` error
3. **Security**: Resolved secret values are redacted in logs and diagnostic output
4. **Immutability**: Once resolved, secret values are treated as regular configuration values

### Runtime Integration

Secret resolution is automatically enabled in the runtime using the configured provider:

```rust
use config_loader::{ConfigManager, SecretProviderFactory, EnvFileSecretProvider};
use runtime::link::router::Router;

// Use provider based on CONFIG_SECRETS_PROVIDER environment variable
let router = Router::new();

// For explicit provider control:
let config_manager = ConfigManager::new();

// Create provider from factory (respects CONFIG_SECRETS_PROVIDER)
let secret_provider = SecretProviderFactory::create().unwrap();
let router = Router::with_config_and_secrets(config_manager, secret_provider);

// Or use specific provider
let secret_provider = EnvFileSecretProvider::with_secrets_file("custom-secrets.json");
let router = Router::with_config_and_secrets(config_manager, secret_provider);
```

### Policy Decision Events with Secrets

**Secret Resolution Failed:**
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
    "reason": "secret_not_found",
    "details": "Secret not found: app/missing_key"
  },
  "validation": {
    "type": "config",
    "schema": "echo-config.v1.json"
  }
}
```

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

# Validate config with secrets
demonctl contracts validate-config echo_config.json --secrets-file secrets.json
```

### Validate from stdin

```bash
# Must specify schema when reading from stdin
cat config.json | demonctl contracts validate-config --stdin --schema echo

# Validate from stdin with secrets
cat config.json | demonctl contracts validate-config --stdin --schema echo --secrets-file secrets.json
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

**Secret resolution failed:**
```
✗ Secret resolution failed for capsule 'echo': Secret not found: app/missing_key
```

## Error Types

The configuration validation system provides detailed error information:

- **SchemaNotFound**: The schema file for the specified capsule doesn't exist
- **ConfigFileNotFound**: The configuration file doesn't exist (only for explicit file validation via CLI)
- **SchemaCompilationFailed**: The schema file contains invalid JSON Schema syntax
- **ValidationFailed**: The configuration doesn't match the schema requirements
- **JsonParsingFailed**: The configuration file contains invalid JSON
- **IoError**: File system error (permissions, disk full, etc.)
- **SecretResolutionFailed**: A secret referenced in the configuration could not be resolved

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
use config_loader::{ConfigManager, EnvFileSecretProvider};

let manager = ConfigManager::new();

// Validate a config file
manager.validate_config_file("echo", Path::new("config.json"))?;

// Validate a config file with secrets
let secret_provider = EnvFileSecretProvider::with_secrets_file("secrets.json");
manager.validate_config_file_with_secrets("echo", Path::new("config.json"), &secret_provider)?;

// Validate a config value
let config_value = serde_json::json!({
    "messagePrefix": "Test: ",
    "enableTrim": true
});
manager.validate_config_value("echo", &config_value)?;

// Validate a config value with secrets
let config_with_secrets = serde_json::json!({
    "messagePrefix": "secret://app/prefix",
    "enableTrim": true
});
manager.validate_config_value_with_secrets("echo", &config_with_secrets, &secret_provider)?;
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

5. **Secrets Management**:
   - Use environment variables for production secrets
   - Store secrets files outside the repository (add to `.gitignore`)
   - Use clear, consistent naming for secret scopes and keys
   - Test secret resolution failures in your test suite
   - Never log or expose resolved secret values

## Managing Secrets with CLI

The `demonctl secrets` command provides tools for managing capsule secrets through the command line. This is the recommended way to set up secrets for development and testing.

All secrets commands support the `--provider` flag to specify which secret provider to use:
- `--provider envfile` (default): Uses environment file provider
- `--provider vault`: Uses Vault stub provider

### Setting Secrets

```bash
# Set a secret value (uses default envfile provider)
demonctl secrets set database/password my_secret_value

# Set with explicit provider
demonctl secrets set database/password my_secret_value --provider envfile
demonctl secrets set api/key vault_secret --provider vault

# Set from environment variable (avoids shell history)
export DB_PASS="secret123"
demonctl secrets set database/password --from-env DB_PASS --provider envfile

# Set from stdin (for scripting)
echo "secret_value" | demonctl secrets set api/key --stdin --provider vault

# Use custom secrets file location (envfile provider only)
demonctl secrets set app/token secret123 --secrets-file /path/to/secrets.json --provider envfile
```

### Getting Secrets

```bash
# Get redacted secret (default envfile provider)
demonctl secrets get database/password
# Output: database/password: my_***

# Get raw secret value with specific provider
demonctl secrets get database/password --raw --provider envfile
# Output: my_secret_value

demonctl secrets get api/key --raw --provider vault
# Output: vault_secret_value

# Use custom secrets file (envfile provider only)
demonctl secrets get api/key --secrets-file /path/to/secrets.json --provider envfile
```

### Listing Secrets

```bash
# List all secrets (redacted, default envfile provider)
demonctl secrets list

# List with specific provider
demonctl secrets list --provider vault
demonctl secrets list --provider envfile

# List secrets for specific scope
demonctl secrets list --scope database --provider vault

# Use custom secrets file (envfile provider only)
demonctl secrets list --secrets-file /path/to/secrets.json --provider envfile
```

### Deleting Secrets

```bash
# Delete a secret (default envfile provider)
demonctl secrets delete database/password

# Delete with specific provider
demonctl secrets delete api/key --provider vault
demonctl secrets delete database/password --provider envfile

# Use custom secrets file (envfile provider only)
demonctl secrets delete api/key --secrets-file /path/to/secrets.json --provider envfile
```

### Security Best Practices

1. **File Permissions**: The CLI automatically sets secrets files to mode 0600 (owner read/write only) on Unix systems
2. **Avoid Shell History**: Use `--from-env` or `--stdin` flags instead of passing secrets as command arguments
3. **Custom Location**: Use `CONFIG_SECRETS_FILE` environment variable or `--secrets-file` flag to store secrets outside the project directory
4. **Raw Access**: Only use `--raw` flag when necessary, as it bypasses redaction
5. **Provider Consistency**: Use the same provider for all operations on a secret scope to avoid confusion
6. **Vault Warnings**: The CLI warns when `--secrets-file` is used with `--provider vault` since file options are ignored

### Integration Example

```bash
# Set up secrets for echo capsule using envfile provider
demonctl secrets set echo/api_key your_api_key_here --provider envfile
demonctl secrets set echo/prefix "Secret: " --provider envfile

# Or using vault provider
export CONFIG_SECRETS_PROVIDER=vault
export VAULT_ADDR=file://vault_stub
demonctl secrets set echo/api_key your_api_key_here --provider vault
demonctl secrets set echo/prefix "Secret: " --provider vault

# Create config file using secret URIs
cat > .demon/config/echo.json << EOF
{
  "messagePrefix": "secret://echo/prefix",
  "enableTrim": true,
  "apiKey": "secret://echo/api_key"
}
EOF

# Validate config with secrets (respects CONFIG_SECRETS_PROVIDER)
demonctl contracts validate-config .demon/config/echo.json

# Or validate with specific secrets file (envfile only)
demonctl contracts validate-config .demon/config/echo.json --secrets-file .demon/secrets.json

# Run ritual (secrets automatically resolved using configured provider)
demonctl run examples/rituals/echo.yaml
```

### File Format

The secrets file uses the same JSON format as the `EnvFileSecretProvider`:

```json
{
  "database": {
    "password": "super_secret_password",
    "connection_string": "postgresql://user:pass@host:5432/db"
  },
  "api": {
    "key": "sk-abcd1234",
    "secret": "secret_token"
  }
}
```

This format is compatible with all existing secret resolution functionality in the runtime.