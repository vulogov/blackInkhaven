#!/usr/bin/env bash
# Verify bdscli ingest correctness and get command behaviour.
# Usage: ./verify_ingestion.sh [path/to/bds.hjson]
set -euo pipefail

CONFIG="${1:-./bds.hjson}"
BIN="./target/debug/bdscli"

pass() { printf '\033[32mPASS\033[0m  %s\n' "$*"; }
fail() { printf '\033[31mFAIL\033[0m  %s\n' "$*"; exit 1; }
step() { printf '\n\033[1m=== %s ===\033[0m\n' "$*"; }

# ── 0. build ──────────────────────────────────────────────────────────────────
step "build"
cargo build --bin bdscli 2>&1 | tail -1
pass "binary ready"

# ── 1. fresh database ─────────────────────────────────────────────────────────
step "init --new (wipe and recreate)"
$BIN -c "$CONFIG" init --new
pass "database created"

# ── 2. ingest unique telemetry docs ───────────────────────────────────────────
#
# Each generated doc picks a random metric/host/env/region.
# Most docs will be primaries, but a few may become secondaries if the
# embedding model scores them as highly similar — this is correct behaviour.
# We only assert the total ingested count, not the primary count.
TELEM_COUNT=10
step "ingest $TELEM_COUNT telemetry docs"
$BIN -c "$CONFIG" generate telemetry -n "$TELEM_COUNT" --ingest
pass "$TELEM_COUNT telemetry docs ingested"

# ── 3. ingest near-duplicate docs → exactly 1 primary + (N-1) secondaries ────
#
# Template: same key/value/unit/host/env for all docs; only a random high-
# cardinality integer index field differs.  This guarantees distinct data_text
# (so the exact-match dedup path is not taken) while keeping embedding
# similarity well above 0.85 (so docs 2..N are classified as secondaries).
# --duration 1min constrains all timestamps to the current hour shard so the
# embedding comparison always operates within a single shard scope.
SEC_TOTAL=5
SEC_PRIMARIES=1
SEC_SECONDARIES=$(( SEC_TOTAL - SEC_PRIMARIES ))
SEC_KEY="verify.secondary"
SEC_TMPL='{"timestamp":"$timestamp","key":"'"$SEC_KEY"'","data":{"value":42.0,"unit":"percent","host":"testhost","env":"prod","idx":"$int(1,10000)"}}'

step "ingest $SEC_TOTAL near-duplicate docs for key '$SEC_KEY'"
$BIN -c "$CONFIG" generate templated -n "$SEC_TOTAL" --duration 1min \
    --template "$SEC_TMPL" --ingest
pass "$SEC_TOTAL near-duplicate docs ingested"

# ── 4. get (no flags) → every stored record regardless of primary/secondary ───
EXPECTED_TOTAL=$(( TELEM_COUNT + SEC_TOTAL ))
step "get (no flags) — expect $EXPECTED_TOTAL total records"
ALL_OUT=$($BIN -c "$CONFIG" get 2>/tmp/bds_all.err)
cat /tmp/bds_all.err
ACTUAL_TOTAL=$(echo "$ALL_OUT" | grep -c '"key"' || true)
if [ "$ACTUAL_TOTAL" -ne "$EXPECTED_TOTAL" ]; then
    fail "total records: expected $EXPECTED_TOTAL, got $ACTUAL_TOTAL"
fi
pass "total record count correct ($ACTUAL_TOTAL)"

# ── 5. get --primary → find the verify.secondary primary ─────────────────────
step "get --primary — locate primary for key '$SEC_KEY'"
PRIMARY_OUT=$($BIN -c "$CONFIG" get --primary 2>/tmp/bds_primary.err)
cat /tmp/bds_primary.err

# Exactly one primary must carry the controlled key
SEC_PRIMARY_LINES=$(echo "$PRIMARY_OUT" | grep "\"$SEC_KEY\"" || true)
SEC_PRIMARY_COUNT=$(echo "$SEC_PRIMARY_LINES" | grep -c '"key"' || true)
if [ "$SEC_PRIMARY_COUNT" -ne "$SEC_PRIMARIES" ]; then
    fail "primaries with key '$SEC_KEY': expected $SEC_PRIMARIES, got $SEC_PRIMARY_COUNT"
fi
pass "$SEC_PRIMARIES primary for key '$SEC_KEY' confirmed"

# Extract its UUID
SEC_PRIMARY_ID=$(echo "$SEC_PRIMARY_LINES" | head -1 \
    | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
if [ -z "$SEC_PRIMARY_ID" ]; then
    fail "could not extract id from primary line: $SEC_PRIMARY_LINES"
fi
pass "primary id: $SEC_PRIMARY_ID"

# ── 6. get --secondary --primary-id → N-1 secondaries ────────────────────────
step "get --secondary --primary-id $SEC_PRIMARY_ID (expect $SEC_SECONDARIES)"
SEC_OUT=$($BIN -c "$CONFIG" get --secondary --primary-id "$SEC_PRIMARY_ID" \
    2>/tmp/bds_sec.err)
cat /tmp/bds_sec.err
ACTUAL_SECONDARIES=$(echo "$SEC_OUT" | grep -c '"key"' || true)
if [ "$ACTUAL_SECONDARIES" -ne "$SEC_SECONDARIES" ]; then
    fail "secondaries: expected $SEC_SECONDARIES, got $ACTUAL_SECONDARIES"
fi
pass "secondary count correct ($ACTUAL_SECONDARIES)"

WRONG_KEY=$(echo "$SEC_OUT" | grep -v "\"$SEC_KEY\"" | grep -c '"key"' || true)
if [ "$WRONG_KEY" -ne 0 ]; then
    fail "$WRONG_KEY secondary docs have unexpected key (want '$SEC_KEY')"
fi
pass "all secondaries carry key '$SEC_KEY'"

# ── 7. get --duration 1h → same total (all docs just ingested) ────────────────
step "get --duration 1h — expect same $EXPECTED_TOTAL records"
WIN_ALL=$($BIN -c "$CONFIG" get --duration 1h 2>/tmp/bds_win.err)
cat /tmp/bds_win.err
ACTUAL_WIN=$(echo "$WIN_ALL" | grep -c '"key"' || true)
if [ "$ACTUAL_WIN" -ne "$EXPECTED_TOTAL" ]; then
    fail "windowed all-records: expected $EXPECTED_TOTAL, got $ACTUAL_WIN"
fi
pass "windowed total correct ($ACTUAL_WIN)"

# ── 8. get --primary --duration 1h → same primary count ─────────────────────
step "get --primary --duration 1h — primary count must match full scan"
ALL_PRIMARY_COUNT=$(echo "$PRIMARY_OUT" | grep -c '"key"' || true)
WIN_PRIMARY=$($BIN -c "$CONFIG" get --primary --duration 1h 2>/tmp/bds_winp.err)
cat /tmp/bds_winp.err
WIN_PRIMARY_COUNT=$(echo "$WIN_PRIMARY" | grep -c '"key"' || true)
if [ "$WIN_PRIMARY_COUNT" -ne "$ALL_PRIMARY_COUNT" ]; then
    fail "windowed primaries ($WIN_PRIMARY_COUNT) differ from full scan ($ALL_PRIMARY_COUNT)"
fi
pass "windowed primary count matches full scan ($WIN_PRIMARY_COUNT)"

# ── 9. exact-match dedup and duplication-timestamps ──────────────────────────
#
# Ingest 3 docs with identical key+data: the first becomes the primary, and
# submissions 2 & 3 are recorded in dedup_tracking (not as secondary records).
DEDUP3_KEY="verify.dedup3"
DEDUP3_TMPL='{"timestamp":"$timestamp","key":"'"$DEDUP3_KEY"'","data":{"value":7,"host":"alpha"}}'

step "ingest 3 exact-match duplicate docs for key '$DEDUP3_KEY'"
$BIN -c "$CONFIG" generate templated -n 3 --duration 1min \
    --template "$DEDUP3_TMPL" --ingest
pass "3 exact-match docs ingested"

# get --duplication-timestamps (no --primary-id) should list ≥1 entry
step "get --duplication-timestamps (all primaries with duplicates)"
DT_ALL=$($BIN -c "$CONFIG" get --duplication-timestamps 2>/tmp/bds_dt.err)
cat /tmp/bds_dt.err
DT_COUNT=$(echo "$DT_ALL" | grep -c '"primary_id"' || true)
if [ "$DT_COUNT" -lt 1 ]; then
    fail "expected ≥1 dedup entry, got $DT_COUNT"
fi
pass "dedup entry count ≥ 1 ($DT_COUNT)"

# The dedup3 key must appear in the output
if ! echo "$DT_ALL" | grep -q "\"$DEDUP3_KEY\""; then
    fail "key '$DEDUP3_KEY' missing from --duplication-timestamps output"
fi
pass "key '$DEDUP3_KEY' present in dedup output"

# Extract the primary_id for dedup3 and verify per-primary lookup
DT3_LINE=$(echo "$DT_ALL" | grep "\"$DEDUP3_KEY\"" | head -1)
DT3_PID=$(echo "$DT3_LINE" | grep -o '"primary_id":"[^"]*"' | cut -d'"' -f4)
if [ -z "$DT3_PID" ]; then
    fail "could not extract primary_id from: $DT3_LINE"
fi
pass "dedup3 primary_id: $DT3_PID"

step "get --duplication-timestamps --primary-id $DT3_PID (expect 2 timestamps)"
DT_SINGLE=$($BIN -c "$CONFIG" get --duplication-timestamps --primary-id "$DT3_PID" 2>/tmp/bds_dts.err)
cat /tmp/bds_dts.err
# The JSON array should contain exactly 2 timestamps (submissions 2 and 3)
TS_COUNT=$(echo "$DT_SINGLE" | python3 -c \
    "import json,sys; d=json.load(sys.stdin); print(len(d['duplicate_timestamps']))" 2>/dev/null || true)
if [ "$TS_COUNT" != "2" ]; then
    fail "expected 2 duplicate_timestamps, got '$TS_COUNT'"
fi
pass "duplicate_timestamps count correct ($TS_COUNT)"

# ── 11. vector index persisted to disk ───────────────────────────────────────
step "vector index persisted to disk after ingest"
VEC_FILES=$(find db -path "*/vec/*" -type f 2>/dev/null | wc -l | tr -d ' ')
if [ "$VEC_FILES" -eq 0 ]; then
    fail "no vector index files under db/*/vec/ — sync may not have flushed"
fi
pass "vector index present ($VEC_FILES file(s) under vec/)"

printf '\n\033[32;1m ALL CHECKS PASSED\033[0m\n\n'
