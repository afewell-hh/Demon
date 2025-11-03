# Schema Registry Service

## Purpose

The Schema Registry service provides centralized storage and retrieval of contract schemas for Demon rituals and capsules. It ensures that all components use compatible, versioned schemas by maintaining a single source of truth for contract definitions including JSON Schemas, WIT interfaces, and descriptor metadata.

## Architecture

The registry is built as a standalone Rust service (`demon-registry`) with:

- **Axum HTTP server**: REST API for schema management
- **JetStream KV backend**: Persistent, distributed storage via NATS
- **JWT authentication**: Placeholder middleware (TODO: real verification in follow-up)
- **Structured logging**: JSON-formatted tracing output for observability

### Storage Layout

Contracts are stored in the JetStream KV bucket `contracts` with key structure:

```
meta.<name>.<version>
```

Examples:
- `meta.ritual.started.v1`
- `meta.approval.granted.v1`
- `meta.graph.commit.created.v1`

Each key stores a JSON-encoded `ContractBundle` containing:
- `name`: Contract identifier
- `version`: Semantic version string
- `description`: Human-readable description (optional)
- `createdAt`: ISO 8601 timestamp
- `jsonSchema`: JSON Schema definition (optional)
- `witPath`: Path to WIT interface file (optional)
- `descriptorPath`: Path to descriptor metadata (optional)

## Endpoints

### GET /healthz

Health check endpoint.

**Response**: `200 OK` with body `"OK"`

**Example**:
```bash
curl http://localhost:3001/healthz
```

### GET /registry/contracts

List all available contracts.

**Response**: `200 OK` with JSON array of contract metadata

**Example**:
```bash
curl -H "Authorization: Bearer <token>" \
  http://localhost:3001/registry/contracts
```

**Response body**:
```json
{
  "contracts": [
    {
      "name": "ritual.started",
      "version": "v1",
      "description": "Ritual execution started event",
      "createdAt": "2024-01-15T10:00:00Z"
    }
  ]
}
```

### GET /registry/contracts/:name/:version

Retrieve a specific contract bundle by name and version.

**Parameters**:
- `name`: Contract name (e.g., `ritual.started`)
- `version`: Version string (e.g., `v1`)

**Response**: `200 OK` with full contract bundle, or `404 Not Found`

**Example**:
```bash
curl -H "Authorization: Bearer <token>" \
  http://localhost:3001/registry/contracts/ritual.started/v1
```

**Response body**:
```json
{
  "name": "ritual.started",
  "version": "v1",
  "description": "Ritual execution started event",
  "createdAt": "2024-01-15T10:00:00Z",
  "jsonSchema": "{\"type\": \"object\", \"properties\": {...}}",
  "witPath": "/contracts/ritual-started.wit",
  "descriptorPath": "/contracts/ritual-started.json"
}
```

## Local Development

### Prerequisites

- Rust nightly toolchain (see `rust-toolchain.toml`)
- NATS server with JetStream enabled

### Start NATS (via Docker Compose)

```bash
make up
```

This starts NATS on `localhost:4222` with JetStream enabled.

### Build and Run

```bash
# Build the registry binary
cargo build -p registry

# Run the server (defaults to 0.0.0.0:3001)
cargo run -p registry

# Or with custom bind address
BIND_ADDR="127.0.0.1:8080" cargo run -p registry
```

### Environment Variables

- `NATS_URL`: NATS server address (default: `nats://127.0.0.1:4222`)
- `NATS_CREDS_PATH`: Path to NATS credentials file (optional)
- `REGISTRY_KV_BUCKET`: JetStream KV bucket name (default: `contracts`)
  - Override for test isolation or multi-tenant deployments
  - Integration tests use per-test isolated buckets via this variable
- `BIND_ADDR`: HTTP server bind address (default: `0.0.0.0:3001`)
- `RUST_LOG`: Logging level (default: `info,registry=debug`)

### Running Tests

```bash
# Run unit tests
cargo test -p registry

# Run integration tests (requires NATS running)
cargo test -p registry -- --nocapture --ignored
```

Integration tests are marked with `#[ignore]` and require a running NATS server with JetStream enabled.

## TODOs

The following features are planned for future stories:

### Authentication & Authorization

- **TODO**: Implement real JWT signature verification
- **TODO**: Add public key configuration for token validation
- **TODO**: Verify token expiration and issuer claims
- **TODO**: Extract and enforce scope-based permissions
- **TODO**: Return 401 Unauthorized for invalid/missing tokens in production mode

### Publishing Contracts

- **TODO**: Add `POST /registry/contracts` endpoint for publishing new contracts
- **TODO**: Implement contract linting and validation before storage
- **TODO**: Support for schema evolution and compatibility checks
- **TODO**: Automated versioning and changelog generation
- **TODO**: Integration with CI/CD for contract promotion workflow

### Schema Validation

- **TODO**: Validate JSON Schema syntax before storage
- **TODO**: Parse and validate WIT interface files
- **TODO**: Check for breaking changes between versions
- **TODO**: Enforce semantic versioning policies

### Observability

- **TODO**: Add Prometheus metrics for request counts, latency, and errors
- **TODO**: Distributed tracing integration (OpenTelemetry)
- **TODO**: Health check endpoint with JetStream connectivity status
- **TODO**: Audit log for schema publish/update operations

### Deployment

- **TODO**: Document production deployment patterns
- **TODO**: Kubernetes manifests and Helm chart
- **TODO**: TLS/HTTPS configuration guide
- **TODO**: High availability and failover setup
- **TODO**: Backup and disaster recovery procedures

## Related Documentation

- [Contract Bundle Releases](contracts/releases.md)
- [WIT Interface Definitions](../contracts/wit/)
- [JSON Schema Fixtures](../contracts/fixtures/)
- [API Versioning](api-versioning.md)
