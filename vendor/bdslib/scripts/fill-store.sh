#!/usr/bin/env bash
# scripts/fill-store.sh
#
# Populate a running bdsnode instance with synthetic telemetry, logs, and
# docstore documents using bdscli (generator) and bdscmd (JSON-RPC client).
#
# Usage:
#   ./scripts/fill-store.sh [OPTIONS]
#
# Options:
#   --addr HOST:PORT      bdsnode address (default: http://127.0.0.1:9000)
#                         Accepts both "HOST:PORT" and "http://HOST:PORT" forms.
#   --config PATH         path to hjson config file (overrides BDS_CONFIG env var)
#   --tel-count N         telemetry records per key (default: 200)
#   --log-count N         log records per format (default: 300)
#   --doc-count N         docstore documents (default: 40); 0 skips docstore entirely
#   --duration DUR        humantime duration (e.g. 6h, 30m, 2d) overriding the
#                         per-key/per-format lookback windows for telemetry, logs
#                         and the mixed batch
#   --no-color            disable colour output
#
# Environment variables (lower precedence than flags):
#   BDSCMD_ADDR           equivalent of --addr
#   BDS_CONFIG            config file path (used by bdscli)
#   BDSCLI                bdscli binary (default: bdscli)
#   BDSCMD                bdscmd binary (default: bdscmd)

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
BDSCLI="${BDSCLI:-bdscli}"
BDSCMD="${BDSCMD:-bdscmd}"
ADDR="${BDSCMD_ADDR:-http://127.0.0.1:9000}"
CONFIG_ARGS=()
TEL_COUNT=200
LOG_COUNT=300
DOC_COUNT=40
DURATION=""
COLOR=1

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case $1 in
        --addr)      ADDR="$2";               shift 2 ;;
        --config)    CONFIG_ARGS=(--config "$2"); shift 2 ;;
        --tel-count) TEL_COUNT="$2";          shift 2 ;;
        --log-count) LOG_COUNT="$2";          shift 2 ;;
        --doc-count) DOC_COUNT="$2";          shift 2 ;;
        --duration)  DURATION="$2";           shift 2 ;;
        --no-color)  COLOR=0;                 shift   ;;
        -h|--help)
            sed -n '3,22p' "$0" | sed 's/^# \{0,1\}//'
            exit 0 ;;
        *) printf 'Unknown option: %s\n' "$1" >&2; exit 1 ;;
    esac
done

# ── Colour helpers ────────────────────────────────────────────────────────────
if [[ $COLOR -eq 1 ]] && [[ -t 1 ]]; then
    _blue='\033[1;34m'; _green='\033[1;32m'; _red='\033[1;31m'
    _cyan='\033[1;36m'; _yellow='\033[0;33m'; _reset='\033[0m'
else
    _blue=''; _green=''; _red=''; _cyan=''; _yellow=''; _reset=''
fi

info()  { printf "${_blue}[info]${_reset}  %s\n"   "$*"; }
ok()    { printf "${_green}[ ok ]${_reset}  %s\n"  "$*"; }
fail()  { printf "${_red}[fail]${_reset}  %s\n"    "$*" >&2; exit 1; }
step()  { printf "\n${_cyan}══ %s ══${_reset}\n"   "$*"; }
tally() { printf "${_yellow}        %-30s %s${_reset}\n" "$1" "$2"; }

# ── Resolve binaries (fall back to cargo target dir) ─────────────────────────
resolve_bin() {
    local name="$1"
    if command -v "$name" &>/dev/null; then
        echo "$name"; return
    fi
    local cargo_bin
    cargo_bin="$(dirname "$0")/../target/debug/$name"
    if [[ -x "$cargo_bin" ]]; then
        echo "$cargo_bin"; return
    fi
    return 1
}

if ! BDSCLI=$(resolve_bin "$BDSCLI"); then
    fail "bdscli not found in PATH or target/debug/ — run 'cargo build --bin bdscli' first"
fi
if ! BDSCMD=$(resolve_bin "$BDSCMD"); then
    fail "bdscmd not found in PATH or target/debug/ — run 'cargo build --bin bdscmd' first"
fi

BDSCMD_OPTS=(-a "$ADDR")

# ── Preflight ─────────────────────────────────────────────────────────────────
step "Preflight"

if ! command -v jq &>/dev/null; then
    fail "jq not found — install it (brew install jq / apt install jq)"
fi
ok "jq found"
ok "bdscli: $BDSCLI"
ok "bdscmd: $BDSCMD"

info "Connecting to bdsnode at $ADDR …"
if ! "$BDSCMD" "${BDSCMD_OPTS[@]}" status &>/dev/null; then
    fail "bdsnode is not reachable at $ADDR — start it first with: bdsnode --config <path>"
fi
NODE_ID=$("$BDSCMD" "${BDSCMD_OPTS[@]}" status 2>/dev/null | jq -r '.node_id // "unknown"')
ok "bdsnode up  (node_id=$NODE_ID)"

# ── Docstore ──────────────────────────────────────────────────────────────────
doc_ok=0; doc_fail=0
if [[ "$DOC_COUNT" -eq 0 ]]; then
    step "Docstore — skipped (--doc-count=0)"
else
    step "Docstore — $DOC_COUNT documents"

    info "Generating and submitting …"

    while IFS= read -r line; do
        meta=$(printf '%s' "$line" | jq -c '.metadata' 2>/dev/null) || continue
        content=$(printf '%s' "$line" | jq -r '.content' 2>/dev/null)   || continue
        if "$BDSCMD" "${BDSCMD_OPTS[@]}" doc-add \
                --metadata "$meta" \
                --content  "$content" &>/dev/null; then
            (( doc_ok++  )) || true
        else
            (( doc_fail++ )) || true
        fi
    done < <("$BDSCLI" "${CONFIG_ARGS[@]}" generate docs --count "$DOC_COUNT" 2>/dev/null)

    tally "documents added:"  "$doc_ok"
    [[ $doc_fail -gt 0 ]] && tally "documents failed:" "$doc_fail"

    info "Rebuilding document vector index …"
    reindexed=$("$BDSCMD" "${BDSCMD_OPTS[@]}" doc-reindex 2>/dev/null | jq -r '.indexed // 0') || reindexed=0
    tally "documents re-indexed:" "$reindexed"
    ok "Docstore done"
fi

# ── Telemetry ─────────────────────────────────────────────────────────────────
# NOTE: telemetry records are ingested ASYNCHRONOUSLY.  `bdscmd add-batch` pipes
# each batch through v2/add.batch, which enqueues records onto an unbounded
# crossbeam channel and returns {"queued": N} immediately — the background ingest
# thread stores them to DuckDB/FTS/vector in parallel.  The "queued" tallies below
# reflect records accepted for processing, not records already on disk.  A query
# run immediately after this script may report fewer records than expected; wait a
# few seconds for the ingest thread to flush before checking counts.
step "Telemetry — ${TEL_COUNT} records × 10 keys"

# key → lookback window (spread data across different time horizons)
declare -A TEL_KEYS=(
    [cpu.usage]="6h"
    [mem.used_pct]="6h"
    [disk.io_wait]="3h"
    [disk.read_bytes]="3h"
    [net.rx_bytes]="12h"
    [net.tx_bytes]="12h"
    [http.latency_ms]="4h"
    [db.connections]="4h"
    [cache.hit_ratio]="2h"
    [queue.depth]="2h"
)

tel_total=0
for key in "${!TEL_KEYS[@]}"; do
    dur="${DURATION:-${TEL_KEYS[$key]}}"
    queued=$(
        "$BDSCLI" "${CONFIG_ARGS[@]}" generate telemetry \
            --key      "$key" \
            --duration "$dur" \
            --count    "$TEL_COUNT" \
            2>/dev/null \
        | "$BDSCMD" "${BDSCMD_OPTS[@]}" add-batch 2>/dev/null \
        | jq -r '.queued // 0'
    ) || queued=0
    (( tel_total += queued )) || true
    tally "$key ($dur):" "$queued queued"
done

ok "Telemetry done  (total queued: $tel_total)"

# ── Logs ──────────────────────────────────────────────────────────────────────
# Like telemetry, log records go through v2/add.batch and are queued asynchronously.
step "Logs — ${LOG_COUNT} records × 4 formats"

declare -A LOG_FORMATS=(
    [syslog]="24h"
    [http]="12h"
    [http-nginx]="12h"
    [traceback]="6h"
)

log_total=0
for fmt in "${!LOG_FORMATS[@]}"; do
    dur="${DURATION:-${LOG_FORMATS[$fmt]}}"
    queued=$(
        "$BDSCLI" "${CONFIG_ARGS[@]}" generate log \
            --format   "$fmt" \
            --duration "$dur" \
            --count    "$LOG_COUNT" \
            2>/dev/null \
        | "$BDSCMD" "${BDSCMD_OPTS[@]}" add-batch 2>/dev/null \
        | jq -r '.queued // 0'
    ) || queued=0
    (( log_total += queued )) || true
    tally "$fmt ($dur):" "$queued queued"
done

ok "Logs done  (total queued: $log_total)"

# ── Mixed (telemetry + logs interleaved) ──────────────────────────────────────
step "Mixed batch"

MIXED_COUNT=$(( TEL_COUNT * 2 ))
MIXED_DUR="${DURATION:-8h}"
mixed_queued=$(
    "$BDSCLI" "${CONFIG_ARGS[@]}" generate mixed \
        --duration "$MIXED_DUR" \
        --count    "$MIXED_COUNT" \
        --ratio    0.5 \
        2>/dev/null \
    | "$BDSCMD" "${BDSCMD_OPTS[@]}" add-batch 2>/dev/null \
    | jq -r '.queued // 0'
) || mixed_queued=0
tally "mixed ($MIXED_DUR, ratio=0.5):" "$mixed_queued queued"
ok "Mixed done"

# ── Summary ───────────────────────────────────────────────────────────────────
# The count below is a point-in-time snapshot queried immediately after all batches
# are queued.  Because telemetry, log, and mixed records are ingested asynchronously
# (v2/add.batch), the ingest thread may still be writing when this runs.  The
# reported total may be lower than the sum of queued records; re-run
# `bdscmd count` a few seconds later for the final tally.
# Docstore documents (v2/doc.add) are committed synchronously and are always
# reflected accurately in the snapshot.
step "Store summary"

COUNT_JSON=$("$BDSCMD" "${BDSCMD_OPTS[@]}" count 2>/dev/null) || COUNT_JSON='{}'
TIMELINE_JSON=$("$BDSCMD" "${BDSCMD_OPTS[@]}" timeline 2>/dev/null) || TIMELINE_JSON='{}'

total_rec=$(printf '%s' "$COUNT_JSON"   | jq -r '.count    // "?"')
min_ts=$(   printf '%s' "$TIMELINE_JSON" | jq -r '.min_ts  // "?"')
max_ts=$(   printf '%s' "$TIMELINE_JSON" | jq -r '.max_ts  // "?"')

min_human=$(  [[ "$min_ts" != "?" ]] && date -r "$min_ts" '+%Y-%m-%d %H:%M:%S %Z' 2>/dev/null || echo "$min_ts")
max_human=$(  [[ "$max_ts" != "?" ]] && date -r "$max_ts" '+%Y-%m-%d %H:%M:%S %Z' 2>/dev/null || echo "$max_ts")

tally "total observability records:" "$total_rec"
tally "docstore documents added:"    "$doc_ok"
tally "oldest event:"                "$min_human"
tally "newest event:"                "$max_human"

printf "\n${_green}Done.${_reset}\n"
