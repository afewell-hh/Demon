REQUEST: Milestone 1C / Operate UI (Read-only)
Objective: Basic web UI to list executions and show per-run state history.

Files:
  /operate-ui/README.md
  /operate-ui/src/main.rs              # minimal HTTP server (Rust or Node ok)
  /operate-ui/src/routes.rs            # /runs, /runs/:id
  /operate-ui/Cargo.toml (if Rust)

Data source:
  - Read from JetStream (from M1B) or in-memory stub if M1B not merged yet.

Acceptance:
  - GET /runs returns recent runIds with ritualId, startTs, status.
  - GET /runs/:id returns ordered state transitions with timestamps.
  - Snapshot tests for HTML/JSON responses.

Timebox: 4–6 hours
Commit size: ≤200 LOC per commit, 2–3 commits

Test plan:
  - Unit: route handlers.
  - E2E: start server, run echo + timer rituals, verify list/detail.