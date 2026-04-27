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
