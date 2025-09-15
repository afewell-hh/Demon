# ADR-0008: Tenant Namespace Isolation

**Status**: Accepted
**Date**: 2025-09-15

## Context

As Demon grows to support multiple customers/tenants, we need to ensure proper isolation of events, resources, and data between different tenant environments. Without tenant isolation, customers could potentially access or interfere with each other's ritual executions and data.

## Decision

We will implement tenant namespace isolation using a flagged rollout approach that modifies NATS JetStream subject schemas to include tenant identifiers.

### Subject Schema Evolution

- **Legacy (current)**: `demon.ritual.v1.<ritualId>.<runId>.events`
- **Tenant-scoped**: `demon.ritual.v1.<tenant>.<ritualId>.<runId>.events`

### Configuration

Environment variables control tenant behavior:

- `TENANTING_ENABLED=0|1` — Feature flag (default: 0, disabled)
- `TENANT_DEFAULT=default` — Fallback tenant (default: "default")
- `TENANT_ALLOWLIST=tenant1,tenant2` — Optional allowlist validation
- `TENANT_DUAL_PUBLISH=0|1` — Migration support (default: 0)

### Implementation Strategy

1. **Engine**: Modify EventLog to publish events to tenant-scoped subjects when enabled
2. **Operate UI**: Already supports tenant query/header resolution and filtering
3. **Legacy Support**: When disabled, use existing subject schema (no breaking changes)
4. **Migration Support**: Optional dual-publish to both tenant and legacy subjects

### Tenant Resolution

- HTTP Header: `X-Demon-Tenant`
- Query Parameter: `?tenant=`
- Environment Default: `TENANT_DEFAULT`
- Fallback: "default"

## Consequences

### Positive

- **Isolation**: Complete event isolation between tenants
- **Backward Compatible**: Legacy behavior preserved when disabled
- **Safe Rollout**: Feature flag allows gradual deployment
- **Migration Path**: Dual-publish enables smooth transitions
- **Consistent**: Same pattern across engine and UI components

### Negative

- **Subject Proliferation**: More NATS subjects when enabled
- **Configuration Complexity**: Additional environment variables
- **Migration Overhead**: Dual-publish doubles event volume temporarily

### Risks and Mitigation

- **Risk**: Subject explosion with many tenants
  - **Mitigation**: Monitor NATS performance; consider tenant consolidation strategies
- **Risk**: Tenant leakage during migration
  - **Mitigation**: Integration tests verify cross-tenant isolation
- **Risk**: Legacy systems unable to adapt
  - **Mitigation**: Dual-publish support and feature flag allow gradual rollout

## Rollback Plan

1. Set `TENANTING_ENABLED=0` to disable tenant-scoped subjects
2. Events automatically revert to legacy schema
3. No data loss; existing events remain accessible
4. Optional: Set `TENANT_DUAL_PUBLISH=1` during rollback for hybrid mode

## Testing

- Integration tests verify tenant isolation at engine level
- Operate UI tests ensure tenant-scoped API filtering
- Performance tests validate NATS behavior under tenant load
- Migration tests verify dual-publish functionality

## References

- Sprint 4 implementation: [tenant namespace isolation]
- Related: Policy quotas per-tenant (Sprint 5)
- NATS JetStream subject patterns documentation