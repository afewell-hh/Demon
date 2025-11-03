# Schema Registry Service

## Purpose

The Schema Registry service provides centralized storage and retrieval of contract schemas for Demon rituals and capsules. It ensures that all components use compatible, versioned schemas by maintaining a single source of truth for contract definitions including JSON Schemas, WIT interfaces, and descriptor metadata.

## Architecture

The registry is built as a standalone Rust service (`demon-registry`) with:

- **Axum HTTP server**: REST API for schema management
- **JetStream KV backend**: Persistent, distributed storage via NATS
- **JWT authentication**: HS256/384/512 signature verification with scope-based authorization
- **Content integrity**: SHA-256 digest computation for published bundles
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
- `digest`: SHA-256 hash of bundle content for integrity verification

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
  "descriptorPath": "/contracts/ritual-started.json",
  "digest": "a1b2c3d4e5f6..."
}
```

### POST /registry/contracts

Publish a new contract bundle to the registry.

**Authentication**: Requires JWT token with `contracts:write` scope

**Request body**:
```json
{
  "name": "my-contract",
  "version": "1.0.0",
  "description": "My contract description",
  "jsonSchema": "{\"type\": \"object\", ...}",
  "witPath": "/path/to/schema.wit",
  "descriptorPath": "/path/to/descriptor.json"
}
```

**Response**: `201 Created` with published contract metadata

**Example**:
```bash
curl -X POST http://localhost:8090/registry/contracts \
  -H "Authorization: Bearer <jwt-token>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-contract",
    "version": "1.0.0",
    "description": "My contract",
    "jsonSchema": "{\"type\": \"object\"}"
  }'
```

**Success response (201)**:
```json
{
  "status": "created",
  "name": "my-contract",
  "version": "1.0.0",
  "digest": "a1b2c3d4e5f6789...",
  "createdAt": "2024-11-03T12:00:00Z"
}
```

**Error responses**:
- `401 Unauthorized`: Missing or invalid JWT token
- `403 Forbidden`: Token valid but missing `contracts:write` scope
- `409 Conflict`: Contract with same name and version already exists
- `400 Bad Request`: Malformed request body

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
cargo build -p demon-registry

# Run the server (defaults to 0.0.0.0:3001)
cargo run -p demon-registry

# Or with custom bind address
BIND_ADDR="127.0.0.1:8080" cargo run -p demon-registry
```

### Environment Variables

- `NATS_URL`: NATS server address (default: `nats://127.0.0.1:4222`)
- `NATS_CREDS_PATH`: Path to NATS credentials file (optional)
- `REGISTRY_KV_BUCKET`: JetStream KV bucket name (default: `contracts`)
  - Override for test isolation or multi-tenant deployments
  - Integration tests use per-test isolated buckets via this variable
- `BIND_ADDR`: HTTP server bind address (default: `0.0.0.0:8090`)
- `JWT_SECRET`: Secret key for JWT signature verification (default: `dev-secret-change-in-production`)
- `JWT_ALGORITHM`: JWT algorithm (HS256, HS384, or HS512; default: `HS256`)
- `RUST_LOG`: Logging level (default: `info,registry=debug`)

### Running Tests

```bash
# Run unit tests
cargo test -p demon-registry

# Run integration tests (requires NATS running)
cargo test -p demon-registry -- --nocapture --ignored
```

Integration tests are marked with `#[ignore]` and require a running NATS server with JetStream enabled.

## Authentication Setup

The registry uses JWT (JSON Web Tokens) for authentication with scope-based authorization.

### JWT Token Requirements

Tokens must include the following claims:
- `sub`: Subject (user identifier)
- `exp`: Expiration time (Unix timestamp)
- `iat`: Issued at time (Unix timestamp, optional)
- `scopes`: Array of permission scopes

### Required Scopes

- `contracts:read`: Read access to contract metadata (GET endpoints)
- `contracts:write`: Publish new contracts (POST endpoints)

### Generating Test Tokens

For development/testing, you can generate tokens using the `jsonwebtoken` CLI or any JWT library:

```bash
# Using Node.js and crypto (no external dependencies)
node -e "
const crypto = require('crypto');
const b64 = (s) => Buffer.from(s).toString('base64').replace(/\+/g,'-').replace(/\//g,'_').replace(/=/g,'');
const header = b64(JSON.stringify({alg:'HS256',typ:'JWT'}));
const payload = b64(JSON.stringify({
  sub: 'dev-user',
  exp: Math.floor(Date.now()/1000) + 3600,
  iat: Math.floor(Date.now()/1000),
  scopes: ['contracts:write', 'contracts:read']
}));
const secret = 'dev-secret-change-in-production';
const sig = b64(crypto.createHmac('sha256', secret).update(header+'.'+payload).digest());
console.log(header+'.'+payload+'.'+sig);
"
```

Store the token and use it in requests:

```bash
export JWT_TOKEN="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."

# List contracts
curl -H "Authorization: Bearer $JWT_TOKEN" \
  http://localhost:8090/registry/contracts

# Publish contract
curl -X POST http://localhost:8090/registry/contracts \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d @contract.json
```

### Error Scenarios

#### Missing Authorization Header

```bash
curl -X POST http://localhost:8090/registry/contracts \
  -H "Content-Type: application/json" \
  -d '{"name":"test","version":"1.0.0"}'
```

**Response (401)**:
```
Missing Authorization header
```

#### Invalid Token

```bash
curl -H "Authorization: Bearer invalid-token" \
  http://localhost:8090/registry/contracts
```

**Response (401)**:
```
Invalid token: Token decode error: ...
```

#### Missing Scope

```bash
# Token with only contracts:read scope trying to publish
curl -X POST http://localhost:8090/registry/contracts \
  -H "Authorization: Bearer $READ_ONLY_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"test","version":"1.0.0"}'
```

**Response (403)**:
```
Insufficient permissions: contracts:write scope required
```

## CLI Usage

The `demonctl` CLI provides a convenient interface for publishing contracts:

```bash
# Publish a contract with JSON schema
demonctl contracts publish \
  --name my-contract \
  --version 1.0.0 \
  --description "My contract description" \
  --json-schema /path/to/schema.json \
  --registry-endpoint http://localhost:8090 \
  --jwt "$JWT_TOKEN"

# Or use JWT_TOKEN environment variable
export JWT_TOKEN="eyJhbGci..."
demonctl contracts publish \
  --name my-contract \
  --version 1.0.0 \
  --json-schema /path/to/schema.json
```

**Output**:
```
Contract published successfully!

Name:      my-contract
Version:   1.0.0
Digest:    a1b2c3d4e5f6789abcdef...
CreatedAt: 2024-11-03T12:00:00Z
```

## Contract Linting

The `contract-linter` tool validates schema changes for breaking changes and semver compliance:

```bash
# Compare two schema versions
cargo run -p contract-linter -- compare \
  --current contracts/schemas/my-contract-v1.0.0.json \
  --proposed contracts/schemas/my-contract-v1.1.0.json \
  --current-version 1.0.0 \
  --proposed-version 1.1.0
```

**Breaking change detection**:
- Removed required properties
- Type changes (e.g., string → integer)
- Stricter constraints (reduced maxLength, increased minimum, etc.)

**Version validation**:
- For 0.x versions: minor bump acceptable for breaking changes (0.1.0 → 0.2.0)
- For 1.x+ versions: major bump required for breaking changes (1.0.0 → 2.0.0)

### CI Integration

The linter runs automatically in CI on all PRs:

```yaml
# .github/workflows/ci.yml
contract-linter:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - run: make lint-contracts
```

Test fixtures in `contracts/fixtures/linter/` provide examples of compatible and breaking changes.

## TODOs

The following features are planned for future stories:

### Authentication & Authorization

- ✅ Implement real JWT signature verification (HS256/384/512)
- ✅ Verify token expiration and issuer claims
- ✅ Extract and enforce scope-based permissions
- ✅ Return 401 Unauthorized for invalid/missing tokens
- **TODO**: Add public key configuration for RS256/RS384/RS512 algorithms

### Publishing Contracts

- ✅ Add `POST /registry/contracts` endpoint for publishing new contracts
- ✅ Implement contract linting and validation (breaking change detection)
- ✅ Support for semantic versioning policies
- ✅ Integration with CI/CD for contract validation workflow
- **TODO**: Automated changelog generation from schema diffs
- **TODO**: Contract deprecation and sunset policies

### Schema Validation

- ✅ Validate JSON Schema syntax before storage
- ✅ Check for breaking changes between versions
- ✅ Enforce semantic versioning policies
- **TODO**: Parse and validate WIT interface files
- **TODO**: Cross-schema dependency validation

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
