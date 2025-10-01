# Contracts

Event schemas, test fixtures, and interface definitions for Demon.

## Directory Structure

- **[schemas/](./schemas/)** — JSON Schema definitions for events and messages
- **[fixtures/](./fixtures/)** — Test fixtures and golden files
- **[wit/](./wit/)** — WebAssembly Interface Type (WIT) definitions

## Overview

This directory contains the contract definitions that specify the structure and validation rules for all events, messages, and interfaces in Demon. These contracts ensure compatibility between:
- Engine and Runtime components
- Event producers and consumers
- NATS JetStream and application code
- WebAssembly capsules and the host runtime

## Schema Validation

All events published to NATS are validated against their JSON schemas:
1. **At publish time** — Engine validates before sending to JetStream
2. **At consume time** — Runtime validates before processing
3. **In tests** — Fixtures are validated during CI

## See Also

- [API Documentation](../docs/api/) — API reference and event catalog
- [Engine](../engine/) — Event publishing and ritual orchestration
- [Runtime](../runtime/) — Event consumption and capsule execution
