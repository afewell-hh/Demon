# API Versioning Policy

## Overview

Demon uses header-based API versioning to provide stable, backward-compatible APIs for ritual execution, contract validation, and approval management. This document describes the versioning strategy, client integration guidelines, and upgrade procedures.

## Versioning Strategy

### Header-Based Versioning

All API endpoints under `/api/` use the `X-Demon-API-Version` header for version negotiation:

```http
X-Demon-API-Version: v1
```

**Server behavior:**
- The server ALWAYS responds with `X-Demon-API-Version: v1` header on all API responses
- If a client sends an unsupported version, the server returns `406 Not Acceptable`
- If no version header is provided, the server assumes `v1` (backwards compatible)

### Supported Versions

| Version | Status    | Endpoints                           | Notes                          |
|---------|-----------|-------------------------------------|--------------------------------|
| v1      | Current   | All `/api/*` endpoints              | Initial stable release         |

## Version Negotiation

### Client Request

Clients MAY send the desired API version in the request header:

```bash
curl -H "X-Demon-API-Version: v1" http://localhost:3000/api/runs
```

If omitted, the server assumes `v1` for backwards compatibility with existing clients.

### Server Response

The server ALWAYS includes the API version in the response header:

```http
HTTP/1.1 200 OK
X-Demon-API-Version: v1
Content-Type: application/json
...
```

### Unsupported Version

If a client requests an unsupported version, the server returns `406 Not Acceptable`:

```http
HTTP/1.1 406 Not Acceptable
X-Demon-API-Version: v1
Content-Type: application/json

{
  "error": "unsupported API version",
  "requested_version": "v2",
  "supported_versions": ["v1"],
  "message": "API version 'v2' is not supported. Please use one of: v1"
}
```

## Versioned Endpoints

The following API endpoints are covered by the versioning policy:

### Ritual Runs API
- `GET /api/runs` — List ritual runs
- `GET /api/runs/:run_id` — Get run details
- `GET /api/runs/:run_id/events/stream` — Stream run events (SSE)
- `GET /api/tenants/:tenant/runs` — List runs for tenant
- `GET /api/tenants/:tenant/runs/:run_id` — Get tenant run details
- `GET /api/tenants/:tenant/runs/:run_id/events/stream` — Stream tenant run events

### Approval Management API
- `POST /api/approvals/:run_id/:gate_id/grant` — Grant approval
- `POST /api/approvals/:run_id/:gate_id/deny` — Deny approval
- `POST /api/tenants/:tenant/approvals/:run_id/:gate_id/grant` — Grant tenant approval
- `POST /api/tenants/:tenant/approvals/:run_id/:gate_id/deny` — Deny tenant approval
- `POST /api/tenants/:tenant/approvals/:run_id/:gate_id/override` — Override tenant approval

### Contract Registry API
- `POST /api/contracts/validate/envelope` — Validate single envelope
- `POST /api/contracts/validate/envelope/bulk` — Validate multiple envelopes
- `GET /api/contracts/status` — Get contract bundle status

### Excluded Endpoints

The following endpoints are NOT covered by API versioning:
- `/health` — Health check endpoint (always stable)
- `/runs` — HTML pages (UI routes, not API)
- `/admin/*` — Admin endpoints (internal use only)
- `/static/*` — Static assets
- `/ui/*` — UI-specific endpoints (forms, workflows, etc.)

## Client Integration

### HTTP Clients

Always include the API version header in your requests:

**Python example:**
```python
import requests

headers = {"X-Demon-API-Version": "v1"}
response = requests.get("http://localhost:3000/api/runs", headers=headers)
assert response.headers["X-Demon-API-Version"] == "v1"
```

**Rust example:**
```rust
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

let mut headers = HeaderMap::new();
headers.insert(
    HeaderName::from_static("x-demon-api-version"),
    HeaderValue::from_static("v1"),
);

let client = reqwest::Client::new();
let response = client
    .get("http://localhost:3000/api/runs")
    .headers(headers)
    .send()
    .await?;

assert_eq!(response.headers()["x-demon-api-version"], "v1");
```

**cURL example:**
```bash
curl -H "X-Demon-API-Version: v1" http://localhost:3000/api/runs
```

### demonctl CLI

The `demonctl` CLI automatically sends the appropriate API version header when communicating with the Demon platform. No configuration is required.

### Runtime Capsules

Capsules using the runtime API should include the version header when making HTTP requests to the operate-ui or other Demon services.

## Backward Compatibility Guarantees

### Within a Major Version

Within a major version (e.g., `v1`), Demon guarantees:

✅ **Additive changes only:**
- New optional fields in request/response bodies
- New query parameters (always optional)
- New endpoints

✅ **No breaking changes:**
- Existing fields remain unchanged (same name, type, semantics)
- Existing endpoints remain available
- Existing HTTP status codes remain unchanged
- Existing error response formats remain unchanged

❌ **Breaking changes require new major version:**
- Removing or renaming fields
- Changing field types or semantics
- Removing endpoints
- Changing HTTP methods or status codes
- Changing error response formats

### Deprecation Process

When deprecating functionality:

1. **Announce deprecation** — Add deprecation notice to docs and API responses (e.g., `X-Demon-API-Deprecated: true`)
2. **Grace period** — Maintain deprecated functionality for at least one minor version
3. **Remove** — Only remove in next major version (e.g., v1 → v2)

## Contract Specifications

The full API contract specification is maintained in `contracts/api-contracts/v1.yaml` using OpenAPI 3.0 format.

**Key contract artifacts:**
- `contracts/api-contracts/v1.yaml` — OpenAPI specification
- `contracts/schemas/` — JSON Schemas for event validation
- `contracts/fixtures/` — Golden test fixtures

These contracts are validated in CI via the `contracts-validate` required check.

## Compatibility Testing

### Automated Smoke Tests

The compatibility smoke test workflow runs against the last tagged release to ensure:
- Current build works with previous stable APIs
- No breaking changes were introduced
- Contract schemas remain compatible

**To run compatibility smoke tests:**
```bash
# Run against last release
./scripts/compat-smoke.sh

# Run against specific version
./scripts/compat-smoke.sh v0.1.0
```

### CI Integration

The compatibility smoke test is integrated into CI and will fail the build if:
- API responses change in incompatible ways
- Required fields are removed or renamed
- HTTP status codes change
- Error response formats change

## Upgrade Guidance

### For API Clients

When upgrading to a new Demon version:

1. **Check release notes** — Review breaking changes and new features
2. **Update version header** — If using a new major version, update your requests
3. **Test integration** — Run your integration tests against the new version
4. **Monitor deprecations** — Watch for `X-Demon-API-Deprecated` headers
5. **Gradual rollout** — Test in staging before production

### For Demon Operators

When deploying a new Demon version:

1. **Review changelog** — Check for breaking changes and deprecations
2. **Run smoke tests** — Execute compatibility smoke tests before deploy
3. **Monitor metrics** — Watch for `406 Not Acceptable` responses (version mismatches)
4. **Update documentation** — Ensure API docs reflect current version
5. **Notify clients** — Inform API consumers of upcoming changes

## Version History

### v1 (Current)

**Release:** Demon Sprint B (API Stability)

**Endpoints:**
- Ritual Runs API (list, detail, streaming)
- Approval Management API (grant, deny, override)
- Contract Registry API (validate, bulk validate, status)

**Changes from pre-v1:**
- Added `X-Demon-API-Version` header to all API responses
- Added version negotiation middleware
- Formalized API contract in OpenAPI specification
- Added compatibility smoke tests

**Known limitations:**
- No pagination support for large result sets (mitigated by `limit` parameter)
- No batch approval operations (must approve gates individually)
- No webhook/callback mechanism for async approvals

### Future Versions

**v2 (Planned):**
- Pagination support with cursor-based navigation
- Batch approval operations
- GraphQL alternative endpoint (opt-in)
- Enhanced filtering and search capabilities

## FAQ

### Q: Do I need to send the version header?

**A:** It's recommended but not required. If omitted, the server assumes `v1`. However, explicitly sending the header makes your code more future-proof.

### Q: What happens if I send an invalid version?

**A:** The server returns `406 Not Acceptable` with details about supported versions.

### Q: Can I use different versions for different endpoints?

**A:** No. The version applies to all API endpoints globally. You cannot mix versions within a single deployment.

### Q: How do I know which version is running?

**A:** Check the `X-Demon-API-Version` header in any API response. You can also query the `/health` endpoint (though it doesn't include versioning).

### Q: Will old clients break when a new version is released?

**A:** No. Old clients using `v1` (or no header) will continue to work. Only when the major version increments (e.g., `v1` → `v2`) and `v1` is eventually removed would old clients need updates.

## References

- [OpenAPI Specification](https://spec.openapis.org/oas/v3.0.3)
- [API Contract v1](../contracts/api-contracts/v1.yaml)
- [Operate UI README](./operate-ui/README.md)
- [Branch Protection MVP](./process/branch_protection_mvp.md)

## Support

For questions about API versioning:
- Open an issue: https://github.com/afewell-hh/demon/issues
- Review Sprint B plan: [docs/mvp/02-epics.md](./mvp/02-epics.md)
- Check compatibility smoke: [docs/ops/compat-smoke.md](./ops/compat-smoke.md)
