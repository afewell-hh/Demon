## MVP Alpha (M0) - COMPLETE ✅

| Epic   | Description | Milestone  | Owner | Status       | Links           |
|--------|-------------|------------|-------|--------------|-----------------|
| MVP-E1 | Core Execution & Events | MVP-Alpha  | @afewell-hh | ✅ Complete  | M0-1, M0-2: issues #56, #57 |
| MVP-E2 | Policy & Approval Engine | MVP-Alpha  | @afewell-hh | ✅ Complete  | M0-3, M0-4, M0-5: issues #58, #59, #60 |
| MVP-E3 | UI & API Interfaces | MVP-Alpha  | @afewell-hh | ✅ Complete  | M0-6, M0-7: issues #61, #62 |
| MVP-E4 | Developer Experience | MVP-Alpha  | @afewell-hh | ✅ Complete  | M0-8: issue #63 |
| MVP-E5 | CI/Protections Simplification | MVP-Alpha  | @afewell-hh | ✅ Complete | PRs: #53, #64, #65 |

**Alpha RC:** `preview-alpha-rc-1` - All 8 M0 capabilities delivered and verified

## MVP Beta (M1) - PLANNED

| Epic   | Description | Milestone  | Owner | Status | Priority | Links |
|--------|-------------|------------|-------|---------|----------|-------|
| MVP-E6 | Enhanced UI Dashboard | MVP-Beta   | @afewell-hh | Stories Created | P0 | Issues #83, #84, #85 |
| MVP-E7 | Multi-tenant Foundations | MVP-Beta   | @afewell-hh | Stories Created | P0 | Issues #86, #87 |
| MVP-E8 | Advanced Policy Engine | MVP-Beta   | @afewell-hh | Stories Created | P1 | Issues #88, #89 |

### M1 Capabilities (Beta)

**MVP-E6: Enhanced UI Dashboard**
- Real-time event streaming to browser
- Filtering and search capabilities
- Approval action buttons in UI
- Interactive run timeline visualization

**MVP-E7: Multi-tenant Foundations**
- Namespace isolation for workloads
- Per-tenant quotas and policies
- Resource boundary enforcement
- Tenant-specific API endpoints

**MVP-E8: Advanced Policy Engine**
- Time-based policies and schedules
- Complex approval workflows
- Escalation chains and timeouts
- Policy composition and inheritance

## Engineering Implementation Plan (Suggested Order)

### Phase 1: UI Real-time Foundation (Sprint 1)
**Priority:** P0 - Foundation for all UI enhancements
- **Issue #83:** Real-time Event Streaming in UI
  - Server-sent events endpoint: `/api/runs/<id>/events/stream`
  - JavaScript EventSource integration with auto-reconnect
  - Connection status indicator and resilient retry
  - **Acceptance:** Live timeline updates without page reload

### Phase 2: UI Search & Filter (Sprint 2)
**Priority:** P0 - User experience enhancement
- **Issue #84:** Filtering and Search in UI
  - Backend API query parameters for filtering
  - Frontend filter controls and URL state management
  - Performance optimization for large datasets
  - **Acceptance:** Shareable filtered views, fast search

### Phase 3: UI Approval Actions (Sprint 3) ✅ COMPLETE
**Priority:** P0 - Complete interactive dashboard
- **Issue #85:** In-UI Approval Action Buttons ✅ COMPLETE
  - Grant/deny buttons with form inputs ✅
  - CSRF protection and authorization ✅ (APPROVER_ALLOWLIST)
  - Real-time approval status updates ✅ (SSE integration)
  - **Acceptance:** Full approval workflow in UI ✅

### Phase 4: Multi-tenant Isolation (Sprint 4)
**Priority:** P0 - Foundation for scaling
- **Issue #86:** Tenant Namespace Isolation
  - Event schema updates with tenant fields
  - JetStream subject partitioning by tenant
  - API route scoping and data isolation
  - **Acceptance:** Zero cross-tenant data leakage

### Phase 5: Per-tenant Quotas (Sprint 5)
**Priority:** P0 - Multi-tenant policy enforcement
- **Issue #87:** Per-Tenant Quotas and Policies
  - Tenant-specific configuration system
  - Isolated quota tracking and enforcement
  - Admin APIs for quota management
  - **Acceptance:** Independent tenant quotas working

### Phase 6: Advanced Policies (Future Sprints)
**Priority:** P1 - Advanced features
- **Issue #88:** Time-Based Policy Engine (Sprint 6)
- **Issue #89:** Approval Escalation Chains (Sprint 7)

## Dependencies & Risks
- **Real-time streaming** (Phase 1) enables all subsequent UI work
- **Tenant isolation** (Phase 4) must complete before quota work (Phase 5)
- **Frontend** work (Phases 1-3) can proceed in parallel with **backend** tenant work (Phases 4-5)
- **Risk:** Multi-tenant changes may require event schema migrations
