REQUEST: Milestone 1A / Durable Timer Wheel
Objective: Implement durable timers with at-least-once firing and replay-on-restart.

Files:
  /engine/rituals/timers.rs
  /engine/rituals/state.rs            # add TimerScheduled/TimerFired transitions
  /engine/rituals/log.rs              # append-only event log facade (stub ok for 1A)
  /contracts/schemas/events.timer.scheduled.v1.json
  /contracts/schemas/events.timer.fired.v1.json
  /examples/rituals/timer.yaml

Contracts:
  - timer.scheduled:v1
  - timer.fired:v1

Acceptance:
  - Given a 5s timer, when engine restarts at T+3s, it fires once at ~T+5s and marks delivered.
  - Idempotent replays: repeated "fired" deliveries do not cause duplicate state transitions.
  - examples/rituals/timer.yaml runs via demonctl and produces scheduled→fired lifecycle in stdout (JetStream wiring may be stubbed in 1A; real persistence comes in 1B).

Timebox: 6–8 hours
Commit size: ≤200 LOC per commit, 2–4 commits

Test plan:
  - Unit: schedule parsing, timer due calculation, de-duplication guard.
  - Integration: run timer ritual; simulate restart; verify single logical firing.