REQUEST: Milestone 1B / Event Log & Replay (JetStream)
Objective: Persist execution events to JetStream and enable deterministic replay.

Files:
  /engine/rituals/log.rs
  /engine/rituals/state.rs            # persist all transitions
  /engine/rituals/timers.rs           # persist schedule/fired events
  /contracts/schemas/events.ritual.started.v1.json
  /contracts/schemas/events.ritual.transitioned.v1.json
  /contracts/schemas/events.ritual.completed.v1.json

Contracts:
  - ritual.started:v1
  - ritual.state.transitioned:v1
  - ritual.completed:v1

Acceptance:
  - Replaying from the persisted log reconstructs the same final state with no duplicate side effects.
  - Backpressure-safe JetStream usage (consumer acks; subject naming: ritual.<ritualId>.<runId>.events).
  - M0 echo ritual still works unchanged.

Timebox: 6–8 hours
Commit size: ≤200 LOC per commit, 2–4 commits

Test plan:
  - Unit: append/iterate log; idempotency keys.
  - Integration: record→restart→replay and assert same state.