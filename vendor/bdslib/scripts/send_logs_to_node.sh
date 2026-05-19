#!/usr/bin/env bash
# send_logs_to_node.sh — generate synthetic docs and submit to bdsnode via v2/add.batch
set -euo pipefail

# ── defaults ──────────────────────────────────────────────────────────────────
NODE_ADDR="127.0.0.1:9000"
COUNT=100
DURATION="1h"
RATIO=0.5
FORMAT="random"
DUPLICATE=0.0
CONFIG=""
BDSCLI="${BDSCLI:-bdscli}"

# ── usage ─────────────────────────────────────────────────────────────────────
usage() {
    cat <<'EOF'
Usage: send_logs_to_node.sh [OPTIONS]

Generate synthetic documents via bdscli (mixed telemetry + log entries) and
submit them to a running bdsnode as a single v2/add.batch JSON-RPC call.
Both generators run back-to-back; total documents sent = 2 * --count.

Options:
  -a, --address ADDR       bdsnode host:port or full URL  (default: 127.0.0.1:9000)
  -n, --count N            documents per generator        (default: 100)
  -d, --duration DUR       timestamp window, humantime    (default: 1h)
  -r, --ratio FLOAT        telemetry fraction for mixed   (default: 0.5)
  -f, --format FMT         log format for the log generator:
                             random | syslog | http | http-nginx | traceback
                             (default: random)
      --duplicate FLOAT    fraction re-emitted as duplicates  (default: 0.0)
  -c, --config FILE        bdscli config file (optional)
      --bdscli PATH        path to bdscli binary  (default: bdscli or $BDSCLI)
  -h, --help               show this help

Environment:
  BDSCLI   override bdscli binary path (overridden by --bdscli)

Examples:
  # 200 docs (100 mixed + 100 log) to local node:
  ./send_logs_to_node.sh -n 100

  # 500 docs over 6h window, 20% duplicates, to a remote node:
  ./send_logs_to_node.sh -a 10.0.0.5:9000 -n 250 -d 6h --duplicate 0.2

  # Syslog-only logs, all-telemetry mixed batch:
  ./send_logs_to_node.sh -f syslog -r 1.0 -n 50
EOF
}

# ── parse arguments ───────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        -a|--address)   NODE_ADDR="$2";  shift 2 ;;
        -n|--count)     COUNT="$2";      shift 2 ;;
        -d|--duration)  DURATION="$2";   shift 2 ;;
        -r|--ratio)     RATIO="$2";      shift 2 ;;
        -f|--format)    FORMAT="$2";     shift 2 ;;
        --duplicate)    DUPLICATE="$2";  shift 2 ;;
        -c|--config)    CONFIG="$2";     shift 2 ;;
        --bdscli)       BDSCLI="$2";     shift 2 ;;
        -h|--help)      usage; exit 0 ;;
        *) printf 'Unknown option: %s\n\n' "$1" >&2; usage >&2; exit 1 ;;
    esac
done

# ── normalise node URL ────────────────────────────────────────────────────────
case "$NODE_ADDR" in
    http://*|https://*) NODE_URL="$NODE_ADDR" ;;
    *)                  NODE_URL="http://${NODE_ADDR}" ;;
esac

# ── preflight checks ──────────────────────────────────────────────────────────
MISSING=0
for tool in "$BDSCLI" curl jq; do
    if ! command -v "$tool" &>/dev/null; then
        echo "error: required tool not found on PATH: $tool" >&2
        MISSING=1
    fi
done
[[ $MISSING -eq 1 ]] && exit 1

# ── build bdscli base command (with optional config) ─────────────────────────
BDSCLI_CMD=("$BDSCLI")
[[ -n "$CONFIG" ]] && BDSCLI_CMD+=(--config "$CONFIG")

# ── generate documents ────────────────────────────────────────────────────────
echo ">>> generating ${COUNT} mixed documents  (duration=${DURATION}, ratio=${RATIO}, duplicate=${DUPLICATE})"
echo ">>> generating ${COUNT} log entries      (duration=${DURATION}, format=${FORMAT}, duplicate=${DUPLICATE})"

DOCS=$(
    {
        "${BDSCLI_CMD[@]}" generate --duplicate "$DUPLICATE" mixed \
            --duration "$DURATION" --count "$COUNT" --ratio "$RATIO"

        "${BDSCLI_CMD[@]}" generate --duplicate "$DUPLICATE" log \
            --duration "$DURATION" --count "$COUNT" --format "$FORMAT"
    } | jq -s '.'
)

TOTAL=$(jq 'length' <<< "$DOCS")
echo ">>> collected ${TOTAL} documents total"

# ── build JSON-RPC request ────────────────────────────────────────────────────
PAYLOAD=$(jq -n --argjson docs "$DOCS" \
    '{"jsonrpc":"2.0","method":"v2/add.batch","params":{"docs":$docs},"id":1}')

# ── submit ────────────────────────────────────────────────────────────────────
echo ">>> submitting to ${NODE_URL} …"

RESPONSE=$(curl -sf \
    --connect-timeout 10 \
    --max-time 60 \
    -X POST "$NODE_URL" \
    -H "Content-Type: application/json" \
    -d "$PAYLOAD")

echo ">>> response:"
jq . <<< "$RESPONSE"
