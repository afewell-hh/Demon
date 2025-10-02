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

## Docker Build

Build the Docker image from the repository root:

```bash
# Build from the root directory (includes workspace dependencies)
docker build -f engine/Dockerfile -t demon-engine:latest .

# Run the container
docker run -p 8081:8081 demon-engine:latest

# Test the container
docker run --rm demon-engine:latest /usr/local/bin/engine
```

The Dockerfile uses a multi-stage build with:
- **Builder stage**: cargo-chef for dependency caching, Alpine-based Rust toolchain
- **Runtime stage**: Distroless static image (~5MB)
- **Security**: Runs as non-root user, static musl linking

## See Also

- [Main README](../README.md) — Project overview and quickstart
- [Runtime](../runtime/) — Capsule execution runtime
- [Demonctl](../demonctl/) — CLI tool for running rituals
