# Folio - Standard Dockerfile (Full: Chromium + LibreOffice)
# Tags: latest, 8, v0.1.0
# Multi-arch support: amd64, arm64
# Pushes to: docker push deesh2025/no-name:tagname

ARG RUST_VERSION=1.88
ARG FOLIO_VERSION=0.1.0
ARG FOLIO_USER_UID=1001
ARG FOLIO_USER_GID=1001

# =============================================================================
# Stage 1: Chef (prepares dependency recipe)
# =============================================================================
FROM rust:${RUST_VERSION} AS chef
WORKDIR /app
RUN cargo install cargo-chef --locked

# =============================================================================
# Stage 2: Planner
# =============================================================================
FROM chef AS planner
COPY --link . .
RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# Stage 3: Builder
# =============================================================================
FROM rust:${RUST_VERSION} AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update -qq && apt-get upgrade -yqq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
    libgtk-3-0 libx11-xcb1 libxcomposite1 libxcursor1 \
    libxdamage1 libxi6 libxtst6 libnss3 libcups2 libxss1 \
    libxrandr2 libasound2 libatk1.0-0 libatk-bridge2.0-0 \
    libpangocairo-1.0-0 libpango-1.0-0 libcairo2 \
    libgdk-pixbuf2.0-0 libgconf-2-4 libgdm1 libglib2.0-0 \
    libgl1-mesa-glx fonts-liberation xdg-utils wget curl unzip \
    chromium libreoffice \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

ENV CHROME_PATH=/usr/bin/chromium
RUN cargo install cargo-chef --locked

# Cache dependencies (cached layer - only rebuilds if deps change)
COPY --link --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build with optimizations
COPY --link . .
RUN cargo build --release && \
    strip target/release/folio-server && \
    strip target/release/folio

# =============================================================================
# Stage 4: Runtime - Production
# =============================================================================
FROM debian:bookworm-slim

ARG FOLIO_VERSION
ARG FOLIO_USER_UID
ARG FOLIO_USER_GID

# Metadata
LABEL org.opencontainers.image.title="Folio" \
    org.opencontainers.image.description="A Docker-based API for converting documents to PDF" \
    org.opencontainers.image.version="${FOLIO_VERSION}" \
    org.opencontainers.image.authors="Folio Team" \
    org.opencontainers.image.source="https://github.com/been-there-done-that/folio"

# Set UTF-8 locale for consistent behavior
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8
ENV TZ=UTC

# Create non-root user for security
# All processes run with this dedicated user
RUN groupadd --gid "${FOLIO_USER_GID}" folio && \
    useradd --uid "${FOLIO_USER_UID}" --gid folio --shell /bin/bash \
    --home /home/folio --no-create-home folio && \
    mkdir -p /home/folio /app && \
    chown -R folio:folio /home/folio /app

# Install runtime dependencies with comprehensive font support
RUN apt-get update -qq && apt-get upgrade -yqq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
    # Init system for proper zombie process reaping
    tini \
    # Health checks
    curl \
    ca-certificates \
    # Chromium dependencies
    chromium \
    # LibreOffice components
    libreoffice-writer \
    libreoffice-calc \
    libreoffice-impress \
    libreoffice-draw \
    # Comprehensive font support (metric-compatible with MS fonts)
    fonts-crosextra-carlito \
    fonts-crosextra-caladea \
    fonts-liberation \
    fonts-liberation2 \
    fonts-dejavu \
    # CJK (Chinese, Japanese, Korean) support
    fonts-noto-cjk \
    # Emoji support
    fonts-noto-color-emoji \
    # Core fonts (tofu prevention)
    fonts-noto \
    fontconfig \
    # PDF tools
    qpdf \
    ghostscript \
    # Chromium runtime libs
    libgtk-3-0 libx11-xcb1 libxcomposite1 libxcursor1 \
    libxdamage1 libxi6 libxtst6 libnss3 libcups2 libxss1 \
    libxrandr2 libasound2 libatk1.0-0 libatk-bridge2.0-0 \
    libpangocairo-1.0-0 libpango-1.0-0 libcairo2 \
    libgdk-pixbuf2.0-0 libglib2.0-0 libgl1-mesa-glx \
    # Cleanup
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

# Copy binaries with explicit ownership
COPY --link --chown="${FOLIO_USER_UID}:${FOLIO_USER_GID}" --from=builder \
    /app/target/release/folio-server /usr/bin/
COPY --link --chown="${FOLIO_USER_UID}:${FOLIO_USER_GID}" --from=builder \
    /app/target/release/folio /usr/bin/

# Environment variables
ENV CHROME_PATH=/usr/bin/chromium
ENV CHROME_BIN=/usr/bin/chromium
ENV GS_BIN=/usr/bin/gs
ENV RUST_LOG=info
ENV FOLIO_VERSION=${FOLIO_VERSION}

# Use non-root user
USER folio
WORKDIR /home/folio

# Expose API port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Use tini for proper signal handling and zombie reaping
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/bin/folio-server", "serve"]
