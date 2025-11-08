# Agent Flow API

REST and NATS APIs for LLM/CLI agents to draft and submit flow manifests securely.

## Feature Flag

Enable via environment variable:

```bash
export OPERATE_UI_FLAGS=agent-flows
```

## Authentication

All endpoints require JWT authentication with appropriate scopes.

### JWT Configuration

Configure JWT validation via environment variables:

```bash
# Required
export JWT_SECRET="your-secret-key"

# Optional
export JWT_ISSUER="https://your-auth0-domain.auth0.com/"
export JWT_AUDIENCE="https://api.demon.example.com"
```

### Required Scopes

- `flows:read` - List contracts
- `flows:write` - Draft and submit flows

### Example JWT Payload

```json
{
  "sub": "agent-001",
  "iss": "https://your-auth0-domain.auth0.com/",
  "aud": ["https://api.demon.example.com"],
  "scope": "flows:read flows:write",
  "exp": 1699564800
}
```

## Endpoints

### GET /api/contracts

List available contract metadata for flow authoring.

**Headers:**
- `Authorization: Bearer <jwt-token>` (requires `flows:read` scope)
- `X-Demon-API-Version: v1`

**Query Parameters:**
- `kind` (optional) - Filter by contract kind
- `version` (optional) - Filter by version
- `limit` (optional) - Limit results

**Example:**

```bash
curl -H "Authorization: Bearer $JWT_TOKEN" \
     -H "X-Demon-API-Version: v1" \
     "http://localhost:3000/api/contracts?kind=capsule"
```

**Response:**

```json
[
  {
    "name": "echo",
    "kind": "capsule",
    "version": "v1",
    "description": "Echo capsule contract"
  }
]
```

### POST /api/flows/draft

Draft a flow manifest without validation.

**Headers:**
- `Authorization: Bearer <jwt-token>` (requires `flows:write` scope)
- `Content-Type: application/json`
- `Idempotency-Key: <uuid>` (optional, recommended)
- `X-Demon-API-Version: v1`

**Body:**

```json
{
  "manifest": {
    "schema_version": "v1",
    "metadata": {
      "flow_id": "my-flow-001",
      "name": "My Agent Flow",
      "created_by": "agent-001"
    },
    "nodes": [...],
    "edges": [...]
  }
}
```

**Example:**

```bash
curl -X POST \
     -H "Authorization: Bearer $JWT_TOKEN" \
     -H "Content-Type: application/json" \
     -H "Idempotency-Key: $(uuidgen)" \
     -H "X-Demon-API-Version: v1" \
     -d @examples/flows/hello-agent.json \
     "http://localhost:3000/api/flows/draft"
```

**Response:**

```json
{
  "draft_id": "draft-abc-123",
  "flow_id": "my-flow-001",
  "manifest_digest": "sha256:abc123...",
  "created_at": "2025-11-08T12:00:00Z"
}
```

### POST /api/flows/submit

Validate and submit a flow for execution.

**Headers:**
- `Authorization: Bearer <jwt-token>` (requires `flows:write` scope)
- `Content-Type: application/json`
- `Idempotency-Key: <uuid>` (optional, recommended)
- `X-Demon-API-Version: v1`

**Body:**

```json
{
  "manifest": {
    "schema_version": "v1",
    "metadata": {
      "flow_id": "my-flow-001",
      "name": "My Agent Flow",
      "created_by": "agent-001"
    },
    "nodes": [...],
    "edges": [...]
  }
}
```

**Example:**

```bash
curl -X POST \
     -H "Authorization: Bearer $JWT_TOKEN" \
     -H "Content-Type: application/json" \
     -H "Idempotency-Key: $(uuidgen)" \
     -H "X-Demon-API-Version: v1" \
     -d '{"manifest": ...}' \
     "http://localhost:3000/api/flows/submit"
```

**Response (Success):**

```json
{
  "flow_id": "my-flow-001",
  "manifest_digest": "sha256:abc123...",
  "validation_result": {
    "valid": true,
    "errors": [],
    "warnings": []
  },
  "submitted_at": "2025-11-08T12:00:00Z"
}
```

**Response (Validation Error):**

```json
{
  "flow_id": "my-flow-001",
  "manifest_digest": "sha256:abc123...",
  "validation_result": {
    "valid": false,
    "errors": [
      {
        "code": "flow.schema_version_missing",
        "message": "schema_version field is required",
        "path": "schema_version"
      }
    ],
    "warnings": []
  }
}
```

## Error Codes

### Authentication Errors

- `missing_token` (401) - Authorization header not provided
- `invalid_token` (401) - JWT token is invalid or expired
- `insufficient_scope` (403) - Token lacks required scope
- `server_configuration_error` (500) - JWT not configured

### Validation Errors

- `flow.schema_version_missing` - Required field missing
- `flow.metadata_missing` - Required field missing
- `flow.nodes_missing` - Required field missing
- `flow.edges_missing` - Required field missing
- `flow.unsupported_schema_version` - Schema version not supported

## Flow Manifest Schema

See `contracts/schemas/flow_manifest.v1.json` for the complete JSON Schema.

Example manifest: `examples/flows/hello-agent.json`

## Events

Submitted flows emit the following events to JetStream:

- `flow.drafted:v1` - Flow draft created
- `flow.submitted:v1` - Flow validated and submitted
- `agent.flow.audit:v1` - Audit trail for all flow operations

See `contracts/schemas/` for event schemas.

## Rate Limiting

Draft and submit endpoints are rate limited to 10 requests/minute per caller (configurable).

## Idempotency

Use the `Idempotency-Key` header to safely retry requests. Duplicate keys within the TTL window return the cached response.

## Security Notes

- JWT tokens must be kept secure
- Never commit JWT secrets to version control
- Use HTTPS in production
- Token scope enforcement is strict
- All operations are audited

## Local Development

For local development without Auth0:

```bash
# Generate a test token with a simple secret
export JWT_SECRET="dev-secret-change-in-production"

# Create a test token (simplified - use proper JWT library in production)
# Token payload: {"sub": "dev-agent", "scope": "flows:read flows:write"}
```

For production, configure Auth0 or compatible JWT issuer.
