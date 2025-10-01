# Test Fixtures

Golden files and test data for Demon events, envelopes, and configuration.

## Directory Structure

- **approvals/** — Approval gate test fixtures (granted, denied, requested)
- **config/** — Configuration file fixtures for testing
- **envelopes/** — NATS message envelope examples
- **events/** — Event payload examples (ritual lifecycle, timers, approvals)

## Usage

These fixtures serve multiple purposes:
- **Contract tests** — Validate event schemas in `../schemas/`
- **Integration tests** — Seed test data for engine and runtime tests
- **Documentation** — Examples of well-formed events and messages
- **Golden files** — Expected output for regression testing

## Validation

All fixtures are validated against their corresponding JSON schemas during CI.
Changes to event structure must update both schemas and fixtures together.

## See Also

- [Event Schemas](../schemas/) — JSON Schema definitions
- [WIT Definitions](../wit/) — WebAssembly Interface Type definitions
- [Testing Guidelines](../../docs/process/DOC_STANDARDS.md#testing-guidelines) — Test documentation standards
