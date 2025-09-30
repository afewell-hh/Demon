# WIT Contracts (Sigils)

This folder contains WebAssembly Interface Type (WIT) definitions for Demon contracts.

## Available Interfaces

- `demon-envelope.wit` - Result envelope interface with typed bindings for operation results, diagnostics, suggestions, metrics, and provenance
- `demon-graph.wit` - Graph store interface for commits, queries, and tag management

## Usage

These WIT definitions provide typed interfaces that can be used by capsule authors and downstream tooling to ensure type safety when working with Demon contracts.

To export all contracts including WIT definitions:
```bash
demonctl contracts bundle --include-wit --format json
```
