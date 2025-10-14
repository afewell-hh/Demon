ID: REQUEST-BUG-bootstrapper-digest-test
Title: Fix bootstrapper digest expectation test
Type: Bug
Priority: p1
Area: area:backend
Milestone: MVP-Beta

Problem
-------
`demonctl` bootstrapper test fails locally on digest expectation:

```
test k8s_bootstrap::templates::tests::test_build_image_reference_with_digest ... FAILED

assertion failed
  left: "ghcr.io/afewell-hh/demon-operate-ui:sha-operate"
 right: "ghcr.io/afewell-hh/demon-operate-ui@sha256:abc"
```

Hypothesis: environment or branch-dependent tag resolution (expects `@sha256:` digest, got `:sha-...` tag). Needs deterministic fixture or branch-aware logic.

Acceptance Criteria
-------------------
- Reproduce failure on main branch.
- Make the test deterministic by isolating env/branch inputs or by injecting a fixed digest.
- Update test to assert stable output across environments.

Notes
-----
- Unrelated to SSE/cancel batch work in Issue #282.
- Open as a separate GitHub issue and link this request file in the PR.
