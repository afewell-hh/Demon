# Wards Quotas and Time-Based Policies

## Overview

Wards provides both quota-based rate limiting and time-based policy enforcement:

- **Quotas**: Rate limiting based on request counts within time windows
- **Time-Based Policies**: Allow/deny rules based on time, timezone, and day of week

## Policy Evaluation Order

1. **Time-Based Policies**: Evaluated first against current time
   - If explicitly denied → request rejected with `deny_reason: "time_policy_denied"`
   - If explicitly allowed → proceed to quota check
   - If no rules match → proceed to quota check
2. **Quota Limits**: Standard rate limiting
   - If quota exceeded → request rejected with `deny_reason: "quota_exceeded"`
   - If under quota → request allowed

## Quota Configuration

- Precedence (per tenant, capability):
  - `WARDS_CAP_QUOTAS` (cap-level) → `WARDS_QUOTAS` (tenant-level) → `WARDS_GLOBAL_QUOTA` (global) → fallback `{limit:0, windowSeconds:60}`.
- Counters are scoped to `(tenant, capability)` using key `ten:<tenant>|cap:<capability>`.
- Emitted events always use `windowSeconds` (camelCase) even though internal Rust structs use `window_seconds` (snake_case).

## Env Examples

```json
// WARDS_CAP_QUOTAS
{
  "tenant-a": {
    "capsule.http": { "limit": 1, "windowSeconds": 60 },
    "capsule.echo": { "limit": 5, "windowSeconds": 60 }
  },
  "tenant-b": {
    "capsule.echo": { "limit": 2, "windowSeconds": 30 }
  }
}
```

```json
// WARDS_QUOTAS
{
  "tenant-a": { "limit": 2, "windowSeconds": 60 },
  "tenant-b": { "limit": 10, "windowSeconds": 60 }
}
```

```json
// WARDS_GLOBAL_QUOTA
{ "limit": 100, "windowSeconds": 300 }
```

## Time-Based Policy Configuration

### WARDS_SCHEDULES Environment Variable

Configure time-based policies using the `WARDS_SCHEDULES` environment variable with JSON format:

```json
{
  "global": {
    "capsule.audit": [
      {
        "action": "allow",
        "timezone": "UTC",
        "start": "00:00",
        "end": "23:59"
      }
    ],
    "capsule.deploy": [
      {
        "action": "deny",
        "timezone": "UTC",
        "days": ["Sun"],
        "start": "02:00",
        "end": "04:00"
      }
    ]
  },
  "tenant-a": {
    "capsule.deploy": [
      {
        "action": "allow",
        "timezone": "America/Los_Angeles",
        "days": ["Mon", "Tue", "Wed", "Thu", "Fri"],
        "start": "09:00",
        "end": "17:00",
        "escalation_timeout_seconds": 3600
      }
    ]
  }
}
```

### Schedule Rule Format

Each schedule rule contains:

- **action**: `"allow"` or `"deny"`
- **timezone**: IANA timezone name (e.g., `"UTC"`, `"America/Los_Angeles"`, `"Asia/Tokyo"`)
- **days**: Optional array of weekdays (`["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]`)
  - If omitted, applies to all days
- **start**: Start time in `"HH:MM"` format (24-hour)
- **end**: End time in `"HH:MM"` format (24-hour)
- **escalation_timeout_seconds**: Optional timeout for future auto-approval feature

### Schedule Precedence

1. **Tenant-specific rules** → **Global rules**
2. **First matching rule wins** within each scope
3. **No matching rules** → allow (proceed to quota evaluation)

### Time Range Handling

- **Same-day ranges**: `"09:00"` to `"17:00"` = 9 AM to 5 PM
- **Cross-midnight ranges**: `"22:00"` to `"06:00"` = 10 PM to 6 AM next day
- **Timezone awareness**: All times evaluated in the specified timezone with DST support

### Examples

#### Business Hours Only
```json
{
  "tenant-corp": {
    "capsule.prod_deploy": [
      {
        "action": "allow",
        "timezone": "America/New_York",
        "days": ["Mon", "Tue", "Wed", "Thu", "Fri"],
        "start": "09:00",
        "end": "17:00"
      }
    ]
  }
}
```

#### Maintenance Windows
```json
{
  "global": {
    "capsule.deploy": [
      {
        "action": "deny",
        "timezone": "UTC",
        "days": ["Sun"],
        "start": "02:00",
        "end": "04:00"
      }
    ]
  }
}
```

#### Multiple Rules (First Match Wins)
```json
{
  "global": {
    "capsule.sensitive": [
      {
        "action": "deny",
        "timezone": "UTC",
        "days": ["Sat", "Sun"],
        "start": "00:00",
        "end": "23:59"
      },
      {
        "action": "allow",
        "timezone": "UTC",
        "days": ["Mon", "Tue", "Wed", "Thu", "Fri"],
        "start": "08:00",
        "end": "18:00"
      }
    ]
  }
}
```

## Policy Evaluation Flow

```
Request for (tenant, capability)
    ↓
1. Evaluate Time-Based Policies
    ├─ Tenant-specific schedules for capability
    ├─ Global schedules for capability
    └─ First matching rule wins
    ↓
2. If time policy result:
    ├─ DENY → reject with "time_policy_denied"
    ├─ ALLOW → proceed to quota check
    └─ NO_MATCH → proceed to quota check
    ↓
3. Evaluate Quota Limits
    ├─ WARDS_CAP_QUOTAS (tenant + capability)
    ├─ WARDS_QUOTAS (tenant-wide)
    ├─ WARDS_GLOBAL_QUOTA (global)
    └─ Fallback {limit:0, windowSeconds:60}
    ↓
4. Final Decision
    └─ allowed: boolean + deny_reason: string?
```

## Legacy Quota Precedence Table

| Tenant | Capability   | Effective Quota  |
|--------|--------------|------------------|
| a      | capsule.http | cap (1/60)       |
| a      | capsule.echo | cap (5/60)       |
| a      | other        | tenant (2/60)    |
| b      | capsule.echo | cap (2/30)       |
| b      | other        | tenant (10/60)   |
| other  | any          | global (100/300) |
| none   | any          | fallback (0/60)  |

