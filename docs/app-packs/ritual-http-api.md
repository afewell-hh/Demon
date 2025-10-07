# Ritual Execution HTTP API

The Demon runtime server provides REST endpoints for executing rituals defined in App Packs programmatically.

## Base URL

When running the runtime server locally:
```
http://localhost:8080/api/v1/rituals
```

## Authentication

**Current Status:** Not implemented (Milestone 0)

Authentication stubs are in place for future implementation. All endpoints currently accept requests without authentication.

**Future:** OAuth2/JWT bearer tokens will be required.

## Endpoints

### POST `/api/v1/rituals/{ritual}/runs`

Execute a ritual asynchronously and return immediately with a run ID.

**Path Parameters:**
- `ritual` (string, required): The name of the ritual to execute (e.g., `noop`, `validate`, `analyze`)

**Request Body:**
```json
{
  "app": "string (required)",
  "version": "string (optional, defaults to latest)",
  "parameters": {
    // Ritual-specific parameters as JSON object
  }
}
```

**Success Response (202 Accepted):**
```json
{
  "runId": "550e8400-e29b-41d4-a716-446655440000",
  "status": "Running",
  "createdAt": "2025-10-06T12:00:00Z",
  "links": {
    "run": "/api/v1/rituals/noop/runs/550e8400-e29b-41d4-a716-446655440000?app=hoss",
    "envelope": "/api/v1/rituals/noop/runs/550e8400-e29b-41d4-a716-446655440000/envelope?app=hoss"
  }
}
```

**Example:**
```bash
curl -X POST http://localhost:8080/api/v1/rituals/noop/runs \
  -H "Content-Type: application/json" \
  -d '{
    "app": "hoss",
    "parameters": {
      "message": "hello world"
    }
  }'
```

---

### GET `/api/v1/rituals/{ritual}/runs`

List all runs for a specific ritual, with optional filtering.

**Path Parameters:**
- `ritual` (string, required): The name of the ritual

**Query Parameters:**
- `app` (string, required): App Pack name to filter by
- `limit` (integer, optional): Maximum number of results to return (default: no limit, max: 500)
- `status` (string, optional): Filter by status (`Pending`, `Running`, `Completed`, `Failed`)

**Success Response (200 OK):**
```json
{
  "runs": [
    {
      "runId": "550e8400-e29b-41d4-a716-446655440000",
      "app": "hoss",
      "ritual": "noop",
      "version": "0.1.0",
      "status": "Completed",
      "createdAt": "2025-10-06T12:00:00Z",
      "updatedAt": "2025-10-06T12:00:05Z",
      "completedAt": "2025-10-06T12:00:05Z"
    }
  ],
  "nextPageToken": null
}
```

**Example:**
```bash
# List all completed runs
curl "http://localhost:8080/api/v1/rituals/noop/runs?app=hoss&status=Completed&limit=10"
```

---

### GET `/api/v1/rituals/{ritual}/runs/{runId}`

Get detailed information about a specific run.

**Path Parameters:**
- `ritual` (string, required): The name of the ritual
- `runId` (string, required): The UUID of the run

**Query Parameters:**
- `app` (string, required): App Pack name

**Success Response (200 OK):**
```json
{
  "runId": "550e8400-e29b-41d4-a716-446655440000",
  "app": "hoss",
  "ritual": "noop",
  "version": "0.1.0",
  "status": "Completed",
  "createdAt": "2025-10-06T12:00:00Z",
  "updatedAt": "2025-10-06T12:00:05Z",
  "completedAt": "2025-10-06T12:00:05Z",
  "parameters": {
    "message": "hello world"
  },
  "resultEnvelope": {
    "event": "ritual.completed:v1",
    "ritualId": "hoss::noop",
    "runId": "550e8400-e29b-41d4-a716-446655440000",
    "ts": "2025-10-06T12:00:05Z",
    "outputs": {
      "result": "ok"
    }
  },
  "error": null
}
```

**Example:**
```bash
curl "http://localhost:8080/api/v1/rituals/noop/runs/550e8400-e29b-41d4-a716-446655440000?app=hoss"
```

---

### GET `/api/v1/rituals/{ritual}/runs/{runId}/envelope`

Retrieve the full result envelope for a completed run.

**Path Parameters:**
- `ritual` (string, required): The name of the ritual
- `runId` (string, required): The UUID of the run

**Query Parameters:**
- `app` (string, required): App Pack name

**Success Response (200 OK):**
```json
{
  "runId": "550e8400-e29b-41d4-a716-446655440000",
  "envelope": {
    "event": "ritual.completed:v1",
    "ritualId": "hoss::noop",
    "runId": "550e8400-e29b-41d4-a716-446655440000",
    "ts": "2025-10-06T12:00:05Z",
    "outputs": {
      "result": "ok",
      "data": {
        // Ritual-specific output data
      }
    }
  }
}
```

**Example:**
```bash
curl "http://localhost:8080/api/v1/rituals/noop/runs/550e8400-e29b-41d4-a716-446655440000/envelope?app=hoss"
```

---

## Error Model

All error responses follow a consistent JSON format:

```json
{
  "error": "Human-readable error message"
}
```

### HTTP Status Codes

| Status Code | Meaning | Example |
|-------------|---------|---------|
| `200 OK` | Request successful | GET requests for existing resources |
| `202 Accepted` | Run scheduled successfully | POST to create new run |
| `400 Bad Request` | Invalid request parameters | Invalid status filter value |
| `404 Not Found` | Resource not found | Non-existent run ID, unknown ritual |
| `422 Unprocessable Entity` | Validation failed | Missing required `app` field |
| `500 Internal Server Error` | Server error | Database failure, unexpected errors |

### Common Error Scenarios

**Missing Required Field:**
```json
{
  "error": "missing field `app`"
}
```
Status: `422 Unprocessable Entity`

**Unknown App Pack:**
```json
{
  "error": "App Pack 'unknown-app' is not installed"
}
```
Status: `404 Not Found` or `500 Internal Server Error`

**Unknown Ritual:**
```json
{
  "error": "Ritual 'unknown-ritual' is not defined in App Pack hoss@0.1.0"
}
```
Status: `404 Not Found` or `500 Internal Server Error`

**Run Not Found:**
```json
{
  "error": "Run not found",
  "app": "hoss",
  "ritual": "noop",
  "runId": "invalid-id"
}
```
Status: `404 Not Found`

**Invalid Status Filter:**
```json
{
  "error": "Invalid status filter",
  "allowed": ["Pending", "Running", "Completed", "Failed"]
}
```
Status: `400 Bad Request`

---

## Pagination

**Current Status:** Partial implementation (Milestone 0)

The `limit` query parameter is supported on list endpoints. Token-based pagination (`nextPageToken`) is included in the response schema but not yet fully implemented.

**Current behavior:**
- `limit` parameter truncates results (max: 500)
- `nextPageToken` is always `null`
- All matching results are returned up to the limit

**Future enhancement:** Full cursor-based pagination will be implemented for large result sets.

---

## Run Status Lifecycle

Rituals execute asynchronously. Run status transitions:

```
POST /runs
    ↓
  Running ────→ Completed (success)
    ↓
  Failed (error during execution)
```

**Status Values:**
- `Pending`: Run queued but not started (future state)
- `Running`: Execution in progress
- `Completed`: Execution finished successfully, envelope available
- `Failed`: Execution failed, error message in `error` field

---

## Testing

See `runtime/tests/ritual_api_spec.rs` for comprehensive integration tests covering:
- Successful ritual execution and persistence
- Required parameter validation
- Error handling (unknown app, unknown ritual, invalid filters)
- Status filtering
- Limit/pagination
- 404 scenarios

Run tests:
```bash
cargo test --package runtime ritual_http_api
```

---

## Future Enhancements

### Authentication & Authorization
- OAuth2/JWT bearer tokens
- App-level permissions
- Rate limiting

### Pagination
- Cursor-based token pagination for large result sets
- Configurable page sizes

### Additional Features
- Webhook notifications on run completion
- SSE streaming for real-time status updates
- Bulk run creation
- Run cancellation endpoint

---

## See Also

- [App Pack Schema](./schema.md) - App Pack manifest format
- [Installer Guarantees](./installer-guarantees.md) - Installation contract
- [HOSS Promotion Runbook](./hoss-promotion-runbook.md) - HOSS App Pack promotion process
