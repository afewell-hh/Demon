REQUEST: Milestone 1 / Workflow Timer Wheel
Objective: Implement durable timers with at-least-once firing, event-sourced execution log, and read-only Operate UI.

Files:
  /engine/rituals/state.rs
  /engine/rituals/timers.rs
  /engine/rituals/log.rs
  /operate-ui/src/main.rs
  /operate-ui/Cargo.toml
  /contracts/schemas/events.ritual.started.v1.json
  /contracts/schemas/events.ritual.transitioned.v1.json
  /contracts/schemas/events.ritual.completed.v1.json
  /contracts/schemas/events.timer.scheduled.v1.json
  /contracts/schemas/events.timer.fired.v1.json
  /examples/rituals/timer.yaml

Contracts:
  - ritual.started:v1
  - ritual.state.transitioned:v1
  - ritual.completed:v1
  - timer.scheduled:v1
  - timer.fired:v1

Acceptance:
  - Given a 5s timer, when engine restarts at T+3s, it fires once at ~T+5s and marks delivered.
  - Idempotent replays keep exactly-once semantics at the workflow level (at-least-once at the bus).
  - Operate UI (read-only) shows execution list, state transitions, and completion status.
  - Switch state executes the correct branch deterministically.
  - Examples/rituals/timer.yaml runs successfully through demonctl.

Timebox: 8–12 hours

Commit size: ≤200 LOC per commit, 2–4 commits per feature.

Test plan:
  - Unit tests: engine state transitions, timer scheduling and replay.
  - Integration tests: ritual with timer, engine restart at T+3s, verify single firing at T+5s.
  - Contract tests: JSON Schema validation for ritual and timer events.
  - UI tests: snapshot test for executions list and detail.
  - Manual: run cargo run -p demonctl -- run examples/rituals/timer.yaml and verify output + UI.
