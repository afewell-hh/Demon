SHELL := /bin/bash
CARGO := cargo

.PHONY: dev up down build test fmt lint lint-contracts deploy-ci-hardening audit-triage audit-triage-issue

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

lint-contracts:
	@echo "Running contract linter tests..."
	@$(CARGO) run -p contract-linter -- compare \
		--current contracts/fixtures/linter/base-v1.0.0.json \
		--proposed contracts/fixtures/linter/compatible-v1.1.0.json \
		--current-version 1.0.0 \
		--proposed-version 1.1.0 || (echo "✗ Compatible change test failed" && exit 1)
	@echo "✓ Compatible change test passed"
	@! $(CARGO) run -p contract-linter -- compare \
		--current contracts/fixtures/linter/base-v1.0.0.json \
		--proposed contracts/fixtures/linter/breaking-v1.1.0.json \
		--current-version 1.0.0 \
		--proposed-version 1.1.0 >/dev/null 2>&1 || (echo "✗ Breaking change without major bump test failed (expected failure)" && exit 1)
	@echo "✓ Breaking change without major bump test correctly failed"
	@$(CARGO) run -p contract-linter -- compare \
		--current contracts/fixtures/linter/base-v1.0.0.json \
		--proposed contracts/fixtures/linter/breaking-v2.0.0.json \
		--current-version 1.0.0 \
		--proposed-version 2.0.0 || (echo "✗ Breaking change with major bump test failed" && exit 1)
	@echo "✓ Breaking change with major bump test passed"
	@echo "✓ All contract linter tests passed"

bootstrap-smoke:
	@echo "Running Kubernetes bootstrapper smoke test..."
	@./scripts/tests/smoke-k8s-bootstrap.sh $(ARGS)

deploy-ci-hardening:
	@GIT_USER_EMAIL=$${GIT_USER_EMAIL:-ops@example.com} \
	 GIT_USER_NAME=$${GIT_USER_NAME:-demon-ci-ops} \
	 bash scripts/deploy-ci-hardening.sh

# Generate triage MD for last N PRs (default 30) and print the newest file
audit-triage:
	@COUNT=$${COUNT:-30} ./audit-pr-triage-md.sh
	@ls -t pr-review-triage-*.md | head -n1 | xargs -I{} sh -c 'echo "\n---\nGenerated: {}"; head -n 20 {}'

# Optional: open a tracking issue with today’s report attached
audit-triage-issue:
	@./audit-pr-triage-md.sh
	@T="Review triage report — $$(date -u +%F)"; F=$$(ls -t pr-review-triage-*.md | head -n1); \
	  gh issue create -t "$$T" -F "$$F" -l ops-audit >/dev/null || true; \
	  echo "Created/attempted issue for $$F"
