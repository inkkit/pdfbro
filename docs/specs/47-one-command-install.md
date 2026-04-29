# Spec 47 — One-Command Install

> Make Folio the easiest PDF generation tool to install.
> Gotenberg requires Docker + Chrome + LibreOffice setup.
> Folio should be: `curl -sSL https://folio.dev/install.sh | bash`

## Goal

Create a frictionless installation experience that gets
users from "nothing" to "first PDF in 30 seconds".
This is critical for adoption (see wkhtmltopdf archived 2023
due to installation complexity).

## Problem Analysis#

### Current State (Painful)#

#### Gotenberg (Requires Docker)#

```bash
# Gotenberg installation (complex)
docker pull gotenberg/gotenberg:8
docker run -p 3000:3000 gotenberg/gotenberg:8

# Need Chrome + LibreOffice in container
# Custom fonts? Edit Dockerfile
# Upgrade? Re-pull image
```

#### Folio (Current State)#

```bash
# Install Rust (if not installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone repo
git clone https://github.com/yourusername/folio.git
cd folio

# Build (long!)
cargo build --release -p server

# Install Chrome + LibreOffice
apt-get install chromium libreoffice  # Linux
brew install chromium libreoffice       # macOS
```

### Desired State (One Command)#

```bash
# The dream
curl -sSL https://folio.dev/install.sh | bash

# Or via package managers
brew install folio
npm install -g folio
pip install folio
```

## Scope#

**In:**

- **Install scripts** for Linux (apt/yum), macOS (brew), Windows (chocolatey)
- **Pre-built binaries** for all platforms (GitHub Releases)
- **Package manager support**: Homebrew, npm, pip, cargo
- **Docker images** (slim + full variants)
- **Auto-detection** of Chrome/LibreOffice paths
- **Font installation** helper in install script
- **Post-install test**: verify conversion works

**Out:**

- Auto-update mechanism (security risk)
- In-app installation of Chrome/LibreOffice (complex)
- Cloud deployment (separate: spec-40)

## Implementation#

### 1. Install Script (Unix)#

```bash
#!/bin/bash
# install.sh - One-command Folio installer
# Usage: curl -sSL https://folio.dev/install.sh | bash

set -e

FOLIO_VERSION="latest"
INSTALL_DIR="/usr/local/bin"
REPO="yourusername/folio"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Detect OS
detect_os() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]]; then
        echo "windows"
    else
        error "Unsupported OS: $OSTYPE"
    fi
}

OS=$(detect_os)
info "Detected OS: $OS"

# Check for required tools
check_dependencies() {
    if ! command -v curl &> /dev/null; then
        error "curl is required but not installed"
    fi

    if ! command -v tar &> /dev/null; then
        error "tar is required but not installed"
    fi
}

# Download and install binary
install_folio() {
    info "Downloading Folio $FOLIO_VERSION..."

    ARCH=$(uname -m)
    case "$ARCH" in
        x86_64)
            ARCH="amd64"
            ;;
        aarch64|arm64)
            ARCH="arm64"
            ;;
        *)
            error "Unsupported architecture: $ARCH"
            ;;
    esac

    BINARY="folio-server-${OS}-${ARCH}.tar.gz"
    DOWNLOAD_URL="https://github.com/${REPO}/releases/${FOLIO_VERSION}/download/${BINARY}"

    info "Downloading from $DOWNLOAD_URL"
    curl -sSL -o /tmp/folio.tar.gz "$DOWNLOAD_URL" || error "Download failed"

    info "Installing to $INSTALL_DIR"
    tar -xzf /tmp/folio.tar.gz -C "$INSTALL_DIR"
    chmod +x "$INSTALL_DIR/folio-server"

    rm /tmp/folio.tar.gz
}

# Check for Chrome/Chromium
check_chromium() {
    if command -v chromium-browser &> /dev/null; then
        info "Found Chromium: $(which chromium-browser)"
    elif command -v chromium &> /dev/null; then
        info "Found Chromium: $(which chromium)"
    elif command -v google-chrome &> /dev/null; then
        info "Found Chrome: $(which google-chrome)"
    else
        warn "Chromium/Chrome not found. Installing..."
        if [[ "$OS" == "linux" ]]; then
            if command -v apt-get &> /dev/null; then
                sudo apt-get update && sudo apt-get install -y chromium-browser
            elif command -v yum &> /dev/null; then
                sudo yum install -y chromium
            fi
        elif [[ "$OS" == "macos" ]]; then
            brew install chromium
        fi
    fi
}

# Check for LibreOffice
check_libreoffice() {
    if command -v soffice &> /dev/null; then
        info "Found LibreOffice: $(which soffice)"
    else
        warn "LibreOffice not found. Installing..."
        if [[ "$OS" == "linux" ]]; then
            if command -v apt-get &> /dev/null; then
                sudo apt-get update && sudo apt-get install -y libreoffice
            elif command -v yum &> /dev/null; then
                sudo yum install -y libreoffice
            fi
        elif [[ "$OS" == "macos" ]]; then
            brew install libreoffice
        fi
    fi
}

# Install common fonts
install_fonts() {
    info "Installing common fonts..."
    if [[ "$OS" == "linux" ]]; then
        if command -v apt-get &> /dev/null; then
            sudo apt-get install -y ttf-mscorefonts-installer || warn "Failed to install MS fonts"
        fi
    fi
}

# Post-install test
test_installation() {
    info "Testing installation..."

    # Start Folio in background
    folio-server --port 13000 &
    PID=$!

    sleep 3

    # Test health endpoint
    if curl -s http://localhost:13000/health | grep -q "up"; then
        info "✅ Folio is working!"
    else
        warn "Health check failed"
    fi

    # Test conversion
    echo "<h1>Test</h1>" > /tmp/test.html
    if curl -s -X POST http://localhost:13000/forms/chromium/convert/html \
        --form files=@/tmp/test.html -o /tmp/test.pdf; then
        info "✅ PDF conversion works!"
    else
        warn "PDF conversion failed"
    fi

    # Cleanup
    kill $PID 2>/dev/null || true
    rm /tmp/test.html /tmp/test.pdf 2>/dev/null || true
}

# Main
main() {
    info "Installing Folio..."

    check_dependencies
    install_folio
    check_chromium
    check_libreoffice
    install_fonts
    test_installation

    info "✅ Folio installation complete!"
    info "Start Folio: folio-server --port 3000"
    info "Convert HTML: curl -X POST http://localhost:3000/forms/chromium/convert/html --form files=@file.html"
}

main
```

### 2. Package Manager Configs#

#### Homebrew (macOS)#

```ruby
# Formula/folio.rb
class Folio < Formula
  desc "Modern, Rust-native PDF generation engine"
  homepage "https://folio.dev"
  url "https://github.com/yourusername/folio/releases/download/v0.1.0/folio-server-darwin-amd64.tar.gz"
  sha256 "..."

  depends_on "chromium"
  depends_on "libreoffice"

  def install
    bin.install "folio-server"
    (bin/"folio-server").chmod 0755
  end

  test do
    system "#{bin}/folio-server", "--version"
  end
end
```

#### npm (Node.js)#

```json
{
  "name": "folio",
  "version": "0.1.0",
  "description": "Folio PDF generation - Gotenberg-compatible API",
  "bin": {
    "folio-server": "./bin/folio-server.js"
  },
  "scripts": {
    "postinstall": "node install.js"
  },
  "dependencies": {}
}
```

#### PyPI (Python)#

```python
# setup.py
from setuptools import setup

setup(
    name="folio",
    version="0.1.0",
    description="Folio PDF generation - Gotenberg-compatible API",
    scripts=["bin/folio-server"],
    install_requires=[],
)
```

### 3. GitHub Actions (Auto-release)#

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        os: [linux, macos, windows]
        arch: [amd64, arm64]
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-rust@v1
      - name: Build
        run: cargo build --release -p server
      - name: Package
        run: |
          tar -czf folio-server-${{ matrix.os }}-${{ matrix.arch }}.tar.gz \
            -C target/release folio-server
      - name: Release
        uses: softprops/action-gh-release@v1
        with:
          files: folio-server-*.tar.gz
```

## Expected Behaviour#

### One-Command Install#

```bash
# Linux/macOS
curl -sSL https://folio.dev/install.sh | bash

# Homebrew
brew install folio

# npm
npm install -g folio

# Python
pip install folio

# Cargo
cargo install folio-server
```

### Post-Install Test#

```bash
$ curl -sSL https://folio.dev/install.sh | bash
[INFO] Detected OS: linux
[INFO] Downloading Folio latest...
[INFO] Installing to /usr/local/bin
[INFO] Found Chromium: /usr/bin/chromium-browser
[INFO] Found LibreOffice: /usr/bin/soffice
[INFO] Installing common fonts...
[INFO] Testing installation...
[INFO] ✅ Folio is working!
[INFO] ✅ PDF conversion works!
[INFO] ✅ Folio installation complete!
[INFO] Start Folio: folio-server --port 3000
```

## Test Plan#

### Unit Tests#

- `install_script_detects_linux`
- `install_script_detects_macos`
- `post_install_test_passes`

### Integration Tests#

- `one_command_install_linux`
- `one_command_install_macos`
- `homebrew_install_works`
- `npm_install_works`

## Acceptance#

- [ ] `install.sh` script for Unix-like systems
- [ ] Homebrew formula (macOS)
- [ ] npm package (Node.js)
- [ ] PyPI package (Python)
- [ ] GitHub Actions for auto-release
- [ ] Pre-built binaries for all platforms
- [ ] Auto-detection of Chrome/LibreOffice
- [ ] Post-install test suite
- [ ] `cargo clippy -p server -- -D warnings` clean

## References#

- Gotenberg Docker install: https://gotenberg.dev/docs/getting-started/installation
- Homebrew formula guide: https://docs.brew.sh/Formula-Cookbook/
- npm package creation: https://docs.npmjs.com/creating-and-publishing-unscoped-public-packages
- PyPI packaging: https://packaging.python.org/tutorials/packaging-projects/
