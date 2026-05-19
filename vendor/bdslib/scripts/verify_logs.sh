#!/usr/bin/env bash
# End-to-end test: log ingestion, exact-match and semantic deduplication,
# primary/secondary split, duplication timestamps, FTS, and vector search.
# Usage: ./verify_logs.sh [path/to/bds.hjson]
set -euo pipefail

CONFIG="${1:-./bds.hjson}"
BIN="./target/debug/bdscli"

pass() { printf '\033[32mPASS\033[0m  %s\n' "$*"; }
fail() { printf '\033[31mFAIL\033[0m  %s\n' "$*"; exit 1; }
step() { printf '\n\033[1m=== %s ===\033[0m\n' "$*"; }
info() { printf '      %s\n' "$*"; }

# ─────────────────────────────────────────────────────────────────────────────
# 0. Build
# ─────────────────────────────────────────────────────────────────────────────
step "build"
cargo build --bin bdscli 2>&1 | tail -1
pass "binary ready"

# ─────────────────────────────────────────────────────────────────────────────
# 1. Fresh database
# ─────────────────────────────────────────────────────────────────────────────
step "init --new"
$BIN -c "$CONFIG" init --new
pass "clean database created"

# ─────────────────────────────────────────────────────────────────────────────
# 2. Ingest syslog entries (random → all primaries)
# ─────────────────────────────────────────────────────────────────────────────
SYSLOG_N=20
step "ingest $SYSLOG_N syslog-format entries"
$BIN -c "$CONFIG" generate log -n "$SYSLOG_N" --format syslog --ingest
pass "$SYSLOG_N syslog entries ingested"

# ─────────────────────────────────────────────────────────────────────────────
# 3. Ingest http-nginx entries (FTS / vector search target)
#    All docs have data.server = "nginx" in their JSON fingerprint, which makes
#    them reliably findable by FTS query "nginx" and by vector queries about
#    HTTP web-server access logs.
# ─────────────────────────────────────────────────────────────────────────────
NGINX_N=10
step "ingest $NGINX_N http-nginx log entries (FTS / vector target)"
$BIN -c "$CONFIG" generate log -n "$NGINX_N" --format http-nginx --ingest
pass "$NGINX_N http-nginx entries ingested"

# ─────────────────────────────────────────────────────────────────────────────
# 4. Ingest near-duplicate syslog entries → 1 primary + (N-1) secondaries
#    Same key / message / host, different high-cardinality idx field so that:
#      • data_text is distinct per doc  (exact-match dedup is NOT triggered)
#      • embeddings are near-identical  (semantic dedup classifies 2..N as secondary)
#    --duration 1min pins all timestamps inside the current 1-hour shard so the
#    embedding comparison always operates within a single shard scope.
# ─────────────────────────────────────────────────────────────────────────────
SEC_N=5
SEC_KEY="test.sshd.auth"
SEC_TMPL='{"timestamp":"$timestamp","key":"'"$SEC_KEY"'","data":{"message":"Accepted publickey for deploy from 10.0.0.1 port 22","host":"web-01","env":"prod","idx":"$int(1,10000)"}}'

step "ingest $SEC_N near-duplicate syslog entries (expect 1 primary + $(( SEC_N - 1 )) secondaries)"
$BIN -c "$CONFIG" generate templated -n "$SEC_N" --duration 1min \
    --template "$SEC_TMPL" --ingest
pass "$SEC_N near-duplicate entries ingested"

# ─────────────────────────────────────────────────────────────────────────────
# 5. Ingest exact-match duplicate entries → 1 stored record + (N-1) timestamps
#    Identical key + data → exact-match dedup path: only 1 record written to
#    telemetry; each extra submission appended to dedup_tracking.timestamps.
# ─────────────────────────────────────────────────────────────────────────────
DEDUP_N=3
DEDUP_KEY="test.nginx.proc"
DEDUP_TMPL='{"timestamp":"$timestamp","key":"'"$DEDUP_KEY"'","data":{"message":"worker process started","host":"web-01","pid":"5678"}}'

step "ingest $DEDUP_N exact-match duplicate entries (1 stored + $(( DEDUP_N - 1 )) timestamps)"
$BIN -c "$CONFIG" generate templated -n "$DEDUP_N" --duration 1min \
    --template "$DEDUP_TMPL" --ingest
pass "$DEDUP_N exact-match entries ingested (only 1 stored)"

# ─────────────────────────────────────────────────────────────────────────────
# 6. Verify total record count
#    Exact-match batch stores only 1 record; near-dup batch stores all 5.
# ─────────────────────────────────────────────────────────────────────────────
EXPECTED_TOTAL=$(( SYSLOG_N + NGINX_N + SEC_N + 1 ))
step "get (no flags) — expect $EXPECTED_TOTAL total records"
ALL_OUT=$($BIN -c "$CONFIG" get 2>/tmp/vl_all.err)
cat /tmp/vl_all.err
ACTUAL_TOTAL=$(echo "$ALL_OUT" | grep -c '"key"' || true)
if [ "$ACTUAL_TOTAL" -ne "$EXPECTED_TOTAL" ]; then
    fail "total records: expected $EXPECTED_TOTAL, got $ACTUAL_TOTAL"
fi
pass "total record count correct ($ACTUAL_TOTAL)"

# ─────────────────────────────────────────────────────────────────────────────
# 7. Primary / secondary split for the near-dup batch
# ─────────────────────────────────────────────────────────────────────────────
step "get --primary — locate the single primary for key '$SEC_KEY'"
PRIMARY_OUT=$($BIN -c "$CONFIG" get --primary 2>/tmp/vl_primary.err)
cat /tmp/vl_primary.err

SEC_PRI_LINES=$(echo "$PRIMARY_OUT" | grep "\"$SEC_KEY\"" || true)
SEC_PRI_COUNT=$(echo "$SEC_PRI_LINES" | grep -c '"key"' || true)
if [ "$SEC_PRI_COUNT" -ne 1 ]; then
    fail "primaries with key '$SEC_KEY': expected 1, got $SEC_PRI_COUNT"
fi
pass "exactly 1 primary for key '$SEC_KEY'"

SEC_PRI_ID=$(echo "$SEC_PRI_LINES" | head -1 \
    | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
if [ -z "$SEC_PRI_ID" ]; then
    fail "could not extract UUID for primary '$SEC_KEY'"
fi
pass "primary id: $SEC_PRI_ID"

step "get --secondary --primary-id $SEC_PRI_ID (expect $(( SEC_N - 1 )) secondaries)"
SEC_OUT=$($BIN -c "$CONFIG" get --secondary --primary-id "$SEC_PRI_ID" 2>/tmp/vl_sec.err)
cat /tmp/vl_sec.err
ACTUAL_SEC=$(echo "$SEC_OUT" | grep -c '"key"' || true)
if [ "$ACTUAL_SEC" -ne $(( SEC_N - 1 )) ]; then
    fail "secondaries: expected $(( SEC_N - 1 )), got $ACTUAL_SEC"
fi
pass "secondary count correct ($ACTUAL_SEC)"

WRONG_KEY=$(echo "$SEC_OUT" | grep -v "\"$SEC_KEY\"" | grep -c '"key"' || true)
if [ "$WRONG_KEY" -ne 0 ]; then
    fail "$WRONG_KEY secondary docs carry an unexpected key"
fi
pass "all secondaries carry key '$SEC_KEY'"

# ─────────────────────────────────────────────────────────────────────────────
# 8. Duplication timestamps for the exact-match batch
# ─────────────────────────────────────────────────────────────────────────────
step "get --duplication-timestamps — all dedup entries"
DT_ALL=$($BIN -c "$CONFIG" get --duplication-timestamps 2>/tmp/vl_dt.err)
cat /tmp/vl_dt.err

if ! echo "$DT_ALL" | grep -q "\"$DEDUP_KEY\""; then
    fail "key '$DEDUP_KEY' missing from --duplication-timestamps output"
fi
pass "dedup key '$DEDUP_KEY' present in global listing"

DEDUP_PID=$(echo "$DT_ALL" | grep "\"$DEDUP_KEY\"" | head -1 \
    | grep -o '"primary_id":"[^"]*"' | cut -d'"' -f4)
if [ -z "$DEDUP_PID" ]; then
    fail "could not extract primary_id for dedup key '$DEDUP_KEY'"
fi
pass "dedup primary_id: $DEDUP_PID"

step "get --duplication-timestamps --primary-id $DEDUP_PID (expect $(( DEDUP_N - 1 )) timestamps)"
DT_SINGLE=$($BIN -c "$CONFIG" get --duplication-timestamps \
    --primary-id "$DEDUP_PID" 2>/tmp/vl_dts.err)
cat /tmp/vl_dts.err
DT_TS_COUNT=$(echo "$DT_SINGLE" | python3 -c \
    "import json,sys; d=json.load(sys.stdin); print(len(d['duplicate_timestamps']))" \
    2>/dev/null || true)
if [ "$DT_TS_COUNT" != "$(( DEDUP_N - 1 ))" ]; then
    fail "expected $(( DEDUP_N - 1 )) duplicate_timestamps, got '$DT_TS_COUNT'"
fi
pass "duplicate timestamp count correct ($DT_TS_COUNT)"

# Confirm the exact-match primary is NOT in the secondaries table (different path)
step "verify exact-match primary has no secondaries (dedup_tracking, not primary_secondary)"
DEDUP_SECS=$($BIN -c "$CONFIG" get --secondary --primary-id "$DEDUP_PID" 2>/tmp/vl_dups.err)
cat /tmp/vl_dups.err
DEDUP_SEC_COUNT=$(echo "$DEDUP_SECS" | grep -c '"key"' || true)
if [ "$DEDUP_SEC_COUNT" -ne 0 ]; then
    fail "exact-match primary should have 0 secondaries, got $DEDUP_SEC_COUNT"
fi
pass "exact-match primary has 0 secondaries (correct: stored in dedup_tracking only)"

# ─────────────────────────────────────────────────────────────────────────────
# 9. Full-text search (FTS)
#    http-nginx docs contain data.server = "nginx" which is emitted by
#    json_fingerprint as "server: nginx".  Tantivy tokenises this and a query
#    for "nginx" must match at least the $NGINX_N docs we just ingested.
# ─────────────────────────────────────────────────────────────────────────────
step "search fts --query 'nginx' (expect ≥ $NGINX_N hits)"
FTS_OUT=$($BIN -c "$CONFIG" search fts --query "nginx" --duration 1h --limit 100 2>&1)
echo "$FTS_OUT"
FTS_HITS=$(echo "$FTS_OUT" | grep -i "hits" | grep -oE '[0-9]+' | head -1 || true)
info "FTS hits: ${FTS_HITS:-0}"
if [ -z "$FTS_HITS" ] || [ "$FTS_HITS" -lt "$NGINX_N" ]; then
    fail "FTS 'nginx': expected ≥ $NGINX_N hits, got '${FTS_HITS:-0}'"
fi
pass "FTS 'nginx' returned $FTS_HITS hits (≥ $NGINX_N)"

# ─────────────────────────────────────────────────────────────────────────────
# 10. FTS — search for a syslog-specific term
#     All syslog docs have a "raw" field containing the full syslog line which
#     includes the program name, host, pid, and message.  The word "root" or a
#     program name like "cron" may appear; use a broader Tantivy disjunction
#     that matches any syslog-typical term.
# ─────────────────────────────────────────────────────────────────────────────
step "search fts --query 'sshd OR cron OR postgres OR kernel' (syslog program names)"
SYSLOG_FTS=$($BIN -c "$CONFIG" search fts \
    --query "sshd OR cron OR postgres OR kernel" --duration 1h --limit 100 2>&1)
echo "$SYSLOG_FTS"
SYSLOG_FTS_HITS=$(echo "$SYSLOG_FTS" | grep -i "hits" | grep -oE '[0-9]+' | head -1 || true)
info "syslog FTS hits: ${SYSLOG_FTS_HITS:-0}"
if [ -z "$SYSLOG_FTS_HITS" ] || [ "$SYSLOG_FTS_HITS" -lt 1 ]; then
    fail "FTS syslog terms: expected ≥ 1 hit, got '${SYSLOG_FTS_HITS:-0}'"
fi
pass "FTS syslog terms returned $SYSLOG_FTS_HITS hits"

# ─────────────────────────────────────────────────────────────────────────────
# 11. Vector search
#     The HNSW index is loaded from disk (VecStore::open) each time the shard
#     is opened.  A semantic query close to the http-nginx log content should
#     rank those docs highly.
# ─────────────────────────────────────────────────────────────────────────────
step "search vector --query 'HTTP web server nginx access log request'"
VEC_OUT=$($BIN -c "$CONFIG" search vector \
    --query "HTTP web server nginx access log request" --duration 1h --limit 20 2>&1)
echo "$VEC_OUT"
VEC_HITS=$(echo "$VEC_OUT" | grep -i "hits" | grep -oE '[0-9]+' | head -1 || true)
info "vector hits: ${VEC_HITS:-0}"
if [ -z "$VEC_HITS" ] || [ "$VEC_HITS" -lt 1 ]; then
    fail "vector search returned 0 hits"
fi
pass "vector search returned $VEC_HITS hits"

step "search vector --query 'SSH authentication public key login'"
VEC2_OUT=$($BIN -c "$CONFIG" search vector \
    --query "SSH authentication public key login" --duration 1h --limit 20 2>&1)
echo "$VEC2_OUT"
VEC2_HITS=$(echo "$VEC2_OUT" | grep -i "hits" | grep -oE '[0-9]+' | head -1 || true)
info "vector hits: ${VEC2_HITS:-0}"
if [ -z "$VEC2_HITS" ] || [ "$VEC2_HITS" -lt 1 ]; then
    fail "vector SSH search returned 0 hits"
fi
pass "vector SSH search returned $VEC2_HITS hits"

# ─────────────────────────────────────────────────────────────────────────────
# 12. Windowed get — all records still visible within 1h
# ─────────────────────────────────────────────────────────────────────────────
step "get --duration 1h — expect same $EXPECTED_TOTAL records"
WIN_OUT=$($BIN -c "$CONFIG" get --duration 1h 2>/tmp/vl_win.err)
cat /tmp/vl_win.err
WIN_TOTAL=$(echo "$WIN_OUT" | grep -c '"key"' || true)
if [ "$WIN_TOTAL" -ne "$EXPECTED_TOTAL" ]; then
    fail "windowed total: expected $EXPECTED_TOTAL, got $WIN_TOTAL"
fi
pass "windowed total correct ($WIN_TOTAL)"

printf '\n\033[32;1m ALL CHECKS PASSED\033[0m\n\n'
