ARG RUST_VERSION=1.88
ARG PDFBRO_VERSION
ARG PDFBRO_USER_UID=1001
ARG PDFBRO_USER_GID=1001
# Pinned for reproducible builds — bump deliberately when upgrading.
# See: https://snapshot.debian.org/package/chromium/
ARG CHROMIUM_VERSION=142.0.7444.175-1

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
# Stage: builder-full — compiles pdfbro with chromium + libreoffice features
# =============================================================================
FROM chef AS builder-full
WORKDIR /app
# No Chrome or LibreOffice needed at compile time; both are runtime subprocesses.
COPY --link --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --features "chromium libreoffice"
COPY --link . .
COPY --link --from=ui-builder /ui/build /app/ui/build
RUN cargo build --release --features "chromium libreoffice" && \
    strip target/release/pdfbro-server && \
    strip target/release/pdfbro

# =============================================================================
# Stage: builder-chromium — compiles pdfbro with chromium feature only
# =============================================================================
FROM chef AS builder-chromium
WORKDIR /app
COPY --link --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --no-default-features --features chromium
COPY --link . .
COPY --link --from=ui-builder /ui/build /app/ui/build
RUN cargo build --release --no-default-features --features chromium && \
    strip target/release/pdfbro-server && \
    strip target/release/pdfbro

# =============================================================================
# Stage: builder-libreoffice — compiles pdfbro with libreoffice feature only
# =============================================================================
FROM chef AS builder-libreoffice
WORKDIR /app
COPY --link --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --no-default-features --features libreoffice
COPY --link . .
COPY --link --from=ui-builder /ui/build /app/ui/build
RUN cargo build --release --no-default-features --features libreoffice && \
    strip target/release/pdfbro-server && \
    strip target/release/pdfbro

# =============================================================================
# Stage: common — non-root user, tini, fonts, PDF tools (no engines yet)
# =============================================================================
FROM debian:bookworm-slim AS common

ARG PDFBRO_VERSION
ARG PDFBRO_USER_UID
ARG PDFBRO_USER_GID

ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8
ENV TZ=UTC

RUN groupadd --gid "${PDFBRO_USER_GID}" pdfbro && \
    useradd --uid "${PDFBRO_USER_UID}" --gid pdfbro --shell /bin/bash \
        --home /home/pdfbro --no-create-home pdfbro && \
    mkdir -p /home/pdfbro && \
    chown pdfbro:pdfbro /home/pdfbro

RUN apt-get update -qq && apt-get upgrade -yqq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
        # Signal handling and zombie reaping.
        tini \
        # Used by health checks.
        curl \
        ca-certificates \
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

ENV PDFBRO_VERSION=${PDFBRO_VERSION}
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
# Suppress chromiumoxide DevTools Protocol noise that is harmless but confusing.
ENV RUST_LOG=info,chromiumoxide::handler=error

# =============================================================================
# Final stage: pdfbro — full image (Chromium + LibreOffice)
# Tags: latest, X.Y.Z
# =============================================================================
FROM common-chromium AS pdfbro

ARG PDFBRO_VERSION
ARG PDFBRO_USER_UID
ARG PDFBRO_USER_GID

LABEL org.opencontainers.image.title="pdfbro" \
      org.opencontainers.image.description="A Docker-based API for converting documents to PDF." \
      org.opencontainers.image.version="${PDFBRO_VERSION}" \
      org.opencontainers.image.authors="pdfbro Team" \
      org.opencontainers.image.source="https://github.com/vel/pdfbro"

# Install LibreOffice from Debian bookworm-backports (newer than bookworm's 7.4).
# python3-uno must match the LO version so it is also pulled from backports.
RUN echo "deb http://deb.debian.org/debian bookworm-backports main" \
      > /etc/apt/sources.list.d/backports.list && \
    apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -t bookworm-backports --no-install-recommends \
        libreoffice-writer \
        libreoffice-calc \
        libreoffice-impress \
        libreoffice-draw \
        python3-uno && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        python3-minimal \
        python3-pip && \
    pip3 install --no-cache-dir --break-system-packages unoserver==2.2.1 && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

# Force headless SVP rendering backend — skips virtual display probe (~50–100ms).
ENV SAL_USE_VCLPLUGIN=svp

COPY --link --chown="${PDFBRO_USER_UID}:${PDFBRO_USER_GID}" \
    --from=builder-full /app/target/release/pdfbro-server /usr/bin/
COPY --link --chown="${PDFBRO_USER_UID}:${PDFBRO_USER_GID}" \
    --from=builder-full /app/target/release/pdfbro /usr/bin/

USER pdfbro
WORKDIR /home/pdfbro
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/bin/pdfbro-server", "serve"]

# =============================================================================
# Final stage: pdfbro-chromium — Chromium only (~30% smaller than full)
# Tags: latest-chromium, X.Y.Z-chromium
# =============================================================================
FROM common-chromium AS pdfbro-chromium

ARG PDFBRO_VERSION
ARG PDFBRO_USER_UID
ARG PDFBRO_USER_GID

LABEL org.opencontainers.image.title="pdfbro (Chromium)" \
      org.opencontainers.image.description="A Docker-based API for converting documents to PDF — Chromium variant." \
      org.opencontainers.image.version="${PDFBRO_VERSION}" \
      org.opencontainers.image.authors="pdfbro Team" \
      org.opencontainers.image.source="https://github.com/vel/pdfbro"

COPY --link --chown="${PDFBRO_USER_UID}:${PDFBRO_USER_GID}" \
    --from=builder-chromium /app/target/release/pdfbro-server /usr/bin/
COPY --link --chown="${PDFBRO_USER_UID}:${PDFBRO_USER_GID}" \
    --from=builder-chromium /app/target/release/pdfbro /usr/bin/

USER pdfbro
WORKDIR /home/pdfbro
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/bin/pdfbro-server", "serve"]

# =============================================================================
# Final stage: pdfbro-libreoffice — LibreOffice only (~40% smaller than full)
# Tags: latest-libreoffice, X.Y.Z-libreoffice
# =============================================================================
FROM common AS pdfbro-libreoffice

ARG PDFBRO_VERSION
ARG PDFBRO_USER_UID
ARG PDFBRO_USER_GID

LABEL org.opencontainers.image.title="pdfbro (LibreOffice)" \
      org.opencontainers.image.description="A Docker-based API for converting documents to PDF — LibreOffice variant." \
      org.opencontainers.image.version="${PDFBRO_VERSION}" \
      org.opencontainers.image.authors="pdfbro Team" \
      org.opencontainers.image.source="https://github.com/vel/pdfbro"

# Install LibreOffice from Debian bookworm-backports (newer than bookworm's 7.4).
# python3-uno must match the LO version so it is also pulled from backports.
RUN echo "deb http://deb.debian.org/debian bookworm-backports main" \
      > /etc/apt/sources.list.d/backports.list && \
    apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y -t bookworm-backports --no-install-recommends \
        libreoffice-writer \
        libreoffice-calc \
        libreoffice-impress \
        libreoffice-draw \
        python3-uno && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        python3-minimal \
        python3-pip && \
    pip3 install --no-cache-dir --break-system-packages unoserver==2.2.1 && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

# Force headless SVP rendering backend — skips virtual display probe (~50–100ms).
ENV SAL_USE_VCLPLUGIN=svp

COPY --link --chown="${PDFBRO_USER_UID}:${PDFBRO_USER_GID}" \
    --from=builder-libreoffice /app/target/release/pdfbro-server /usr/bin/
COPY --link --chown="${PDFBRO_USER_UID}:${PDFBRO_USER_GID}" \
    --from=builder-libreoffice /app/target/release/pdfbro /usr/bin/

USER pdfbro
WORKDIR /home/pdfbro
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/bin/pdfbro-server", "serve"]

# =============================================================================
# Cloud Run variants — thin layers that set platform env vars.
# Cloud Run cannot start a container where the entrypoint is not owned by the
# running user. See https://github.com/gotenberg/gotenberg/issues/90.
# =============================================================================

# pdfbro-cloudrun: full (Chromium + LibreOffice)
# Tags: latest-cloudrun, X.Y.Z-cloudrun
FROM pdfbro AS pdfbro-cloudrun
USER root
RUN chown pdfbro: /usr/bin/tini
# Cloud Run injects PORT; pdfbro-server reads PDFBRO_PORT (or --port flag).
# Set PDFBRO_PORT_FROM_ENV so the server picks up PORT automatically.
ENV PDFBRO_PORT_FROM_ENV=PORT
ENV CHROMIUM_LAZY_START=false
ENV LIBREOFFICE_LAZY_START=false
ENV RUST_LOG=info
USER pdfbro

# pdfbro-cloudrun-chromium: Chromium only
# Tags: latest-chromium-cloudrun, X.Y.Z-chromium-cloudrun
FROM pdfbro-chromium AS pdfbro-cloudrun-chromium
USER root
RUN chown pdfbro: /usr/bin/tini
ENV PDFBRO_PORT_FROM_ENV=PORT
ENV CHROMIUM_LAZY_START=false
ENV RUST_LOG=info
USER pdfbro

# pdfbro-cloudrun-libreoffice: LibreOffice only
# Tags: latest-libreoffice-cloudrun, X.Y.Z-libreoffice-cloudrun
FROM pdfbro-libreoffice AS pdfbro-cloudrun-libreoffice
USER root
RUN chown pdfbro: /usr/bin/tini
ENV PDFBRO_PORT_FROM_ENV=PORT
ENV LIBREOFFICE_LAZY_START=false
ENV RUST_LOG=info
USER pdfbro

# =============================================================================
# AWS Lambda variants — use the Lambda Web Adapter (LWA) sidecar so pdfbro-server
# runs as a normal HTTP server without any Lambda-specific code changes.
# The LWA translates Lambda invoke events into HTTP requests on AWS_LWA_PORT.
# See: https://github.com/awslabs/aws-lambda-web-adapter
# =============================================================================

# pdfbro-lambda: full (Chromium + LibreOffice)
# Tags: latest-lambda, X.Y.Z-lambda
FROM pdfbro AS pdfbro-lambda
USER root
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.9.1 \
    /lambda-adapter /opt/extensions/lambda-adapter
ENV AWS_LWA_PORT=3000
ENV AWS_LWA_READINESS_CHECK_PATH=/health
ENV AWS_LWA_INVOKE_MODE=buffered
USER pdfbro

# pdfbro-lambda-chromium: Chromium only
# Tags: latest-chromium-lambda, X.Y.Z-chromium-lambda
FROM pdfbro-chromium AS pdfbro-lambda-chromium
USER root
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.9.1 \
    /lambda-adapter /opt/extensions/lambda-adapter
ENV AWS_LWA_PORT=3000
ENV AWS_LWA_READINESS_CHECK_PATH=/health
ENV AWS_LWA_INVOKE_MODE=buffered
USER pdfbro

# pdfbro-lambda-libreoffice: LibreOffice only
# Tags: latest-libreoffice-lambda, X.Y.Z-libreoffice-lambda
FROM pdfbro-libreoffice AS pdfbro-lambda-libreoffice
USER root
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:0.9.1 \
    /lambda-adapter /opt/extensions/lambda-adapter
ENV AWS_LWA_PORT=3000
ENV AWS_LWA_READINESS_CHECK_PATH=/health
ENV AWS_LWA_INVOKE_MODE=buffered
USER pdfbro
