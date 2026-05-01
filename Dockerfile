ARG RUST_VERSION=1.88
ARG FOLIO_VERSION=0.1.0
ARG FOLIO_USER_UID=1001
ARG FOLIO_USER_GID=1001
# Pinned for reproducible builds — bump deliberately when upgrading.
# See: https://snapshot.debian.org/package/chromium/
ARG CHROMIUM_VERSION=142.0.7444.175-1
# TDF (The Document Foundation) pinned LibreOffice release.
# Format: MAJOR.MINOR (e.g. 26.2). Maps to apt repo libreoffice-MAJOR-MINOR.
ARG LIBREOFFICE_VERSION=26.2

# =============================================================================
# Stage: ui-builder — builds the operator console SPA
# =============================================================================
FROM node:22-slim AS ui-builder
WORKDIR /ui
COPY ui/package*.json ui/bun.lock* ./
RUN npm install
COPY ui/ ./
RUN npm run build

# =============================================================================
# Stage: chef — installs cargo-chef for dependency caching
# =============================================================================
FROM rust:${RUST_VERSION} AS chef
WORKDIR /app
RUN cargo install cargo-chef --locked

# =============================================================================
# Stage: planner — produces recipe.json (shared by all builders)
# =============================================================================
FROM chef AS planner
COPY --link . .
RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# Stage: builder-full — compiles folio with chromium + libreoffice features
# =============================================================================
FROM chef AS builder-full
WORKDIR /app
# No Chrome or LibreOffice needed at compile time; both are runtime subprocesses.
COPY --link --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --features "chromium libreoffice"
COPY --link . .
COPY --link --from=ui-builder /ui/build /app/ui/build
RUN cargo build --release --features "chromium libreoffice" && \
    strip target/release/folio-server && \
    strip target/release/folio

# =============================================================================
# Stage: builder-chromium — compiles folio with chromium feature only
# =============================================================================
FROM chef AS builder-chromium
WORKDIR /app
COPY --link --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --no-default-features --features chromium
COPY --link . .
COPY --link --from=ui-builder /ui/build /app/ui/build
RUN cargo build --release --no-default-features --features chromium && \
    strip target/release/folio-server && \
    strip target/release/folio

# =============================================================================
# Stage: builder-libreoffice — compiles folio with libreoffice feature only
# =============================================================================
FROM chef AS builder-libreoffice
WORKDIR /app
COPY --link --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --no-default-features --features libreoffice
COPY --link . .
COPY --link --from=ui-builder /ui/build /app/ui/build
RUN cargo build --release --no-default-features --features libreoffice && \
    strip target/release/folio-server && \
    strip target/release/folio

# =============================================================================
# Stage: common — non-root user, tini, fonts, PDF tools (no engines yet)
# =============================================================================
FROM debian:bookworm-slim AS common

ARG FOLIO_VERSION
ARG FOLIO_USER_UID
ARG FOLIO_USER_GID

ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8
ENV TZ=UTC

RUN groupadd --gid "${FOLIO_USER_GID}" folio && \
    useradd --uid "${FOLIO_USER_UID}" --gid folio --shell /bin/bash \
        --home /home/folio --no-create-home folio && \
    mkdir -p /home/folio && \
    chown folio:folio /home/folio

RUN apt-get update -qq && apt-get upgrade -yqq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
        # Signal handling and zombie reaping.
        tini \
        # Used by health checks.
        curl \
        ca-certificates \
        # Required by LibreOffice TDF apt keyring setup (gpg --dearmor).
        gnupg \
        # Metric-compatible substitutes for common MS fonts (LibreOffice layout).
        fonts-crosextra-carlito \
        fonts-crosextra-caladea \
        fonts-liberation \
        fonts-liberation2 \
        # Reliable general-purpose fallback for Chromium.
        fonts-dejavu \
        # CJK (Chinese, Japanese, Korean).
        fonts-noto-cjk \
        # Emoji.
        fonts-noto-color-emoji \
        # Tofu prevention.
        fonts-noto \
        fontconfig \
        # PDF engines.
        qpdf \
        ghostscript \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

ENV FOLIO_VERSION=${FOLIO_VERSION}
ENV RUST_LOG=info
ENV GS_BIN=/usr/bin/gs

# =============================================================================
# Stage: common-chromium — common + Chromium (shared by full and chromium variants)
# =============================================================================
FROM common AS common-chromium

ARG CHROMIUM_VERSION

# On most architectures install the latest Chromium from the repo.
# On amd64/arm64 we pin a specific snapshot version for reproducibility.
RUN /bin/bash -c \
    'set -e; \
    ARCH="$(dpkg --print-architecture)"; \
    if [[ -n "$CHROMIUM_VERSION" && "$CHROMIUM_VERSION" != "latest" && \
          ("$ARCH" == "amd64" || "$ARCH" == "arm64") ]]; then \
      apt-get update -qq && \
      DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends devscripts && \
      debsnap chromium-common "$CHROMIUM_VERSION" -v --force --binary --architecture "$ARCH" && \
      debsnap chromium "$CHROMIUM_VERSION" -v --force --binary --architecture "$ARCH" && \
      dpkg -i --force-depends \
        "./binary-chromium-common/chromium-common_${CHROMIUM_VERSION}_${ARCH}.deb" \
        "./binary-chromium/chromium_${CHROMIUM_VERSION}_${ARCH}.deb" && \
      apt-get install -f -y -qq --no-install-recommends || true && \
      DEBIAN_FRONTEND=noninteractive apt-get purge -y -qq devscripts && \
      rm -rf ./binary-chromium-common ./binary-chromium; \
    else \
      apt-get update -qq && \
      DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends chromium; \
    fi' \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

# Install Chromium runtime libraries (not pulled in automatically by the snap package on some builds).
RUN apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
        libgtk-3-0 libx11-xcb1 libxcomposite1 libxcursor1 \
        libxdamage1 libxi6 libxtst6 libnss3 libcups2 libxss1 \
        libxrandr2 libasound2 libatk1.0-0 libatk-bridge2.0-0 \
        libpangocairo-1.0-0 libpango-1.0-0 libcairo2 \
        libgdk-pixbuf2.0-0 libglib2.0-0 libgl1-mesa-glx \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

ENV CHROME_PATH=/usr/bin/chromium
ENV CHROME_BIN=/usr/bin/chromium

# =============================================================================
# Final stage: folio — full image (Chromium + LibreOffice)
# Tags: latest, X.Y.Z
# =============================================================================
FROM common-chromium AS folio

ARG FOLIO_VERSION
ARG FOLIO_USER_UID
ARG FOLIO_USER_GID
ARG LIBREOFFICE_VERSION

LABEL org.opencontainers.image.title="Folio" \
      org.opencontainers.image.description="A Docker-based API for converting documents to PDF." \
      org.opencontainers.image.version="${FOLIO_VERSION}" \
      org.opencontainers.image.authors="Folio Team" \
      org.opencontainers.image.source="https://github.com/been-there-done-that/folio"

RUN LO_REPO="libreoffice-$(echo "${LIBREOFFICE_VERSION}" | tr '.' '-')" && \
    curl -fsSL "https://deb.libreoffice.org/libreoffice/${LO_REPO}/Release.key" \
      | gpg --dearmor -o /usr/share/keyrings/libreoffice.gpg && \
    echo "deb [signed-by=/usr/share/keyrings/libreoffice.gpg] https://deb.libreoffice.org/libreoffice/${LO_REPO}/ bookworm main" \
      > /etc/apt/sources.list.d/libreoffice.list && \
    apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        libreoffice-writer \
        libreoffice-calc \
        libreoffice-impress \
        libreoffice-draw \
        python3-minimal \
        python3-pip && \
    pip3 install --no-cache-dir --break-system-packages unoserver==2.2.1 && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

# Force headless SVP rendering backend — skips virtual display probe (~50–100ms).
ENV SAL_USE_VCLPLUGIN=svp

COPY --link --chown="${FOLIO_USER_UID}:${FOLIO_USER_GID}" \
    --from=builder-full /app/target/release/folio-server /usr/bin/
COPY --link --chown="${FOLIO_USER_UID}:${FOLIO_USER_GID}" \
    --from=builder-full /app/target/release/folio /usr/bin/

USER folio
WORKDIR /home/folio
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/bin/folio-server", "serve"]

# =============================================================================
# Final stage: folio-chromium — Chromium only (~30% smaller than full)
# Tags: latest-chromium, X.Y.Z-chromium
# =============================================================================
FROM common-chromium AS folio-chromium

ARG FOLIO_VERSION
ARG FOLIO_USER_UID
ARG FOLIO_USER_GID

LABEL org.opencontainers.image.title="Folio (Chromium)" \
      org.opencontainers.image.description="A Docker-based API for converting documents to PDF — Chromium variant." \
      org.opencontainers.image.version="${FOLIO_VERSION}" \
      org.opencontainers.image.authors="Folio Team" \
      org.opencontainers.image.source="https://github.com/been-there-done-that/folio"

COPY --link --chown="${FOLIO_USER_UID}:${FOLIO_USER_GID}" \
    --from=builder-chromium /app/target/release/folio-server /usr/bin/
COPY --link --chown="${FOLIO_USER_UID}:${FOLIO_USER_GID}" \
    --from=builder-chromium /app/target/release/folio /usr/bin/

USER folio
WORKDIR /home/folio
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/bin/folio-server", "serve"]

# =============================================================================
# Final stage: folio-libreoffice — LibreOffice only (~40% smaller than full)
# Tags: latest-libreoffice, X.Y.Z-libreoffice
# =============================================================================
FROM common AS folio-libreoffice

ARG FOLIO_VERSION
ARG FOLIO_USER_UID
ARG FOLIO_USER_GID
ARG LIBREOFFICE_VERSION

LABEL org.opencontainers.image.title="Folio (LibreOffice)" \
      org.opencontainers.image.description="A Docker-based API for converting documents to PDF — LibreOffice variant." \
      org.opencontainers.image.version="${FOLIO_VERSION}" \
      org.opencontainers.image.authors="Folio Team" \
      org.opencontainers.image.source="https://github.com/been-there-done-that/folio"

RUN LO_REPO="libreoffice-$(echo "${LIBREOFFICE_VERSION}" | tr '.' '-')" && \
    curl -fsSL "https://deb.libreoffice.org/libreoffice/${LO_REPO}/Release.key" \
      | gpg --dearmor -o /usr/share/keyrings/libreoffice.gpg && \
    echo "deb [signed-by=/usr/share/keyrings/libreoffice.gpg] https://deb.libreoffice.org/libreoffice/${LO_REPO}/ bookworm main" \
      > /etc/apt/sources.list.d/libreoffice.list && \
    apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        libreoffice-writer \
        libreoffice-calc \
        libreoffice-impress \
        libreoffice-draw \
        python3-minimal \
        python3-pip && \
    pip3 install --no-cache-dir --break-system-packages unoserver==2.2.1 && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

# Force headless SVP rendering backend — skips virtual display probe (~50–100ms).
ENV SAL_USE_VCLPLUGIN=svp

COPY --link --chown="${FOLIO_USER_UID}:${FOLIO_USER_GID}" \
    --from=builder-libreoffice /app/target/release/folio-server /usr/bin/
COPY --link --chown="${FOLIO_USER_UID}:${FOLIO_USER_GID}" \
    --from=builder-libreoffice /app/target/release/folio /usr/bin/

USER folio
WORKDIR /home/folio
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/bin/folio-server", "serve"]

# =============================================================================
# Cloud Run variants — thin layers that set platform env vars.
# Cloud Run cannot start a container where the entrypoint is not owned by the
# running user. See https://github.com/gotenberg/gotenberg/issues/90.
# =============================================================================

# folio-cloudrun: full (Chromium + LibreOffice)
# Tags: latest-cloudrun, X.Y.Z-cloudrun
FROM folio AS folio-cloudrun
USER root
RUN chown folio: /usr/bin/tini
# Cloud Run injects PORT; folio-server reads FOLIO_PORT (or --port flag).
# Set FOLIO_PORT_FROM_ENV so the server picks up PORT automatically.
ENV FOLIO_PORT_FROM_ENV=PORT
ENV CHROMIUM_LAZY_START=false
ENV LIBREOFFICE_LAZY_START=false
ENV RUST_LOG=info
USER folio

# folio-cloudrun-chromium: Chromium only
# Tags: latest-chromium-cloudrun, X.Y.Z-chromium-cloudrun
FROM folio-chromium AS folio-cloudrun-chromium
USER root
RUN chown folio: /usr/bin/tini
ENV FOLIO_PORT_FROM_ENV=PORT
ENV CHROMIUM_LAZY_START=false
ENV RUST_LOG=info
USER folio

# folio-cloudrun-libreoffice: LibreOffice only
# Tags: latest-libreoffice-cloudrun, X.Y.Z-libreoffice-cloudrun
FROM folio-libreoffice AS folio-cloudrun-libreoffice
USER root
RUN chown folio: /usr/bin/tini
ENV FOLIO_PORT_FROM_ENV=PORT
ENV LIBREOFFICE_LAZY_START=false
ENV RUST_LOG=info
USER folio

# =============================================================================
# AWS Lambda variants — use the Lambda Web Adapter (LWA) sidecar so folio-server
# runs as a normal HTTP server without any Lambda-specific code changes.
# The LWA translates Lambda invoke events into HTTP requests on AWS_LWA_PORT.
# See: https://github.com/awslabs/aws-lambda-web-adapter
# =============================================================================

# folio-lambda: full (Chromium + LibreOffice)
# Tags: latest-lambda, X.Y.Z-lambda
FROM folio AS folio-lambda
USER root
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.9.1 \
    /lambda-adapter /opt/extensions/lambda-adapter
ENV AWS_LWA_PORT=3000
ENV AWS_LWA_READINESS_CHECK_PATH=/health
ENV AWS_LWA_INVOKE_MODE=buffered
USER folio

# folio-lambda-chromium: Chromium only
# Tags: latest-chromium-lambda, X.Y.Z-chromium-lambda
FROM folio-chromium AS folio-lambda-chromium
USER root
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.9.1 \
    /lambda-adapter /opt/extensions/lambda-adapter
ENV AWS_LWA_PORT=3000
ENV AWS_LWA_READINESS_CHECK_PATH=/health
ENV AWS_LWA_INVOKE_MODE=buffered
USER folio

# folio-lambda-libreoffice: LibreOffice only
# Tags: latest-libreoffice-lambda, X.Y.Z-libreoffice-lambda
FROM folio-libreoffice AS folio-lambda-libreoffice
USER root
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.9.1 \
    /lambda-adapter /opt/extensions/lambda-adapter
ENV AWS_LWA_PORT=3000
ENV AWS_LWA_READINESS_CHECK_PATH=/health
ENV AWS_LWA_INVOKE_MODE=buffered
USER folio
