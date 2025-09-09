# Wards Quotas (Per-Capability)

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

## Precedence Table

| Tenant | Capability   | Effective        |
|--------|--------------|------------------|
| a      | capsule.http | cap (1/60)       |
| a      | capsule.echo | cap (5/60)       |
| a      | other        | tenant (2/60)    |
| b      | capsule.echo | cap (2/30)       |
| b      | other        | tenant (10/60)   |
| other  | any          | global (100/300) |
| none   | any          | fallback (0/60)  |

