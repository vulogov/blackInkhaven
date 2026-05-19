#!/usr/bin/env bash
# send_file_to_node.sh — generate synthetic docs into a file, submit to
#                         bdsnode via v2/add.file, then monitor v2/status
#                         until ingestion completes before exiting.
set -euo pipefail

# ── defaults ──────────────────────────────────────────────────────────────────
NODE_ADDR="127.0.0.1:9000"
COUNT=100
DURATION="1h"
RATIO=0.5
DUPLICATE=0.0
OUTPUT_FILE=""
CONFIG=""
BDSCLI="${BDSCLI:-bdscli}"
KEEP=0
POLL_INTERVAL=1
POLL_TIMEOUT=300
SESSION="a1b2c3d4-e5f6-7a8b-9c0d-e1f2a3b4c5d6"

# ── usage ─────────────────────────────────────────────────────────────────────
usage() {
    cat <<'EOF'
Usage: send_file_to_node.sh [OPTIONS]

Generate synthetic mixed documents (telemetry + log entries) via
"bdscli generate mixed", write them as newline-delimited JSON to a file,
submit the file path to a running bdsnode via v2/add.file, then poll
v2/status until both json_file_queue is 0 and json_file_name is null
(ingestion complete).  The file is removed on completion unless --keep
or -o is specified.

Options:
  -a, --address ADDR       bdsnode host:port or full URL  (default: 127.0.0.1:9000)
  -n, --count N            number of documents to generate (default: 100)
  -d, --duration DUR       timestamp window, humantime    (default: 1h)
  -r, --ratio FLOAT        telemetry fraction (0.0=all logs, 1.0=all telemetry)
                             (default: 0.5)
      --duplicate FLOAT    fraction re-emitted as duplicates  (default: 0.0)
  -o, --output FILE        write generated docs to FILE instead of a temp file;
                             implies --keep
      --keep               keep the generated file after ingestion completes
      --poll-interval N    seconds between v2/status polls  (default: 1)
      --timeout N          max seconds to wait for ingestion; exit 2 on timeout
                             (default: 300)
  -c, --config FILE        bdscli config file (optional)
      --bdscli PATH        path to bdscli binary  (default: bdscli or $BDSCLI)
  -h, --help               show this help

Environment:
  BDSCLI   override bdscli binary path (overridden by --bdscli)

Examples:
  # 200 mixed docs over a 2h window, submitted to local node:
  ./send_file_to_node.sh -n 200 -d 2h

  # 500 docs, keep the generated file, submit to a remote node:
  ./send_file_to_node.sh -a 10.0.0.5:9000 -n 500 --keep

  # Write to a specific file (kept automatically):
  ./send_file_to_node.sh -n 1000 -o /tmp/telemetry_batch.jsonl

  # 20% duplicates, all-telemetry mix:
  ./send_file_to_node.sh --duplicate 0.2 -r 1.0 -n 300

  # Poll every 2 seconds, time out after 10 minutes:
  ./send_file_to_node.sh -n 5000 --poll-interval 2 --timeout 600
EOF
}

# ── parse arguments ───────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        -a|--address)       NODE_ADDR="$2";       shift 2 ;;
        -n|--count)         COUNT="$2";           shift 2 ;;
        -d|--duration)      DURATION="$2";        shift 2 ;;
        -r|--ratio)         RATIO="$2";           shift 2 ;;
        --duplicate)        DUPLICATE="$2";       shift 2 ;;
        -o|--output)        OUTPUT_FILE="$2";     KEEP=1; shift 2 ;;
        --keep)             KEEP=1;               shift ;;
        --poll-interval)    POLL_INTERVAL="$2";   shift 2 ;;
        --timeout)          POLL_TIMEOUT="$2";    shift 2 ;;
        -c|--config)        CONFIG="$2";          shift 2 ;;
        --bdscli)           BDSCLI="$2";          shift 2 ;;
        -h|--help)          usage; exit 0 ;;
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

# ── resolve session UUID ──────────────────────────────────────────────────────
if command -v uuidgen &>/dev/null; then
    SESSION=$(uuidgen | tr '[:upper:]' '[:lower:]')
fi

# ── determine output file ─────────────────────────────────────────────────────
TEMP_FILE=""
if [[ -z "$OUTPUT_FILE" ]]; then
    TEMP_FILE=$(mktemp /tmp/bds_ingest_XXXXXX.jsonl)
    OUTPUT_FILE="$TEMP_FILE"
fi

# Clean up the temp file if we exit before completing ingestion monitoring.
_early_exit_cleanup() {
    if [[ -n "$TEMP_FILE" && -f "$TEMP_FILE" ]]; then
        rm -f "$TEMP_FILE"
    fi
}
trap '_early_exit_cleanup' EXIT

# Resolve to an absolute path so the server can always find the file.
OUTPUT_FILE=$(cd "$(dirname "$OUTPUT_FILE")" && pwd)/$(basename "$OUTPUT_FILE")

# ── build bdscli base command ─────────────────────────────────────────────────
BDSCLI_CMD=("$BDSCLI")
[[ -n "$CONFIG" ]] && BDSCLI_CMD+=(--config "$CONFIG")

# ── generate documents into file ─────────────────────────────────────────────
echo ">>> generating ${COUNT} mixed documents (duration=${DURATION}, ratio=${RATIO}, duplicate=${DUPLICATE})"
echo ">>> output file: ${OUTPUT_FILE}"

"${BDSCLI_CMD[@]}" generate --duplicate "$DUPLICATE" mixed \
    --duration "$DURATION" --count "$COUNT" --ratio "$RATIO" \
    > "$OUTPUT_FILE"

LINE_COUNT=$(wc -l < "$OUTPUT_FILE" | tr -d ' ')
if [[ "$LINE_COUNT" -eq 0 ]]; then
    echo "error: bdscli produced an empty file" >&2
    exit 1
fi

echo ">>> generated ${LINE_COUNT} records"

# ── submit via v2/add.file ────────────────────────────────────────────────────
PAYLOAD=$(jq -n \
    --arg session "$SESSION" \
    --arg path    "$OUTPUT_FILE" \
    '{"jsonrpc":"2.0","method":"v2/add.file","params":{"session":$session,"path":$path},"id":1}')

echo ">>> submitting ${OUTPUT_FILE} to ${NODE_URL} …"

RESPONSE=$(curl -sf \
    --connect-timeout 10 \
    --max-time 30 \
    -X POST "$NODE_URL" \
    -H "Content-Type: application/json" \
    -d "$PAYLOAD")

echo ">>> response:"
jq . <<< "$RESPONSE"

# Check for a JSON-RPC error in the submission response.
if jq -e '.error' <<< "$RESPONSE" >/dev/null 2>&1; then
    echo "error: v2/add.file returned an error — aborting" >&2
    exit 1
fi

# ── monitor ingestion via v2/status ──────────────────────────────────────────
STATUS_PAYLOAD='{"jsonrpc":"2.0","method":"v2/status","params":{},"id":2}'

echo ">>> monitoring ingestion (polling every ${POLL_INTERVAL}s, timeout ${POLL_TIMEOUT}s)…"

ELAPSED=0
while true; do
    STATUS=$(curl -sf \
        --connect-timeout 5 \
        --max-time 10 \
        -X POST "$NODE_URL" \
        -H "Content-Type: application/json" \
        -d "$STATUS_PAYLOAD" 2>/dev/null) || {
        echo "warning: v2/status request failed — retrying…" >&2
        sleep "$POLL_INTERVAL"
        ELAPSED=$(( ELAPSED + POLL_INTERVAL ))
        continue
    }

    FILE_QUEUE=$(jq -r '.result.json_file_queue // 0'    <<< "$STATUS")
    FILE_NAME=$( jq -r '.result.json_file_name  // "null"' <<< "$STATUS")

    printf '\r    queued=%-4s  processing=%s                    ' \
        "$FILE_QUEUE" "$FILE_NAME"

    if [[ "$FILE_QUEUE" -eq 0 && "$FILE_NAME" == "null" ]]; then
        echo ""
        echo ">>> ingestion complete (waited ${ELAPSED}s)"
        break
    fi

    if [[ "$POLL_TIMEOUT" -gt 0 && "$ELAPSED" -ge "$POLL_TIMEOUT" ]]; then
        echo "" >&2
        echo "error: timed out after ${POLL_TIMEOUT}s waiting for ingestion to complete" >&2
        exit 2
    fi

    sleep "$POLL_INTERVAL"
    ELAPSED=$(( ELAPSED + POLL_INTERVAL ))
done

# ── file disposal ─────────────────────────────────────────────────────────────
# Disarm the early-exit trap — we handle disposal ourselves from here.
trap - EXIT

if [[ $KEEP -eq 1 ]]; then
    echo ">>> file kept: ${OUTPUT_FILE}"
else
    rm -f "$OUTPUT_FILE"
    echo ">>> file removed: ${OUTPUT_FILE}"
fi
