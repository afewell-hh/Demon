# Demonctl

Command-line interface for running rituals and interacting with the Demon system.

## Overview

Demonctl provides commands for:
- Running ritual definitions from YAML files
- Validating ritual syntax
- Managing ritual executions
- Debugging and troubleshooting

## Quick Start

```bash
# Build demonctl
cargo build -p demonctl

# Run a ritual
cargo run -p demonctl -- run examples/rituals/echo.yaml

# Show help
cargo run -p demonctl -- --help
```

## Installed App Pack Mounts

When you run a ritual from an installed App Pack (e.g., `demonctl run hoss:hoss-validate`),
the runtime mounts the pack into the container with hardened settings:

- App Pack root is mounted read-only at `/workspace`.
- The pack’s artifacts directory is mounted read-write at `/workspace/.artifacts`.
- A direct file bind is added from the host placeholder to the container target
  envelope path (e.g., `/workspace/.artifacts/result.json`) to guarantee writes
  with non-root users.

Capsule commands should reference scripts relative to `/workspace`, for example:
`/workspace/capsules/<capsule>/scripts/run.sh`. If you see "script not found",
ensure you installed the pack via `demonctl app install <pack_dir>` before running.

## See Also

- [Main README](../README.md) — Project overview and quickstart
- [Engine](../engine/) — Ritual orchestration engine
- [Examples](../examples/rituals/) — Sample ritual definitions
