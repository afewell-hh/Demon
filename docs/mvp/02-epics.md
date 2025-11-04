| Epic   | Description | Milestone  | Owner | Status       | Links           |
|--------|-------------|------------|-------|--------------|-----------------|
| MVP-E1 | Core Execution & Events | MVP-Alpha  | @afewell-hh | Complete  | issues: #56, #57 |
| MVP-E2 | Policy & Approval Engine | MVP-Beta  | @afewell-hh | Complete (Sprint 5) | issues: #58, #59, #60; PR: #93 |
| MVP-E3 | UI & API Interfaces | MVP-Alpha  | @afewell-hh | Complete  | issues: #61, #62 |
| MVP-E4 | Developer Experience | MVP-Alpha  | @afewell-hh | Complete  | issues: #63 |
| MVP-E6 | UI Dashboard | MVP-Alpha  | @afewell-hh | Complete (M1-1) | PR: #105 |
| MVP-E7 | Multi-tenant Foundations | MVP-Alpha  | @afewell-hh | Complete (M1-2) | PR: #107 |
| MVP-E8 | Advanced Policy Engine | MVP-Alpha  | @afewell-hh | Complete (M1-3) | PRs: #108, #109 |
| MVP-E9 | Schema Registry & Observability | MVP-Beta | @afewell-hh | In Progress (Sprint C) | Epic: #304 |

| Epic | Title | Outcome | Links |
|------|-------|---------|-------|
| MVP-E5 | CI/Protections Simplification | MVP-grade protections documented and enforced without blocking velocity | PRs: #53, #64, #65 |
| #121 | Contract & Schema Registry | Contract bundles publish via CI, versioned/signed, smoke-verified, fetched by tag, runtime ingestion, UI alerts/metrics | Stories: #124-#140; PRs: #132, #135-#137, #140, #217 (Track B: bundle-verify) |
| EPIC-4 | Graph Capsule MVP | ✅ Complete: Commit/tag engine, KV persistence, REST query API, CLI integration, Operate UI viewer | PRs: #209 (storage), #210 (engine), #211 (runtime), #212 (API), #213 (query), #214 (operate-ui) |
| EPIC-5 | Workflow Viewer | ✅ Complete: Serverless Workflow renderer, YAML parser, state visualization, SSE infrastructure | PR: #216 |

> 2025-09-30: EPIC-5 Workflow Viewer implementation complete: Serverless Workflow 1.0 support, legacy format support, state visualization with 6 states (pending/running/waiting/completed/faulted/suspended), SSE infrastructure for live updates, accessible design with ARIA labels, path traversal protection, 4.4 KB gzipped (well under 150 KB budget). Supports both local (examples/rituals/) and remote workflow loading.
> 2025-09-30: EPIC-4 Graph delivery complete with PRs #209-#214: storage layer, commit/tag engine, runtime wiring, REST API, query operations (get-node, neighbors, path-exists), and Operate UI graph viewer.
> 2025-09-30: Graph tag contract fixture corrections staged locally for PR #201 review feedback; awaiting remote access to push and update the thread.
> 2025-09-30: Secrets env-lock fix folded into PR #203 to serialize env-mutating tests and unblock CI.
