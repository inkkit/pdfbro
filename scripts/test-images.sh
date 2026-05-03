#!/usr/bin/env bash
# Test a pdfbro Docker image with real API calls.
#
# Usage:
#   scripts/test-images.sh <image>           # test one image
#   scripts/test-images.sh                   # test all 9 variants (uses DOCKER_REGISTRY or ghcr.io/inkkit/pdfbro)
#
# Examples:
#   scripts/test-images.sh ghcr.io/inkkit/pdfbro:latest
#   scripts/test-images.sh ghcr.io/inkkit/pdfbro:latest-chromium
#   DOCKER_REGISTRY=myrepo/pdfbro scripts/test-images.sh
set -uo pipefail

REPO_ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
FIXTURES="${REPO_ROOT}/bench/fixtures"
PORT=13000
BASE="http://localhost:${PORT}"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[1;33m'; DIM='\033[2m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}✓${NC} $1"; }
fail() { echo -e "  ${RED}✗${NC} $1"; FAILURES=$((FAILURES+1)); }
skip() { echo -e "  ${YELLOW}–${NC} $1"; }

# ── test one image ────────────────────────────────────────────────────
test_image() {
  local IMAGE=$1
  local CONTAINER="pdfbro-test-$$"
  FAILURES=0

  echo ""
  echo -e "${DIM}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  echo -e "  Testing: ${IMAGE}"
  echo -e "${DIM}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

  cleanup() {
    docker rm -f "$CONTAINER" &>/dev/null || true
    sleep 1
  }
  trap cleanup EXIT

  # kill anything already on the test port
  docker ps --format "{{.ID}} {{.Ports}}" | grep ":${PORT}->" | awk '{print $1}' | xargs -r docker rm -f &>/dev/null || true
  sleep 1

  echo -e "${DIM}▶ pulling $IMAGE${NC}"
  docker pull "$IMAGE" -q

  echo -e "${DIM}▶ starting container${NC}"
  docker run -d --name "$CONTAINER" \
    -p ${PORT}:3000 \
    -e PDFBRO_NO_SANDBOX=true \
    -e CHROMIUM_LAZY_START=false \
    -e LIBREOFFICE_LAZY_START=false \
    -e RUST_LOG=error \
    "$IMAGE" > /dev/null

  # wait for HTTP server (up to 60s)
  echo -e "${DIM}▶ waiting for server...${NC}"
  for i in $(seq 1 60); do
    code=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/health" 2>/dev/null || echo "000")
    [[ "$code" == "200" ]] && break
    sleep 1
    [[ $i -eq 60 ]] && { echo -e "${RED}  timed out waiting for server${NC}"; cleanup; return 1; }
  done

  # helpers
  get_code() { curl -s -o /dev/null -w "%{http_code}" "$BASE$1"; }
  engine_status() {
    curl -s "$BASE/health" | python3 -c "
import sys,json
d=json.load(sys.stdin).get('details',{})
print(d.get('$1',{}).get('status','—'))
" 2>/dev/null || echo "—"
  }
  post_mp() {
    local endpoint=$1; shift
    rm -f /tmp/pdfbro-resp
    curl -s -o /tmp/pdfbro-resp -w "%{http_code}:%{content_type}" -X POST "$BASE$endpoint" "$@" 2>/dev/null || echo "000:"
  }
  is_pdf()   { file /tmp/pdfbro-resp 2>/dev/null | grep -qi "pdf"; }
  is_image() { file /tmp/pdfbro-resp 2>/dev/null | grep -qiE "image|PNG|JPEG|WebP"; }

  # ── always-present routes ─────────────────────────────────────────
  for route in /health /version /prometheus/metrics /_/; do
    c=$(get_code $route)
    [[ "$c" == "200" ]] && pass "GET $route → 200" || fail "GET $route → $c (expected 200)"
  done

  # ── pdfengines — always compiled ──────────────────────────────────
  result=$(post_mp /forms/pdfengines/merge \
    -F "files=@${FIXTURES}/page_1.pdf;type=application/pdf" \
    -F "files=@${FIXTURES}/page_2.pdf;type=application/pdf")
  code="${result%%:*}"
  if [[ "$code" == "200" ]] && is_pdf; then
    pass "POST /forms/pdfengines/merge → 200 PDF"
  else
    fail "POST /forms/pdfengines/merge → $code (expected 200 PDF)"
  fi

  # ── chromium ──────────────────────────────────────────────────────
  probe=$(get_code /forms/chromium/convert/html)
  if [[ "$probe" == "404" ]]; then
    skip "chromium not compiled → /forms/chromium/* (correct 404)"
  else
    echo -e "${DIM}    waiting for Chromium engine...${NC}"
    for i in $(seq 1 30); do
      [[ "$(engine_status chromium)" == "up" ]] && break
      sleep 1
    done

    result=$(post_mp /forms/chromium/convert/html \
      -F "files=@${FIXTURES}/html_small.html;filename=index.html;type=text/html" \
      --max-time 60)
    code="${result%%:*}"
    if [[ "$code" == "200" ]] && is_pdf; then
      pass "POST /forms/chromium/convert/html → 200 PDF"
    else
      fail "POST /forms/chromium/convert/html → $code (expected 200 PDF)"
    fi

    result=$(post_mp /forms/chromium/convert/url \
      -F "url=https://example.com" --max-time 60)
    code="${result%%:*}"
    if [[ "$code" == "200" ]] && is_pdf; then
      pass "POST /forms/chromium/convert/url → 200 PDF"
    else
      fail "POST /forms/chromium/convert/url → $code (expected 200 PDF)"
    fi

    result=$(post_mp /forms/chromium/screenshot/url \
      -F "url=https://example.com" --max-time 60)
    code="${result%%:*}"
    if [[ "$code" == "200" ]] && is_image; then
      pass "POST /forms/chromium/screenshot/url → 200 image"
    else
      fail "POST /forms/chromium/screenshot/url → $code (expected 200 image)"
    fi
  fi

  # ── libreoffice ───────────────────────────────────────────────────
  probe=$(get_code /forms/libreoffice/convert)
  if [[ "$probe" == "404" ]]; then
    skip "libreoffice not compiled → /forms/libreoffice/convert (correct 404)"
  else
    echo -e "${DIM}    waiting for LibreOffice engine (LOK)...${NC}"
    for i in $(seq 1 60); do
      [[ "$(engine_status libreoffice)" == "up" ]] && break
      sleep 1
      [[ $i -eq 60 ]] && { fail "LibreOffice engine never came up"; break; }
    done

    result=$(post_mp /forms/libreoffice/convert \
      -F "files=@${FIXTURES}/sample.docx" --max-time 90)
    code="${result%%:*}"
    if [[ "$code" == "200" ]] && is_pdf; then
      pass "POST /forms/libreoffice/convert (docx) → 200 PDF"
    else
      fail "POST /forms/libreoffice/convert (docx) → $code (expected 200 PDF)"
    fi
  fi

  # ── summary ───────────────────────────────────────────────────────
  CR=$(engine_status chromium); LO=$(engine_status libreoffice)
  echo -e "${DIM}    engines → chromium: $CR  libreoffice: $LO${NC}"

  cleanup
  trap - EXIT

  echo ""
  if [[ $FAILURES -eq 0 ]]; then
    echo -e "  ${GREEN}✓ All checks passed${NC}"
    return 0
  else
    echo -e "  ${RED}✗ $FAILURES check(s) failed${NC}"
    return 1
  fi
}

# ── entry point ───────────────────────────────────────────────────────
if [[ $# -ge 1 ]]; then
  test_image "$1"
else
  REGISTRY="${DOCKER_REGISTRY:-ghcr.io/inkkit/pdfbro}"
  VARIANTS=(
    "${REGISTRY}:latest"
    "${REGISTRY}:latest-chromium"
    "${REGISTRY}:latest-libreoffice"
    "${REGISTRY}:latest-cloudrun"
    "${REGISTRY}:latest-chromium-cloudrun"
    "${REGISTRY}:latest-libreoffice-cloudrun"
    "${REGISTRY}:latest-lambda"
    "${REGISTRY}:latest-chromium-lambda"
    "${REGISTRY}:latest-libreoffice-lambda"
  )

  TOTAL_FAILURES=0
  for img in "${VARIANTS[@]}"; do
    test_image "$img" || TOTAL_FAILURES=$((TOTAL_FAILURES+1))
  done

  echo ""
  echo -e "${DIM}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  if [[ $TOTAL_FAILURES -eq 0 ]]; then
    echo -e "  ${GREEN}All ${#VARIANTS[@]} variants passed${NC}"
  else
    echo -e "  ${RED}$TOTAL_FAILURES / ${#VARIANTS[@]} variant(s) failed${NC}"
    exit 1
  fi
fi
