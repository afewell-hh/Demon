# Contract Specifications

This directory contains API schemas, event definitions, and contract specifications for the Demon platform.

## Overview

Contracts define the structured data formats and APIs used throughout Demon:

- **Event Schemas** - JSON schemas for all event types
- **API Specifications** - Request/response formats for REST APIs
- **Configuration Schemas** - Policy and system configuration formats
- **Release Management** - Contract versioning and distribution

## Key Files

- [`result-envelope.md`](result-envelope.md) - Standard result wrapper format
- [`playbook.md`](playbook.md) - Ritual execution specifications
- [`releases.md`](releases.md) - Contract bundle release process
- [`config-validation.md`](config-validation.md) - Configuration validation schemas

## Event Types

Demon emits structured events for all operations:

- `ritual.started:v1` - Workflow execution begins
- `ritual.completed:v1` - Workflow execution finishes
- `ritual.state.transitioned:v1` - State changes during execution
- `approval.requested:v1` - Human approval required
- `approval.granted:v1` - Approval granted
- `approval.denied:v1` - Approval denied
- `policy.decision:v1` - Automated policy decisions

## Working with Contracts

### Exporting Contracts
```bash
# Summary view of all contracts
cargo run -p demonctl -- contracts bundle

# Include WIT definitions
cargo run -p demonctl -- contracts bundle --include-wit

# Export as JSON for automation
cargo run -p demonctl -- contracts bundle --format json --include-wit
```

### Fetching Latest Contracts
```bash
# Download latest bundle from CI artifacts
GH_TOKEN=your_token cargo run -p demonctl -- contracts fetch-bundle

# Fetch to custom location with manifest
GH_TOKEN=your_token cargo run -p demonctl -- contracts fetch-bundle \
  --dest contracts.json --manifest
```

### Contract Validation
```bash
# Validate data against schema
cargo run -p demonctl -- contracts validate --schema event.schema.json --data event.json
```

## Bundle Distribution

Contract bundles are automatically:

- Generated on main branch merges
- Uploaded as CI artifacts
- SHA-256 verified for integrity
- Versioned with git metadata

See [`releases.md`](releases.md) for detailed release process.

## For Developers

When building integrations:

1. Download the latest contract bundle
2. Generate client code from schemas
3. Validate against event specifications
4. Handle version evolution gracefully

## For API Consumers

See [API Consumers Guide](../personas/api-consumers.md) for integration patterns and examples.

---

**🔗 Related**: [Event Schemas](.) | [API Reference](../personas/api-consumers.md) | [Bundle Releases](releases.md)