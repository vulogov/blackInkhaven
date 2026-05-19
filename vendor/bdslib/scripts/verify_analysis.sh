#!/usr/bin/env bash
# Prove LDA topic analysis over a structured log corpus.
#
# Corpus design (key = "corpus.logs"):
#   Cluster A – auth/security  : category=security, action ∈ {login, password, token, session, …}
#   Cluster B – infrastructure : category=system,   action ∈ {service, disk, memory, cpu, …}
#   Cluster C – app errors     : category=application, action ∈ {null pointer, timeout, overflow, …}
#
# Each cluster has 20 documents.  A high-cardinality $int idx field keeps
# every document's data_text unique so exact-match dedup never fires —
# all 60 records are stored and counted by LDA.
#
# A separate key "corpus.near" carries 5 near-duplicate docs (same content,
# different idx) to verify the primary/secondary split and confirm LDA still
# counts secondaries as part of the corpus when they share a key.
#
# Usage: ./verify_analysis.sh [path/to/bds.hjson]
set -euo pipefail

CONFIG="${1:-./bds.hjson}"
BIN="./target/debug/bdscli"
CORPUS_KEY="corpus.logs"
NEAR_KEY="corpus.near"
CLUSTER_N=20    # docs per cluster
NEAR_N=5        # near-duplicate batch size

pass() { printf '\033[32mPASS\033[0m  %s\n' "$*"; }
fail() { printf '\033[31mFAIL\033[0m  %s\n' "$*"; exit 1; }
step() { printf '\n\033[1m=== %s ===\033[0m\n' "$*"; }
info() { printf '      %s\n' "$*"; }
banner() { printf '\n\033[34;1m>>> %s\033[0m\n' "$*"; }

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
# 2. Ingest corpus — Cluster A: security / authentication events
#    LDA discriminative tokens: "security", "login", "password", "token",
#    "session", "mfa", "account"
# ─────────────────────────────────────────────────────────────────────────────
TMPL_A='{"timestamp":"$timestamp","key":"'"$CORPUS_KEY"'","data":{
  "category":"security",
  "action":"$choice(login success,login failure,password reset,token refresh,session start,session end,mfa challenge,account locked)",
  "user":"$choice(alice,bob,carol,dave,eve,frank)",
  "host":"$choice(web-01,api-01,db-01,auth-01)",
  "severity":"$choice(info,warning,error)",
  "idx":"$int(1,1000000)"}}'

step "ingest $CLUSTER_N docs — Cluster A (security/auth)"
$BIN -c "$CONFIG" generate templated -n "$CLUSTER_N" \
    --duration 1min --template "$TMPL_A" --ingest
pass "$CLUSTER_N auth docs ingested"

# ─────────────────────────────────────────────────────────────────────────────
# 3. Cluster B: infrastructure / system events
#    LDA discriminative tokens: "system", "service", "disk", "memory",
#    "cpu", "network", "process", "health"
# ─────────────────────────────────────────────────────────────────────────────
TMPL_B='{"timestamp":"$timestamp","key":"'"$CORPUS_KEY"'","data":{
  "category":"system",
  "action":"$choice(service restart,disk warning,memory pressure,cpu spike,network timeout,process crash,health check,load balanced)",
  "host":"$choice(web-01,api-01,db-01,cache-01,worker-01)",
  "region":"$choice(us-east-1,us-west-2,eu-west-1)",
  "severity":"$choice(info,warning,critical)",
  "idx":"$int(1,1000000)"}}'

step "ingest $CLUSTER_N docs — Cluster B (infrastructure/system)"
$BIN -c "$CONFIG" generate templated -n "$CLUSTER_N" \
    --duration 1min --template "$TMPL_B" --ingest
pass "$CLUSTER_N system docs ingested"

# ─────────────────────────────────────────────────────────────────────────────
# 4. Cluster C: application / error events
#    LDA discriminative tokens: "application", "null", "pointer", "stack",
#    "overflow", "connection", "timeout", "validation", "schema"
# ─────────────────────────────────────────────────────────────────────────────
TMPL_C='{"timestamp":"$timestamp","key":"'"$CORPUS_KEY"'","data":{
  "category":"application",
  "action":"$choice(null pointer,stack overflow,connection refused,timeout exceeded,rate limited,validation failed,schema mismatch,data corruption)",
  "service":"$choice(auth,api,database,cache,queue,scheduler)",
  "severity":"$choice(error,critical,fatal)",
  "idx":"$int(1,1000000)"}}'

step "ingest $CLUSTER_N docs — Cluster C (application/errors)"
$BIN -c "$CONFIG" generate templated -n "$CLUSTER_N" \
    --duration 1min --template "$TMPL_C" --ingest
pass "$CLUSTER_N error docs ingested"

# ─────────────────────────────────────────────────────────────────────────────
# 5. Ingest near-duplicate batch (separate key)
#    Same action/category/host in every doc; only idx differs.
#    → different data_text per doc (exact-match dedup never fires)
#    → near-identical embeddings → 1 primary + (NEAR_N - 1) secondaries
# ─────────────────────────────────────────────────────────────────────────────
NEAR_SEC=$(( NEAR_N - 1 ))
TMPL_NEAR='{"timestamp":"$timestamp","key":"'"$NEAR_KEY"'","data":{
  "category":"security",
  "action":"login success",
  "user":"deployer",
  "host":"gateway-01",
  "severity":"info",
  "idx":"$int(1,1000000)"}}'

step "ingest $NEAR_N near-duplicate docs (key=$NEAR_KEY, expect 1 primary + $NEAR_SEC secondaries)"
$BIN -c "$CONFIG" generate templated -n "$NEAR_N" \
    --duration 1min --template "$TMPL_NEAR" --ingest
pass "$NEAR_N near-duplicate docs ingested"

# ─────────────────────────────────────────────────────────────────────────────
# 6. Verify stored record counts
# ─────────────────────────────────────────────────────────────────────────────
CORPUS_TOTAL=$(( CLUSTER_N * 3 ))          # 60
EXPECTED_TOTAL=$(( CORPUS_TOTAL + NEAR_N )) # 65

step "get (no flags) — expect $EXPECTED_TOTAL total stored records"
ALL_OUT=$($BIN -c "$CONFIG" get 2>/tmp/va_all.err)
cat /tmp/va_all.err
ACTUAL_TOTAL=$(echo "$ALL_OUT" | grep -c '"key"' || true)
if [ "$ACTUAL_TOTAL" -ne "$EXPECTED_TOTAL" ]; then
    fail "total records: expected $EXPECTED_TOTAL, got $ACTUAL_TOTAL"
fi
pass "total stored records correct ($ACTUAL_TOTAL)"

# Verify corpus.logs contributes exactly CORPUS_TOTAL records
LOGS_COUNT=$(echo "$ALL_OUT" | grep -c "\"$CORPUS_KEY\"" || true)
if [ "$LOGS_COUNT" -ne "$CORPUS_TOTAL" ]; then
    fail "corpus.logs records: expected $CORPUS_TOTAL, got $LOGS_COUNT"
fi
pass "corpus.logs record count correct ($LOGS_COUNT)"

# Verify near-dup key contributes NEAR_N records
NEAR_COUNT=$(echo "$ALL_OUT" | grep -c "\"$NEAR_KEY\"" || true)
if [ "$NEAR_COUNT" -ne "$NEAR_N" ]; then
    fail "corpus.near records: expected $NEAR_N, got $NEAR_COUNT"
fi
pass "corpus.near record count correct ($NEAR_COUNT)"

# ─────────────────────────────────────────────────────────────────────────────
# 7. Primary / secondary split for the near-duplicate batch
# ─────────────────────────────────────────────────────────────────────────────
banner "Primary / secondary split"

step "get --primary — find the single primary for key '$NEAR_KEY'"
PRIM_OUT=$($BIN -c "$CONFIG" get --primary 2>/tmp/va_prim.err)
cat /tmp/va_prim.err

NEAR_PRIM_LINES=$(echo "$PRIM_OUT" | grep "\"$NEAR_KEY\"" || true)
NEAR_PRIM_COUNT=$(echo "$NEAR_PRIM_LINES" | grep -c '"key"' || true)
if [ "$NEAR_PRIM_COUNT" -ne 1 ]; then
    fail "primaries with key '$NEAR_KEY': expected 1, got $NEAR_PRIM_COUNT"
fi
pass "exactly 1 primary for '$NEAR_KEY'"

NEAR_PID=$(echo "$NEAR_PRIM_LINES" | head -1 \
    | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
[ -n "$NEAR_PID" ] || fail "could not extract UUID for '$NEAR_KEY' primary"
pass "primary id: $NEAR_PID"

step "get --secondary --primary-id $NEAR_PID (expect $NEAR_SEC secondaries)"
SEC_OUT=$($BIN -c "$CONFIG" get --secondary --primary-id "$NEAR_PID" 2>/tmp/va_sec.err)
cat /tmp/va_sec.err
ACTUAL_SEC=$(echo "$SEC_OUT" | grep -c '"key"' || true)
if [ "$ACTUAL_SEC" -ne "$NEAR_SEC" ]; then
    fail "secondaries: expected $NEAR_SEC, got $ACTUAL_SEC"
fi
pass "secondary count correct ($ACTUAL_SEC)"

WRONG=$(echo "$SEC_OUT" | grep -v "\"$NEAR_KEY\"" | grep -c '"key"' || true)
[ "$WRONG" -eq 0 ] || fail "$WRONG secondaries carry an unexpected key"
pass "all secondaries carry key '$NEAR_KEY'"

# Show primary/secondary breakdown for the full corpus informally
TOTAL_PRIM=$(echo "$PRIM_OUT" | grep -c '"key"' || true)
TOTAL_LOGS_PRIM=$(echo "$PRIM_OUT" | grep -c "\"$CORPUS_KEY\"" || true)
info "corpus.logs primaries : $TOTAL_LOGS_PRIM  (embedding-based dedup may create secondaries)"
info "corpus.logs secondaries: $(( LOGS_COUNT - TOTAL_LOGS_PRIM ))  (same key, high embedding similarity)"
info "corpus.near primaries : 1"
info "corpus.near secondaries: $ACTUAL_SEC"

# ─────────────────────────────────────────────────────────────────────────────
# 8. LDA topic analysis — main corpus (k = 3)
# ─────────────────────────────────────────────────────────────────────────────
banner "LDA topic analysis  (k=3, seed=42)"

step "analyze topics --key $CORPUS_KEY --k 3 --duration 1h"
TOPICS_OUT=$($BIN -c "$CONFIG" analyze topics \
    --key "$CORPUS_KEY" --k 3 --duration 1h \
    --seed 42 --iters 300 --top-n 15 2>&1)
echo "$TOPICS_OUT"

N_DOCS=$(echo "$TOPICS_OUT"    | grep "^docs"     | grep -oE '[0-9]+' | head -1 || true)
N_TOPICS=$(echo "$TOPICS_OUT"  | grep "^topics"   | grep -oE '[0-9]+' | head -1 || true)
KEYWORDS=$(echo "$TOPICS_OUT"  | grep "^keywords" | cut -d: -f2-)

# docs must equal the full corpus (primaries + secondaries)
if [ "${N_DOCS:-0}" -ne "$CORPUS_TOTAL" ]; then
    fail "LDA n_docs: expected $CORPUS_TOTAL (primaries+secondaries), got '${N_DOCS}'"
fi
pass "n_docs correct ($N_DOCS) — LDA counted every record including secondaries"

# topics must match k
if [ "${N_TOPICS:-0}" -ne 3 ]; then
    fail "LDA n_topics: expected 3, got '${N_TOPICS}'"
fi
pass "n_topics correct ($N_TOPICS)"

# keywords must be non-empty
if [ -z "$(echo "$KEYWORDS" | tr -d ' ,')" ]; then
    fail "keywords string is empty"
fi
pass "keywords non-empty"

# ─────────────────────────────────────────────────────────────────────────────
# 9. Verify each cluster's discriminative term appears in keywords
#    The category values "security", "system", "application" appear in EVERY
#    doc of their respective cluster and in NO doc of the other clusters —
#    they are the most informative tokens for LDA topic separation.
# ─────────────────────────────────────────────────────────────────────────────
banner "Keyword verification"

check_keyword() {
    local label="$1"; shift
    local kw
    for kw in "$@"; do
        if echo "$KEYWORDS" | grep -qiw "$kw"; then
            pass "Cluster $label: keyword '$kw' found  (keywords:$KEYWORDS)"
            return 0
        fi
    done
    fail "Cluster $label: none of [$*] found in keywords '$KEYWORDS'"
}

# Cluster A discriminators: category value "security" + action terms
check_keyword "A (auth/security)"   security login password token session mfa account

# Cluster B discriminators: category value "system" + action terms
check_keyword "B (infra/system)"    system service disk memory cpu network process health

# Cluster C discriminators: category value "application" + action terms
check_keyword "C (app/errors)"      application null pointer stack overflow timeout validation schema

# ─────────────────────────────────────────────────────────────────────────────
# 10. Show sensitivity to k — same corpus, different number of topics
# ─────────────────────────────────────────────────────────────────────────────
banner "Sensitivity to k"

for K in 2 5; do
    step "analyze topics --key $CORPUS_KEY --k $K"
    OUT_K=$($BIN -c "$CONFIG" analyze topics \
        --key "$CORPUS_KEY" --k "$K" --duration 1h \
        --seed 42 --iters 300 --top-n 10 2>&1)
    echo "$OUT_K"
    NK=$(echo "$OUT_K" | grep "^topics" | grep -oE '[0-9]+' | head -1 || true)
    if [ "${NK:-0}" -ne "$K" ]; then
        fail "k=$K: reported topics=${NK}"
    fi
    pass "k=$K: topics=$NK, keywords=$(echo "$OUT_K" | grep '^keywords' | cut -d: -f2- | tr -d ' ' | wc -c | tr -d ' ') chars"
done

# ─────────────────────────────────────────────────────────────────────────────
# 11. LDA on near-duplicate corpus (k auto-clamped to n_docs when k > n_docs)
# ─────────────────────────────────────────────────────────────────────────────
banner "LDA on small near-duplicate corpus  (key=$NEAR_KEY)"

step "analyze topics --key $NEAR_KEY --k 3 (auto-clamped: n_docs=$NEAR_N < k=3)"
NEAR_TOPICS=$($BIN -c "$CONFIG" analyze topics \
    --key "$NEAR_KEY" --k 3 --duration 1h \
    --seed 42 --iters 200 --top-n 10 2>&1)
echo "$NEAR_TOPICS"

NEAR_NDOCS=$(echo "$NEAR_TOPICS"   | grep "^docs"     | grep -oE '[0-9]+' | head -1 || true)
NEAR_NTOPICS=$(echo "$NEAR_TOPICS" | grep "^topics"   | grep -oE '[0-9]+' | head -1 || true)
NEAR_KW=$(echo "$NEAR_TOPICS"      | grep "^keywords" | cut -d: -f2-)

if [ "${NEAR_NDOCS:-0}" -ne "$NEAR_N" ]; then
    fail "near-dup LDA n_docs: expected $NEAR_N, got '${NEAR_NDOCS}'"
fi
pass "near-dup n_docs correct ($NEAR_NDOCS)"

# k is clamped to min(k, n_docs): with k=3 and n_docs=5, topics=3 (not clamped)
if [ "${NEAR_NTOPICS:-0}" -lt 1 ]; then
    fail "near-dup topics: expected ≥ 1, got '${NEAR_NTOPICS}'"
fi
pass "near-dup n_topics: $NEAR_NTOPICS"

[ -n "$(echo "$NEAR_KW" | tr -d ' ,')" ] || fail "near-dup keywords empty"
pass "near-dup keywords non-empty: $NEAR_KW"

# The near-dup corpus is all security/login docs — "security" or "login" must appear
if echo "$NEAR_KW" | grep -qiE 'security|login|deployer|gateway'; then
    pass "near-dup keywords contain corpus-specific terms"
else
    info "near-dup keywords: $NEAR_KW"
    fail "near-dup keywords do not contain expected terms (security, login, deployer, gateway)"
fi

printf '\n\033[32;1m ALL CHECKS PASSED\033[0m\n\n'
