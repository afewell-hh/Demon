# Development Docker Environment

Docker Compose configuration for local development with NATS JetStream.

## Overview

This directory contains Docker Compose files for running:
- NATS JetStream server (port 4222)
- NATS monitoring interface (port 8222)

## Usage

```bash
# Start NATS
make up

# Stop NATS
make down

# View logs
docker logs nats
```

## Configuration

- NATS server: `nats://127.0.0.1:4222`
- Monitoring: `http://127.0.0.1:8222`

## See Also

- [Main README](../../README.md) — Project quickstart
- [Operations Guide](../../docs/ops/) — Production deployment
- [Troubleshooting](../../docs/ops/docker-troubleshooting.md) — Docker issues
