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
build: ## Build full image for local platform (fast, for local testing)
	docker build --target pdfbro -t pdfbro:latest -f Dockerfile .

.PHONY: build-amd64
build-amd64: ## Build full image for linux/amd64 (required for Fly.io deploy)
	docker buildx build --platform linux/amd64 --target pdfbro -t pdfbro:amd64 -f Dockerfile .

.PHONY: build-chromium
build-chromium: ## Build Chromium-only image
	docker build --target pdfbro-chromium -t pdfbro:chromium -f Dockerfile .

.PHONY: build-libreoffice
build-libreoffice: ## Build LibreOffice-only image
	docker build --target pdfbro-libreoffice -t pdfbro:libreoffice -f Dockerfile .

.PHONY: run
run: ## Run full image (Chromium + LibreOffice) via Docker Compose
	docker compose up pdfbro

.PHONY: run-chromium
run-chromium: ## Run Chromium-only image via Docker Compose
	docker compose --profile chromium up pdfbro-chromium

.PHONY: run-libreoffice
run-libreoffice: ## Run LibreOffice-only image via Docker Compose
	docker compose --profile libreoffice up pdfbro-libreoffice

# FLY_APP is read from fly.toml; override with: make deploy FLY_APP=myapp
FLY_APP ?= $(shell grep '^app' fly.toml 2>/dev/null | sed "s/app = '//;s/'//")

.PHONY: deploy
deploy: ## Build linux/amd64 image and deploy to Fly.io
	fly auth docker
	docker buildx build --platform linux/amd64 --target pdfbro \
		-t registry.fly.io/$(FLY_APP):latest \
		--load -f Dockerfile .
	docker push registry.fly.io/$(FLY_APP):latest
	fly deploy --app $(FLY_APP) --image registry.fly.io/$(FLY_APP):latest

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
	docker compose exec pdfbro /bin/bash || docker exec -it $$(docker ps -qf name=pdfbro) /bin/bash

.PHONY: logs
logs: ## View container logs
	docker compose logs -f pdfbro

.PHONY: health
health: ## Check pdfbro health endpoint
	@curl -s http://localhost:$(API_PORT)/health | jq . || curl -s http://localhost:$(API_PORT)/health

.PHONY: version
version: ## Check pdfbro version
	@curl -s http://localhost:$(API_PORT)/version || cargo run -p server -- version

.PHONY: build-release
build-release: ## Build release binaries
	cargo build --release

.PHONY: ui-build
ui-build: ## Build the operator console UI (requires Node in ui/)
	cd ui && npm run build

.PHONY: ui-dev
ui-dev: ## Start UI dev server with hot reload (run alongside pdfbro-server)
	cd ui && npm run dev

.PHONY: build-with-ui
build-with-ui: ui-build build-release ## Build UI then Rust binary (for local testing)

.PHONY: docker-test
docker-test: ## Run tests inside Docker container (with Chrome + LibreOffice)
	docker build -t pdfbro-test -f Dockerfile.test .
	docker run --rm pdfbro-test

# Export all variables
export

# =============================================================================
# Docker Multi-Variant Build & Push
#
# All 9 variants live in a single Dockerfile as named --target stages.
#
#   Target name               Tag suffix        Description
#   ─────────────────────     ──────────────    ──────────────────────────────
#   pdfbro                    (none)            Full: Chromium + LibreOffice
#   pdfbro-chromium           -chromium         Chromium only (~30% smaller)
#   pdfbro-libreoffice        -libreoffice      LibreOffice only (~40% smaller)
#   pdfbro-cloudrun           -cloudrun         Full + Cloud Run env vars
#   pdfbro-cloudrun-chromium  -chromium-cloudrun  Chromium + Cloud Run
#   pdfbro-cloudrun-libreoffice -libreoffice-cloudrun LibreOffice + Cloud Run
#   pdfbro-lambda             -lambda           Full + Lambda Web Adapter
#   pdfbro-lambda-chromium    -chromium-lambda  Chromium + Lambda Web Adapter
#   pdfbro-lambda-libreoffice -libreoffice-lambda LibreOffice + Lambda Web Adapter
#
# Override registry:   make docker-build-all DOCKER_REGISTRY=myrepo/pdfbro
# =============================================================================

DOCKER_REGISTRY ?= ghcr.io/inkkit/pdfbro
VERSION ?= 0.1.0
MAJOR_VERSION ?= 0

# Internal helper — build a target and tag it.
# $(1) = Dockerfile target name, $(2) = tag suffix (e.g. "-chromium", or "" for full)
define docker_build
	docker build --target $(1) \
	  -t $(DOCKER_REGISTRY):latest$(2) \
	  -t $(DOCKER_REGISTRY):$(MAJOR_VERSION)$(2) \
	  .
endef

# ---- Build targets ----------------------------------------------------------

.PHONY: docker-build
docker-build: ## Build full image (Chromium + LibreOffice)
	$(call docker_build,pdfbro,)
	docker tag $(DOCKER_REGISTRY):latest $(DOCKER_REGISTRY):v$(VERSION)

.PHONY: docker-build-chromium
docker-build-chromium: ## Build Chromium-only image
	$(call docker_build,pdfbro-chromium,-chromium)

.PHONY: docker-build-libreoffice
docker-build-libreoffice: ## Build LibreOffice-only image
	$(call docker_build,pdfbro-libreoffice,-libreoffice)

.PHONY: docker-build-cloudrun
docker-build-cloudrun: ## Build Cloud Run full image
	$(call docker_build,pdfbro-cloudrun,-cloudrun)

.PHONY: docker-build-cloudrun-chromium
docker-build-cloudrun-chromium: ## Build Cloud Run Chromium-only image
	$(call docker_build,pdfbro-cloudrun-chromium,-chromium-cloudrun)

.PHONY: docker-build-cloudrun-libreoffice
docker-build-cloudrun-libreoffice: ## Build Cloud Run LibreOffice-only image
	$(call docker_build,pdfbro-cloudrun-libreoffice,-libreoffice-cloudrun)

.PHONY: docker-build-lambda
docker-build-lambda: ## Build AWS Lambda full image (Lambda Web Adapter)
	$(call docker_build,pdfbro-lambda,-lambda)

.PHONY: docker-build-lambda-chromium
docker-build-lambda-chromium: ## Build AWS Lambda Chromium-only image
	$(call docker_build,pdfbro-lambda-chromium,-chromium-lambda)

.PHONY: docker-build-lambda-libreoffice
docker-build-lambda-libreoffice: ## Build AWS Lambda LibreOffice-only image
	$(call docker_build,pdfbro-lambda-libreoffice,-libreoffice-lambda)

.PHONY: docker-build-all
docker-build-all: ## Build all 9 variants sequentially
docker-build-all: docker-build docker-build-chromium docker-build-libreoffice \
                  docker-build-cloudrun docker-build-cloudrun-chromium docker-build-cloudrun-libreoffice \
                  docker-build-lambda docker-build-lambda-chromium docker-build-lambda-libreoffice

# ---- Push targets (depend on their build counterpart) -----------------------

.PHONY: docker-push
docker-push: docker-build ## Push full image
	docker push $(DOCKER_REGISTRY):latest
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)
	docker push $(DOCKER_REGISTRY):v$(VERSION)

.PHONY: docker-push-chromium
docker-push-chromium: docker-build-chromium ## Push Chromium-only image
	docker push $(DOCKER_REGISTRY):latest-chromium
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-chromium

.PHONY: docker-push-libreoffice
docker-push-libreoffice: docker-build-libreoffice ## Push LibreOffice-only image
	docker push $(DOCKER_REGISTRY):latest-libreoffice
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-libreoffice

.PHONY: docker-push-cloudrun
docker-push-cloudrun: docker-build-cloudrun ## Push Cloud Run full image
	docker push $(DOCKER_REGISTRY):latest-cloudrun
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-cloudrun

.PHONY: docker-push-cloudrun-chromium
docker-push-cloudrun-chromium: docker-build-cloudrun-chromium ## Push Cloud Run Chromium image
	docker push $(DOCKER_REGISTRY):latest-chromium-cloudrun
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-chromium-cloudrun

.PHONY: docker-push-cloudrun-libreoffice
docker-push-cloudrun-libreoffice: docker-build-cloudrun-libreoffice ## Push Cloud Run LibreOffice image
	docker push $(DOCKER_REGISTRY):latest-libreoffice-cloudrun
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-libreoffice-cloudrun

.PHONY: docker-push-lambda
docker-push-lambda: docker-build-lambda ## Push Lambda full image
	docker push $(DOCKER_REGISTRY):latest-lambda
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-lambda

.PHONY: docker-push-lambda-chromium
docker-push-lambda-chromium: docker-build-lambda-chromium ## Push Lambda Chromium image
	docker push $(DOCKER_REGISTRY):latest-chromium-lambda
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-chromium-lambda

.PHONY: docker-push-lambda-libreoffice
docker-push-lambda-libreoffice: docker-build-lambda-libreoffice ## Push Lambda LibreOffice image
	docker push $(DOCKER_REGISTRY):latest-libreoffice-lambda
	docker push $(DOCKER_REGISTRY):$(MAJOR_VERSION)-libreoffice-lambda

.PHONY: docker-push-all
docker-push-all: ## Build and push all 9 variants
docker-push-all: docker-push docker-push-chromium docker-push-libreoffice \
                 docker-push-cloudrun docker-push-cloudrun-chromium docker-push-cloudrun-libreoffice \
                 docker-push-lambda docker-push-lambda-chromium docker-push-lambda-libreoffice

# ---- Utility ----------------------------------------------------------------

.PHONY: docker-login
docker-login: ## Login to Docker Hub
	docker login

.PHONY: docker-list-tags
docker-list-tags: ## List all local pdfbro images and sizes
	docker images $(DOCKER_REGISTRY) --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedAt}}"

.PHONY: docker-clean
docker-clean: ## Remove all local pdfbro images
	docker images $(DOCKER_REGISTRY) -q | xargs -r docker rmi -f
