# Multi-stage Dockerfile for Folio
# Supports both amd64 and arm64 architectures
# Pushes to: docker push deesh2025/no-name:tagname

# Stage 1: Chef (prepares dependency recipe)
FROM rust:1.88 AS chef
WORKDIR /app
RUN cargo install cargo-chef --locked

# Stage 2: Planner (analyzes project dependencies)
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder (compiles dependencies and project)
FROM rust:1.88 AS builder

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    libgtk-3-0 \
    libx11-xcb1 \
    libxcomposite1 \
    libxcursor1 \
    libxdamage1 \
    libxi6 \
    libxtst6 \
    libnss3 \
    libcups2 \
    libxss1 \
    libxrandr2 \
    libasound2 \
    libatk1.0-0 \
    libatk-bridge2.0-0 \
    libpangocairo-1.0-0 \
    libpango-1.0-0 \
    libcairo2 \
    libgdk-pixbuf2.0-0 \
    libgconf-2-4 \
    libgdm1 \
    libglib2.0-0 \
    libgl1-mesa-glx \
    fonts-liberation \
    xdg-utils \
    wget \
    curl \
    unzip \
    chromium \
    libreoffice \
    && rm -rf /var/lib/apt/lists/*

# Set Chromium as the browser
ENV CHROME_PATH=/usr/bin/chromium

# Install cargo-chef in builder stage too
RUN cargo install cargo-chef --locked

# Copy recipe and cook dependencies (cached layer - only rebuilds if deps change)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy actual source code
COPY . .

# Build the actual project
RUN cargo build --release

# Stage 4: Runtime (minimal image with just binaries)
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libgtk-3-0 \
    libx11-xcb1 \
    libxcomposite1 \
    libxcursor1 \
    libxdamage1 \
    libxi6 \
    libxtst6 \
    libnss3 \
    libcups2 \
    libxss1 \
    libxrandr2 \
    libasound2 \
    libatk1.0-0 \
    libatk-bridge2.0-0 \
    libpangocairo-1.0-0 \
    libpango-1.0-0 \
    libcairo2 \
    libgdk-pixbuf2.0-0 \
    libgconf-2-4 \
    libgdm1 \
    libglib2.0-0 \
    libgl1-mesa-glx \
    fonts-liberation \
    xdg-utils \
    chromium \
    libreoffice \
    && rm -rf /var/lib/apt/lists/*

ENV CHROME_PATH=/usr/bin/chromium
ENV RUST_LOG=info

# Copy built binaries
COPY --from=builder /app/target/release/folio-server /app/folio-server
COPY --from=builder /app/target/release/folio /app/folio

# Copy test data and docs
COPY docs /app/docs
COPY crates/server/tests /app/tests

EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

CMD ["/app/folio-server", "serve"]
