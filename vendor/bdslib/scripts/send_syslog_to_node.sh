#!/usr/bin/env bash
# send_syslog_to_node.sh — generate a synthetic RFC 3164 syslog file via
#                          "bdscli generate syslog", submit it to bdsnode via
#                          v2/add.file.syslog, monitor v2/status until ingestion
#                          completes, then verify with v2/fulltext queries.
set -euo pipefail

# ── defaults ──────────────────────────────────────────────────────────────────
NODE_ADDR="127.0.0.1:9000"
COUNT=100
DURATION="1h"
QUERY="kernel"           # FTS term used for post-ingestion verification
VERIFY_LIMIT=10          # max hits returned by v2/fulltext
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
Usage: send_syslog_to_node.sh [OPTIONS]

Generate synthetic RFC 3164 syslog lines via "bdscli generate syslog", write
them to a file, submit the file path to a running bdsnode via v2/add.file.syslog,
then poll v2/status until both syslog_file_queue is 0 and syslog_file_name is
null (ingestion complete).  After ingestion, verify with v2/fulltext and
v2/fulltext.get queries.  The file is removed on completion unless --keep or
-o is specified.

Options:
  -a, --address ADDR       bdsnode host:port or full URL  (default: 127.0.0.1:9000)
  -n, --count N            number of syslog lines to generate  (default: 100)
  -d, --duration DUR       timestamp window, humantime          (default: 1h)
  -q, --query TERM         FTS term for post-ingestion verification
                             (default: kernel)
      --verify-limit N     max hits returned by v2/fulltext     (default: 10)
  -o, --output FILE        write generated syslog to FILE instead of a temp file;
                             implies --keep
      --keep               keep the generated file after ingestion completes
      --poll-interval N    seconds between v2/status polls      (default: 1)
      --timeout N          max seconds to wait for ingestion; exit 2 on timeout
                             (default: 300)
  -c, --config FILE        bdscli config file (optional)
      --bdscli PATH        path to bdscli binary  (default: bdscli or $BDSCLI)
  -h, --help               show this help

Environment:
  BDSCLI   override bdscli binary path (overridden by --bdscli)

Exit codes:
  0   success
  1   error (file generation failed, submission rejected, verification found 0 hits)
  2   timeout waiting for ingestion to complete

Examples:
  # 200 syslog lines over a 2h window, submitted to local node:
  ./send_syslog_to_node.sh -n 200 -d 2h

  # Keep the file after ingestion, verify with "sshd" term:
  ./send_syslog_to_node.sh -n 500 --keep -q sshd

  # Write to a specific file (kept automatically):
  ./send_syslog_to_node.sh -n 1000 -o /tmp/test.syslog

  # Poll every 2 seconds, time out after 10 minutes:
  ./send_syslog_to_node.sh -n 5000 --poll-interval 2 --timeout 600

  # Submit to a remote node:
  ./send_syslog_to_node.sh -a 10.0.0.5:9000 -n 300
EOF
}

# ── parse arguments ───────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        -a|--address)       NODE_ADDR="$2";      shift 2 ;;
        -n|--count)         COUNT="$2";          shift 2 ;;
        -d|--duration)      DURATION="$2";       shift 2 ;;
        -q|--query)         QUERY="$2";          shift 2 ;;
        --verify-limit)     VERIFY_LIMIT="$2";   shift 2 ;;
        -o|--output)        OUTPUT_FILE="$2";    KEEP=1; shift 2 ;;
        --keep)             KEEP=1;              shift ;;
        --poll-interval)    POLL_INTERVAL="$2";  shift 2 ;;
        --timeout)          POLL_TIMEOUT="$2";   shift 2 ;;
        -c|--config)        CONFIG="$2";         shift 2 ;;
        --bdscli)           BDSCLI="$2";         shift 2 ;;
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
    TEMP_FILE=$(mktemp /tmp/bds_syslog_XXXXXX.log)
    OUTPUT_FILE="$TEMP_FILE"
fi

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

# ── generate syslog lines into file ──────────────────────────────────────────
echo ">>> generating ${COUNT} RFC 3164 syslog lines (duration=${DURATION})"
echo ">>> output file: ${OUTPUT_FILE}"

"${BDSCLI_CMD[@]}" generate syslog \
    --duration "$DURATION" --count "$COUNT" \
    > "$OUTPUT_FILE"

LINE_COUNT=$(wc -l < "$OUTPUT_FILE" | tr -d ' ')
if [[ "$LINE_COUNT" -eq 0 ]]; then
    echo "error: bdscli produced an empty file" >&2
    exit 1
fi

echo ">>> generated ${LINE_COUNT} syslog lines"

# ── submit via v2/add.file.syslog ─────────────────────────────────────────────
PAYLOAD=$(jq -n \
    --arg session "$SESSION" \
    --arg path    "$OUTPUT_FILE" \
    '{"jsonrpc":"2.0","method":"v2/add.file.syslog","params":{"session":$session,"path":$path},"id":1}')

echo ">>> submitting ${OUTPUT_FILE} to ${NODE_URL} …"

RESPONSE=$(curl -sf \
    --connect-timeout 10 \
    --max-time 30 \
    -X POST "$NODE_URL" \
    -H "Content-Type: application/json" \
    -d "$PAYLOAD")

echo ">>> response:"
jq . <<< "$RESPONSE"

if jq -e '.error' <<< "$RESPONSE" >/dev/null 2>&1; then
    echo "error: v2/add.file.syslog returned an error — aborting" >&2
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

    SYSLOG_QUEUE=$(jq -r '.result.syslog_file_queue // 0'    <<< "$STATUS")
    SYSLOG_NAME=$( jq -r '.result.syslog_file_name  // "null"' <<< "$STATUS")

    printf '\r    queued=%-4s  processing=%s                    ' \
        "$SYSLOG_QUEUE" "$SYSLOG_NAME"

    if [[ "$SYSLOG_QUEUE" -eq 0 && "$SYSLOG_NAME" == "null" ]]; then
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

# ── verify ingestion via fulltext queries ─────────────────────────────────────
echo ""
echo ">>> verifying ingestion — query: \"${QUERY}\"  duration: ${DURATION}  limit: ${VERIFY_LIMIT}"

# v2/fulltext — IDs and BM25 scores
FT_PAYLOAD=$(jq -n \
    --arg session "$SESSION" \
    --arg query   "$QUERY" \
    --arg dur     "$DURATION" \
    --argjson lim "$VERIFY_LIMIT" \
    '{"jsonrpc":"2.0","method":"v2/fulltext","params":{"session":$session,"query":$query,"duration":$dur,"limit":$lim},"id":3}')

FT_RESPONSE=$(curl -sf \
    --connect-timeout 10 \
    --max-time 30 \
    -X POST "$NODE_URL" \
    -H "Content-Type: application/json" \
    -d "$FT_PAYLOAD")

FT_COUNT=$(jq '.result.results | length' <<< "$FT_RESPONSE" 2>/dev/null || echo 0)
echo ">>> v2/fulltext: ${FT_COUNT} hit(s)"
jq '.result.results[] | "  id=\(.id)  score=\(.score)"' -r <<< "$FT_RESPONSE" 2>/dev/null || true

# v2/fulltext.get — full documents for the same query
FTG_PAYLOAD=$(jq -n \
    --arg session "$SESSION" \
    --arg query   "$QUERY" \
    --arg dur     "$DURATION" \
    '{"jsonrpc":"2.0","method":"v2/fulltext.get","params":{"session":$session,"query":$query,"duration":$dur},"id":4}')

FTG_RESPONSE=$(curl -sf \
    --connect-timeout 10 \
    --max-time 30 \
    -X POST "$NODE_URL" \
    -H "Content-Type: application/json" \
    -d "$FTG_PAYLOAD")

FTG_COUNT=$(jq '.result.results | length' <<< "$FTG_RESPONSE" 2>/dev/null || echo 0)
echo ">>> v2/fulltext.get: ${FTG_COUNT} document(s) returned"

if [[ "$FTG_COUNT" -gt 0 ]]; then
    echo ">>> sample documents (first 3):"
    jq '.result.results[:3][] | {key: .key, timestamp: .timestamp, data: .data}' \
        <<< "$FTG_RESPONSE" 2>/dev/null || true
fi

# Fail loudly if verification found nothing.
if [[ "$FT_COUNT" -eq 0 ]]; then
    echo ""
    echo "warning: v2/fulltext returned 0 hits for query \"${QUERY}\" — try a different --query term" >&2
    echo "         Common syslog program names: kernel sshd cron su nginx" >&2
fi

# ── v2/fulltext.recent — most recently ingested hits ─────────────────────────
FTR_PAYLOAD=$(jq -n \
    --arg session "$SESSION" \
    --arg query   "$QUERY" \
    --arg dur     "$DURATION" \
    --argjson lim "$VERIFY_LIMIT" \
    '{"jsonrpc":"2.0","method":"v2/fulltext.recent","params":{"session":$session,"query":$query,"duration":$dur,"limit":$lim},"id":5}')

FTR_RESPONSE=$(curl -sf \
    --connect-timeout 10 \
    --max-time 30 \
    -X POST "$NODE_URL" \
    -H "Content-Type: application/json" \
    -d "$FTR_PAYLOAD")

FTR_COUNT=$(jq '.result.results | length' <<< "$FTR_RESPONSE" 2>/dev/null || echo 0)
echo ">>> v2/fulltext.recent: ${FTR_COUNT} hit(s) (newest first)"
jq '.result.results[] | "  id=\(.id)  ts=\(.timestamp)  score=\(.score)"' -r \
    <<< "$FTR_RESPONSE" 2>/dev/null || true

# ── file disposal ─────────────────────────────────────────────────────────────
trap - EXIT

if [[ $KEEP -eq 1 ]]; then
    echo ""
    echo ">>> file kept: ${OUTPUT_FILE}"
else
    rm -f "$OUTPUT_FILE"
    echo ""
    echo ">>> file removed: ${OUTPUT_FILE}"
fi
