# Multi-stage Dockerfile for Folio
# Supports both amd64 and arm64 architectures

# Stage 1: Builder
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

# Copy Cargo.toml and Cargo.lock
COPY Cargo.toml Cargo.lock ./

# Create dummy files to build dependencies (avoids building real code yet)
RUN mkdir -p crates/engine/src crates/server/src crates/cli/src \
    && echo "// dummy" > crates/engine/src/lib.rs \
    && echo "// dummy" > crates/server/src/main.rs \
    && echo "// dummy" > crates/cli/src/main.rs \
    && echo "[package]\nname = \"engine\"\nversion = \"0.1.0\"\nedition = \"2024\"" > crates/engine/Cargo.toml \
    && echo "[package]\nname = \"server\"\nversion = \"0.1.0\"\nedition = \"2024\"" > crates/server/Cargo.toml \
    && echo "[package]\nname = \"cli\"\nversion = \"0.1.0\"\nedition = \"2024\"" > crates/cli/Cargo.toml

# Build dependencies (cached layer)
RUN cargo build --release || true

# Copy actual source code
COPY . .

# Build the actual project
RUN cargo build --release

# Stage 2: Runtime
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
