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

## See Also

- [Main README](../README.md) — Project overview and quickstart
- [Engine](../engine/) — Ritual orchestration engine
- [Echo Capsule](../capsules/echo/) — Sample capsule implementation
