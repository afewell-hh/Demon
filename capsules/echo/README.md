# Echo Capsule

Sample WebAssembly capsule demonstrating basic ritual step execution.

## Overview

The echo capsule is a simple reference implementation that:
- Accepts an input message
- Returns the message with metadata
- Demonstrates capsule interface contract
- Serves as a template for new capsules

## Building

```bash
# Build the capsule
cd capsules/echo
cargo build --target wasm32-unknown-unknown --release
```

## Usage

See `examples/rituals/echo.yaml` for example ritual definition using this capsule.

## See Also

- [Runtime](../../runtime/) — Capsule execution runtime
- [Contracts](../../contracts/wit/) — WIT interface definitions
- [Examples](../../examples/rituals/) — Sample ritual definitions
