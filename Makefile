SHELL := /bin/bash
CARGO := cargo

.PHONY: dev up down build test fmt lint

dev: up build
	@echo "Dev environment ready."

up:
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
