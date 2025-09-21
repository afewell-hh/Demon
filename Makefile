SHELL := /bin/bash
CARGO := cargo

.PHONY: dev up down build test fmt lint deploy-ci-hardening audit-triage audit-triage-issue

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

bundle-contracts:
	@echo "📦 Generating contract bundle..."
	@mkdir -p dist/contracts
	@$(CARGO) run -p demonctl -- contracts bundle \
		--format json \
		--include-wit \
		> dist/contracts/bundle.json
	@echo "📋 Generating manifest..."
	@GIT_SHA=$$(git rev-parse HEAD); \
	 TIMESTAMP=$$(date -u +"%Y-%m-%dT%H:%M:%SZ"); \
	 BUNDLE_SHA256=$$(shasum -a 256 dist/contracts/bundle.json | cut -d' ' -f1); \
	 REPO=$$(git config --get remote.origin.url | sed 's/.*github.com[:/]\(.*\)\.git/\1/'); \
	 REF=$$(git symbolic-ref -q --short HEAD || git rev-parse HEAD); \
	 echo '{"version":"1.0.0","timestamp":"'$$TIMESTAMP'","git":{"sha":"'$$GIT_SHA'","repository":"'$$REPO'","ref":"'$$REF'"},"bundle_sha256":"'$$BUNDLE_SHA256'","trigger":"local"}' | jq '.' > dist/contracts/manifest.json
	@echo "✓ Bundle and manifest created in dist/contracts/"
