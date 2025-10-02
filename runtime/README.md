# Runtime

WebAssembly runtime for executing capsule code in response to ritual events.

## Overview

The runtime is responsible for:
- Loading and executing WebAssembly capsules
- Routing calls between engine and capsules
- Providing host functions to capsule code
- Managing capsule lifecycle and sandboxing

## Quick Start

```bash
# Build the runtime
cargo build -p runtime

# Run tests
cargo test -p runtime
```

## Docker Build

Build the Docker image from the repository root:

```bash
# Build from the root directory (includes workspace dependencies)
docker build -f runtime/Dockerfile -t demon-runtime:latest .

# Run the container
docker run -p 8080:8080 demon-runtime:latest

# Test the container
docker run --rm demon-runtime:latest /usr/local/bin/runtime
```

The Dockerfile uses a multi-stage build with:
- **Builder stage**: cargo-chef for dependency caching, Alpine-based Rust toolchain
- **Runtime stage**: Distroless static image (~13MB)
- **Security**: Runs as non-root user, static musl linking

## See Also

- [Main README](../README.md) — Project overview and quickstart
- [Engine](../engine/) — Ritual orchestration engine
- [Echo Capsule](../capsules/echo/) — Sample capsule implementation
