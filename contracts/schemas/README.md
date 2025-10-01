# Event Schemas

JSON Schema definitions for Demon events and messages.

## Contents

- **Event schemas** — `events.*.v*.json` files defining event structure and validation rules
- **Approval schemas** — `approval.*.v*.json` for approval gate events
- **Timer schemas** — `events.timer.*.v*.json` for timer wheel events
- **Graph schemas** — `events.graph.*.v*.json` for graph commit/tag operations
- **Bootstrap schemas** — `bootstrap.*.v*.json` for bootstrapper bundle format
- **Policy schemas** — `policy.*.v*.json` for policy decision format

## Schema Validation

Schemas are used to validate events at:
- **Publish time** — Engine validates before publishing to NATS
- **Consume time** — Runtime validates before processing
- **Test time** — Fixtures in `../fixtures/` are validated against these schemas

## See Also

- [Event Fixtures](../fixtures/) — Test fixtures and golden files
- [WIT Definitions](../wit/) — WebAssembly Interface Type definitions
- [API Documentation](../../docs/api/) — API reference and event catalog
