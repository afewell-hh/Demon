# API Consumers Guide

Welcome, API consumers! This guide helps you integrate with Demon's REST APIs, consume event streams, and build applications that interact with the Demon platform.

## 🚀 Quick Start

Ready to integrate with Demon? Here's your fast path:

1. **[Understand the APIs](#api-overview)** - REST endpoints and event streams
2. **[Set up authentication](#authentication)** - API access and permissions
3. **[Start with examples](#api-examples)** - Working code samples

```bash
# Quick API test
curl http://localhost:3000/api/runs
```

## 🔧 API Overview

### REST APIs
Demon provides comprehensive REST APIs for programmatic access:

- **Runs API** - Query and monitor workflow executions
- **Approvals API** - Grant/deny approval gates programmatically
- **Events API** - Access event streams and history
- **Health API** - System status and readiness checks

### Event Streams
Real-time event consumption via NATS JetStream:

- **Ritual Events** - Workflow execution events
- **Approval Events** - Human approval workflow events
- **Policy Events** - Automated policy decision events
- **System Events** - Health and operational events

## 📋 Runs API

### List All Runs
```http
GET /api/runs
```

**Response Format:**
```json
{
  "runs": [
    {
      "id": "ritual-123-run-456",
      "ritual_id": "ritual-123",
      "status": "completed",
      "created_at": "2025-09-26T10:00:00Z",
      "completed_at": "2025-09-26T10:05:00Z",
      "events": [...]
    }
  ]
}
```

### Get Specific Run
```http
GET /api/runs/{run_id}
```

**Response Format:**
```json
{
  "id": "ritual-123-run-456",
  "ritual_id": "ritual-123",
  "status": "awaiting_approval",
  "created_at": "2025-09-26T10:00:00Z",
  "events": [
    {
      "type": "ritual.started:v1",
      "timestamp": "2025-09-26T10:00:00Z",
      "data": {...}
    },
    {
      "type": "approval.requested:v1",
      "timestamp": "2025-09-26T10:02:00Z",
      "data": {
        "gate_id": "production-deploy",
        "approver_group": "ops-team"
      }
    }
  ]
}
```

### Query Parameters
- `status` - Filter by run status (`pending`, `running`, `completed`, `failed`)
- `ritual_id` - Filter by specific ritual
- `limit` - Pagination limit (default: 50)
- `offset` - Pagination offset

## 🎯 Approvals API

### Grant Approval
```http
POST /api/approvals/{run_id}/{gate_id}/grant
Content-Type: application/json
```

**Request Body:**
```json
{
  "approver": "ops@example.com",
  "note": "approved for production deployment"
}
```

**Response:**
```json
{
  "status": "granted",
  "approver": "ops@example.com",
  "timestamp": "2025-09-26T10:05:00Z",
  "note": "approved for production deployment"
}
```

### Deny Approval
```http
POST /api/approvals/{run_id}/{gate_id}/deny
Content-Type: application/json
```

**Request Body:**
```json
{
  "approver": "security@example.com",
  "reason": "security review required"
}
```

### Approval Behavior
- **First-Writer-Wins** - First terminal decision is accepted
- **Conflict Resolution** - Subsequent decisions return `409 Conflict`
- **Idempotent** - Duplicate decisions return `200 OK` with noop status
- **TTL Auto-Deny** - Automatic denial after timeout period

## 📡 Event Streaming

### Event Stream Structure
Events are published to NATS JetStream subjects:
```
demon.ritual.v1.{ritual_id}.{run_id}.events
```

### Event Types

#### Ritual Events
```json
{
  "type": "ritual.started:v1",
  "timestamp": "2025-09-26T10:00:00Z",
  "ritual_id": "ritual-123",
  "run_id": "run-456",
  "data": {
    "ritual_name": "deploy-to-production",
    "capsule": "deploy-capsule"
  }
}
```

#### Approval Events
```json
{
  "type": "approval.requested:v1",
  "timestamp": "2025-09-26T10:02:00Z",
  "ritual_id": "ritual-123",
  "run_id": "run-456",
  "data": {
    "gate_id": "production-deploy",
    "approver_group": "ops-team",
    "timeout_seconds": 3600
  }
}
```

#### Policy Events
```json
{
  "type": "policy.decision:v1",
  "timestamp": "2025-09-26T10:01:00Z",
  "ritual_id": "ritual-123",
  "run_id": "run-456",
  "data": {
    "decision": "allow",
    "reason": null,
    "quota": {
      "limit": 100,
      "windowSeconds": 3600,
      "remaining": 99
    }
  }
}
```

### Consuming Events

#### Using NATS CLI
```bash
# Subscribe to all events for a specific run
nats subscribe "demon.ritual.v1.ritual-123.run-456.events"

# Subscribe to all events (wildcard)
nats subscribe "demon.ritual.v1.*.*.events"
```

#### Using NATS Client Libraries
```javascript
// JavaScript example
const nats = await connect({ servers: 'nats://localhost:4222' });
const js = nats.jetstream();

const subscription = await js.subscribe('demon.ritual.v1.*.*.events');
for await (const message of subscription) {
  const event = JSON.parse(message.data);
  console.log('Event:', event.type, event.data);
  message.ack();
}
```

## 🔐 Authentication

### API Access
Currently, Demon APIs are designed for trusted internal networks. For production use:

- **Network Security** - Deploy behind VPN or private networks
- **Proxy Authentication** - Use reverse proxy for auth (nginx, Envoy)
- **Service Mesh** - Integrate with service mesh security (Istio, Linkerd)

### Future Authentication
Planned authentication mechanisms:
- **API Keys** - Token-based access control
- **JWT Tokens** - Integration with identity providers
- **mTLS** - Mutual TLS for service-to-service

## 📊 Integration Patterns

### Webhook Integration
```python
# Python webhook server example
from flask import Flask, request, jsonify
import requests

app = Flask(__name__)

@app.route('/demon-webhook', methods=['POST'])
def handle_demon_event():
    event = request.json

    if event['type'] == 'approval.requested:v1':
        # Auto-approve non-production environments
        if 'staging' in event['data']['gate_id']:
            approve_url = f"http://demon:3000/api/approvals/{event['run_id']}/{event['data']['gate_id']}/grant"
            response = requests.post(approve_url, json={
                "approver": "auto-approval-bot",
                "note": "auto-approved for staging"
            })

    return jsonify({"status": "processed"})
```

### Monitoring Integration
```bash
# Prometheus metrics example
curl http://localhost:3000/metrics

# Custom alerting based on approval patterns
curl http://localhost:3000/api/runs?status=awaiting_approval | \
  jq '.runs | length' | \
  prometheus-push-gateway
```

### CI/CD Integration
```yaml
# GitHub Actions example
- name: Wait for Demon approval
  run: |
    RUN_ID=$(demon-submit-workflow.sh)
    echo "Waiting for approval for run $RUN_ID"

    while true; do
      STATUS=$(curl -s http://demon:3000/api/runs/$RUN_ID | jq -r '.status')
      if [ "$STATUS" = "completed" ]; then
        echo "Workflow approved and completed"
        break
      elif [ "$STATUS" = "failed" ]; then
        echo "Workflow denied or failed"
        exit 1
      fi
      sleep 30
    done
```

## 📖 Contract Specifications

### JSON Schemas
All API requests and responses are validated against JSON schemas:

- [Event Schemas](../contracts/) - Complete event type definitions
- [API Schemas](../contracts/) - Request/response formats
- [Configuration Schemas](../contracts/) - Policy and quota formats

### Contract Management
```bash
# Download latest contracts
cargo run -p demonctl -- contracts fetch-bundle

# Export current contracts
cargo run -p demonctl -- contracts bundle --format json --include-wit

# Verify contract compatibility
cargo run -p demonctl -- contracts validate --schema event.schema.json --data event.json
```

## 🔍 Error Handling

### HTTP Status Codes
- `200 OK` - Successful operation
- `201 Created` - Resource created successfully
- `400 Bad Request` - Invalid request format
- `404 Not Found` - Resource not found
- `409 Conflict` - Approval conflict (first-writer-wins)
- `500 Internal Server Error` - System error

### Error Response Format
```json
{
  "error": {
    "code": "APPROVAL_CONFLICT",
    "message": "Approval already granted by another user",
    "details": {
      "existing_approver": "ops@example.com",
      "existing_timestamp": "2025-09-26T10:05:00Z"
    }
  }
}
```

### Retry Strategies
- **Idempotent Operations** - Safe to retry GET requests and duplicate approvals
- **Non-Idempotent Operations** - Avoid retrying POST requests that create resources
- **Backoff Strategy** - Use exponential backoff for temporary failures

## 🧪 Testing and Development

### Local Development
```bash
# Start Demon with test data
make dev
cargo run -p demonctl -- bootstrap --seed

# Test API endpoints
curl http://localhost:3000/api/runs
curl http://localhost:3000/health
```

### Integration Testing
```python
# Python integration test example
import requests
import json

def test_approval_workflow():
    # Create a test run (implementation specific)
    run_id = create_test_run()

    # Check initial status
    response = requests.get(f'http://localhost:3000/api/runs/{run_id}')
    assert response.status_code == 200
    assert response.json()['status'] == 'awaiting_approval'

    # Grant approval
    approval_response = requests.post(
        f'http://localhost:3000/api/approvals/{run_id}/test-gate/grant',
        json={"approver": "test@example.com", "note": "test approval"}
    )
    assert approval_response.status_code == 200

    # Verify completion
    final_response = requests.get(f'http://localhost:3000/api/runs/{run_id}')
    assert final_response.json()['status'] == 'completed'
```

## 📚 Reference

### Client Libraries
- **Rust** - Built-in support via `demonctl` crate
- **JavaScript/TypeScript** - NATS.js for event streaming
- **Python** - `requests` for REST API, `nats-py` for events
- **Go** - `net/http` for REST API, `nats.go` for events

### External Resources
- [NATS JetStream Client Documentation](https://docs.nats.io/jetstream)
- [OpenAPI Specification](../contracts/) - Machine-readable API spec
- [Postman Collection](../examples/) - Ready-to-import API collection

---

**🔗 Need API help?** Check our [contract specifications](../contracts/) or open an issue with the `api` label.