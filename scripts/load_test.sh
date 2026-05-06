#!/usr/bin/env bash
# load_test.sh — drives the pdfbro server to exercise all UI panels
#
# Usage:
#   ./scripts/load_test.sh [BASE_URL]
#
# Defaults to http://localhost:3000

set -euo pipefail

BASE="${1:-http://localhost:3000}"
ROUNDS="${2:-3}"   # how many full waves to run

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
log()  { echo -e "${GREEN}[load]${NC} $*"; }
warn() { echo -e "${YELLOW}[warn]${NC} $*"; }

# ── helpers ──────────────────────────────────────────────────────────────────

submit_batch() {
  local label="$1"
  curl -s -X POST "$BASE/forms/batch/submit" \
    -F "batch.json=$(cat <<'JSON'
{
  "outputMode":"zip",
  "items":[
    {"file":"https://example.com",              "type":"chromiumUrl"},
    {"file":"https://httpbin.org/html",          "type":"chromiumUrl"},
    {"file":"https://wikipedia.org",             "type":"chromiumUrl"},
    {"file":"https://news.ycombinator.com",      "type":"chromiumUrl"},
    {"file":"https://github.com/trending",       "type":"chromiumUrl"}
  ]
}
JSON
)" | jq -r '.batchId // "ERROR: \(.error)"' && log "batch submitted [$label]"
}

single_url() {
  local url="$1"
  curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE/forms/chromium/convert/url" \
    -F "url=$url" \
    -F "waitDelay=0" | read -r code
  log "single url [$url] → $code"
}

# Deliberate 4xx — validation / field errors (should NOT count as server errors)
bad_requests() {
  warn "sending deliberate 4xx (should not inflate 5xx / 429 panels)"

  # 400: missing required field
  curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE/forms/chromium/convert/url" | grep -q "400\|422" \
    && warn "  missing-url → 4xx ✓" || true

  # 422: bad batch JSON
  curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE/forms/batch/submit" \
    -F 'batch.json={"outputMode":"zip","items":[]}' | grep -q "4" \
    && warn "  empty-items → 4xx ✓" || true

  # 404: unknown route
  curl -s -o /dev/null "$BASE/does/not/exist" && warn "  unknown-route → 404 ✓" || true
}

# ── main ─────────────────────────────────────────────────────────────────────

log "target: $BASE  rounds: $ROUNDS"
log "checking server health..."
curl -sf "$BASE/health" > /dev/null || { echo "server not reachable at $BASE"; exit 1; }
log "server is up"

for round in $(seq 1 "$ROUNDS"); do
  log "═══ round $round / $ROUNDS ═══"

  # 1. Fire 5 batches in parallel (drives batch panel + engine conv + queue)
  log "launching 5 batch jobs..."
  for i in $(seq 1 5); do
    submit_batch "r${round}-b${i}" &
  done
  wait
  log "all 5 batches queued"

  # 2. Fire 10 single URL-to-PDF requests concurrently (drives concurrency + RPS)
  log "launching 10 concurrent single-URL renders..."
  for url in \
    "https://example.com" \
    "https://httpbin.org/html" \
    "https://wikipedia.org" \
    "https://github.com" \
    "https://news.ycombinator.com" \
    "https://example.com" \
    "https://httpbin.org/html" \
    "https://wikipedia.org" \
    "https://github.com" \
    "https://news.ycombinator.com"
  do
    curl -s -o /dev/null -X POST "$BASE/forms/chromium/convert/url" \
      -F "url=$url" &
  done
  wait
  log "single-URL wave complete"

  # 3. Deliberate bad requests (4xx — must not affect 5xx or 429 panels)
  bad_requests

  # 4. Health check (drives health route stats)
  curl -sf "$BASE/health" > /dev/null
  curl -sf "$BASE/version" > /dev/null 2>&1 || true

  log "round $round done — waiting 8s before next wave"
  sleep 8
done

log "load test complete"
