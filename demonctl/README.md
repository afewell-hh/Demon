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

## See Also

- [Main README](../README.md) — Project overview and quickstart
- [Engine](../engine/) — Ritual orchestration engine
- [Examples](../examples/rituals/) — Sample ritual definitions
