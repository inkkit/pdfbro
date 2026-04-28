include .env

.PHONY: help
help: ## Show this help message
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

# Default values
API_PORT=3000
CONCURRENCY=4
REQUEST_TIMEOUT=120s
LOG_LEVEL=info
CHROME_VERSION=stable

.PHONY: build
build: ## Build Folio Docker image
	docker build -t folio:latest -f Dockerfile .

.PHONY: run
run: ## Run Folio container via Docker Compose
	docker compose up folio

.PHONY: stop
stop: ## Stop all containers
	docker compose down -v

.PHONY: test-unit
test-unit: ## Run unit tests (no Chrome/LibreOffice required)
	cargo test --lib

.PHONY: test-integration
test-integration: ## Run integration tests (requires Chrome/LibreOffice)
	@echo "Running integration tests..."
	@if [ -z "$$CHROME_PATH" ] && [ -z "$$LIBREOFFICE_PATH" ]; then \
		echo "Warning: CHROME_PATH and LIBREOFFICE_PATH not set"; \
		echo "Make sure Chrome and LibreOffice are installed"; \
	fi
	cargo test -p server --test integration -- --ignored

.PHONY: test-chromium
test-chromium: ## Run Chromium-specific tests
	cargo test -p engine --test chromium_html -- --ignored

.PHONY: test-libreoffice
test-libreoffice: ## Run LibreOffice-specific tests
	cargo test -p engine --test libreoffice -- --ignored

.PHONY: test-e2e
test-e2e: ## Run end-to-end tests
	cargo test -p server --test e2e -- --ignored

.PHONY: test-all
test-all: test-unit test-chromium test-libreoffice test-e2e test-integration ## Run all tests

# Test tags (similar to Gotenberg's approach)
TAGS=
.PHONY: test-tags
test-tags: ## Run tests with specific tags (make test-tags TAGS=chromium)
	@echo "Running tests matching: $(TAGS)"
	cargo test -p server --test integration -- --ignored $(TAGS)

.PHONY: fmt
fmt: ## Format Rust code
	cargo fmt --all

.PHONY: lint
lint: ## Lint Rust code
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: check
check: fmt lint test-unit ## Run format, lint, and unit tests

.PHONY: docs
docs: ## Generate documentation
	cargo doc --no-deps --all-features

.PHONY: clean
clean: ## Clean build artifacts
	cargo clean
	docker compose down -v
	docker system prune -f

.PHONY: shell
shell: ## Open a shell in the running container
	docker compose exec folio /bin/bash || docker exec -it $$(docker ps -qf name=folio) /bin/bash

.PHONY: logs
logs: ## View container logs
	docker compose logs -f folio

.PHONY: health
health: ## Check Folio health endpoint
	@curl -s http://localhost:$(API_PORT)/health | jq . || curl -s http://localhost:$(API_PORT)/health

.PHONY: version
version: ## Check Folio version
	@curl -s http://localhost:$(API_PORT)/version || cargo run -p server -- version

.PHONY: build-release
build-release: ## Build release binaries
	cargo build --release

.PHONY: docker-test
docker-test: ## Run tests inside Docker container (with Chrome + LibreOffice)
	docker build -t folio-test -f Dockerfile .
	docker run --rm \
		-e CHROME_PATH=/usr/bin/google-chrome \
		-e LIBREOFFICE_PATH=/usr/bin/soffice \
		folio-test \
		cargo test --release -- --ignored

# Export all variables
export

# =============================================================================
# Docker Multi-Variant Build & Push
# =============================================================================

# Docker registry URL (override: make docker-build-all DOCKER_REGISTRY=deesh2025/no-name)
DOCKER_REGISTRY ?= deesh2025/no-name
VERSION ?= 0.1.0
MAJOR_VERSION ?= 8

# All Docker variants
VARIANTS = standard chromium libreoffice cloudrun cloudrun.chromium cloudrun.libreoffice lambda lambda.chromium lambda.libreoffice

.PHONY: docker-build-all
docker-build-all: $(VARIANTS:%=docker-build-%) ## Build all Docker variants sequentially

.PHONY: docker-push-all
docker-push-all: docker-build-all $(VARIANTS:%=docker-push-%) ## Build and push all variants

.PHONY: docker-build-standard
docker-build-standard: ## Build standard (full) variant
	docker build -t $(DOCKER_REGISTRY):latest -t $(DOCKER_REGISTRY):$(MAJOR_VERSION) -t $(DOCKER_REGISTRY):v$(VERSION) -f Dockerfile .

.PHONY: docker-build-chromium
docker-build-chromium: ## Build chromium-only variant (~30% smaller)
	docker build -t $(DOCKER_REGISTRY):latest-chromium -t $(DOCKER_REGISTRY):$(MAJOR_VERSION)-chromium -f Dockerfile.chromium .

.PHONY: docker-build-libreoffice
docker-build-libreoffice: ## Build libreoffice-only variant (~38% smaller)
	docker build -t $(DOCKER_REGISTRY):latest-libreoffice -t $(DOCKER_REGISTRY):$(MAJOR_VERSION)-libreoffice -f Dockerfile.libreoffice .

.PHONY: docker-build-cloudrun
docker-build-cloudrun: ## Build Cloud Run optimized (full)
	docker build -t $(DOCKER_REGISTRY):latest-cloudrun -t $(DOCKER_REGISTRY):$(MAJOR_VERSION)-cloudrun -f Dockerfile.cloudrun .

.PHONY: docker-build-cloudrun.chromium
docker-build-cloudrun.chromium: ## Build Cloud Run chromium-only
	docker build -t $(DOCKER_REGISTRY):latest-chromium-cloudrun -t $(DOCKER_REGISTRY):$(MAJOR_VERSION)-chromium-cloudrun -f Dockerfile.cloudrun.chromium .

.PHONY: docker-build-cloudrun.libreoffice
docker-build-cloudrun.libreoffice: ## Build Cloud Run libreoffice-only
	docker build -t $(DOCKER_REGISTRY):latest-libreoffice-cloudrun -t $(DOCKER_REGISTRY):$(MAJOR_VERSION)-libreoffice-cloudrun -f Dockerfile.cloudrun.libreoffice .

.PHONY: docker-build-lambda
docker-build-lambda: ## Build AWS Lambda optimized (full)
	docker build -t $(DOCKER_REGISTRY):latest-aws-lambda -t $(DOCKER_REGISTRY):$(MAJOR_VERSION)-aws-lambda -f Dockerfile.lambda .

.PHONY: docker-build-lambda.chromium
docker-build-lambda.chromium: ## Build AWS Lambda chromium-only
	docker build -t $(DOCKER_REGISTRY):latest-chromium-aws-lambda -t $(DOCKER_REGISTRY):$(MAJOR_VERSION)-chromium-aws-lambda -f Dockerfile.lambda.chromium .

.PHONY: docker-build-lambda.libreoffice
docker-build-lambda.libreoffice: ## Build AWS Lambda libreoffice-only
	docker build -t $(DOCKER_REGISTRY):latest-libreoffice-aws-lambda -t $(DOCKER_REGISTRY):$(MAJOR_VERSION)-libreoffice-aws-lambda -f Dockerfile.lambda.libreoffice .

# Push targets (sequential to avoid rate limiting)
.PHONY: docker-push-standard
docker-push-standard: ## Push standard variant
	docker push $(DOCKER_REGISTRY):latest
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)
	docker push $(DOCKER_REGISTRY):v$(VERSION)

.PHONY: docker-push-chromium
docker-push-chromium: ## Push chromium-only variant
	docker push $(DOCKER_REGISTRY):latest-chromium
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-chromium

.PHONY: docker-push-libreoffice
docker-push-libreoffice: ## Push libreoffice-only variant
	docker push $(DOCKER_REGISTRY):latest-libreoffice
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-libreoffice

.PHONY: docker-push-cloudrun
docker-push-cloudrun: ## Push Cloud Run full variant
	docker push $(DOCKER_REGISTRY):latest-cloudrun
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-cloudrun

.PHONY: docker-push-cloudrun.chromium
docker-push-cloudrun.chromium: ## Push Cloud Run chromium variant
	docker push $(DOCKER_REGISTRY):latest-chromium-cloudrun
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-chromium-cloudrun

.PHONY: docker-push-cloudrun.libreoffice
docker-push-cloudrun.libreoffice: ## Push Cloud Run libreoffice variant
	docker push $(DOCKER_REGISTRY):latest-libreoffice-cloudrun
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-libreoffice-cloudrun

.PHONY: docker-push-lambda
docker-push-lambda: ## Push AWS Lambda full variant
	docker push $(DOCKER_REGISTRY):latest-aws-lambda
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-aws-lambda

.PHONY: docker-push-lambda.chromium
docker-push-lambda.chromium: ## Push AWS Lambda chromium variant
	docker push $(DOCKER_REGISTRY):latest-chromium-aws-lambda
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-chromium-aws-lambda

.PHONY: docker-push-lambda.libreoffice
docker-push-lambda.libreoffice: ## Push AWS Lambda libreoffice variant
	docker push $(DOCKER_REGISTRY):latest-libreoffice-aws-lambda
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-libreoffice-aws-lambda

# Quick helpers
.PHONY: docker-login
docker-login: ## Login to Docker Hub
	docker login -u $(DOCKER_REGISTRY)

.PHONY: docker-list-tags
docker-list-tags: ## List all local Folio images
	docker images $(DOCKER_REGISTRY) --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedAt}}"

.PHONY: docker-clean
docker-clean: ## Remove all local Folio images
	docker images $(DOCKER_REGISTRY) -q | xargs -r docker rmi -f
