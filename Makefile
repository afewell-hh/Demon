SHELL := /bin/bash
CARGO := cargo

.PHONY: dev up down build test fmt lint

dev: up build
	@echo "Dev environment ready on $$NATS_PORT (default 4222)."

up:
	NATS_PORT=$${NATS_PORT:-4222} NATS_MON_PORT=$${NATS_MON_PORT:-8222} \
	docker compose -f docker/dev/docker-compose.yml up -d

down:
	docker compose -f docker/dev/docker-compose.yml down -v

build:
	$(CARGO) build --workspace

test:
	$(CARGO) test --workspace

fmt:
	$(CARGO) fmt --all || true

lint:
	$(CARGO) clippy --workspace --all-targets -- -D warnings || true
