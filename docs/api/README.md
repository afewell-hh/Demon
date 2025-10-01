# API Documentation Hub

![Status: Current](https://img.shields.io/badge/Status-Current-green)

Comprehensive documentation for Demon's APIs, schemas, and event formats.

## Overview

This hub provides centralized access to all API documentation, including REST endpoints, event schemas, and contract specifications.

## REST API Reference

### Engine API
| Endpoint | Method | Purpose | Status |
|----------|--------|---------|--------|
| `/api/runs` | GET | List ritual runs | Current |
| `/api/runs/{id}` | GET | Get run details | Current |
| `/api/approvals/{run_id}/{gate_id}/grant` | POST | Grant approval | Current |
| `/api/approvals/{run_id}/{gate_id}/deny` | POST | Deny approval | Current |

### Runtime API
| Endpoint | Method | Purpose | Status |
|----------|--------|---------|--------|
| `/health` | GET | Health check | Current |
| *Coming soon* | *Various* | *Capsule management endpoints* | Draft |

## Event Schemas

### Core Events
| Event Type | Schema | Description | Status |
|------------|--------|-------------|--------|
| `ritual.started:v1` | [schema](../../contracts/schemas/) | Ritual execution started | Current |
| `ritual.completed:v1` | [schema](../../contracts/schemas/) | Ritual execution completed | Current |
| `approval.requested:v1` | [schema](../../contracts/schemas/) | Approval gate triggered | Current |
| `approval.granted:v1` | [schema](../../contracts/schemas/) | Approval granted | Current |
| `approval.denied:v1` | [schema](../../contracts/schemas/) | Approval denied | Current |
| `policy.decision:v1` | [schema](../../contracts/schemas/) | Policy decision made | Current |

## Contract Specifications

- **[JSON Schemas](../../contracts/schemas/)** - Event and API data structures
- **[Test Fixtures](../../contracts/fixtures/)** - Golden files for validation
- **[WIT Definitions](../../contracts/wit/)** - WebAssembly Interface Types

## Quick Start

1. **REST API**: Start with `/api/runs` to list available ritual executions
2. **Events**: Subscribe to `ritual.*` events for execution monitoring
3. **Schemas**: Validate payloads against JSON schemas in contracts/

## Authentication

*Authentication documentation will be added as the security model is implemented*

## Rate Limits

*Rate limiting documentation will be added as limits are implemented*

## See Also

- [Reference](../reference/) - Technical specifications and definitions
- [How-to Guides](../how-to-guides/) - Problem-solving oriented guides
- [Contracts](../../contracts/) - API schemas and event definitions

[← Back to Documentation Home](../README.md)