# Engine

Ritual orchestration engine that interprets ritual definitions and emits lifecycle events.

## Overview

The engine is responsible for:
- Parsing ritual YAML definitions
- Orchestrating ritual execution workflows
- Publishing events to NATS JetStream
- Managing approval gates and timers

## Quick Start

```bash
# Build the engine
cargo build -p engine

# Run tests
cargo test -p engine
```

## See Also

- [Main README](../README.md) — Project overview and quickstart
- [Runtime](../runtime/) — Capsule execution runtime
- [Demonctl](../demonctl/) — CLI tool for running rituals
