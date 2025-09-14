![CI](https://github.com/afewell-hh/demon/actions/workflows/ci.yml/badge.svg)
[![Replies guard](https://img.shields.io/github/actions/workflow/status/afewell-hh/Demon/review-threads-guard.yml?label=replies-guard)](../../actions/workflows/review-threads-guard.yml)

# Demon — Meta-PaaS (Milestone 0)

Thin-slice bootstrapping of the Demon project.

## Quickstart

```bash
make dev            # bring up NATS JetStream & build workspace
cargo run -p demonctl -- run examples/rituals/echo.yaml
```


**Labels quickstart**

```bash
make bootstrap-labels   # ensures triage-comment & enforce-review-replies
# On your PR:
# - Add triage-comment to get a sticky summary of review threads
# - Add enforce-review-replies to require author replies before merge
```

Expected output:

The echo capsule prints `Hello from Demon!`

A JSON event for `ritual.completed:v1` is printed to stdout.

## Layout

- `engine/` — minimal ritual interpreter (M0).
- `runtime/` — link-name router (stubs).
- `capsules/echo/` — hello capsule.
- `contracts/` — JSON Schemas + future WIT.
- `demonctl/` — CLI to run rituals.
- `docker/dev` — NATS JetStream profile.

## Next

- Wire the event to NATS (JetStream) instead of stdout (M1).
- Add durable timers & replays.
- Add Operate UI (read-only).

## Labels we use (CI helpers)

- `triage-comment` — Posts/updates a sticky summary with unresolved review threads and quick links. Visibility only; does not block.
- `enforce-review-replies` — Runs the Review Replies Guard:
  - FAILS if any unresolved review thread has no author reply.
  - PASSES once the author has replied in all unresolved threads (even before resolving).
  - PASSES when all threads are resolved.
  - Auto-skips docs-only changes.

Tips:
- Keep “Require conversation resolution before merging” enabled for the final gate.
- Run `make bootstrap-labels` once per repo to create these labels.

See also: `docs/process/review-replies-guard.md`.