# Approval Escalation Chains

## Overview

The Approval Escalation Chains feature provides a mechanism to automatically escalate approval requests through multiple levels of authority when timeouts occur. This ensures critical approvals don't get stuck waiting for unavailable approvers.

## Configuration

Escalation chains are configured via the `APPROVAL_ESCALATION_RULES` environment variable, which accepts a JSON structure defining per-tenant and per-gate escalation policies.

### Configuration Format

```json
{
  "tenants": {
    "<tenant-id>": {
      "gates": {
        "<gate-id>": {
          "levels": [
            {
              "level": 1,
              "roles": ["role1", "role2"],
              "timeoutSeconds": 300,
              "emergencyOverride": false
            },
            {
              "level": 2,
              "roles": ["manager", "director"],
              "timeoutSeconds": 600,
              "emergencyOverride": true
            }
          ]
        }
      }
    }
  }
}
```

### Field Descriptions

- **level**: The escalation level number (must start at 1 and increment sequentially)
- **roles**: List of roles authorized to approve at this level
- **timeoutSeconds**: Time in seconds before escalating to the next level (0 = no timeout)
- **emergencyOverride**: Whether emergency override is allowed at this level

## Events

### approval.escalated:v1

Emitted when an approval request is escalated to a higher level:

```json
{
  "event": "approval.escalated:v1",
  "ts": "2025-09-17T10:00:00Z",
  "tenantId": "default",
  "runId": "run-123",
  "ritualId": "deploy-app",
  "gateId": "production-gate",
  "fromLevel": 1,
  "toLevel": 2,
  "reason": "timeout",
  "escalationState": {
    "current_level": 2,
    "total_levels": 3,
    "emergency_override": false,
    "level_started_at": "2025-09-17T10:00:00Z",
    "next_escalation_at": "2025-09-17T10:10:00Z",
    "escalation_history": [...]
  }
}
```

### approval.override:v1

Emitted when an emergency override is performed:

```json
{
  "event": "approval.override:v1",
  "ts": "2025-09-17T10:00:00Z",
  "tenantId": "default",
  "runId": "run-123",
  "ritualId": "deploy-app",
  "gateId": "production-gate",
  "approver": "ops@example.com",
  "overrideLevel": 2,
  "note": "Emergency security patch",
  "escalationState": {
    "current_level": 2,
    "total_levels": 2,
    "emergency_override": true
  }
}
```

## UI Integration

The Operate UI displays escalation information in the run detail view:

- Current escalation level and total levels
- Time remaining before next escalation
- Escalation history showing when and why escalations occurred
- Emergency override controls (when enabled for the current level)

### REST API Endpoints

#### Emergency Override
```
POST /api/tenants/{tenant}/approvals/{run_id}/{gate_id}/override
Body: {
  "approver": "email@example.com",
  "note": "Reason for override"
}
```

## TTL Worker Integration

The TTL worker (`demon-ttl-worker`) monitors approval expiry timers and triggers escalations when timeouts occur. It:

1. Watches for `timer.scheduled:v1` events with escalation timer IDs
2. Processes expiry by calling `process_expiry_if_pending`
3. Determines whether to escalate or deny based on the escalation configuration
4. Publishes appropriate events (`approval.escalated:v1` or `approval.denied:v1`)

## Example Use Cases

### Multi-tier Approval Workflow
```json
{
  "tenants": {
    "acme-corp": {
      "gates": {
        "production-deploy": {
          "levels": [
            {
              "level": 1,
              "roles": ["team-lead"],
              "timeoutSeconds": 1800
            },
            {
              "level": 2,
              "roles": ["engineering-manager"],
              "timeoutSeconds": 3600
            },
            {
              "level": 3,
              "roles": ["director", "vp-engineering"],
              "timeoutSeconds": 0,
              "emergencyOverride": true
            }
          ]
        }
      }
    }
  }
}
```

This configuration:
1. First requests approval from team leads (30 min timeout)
2. Escalates to engineering managers if not approved (1 hour timeout)
3. Finally escalates to directors/VPs with no timeout but emergency override enabled

## Backward Compatibility

When no escalation configuration is provided:
- The system falls back to traditional approval behavior
- TTL-based auto-deny continues to work as before
- No escalation events are emitted

## Security Considerations

- Emergency overrides create an audit trail via the `approval.override:v1` event
- Override permissions should be carefully controlled via role assignments
- All escalation events are immutably stored in the event stream
- The `note` field in overrides should document the business justification