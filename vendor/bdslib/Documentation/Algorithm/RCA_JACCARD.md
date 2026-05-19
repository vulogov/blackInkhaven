# Jaccard-based Root Cause Analysis (`bdslib::analysis::rca`)

`RcaResult::analyze(duration, &cfg)` and
`RcaResult::analyze_failure(failure_key, duration, &cfg)` are bdslib's
RCA pipelines. They look at every non-telemetry event observed in a
time window, group event keys that **co-occur** in the same time
buckets, and — when a specific failure key is named — rank every
co-occurring key by how consistently it **precedes** the failure,
yielding probable root-cause candidates.

The clustering and ranking signals are both built from one primitive:
**Jaccard similarity** of bucket appearances. This document explains
that primitive, the full pipeline that wraps it, and how it answers two
operationally distinct questions:

- *"What event keys move together in this window?"* → clusters.
- *"What plausibly caused the `service.down` events I'm seeing?"* →
  probable causes.

This document covers:

1. [What problem RCA solves here](#1-what-problem-rca-solves-here)
2. [The Jaccard similarity primitive](#2-the-jaccard-similarity-primitive)
3. [How bdslib uses Jaccard for RCA](#3-how-bdslib-uses-jaccard-for-rca)
4. [The full pipeline, step by step](#4-the-full-pipeline-step-by-step)
5. [Output contract](#5-output-contract)
6. [Configuration knobs](#6-configuration-knobs)
7. [Complexity and scaling](#7-complexity-and-scaling)
8. [Determinism guarantees](#8-determinism-guarantees)
9. [Worked examples](#9-worked-examples)
10. [Failure modes and edge cases](#10-failure-modes-and-edge-cases)
11. [References](#11-references)

---

## 1. What problem RCA solves here

You operate a system that emits structured events: log lines, alert
firings, syslog messages, drain3 templates. Most carry a **key** —
a service name, an alert rule id, a template uuid — and a **timestamp**.

Two questions show up constantly:

- "Last hour, what *groups* of event keys consistently fire together?"
  Answer this once and you've discovered all the implicit dependencies in
  your system: keys that always co-fire are part of the same incident,
  the same deploy, the same upstream outage.
- "Right now, `service.down` is firing. What plausibly caused it?"
  Answer this and you've automated the first step of every on-call
  runbook: list the keys that co-occur with the failure and tend to
  fire *before* it.

bdslib answers both with one pass over the event stream:

- Group events into discrete time buckets (`bucket_secs`).
- For each unordered pair of keys, count buckets they share — Jaccard
  similarity.
- Cluster keys whose Jaccard similarity exceeds a threshold (union-find).
- For the optional failure key, compute mean lead-time per co-occurring
  key and sort.

No supervised data, no distributional assumptions, no explicit
dependency graph. The only inputs are the events themselves and a
single time-bucket width.

---

## 2. The Jaccard similarity primitive

For two sets `A` and `B`, the **Jaccard similarity** is

```
J(A, B) = |A ∩ B| / |A ∪ B|
```

The score is in `[0, 1]`:

- `J = 0` — `A` and `B` are disjoint (no shared elements).
- `J = 1` — `A` and `B` are equal (every element shared).
- Intermediate — proportional to overlap relative to total coverage.

For RCA, the sets are **bucket-id sets**: for each event key `K`, let

```
S(K) = { bucket(t) : event with key K observed at time t }
```

The Jaccard similarity of two keys `K₁` and `K₂` is then

```
J(K₁, K₂) = |S(K₁) ∩ S(K₂)| / |S(K₁) ∪ S(K₂)|
```

Two keys with `J = 1` always fire together; `J = 0.5` means they share
half their buckets; `J = 0` means they never overlap. The same
computation lets you cluster keys (high-J pairs are connected) and rank
candidate causes (`J(candidate, failure)` is one of the ranking
signals).

The implementation uses the inclusion-exclusion identity
`|A ∪ B| = |A| + |B| - |A ∩ B|` to avoid materialising the union set:

```rust
fn jaccard_sim(
    a: &str,
    b: &str,
    cooccurrence: &HashMap<(String, String), usize>,
    frequencies:  &HashMap<String, usize>,
) -> f64 {
    let co = *cooccurrence
        .get(&ordered_pair(a.to_owned(), b.to_owned()))
        .unwrap_or(&0) as f64;
    if co == 0.0 { return 0.0; }
    let fa = *frequencies.get(a).unwrap_or(&0) as f64;
    let fb = *frequencies.get(b).unwrap_or(&0) as f64;
    let union = fa + fb - co;        // |A ∪ B| = |A| + |B| - |A ∩ B|
    if union <= 0.0 { 0.0 } else { co / union }
}
```

That is the entire mathematical machinery. Everything else in the
module is plumbing around it.

---

## 3. How bdslib uses Jaccard for RCA

Three derived quantities form the entire pipeline:

| Quantity | Definition | Used for |
|---|---|---|
| `frequencies[K]` | number of distinct buckets in which key `K` was observed | min-support gate, frequency cap, support per cluster |
| `cooccurrence[(K1, K2)]` | number of distinct buckets in which both keys were observed (`K1 < K2` lexicographically) | Jaccard numerator, candidate ranking |
| `J(K1, K2)` | derived from the two above via `co / (f1 + f2 - co)` | clustering edges + cause-similarity score |

Plus, when ranking causes for a named failure key, a fourth signal:

| Quantity | Definition | Used for |
|---|---|---|
| `avg_lead_secs[K]` | mean of `(failure_ts - K_ts)` over shared buckets | precursor scoring — positive ⇒ `K` fires before the failure ⇒ plausible cause |

Telemetry filtering happens before any of this: a record is dropped if
its `data` is a JSON number, or if `data["value"]` is a JSON number.
The output of metric pipelines (numeric measurements like
`cpu.usage = 72.4`) is fundamentally not an event — it's a sample —
and including it in co-occurrence counting would dilute the signal.

---

## 4. The full pipeline, step by step

Given a humantime window string (`"1h"`, `"30min"`, `"7days"`) and an
`RcaConfig`:

### Step 1 — Fetch non-telemetry events

The `fetch_events` helper enumerates every event key that has more than
one primary record in the window, sorts them most-frequent-first, and
caps the list at `cfg.max_keys` (default 200). For each accepted key, it
pulls the full primary records, drops anything that looks like
telemetry, and returns the surviving `(key, unix_timestamp)` pairs.

```rust
fn is_telemetry(data: &JsonValue) -> bool {
    data.is_number() || data.get("value").map_or(false, JsonValue::is_number)
}
```

Two subtle behaviours:

- The cap counts **event keys**, not records. A key whose records are
  all telemetry is silently skipped without consuming a slot in the
  cap, so the cap always retains the strongest non-telemetry signals.
- `min_support` is applied at the key level (must appear in
  ≥`min_support` distinct primaries) to prune ultra-rare keys before
  the matrix is built.

### Step 2 — Build the bucket index

Partition events into non-overlapping `bucket_secs`-wide buckets:

```rust
let mut buckets: HashMap<u64, HashSet<String>> = HashMap::new();
for (key, ts) in events {
    buckets
        .entry(ts / config.bucket_secs)
        .or_default()
        .insert(key.clone());     // dedup: one key counted once per bucket
}
```

A key fires multiple times in a bucket → still one membership. This
keeps Jaccard a set-similarity (not a multiset-similarity), which is
what the operational interpretation requires: "did keys A and B both
appear in this 5-minute window?", not "how many of each?"

### Step 3 — Count co-occurrences and frequencies

Walk every bucket and update the two HashMaps:

```rust
for keys in buckets.values() {
    for k in keys {
        *frequencies.entry(k.clone()).or_default() += 1;
    }
    let kv: Vec<&String> = keys.iter().collect();
    for i in 0..kv.len() {
        for j in (i + 1)..kv.len() {
            *cooccurrence
                .entry(ordered_pair(kv[i].clone(), kv[j].clone()))
                .or_default() += 1;
        }
    }
}
```

`ordered_pair(a, b)` returns `(min(a, b), max(a, b))` so unordered pairs
get a canonical key. `frequencies[K]` is `|S(K)|` — the number of
distinct buckets `K` appears in. `cooccurrence[(K1, K2)]` is
`|S(K1) ∩ S(K2)|`.

### Step 4 — Cluster keys via Jaccard threshold + union-find

For every pair of keys whose Jaccard similarity meets or exceeds
`cfg.jaccard_threshold` (default 0.2), union them:

```rust
let mut keys: Vec<String> = frequencies.keys().cloned().collect();
keys.sort();   // deterministic ordering before index assignment
let n = keys.len();

let mut uf = UnionFind::new(n);
for i in 0..n {
    for j in (i + 1)..n {
        if jaccard_sim(&keys[i], &keys[j], &cooccurrence, &frequencies)
            >= config.jaccard_threshold
        {
            uf.union(i, j);
        }
    }
}
```

Union-find with path compression and union-by-rank gives effectively
constant amortised cost per operation, so this loop is `O(n²)` over all
key pairs — same asymptotic cost as the cosine matrix in TextRank/LSA,
but with a much cheaper inner constant (one hashmap lookup, no
embedding, no float arithmetic beyond a single division).

For each connected component, build an `EventCluster`:

```rust
EventCluster {
    id,
    members: keys_in_component,           // sorted alphabetically
    support: min(frequencies[K] for K),   // bottleneck visibility
    cohesion: mean(J(Ki, Kj) for all i < j),
}
```

`support` is intentionally **the minimum** bucket-frequency among
members, not the sum or mean. Operationally, the whole cluster is
visible when *every* member fires, so the bottleneck member is the
constraint. `cohesion` is the mean pairwise Jaccard — high cohesion
means "these keys really do fire together every time".

Clusters are sorted by `cohesion` descending, ties broken by `support`
descending, and `id` is assigned `0..n_clusters` after the sort so
output ids are always dense and meaningful.

### Step 5 — Rank probable causes (if a failure key was given)

When the caller invoked `analyze_failure(failure_key, …)`, run an
additional pass to rank precursors:

```rust
let mut failure_buckets: HashMap<u64, u64> = HashMap::new();
for (key, ts) in events {
    if key == failure_key {
        let bucket = ts / config.bucket_secs;
        failure_buckets
            .entry(bucket)
            .and_modify(|e| { if *ts < *e { *e = *ts } })
            .or_insert(*ts);
    }
}
```

For each bucket containing the failure key, record the **earliest
failure timestamp**. This is the reference point against which every
candidate's lead-time is measured.

Then, for every non-failure event in a failure-containing bucket,
accumulate `(count, sum of lead seconds)`:

```rust
let mut acc: HashMap<String, (usize, f64)> = HashMap::new();
for (key, ts) in events {
    if key == failure_key { continue; }
    let bucket = ts / config.bucket_secs;
    if let Some(&fail_ts) = failure_buckets.get(&bucket) {
        let lead = fail_ts as f64 - *ts as f64;   // positive ⇒ K precedes failure
        let e = acc.entry(key.clone()).or_default();
        e.0 += 1;
        e.1 += lead;
    }
}
```

For each accumulated key, compute:

```
co_occurrence_count = e.0
avg_lead_secs       = e.1 / e.0
jaccard             = J(key, failure_key)
```

Sort candidates by `avg_lead_secs` descending (strongest precursors
first); ties broken by `jaccard` descending (highest set-overlap with the
failure first).

The interpretation:

- **Positive `avg_lead_secs`** — this key's events typically arrive
  *before* the earliest failure event in the same bucket. Plausible
  precursor. Higher = earlier = more interesting.
- **Negative `avg_lead_secs`** — this key's events typically arrive
  *after* the failure. Likely a downstream consequence (cascading
  failure, alert fired by something the failure broke).
- **High `jaccard`** alone tells you "these keys are correlated"; high
  `avg_lead_secs` tells you "and the candidate is the leading edge".
  You want both.

Note: a candidate that fires at `failure_ts - 1s` in many buckets
scores higher than one that fires at `failure_ts - 30s` in fewer
buckets. The mean is over **shared buckets**, not over the whole
window — sparse-but-consistent precursors get the credit they deserve.

### Step 6 — Assemble the result

```rust
RcaResult {
    failure_key,
    start, end,                    // earliest / latest event timestamps in window
    n_events,                      // total non-telemetry primary records analysed
    n_keys,                        // distinct event keys after telemetry + min_support
    clusters,                      // co-occurrence clusters, cohesion-sorted
    probable_causes,               // empty unless failure_key was given
}
```

---

## 5. Output contract

`RcaResult` is a Serde `Serialize`/`Deserialize` struct, returned by
both `RcaResult::analyze` and `RcaResult::analyze_failure`. Field
semantics:

| Field | Type | Description |
|---|---|---|
| `failure_key` | `Option<String>` | The failure key passed to `analyze_failure`, or `None` for `analyze`. |
| `start` | `u64` | Unix-seconds of the earliest event observed in the window. |
| `end` | `u64` | Unix-seconds of the latest event observed in the window. |
| `n_events` | `usize` | Total non-telemetry primary records analysed. |
| `n_keys` | `usize` | Number of distinct event keys after telemetry filtering and `min_support` thresholding. |
| `clusters` | `Vec<EventCluster>` | Co-occurrence clusters, sorted by `cohesion` desc, then `support` desc. Always populated. |
| `probable_causes` | `Vec<CausalCandidate>` | Probable precursors, sorted by `avg_lead_secs` desc. Empty when no failure key was given, or when the failure key was not observed in the window. |

`EventCluster`:

| Field | Type | Description |
|---|---|---|
| `id` | `usize` | Dense `0..n_clusters` cluster identifier. |
| `members` | `Vec<String>` | Event keys in the cluster, sorted alphabetically. |
| `support` | `usize` | Minimum bucket-frequency among all members. |
| `cohesion` | `f64` | Mean pairwise Jaccard similarity in `[0, 1]`. |

`CausalCandidate`:

| Field | Type | Description |
|---|---|---|
| `key` | `String` | Event key of the candidate. |
| `co_occurrence_count` | `usize` | Number of records (not buckets) of this key that fell in a bucket also containing the failure. |
| `jaccard` | `f64` | Jaccard similarity between this key and the failure key. |
| `avg_lead_secs` | `f64` | Mean signed delta `failure_ts - K_ts` across shared buckets. Positive ⇒ candidate precedes failure. |

---

## 6. Configuration knobs

```rust
pub struct RcaConfig {
    pub bucket_secs:       u64,    // default: 300 (5 minutes)
    pub min_support:       usize,  // default: 2
    pub jaccard_threshold: f64,    // default: 0.2
    pub max_keys:          usize,  // default: 200
}
```

| Knob | Effect |
|---|---|
| **`bucket_secs`** | The grain of "co-occurrence". Smaller buckets ⇒ stricter notion of "fired together" (events must be within seconds), larger ⇒ looser (events anywhere in the same 30 minutes count). Operationally: pick a value comparable to your alert-firing latency. |
| **`min_support`** | Minimum number of distinct buckets a key must appear in to enter the analysis. Prunes one-off events that can't form meaningful clusters or causal links. Default 2 is the absolute minimum useful; raise to 5 for noisy environments. |
| **`jaccard_threshold`** | Minimum Jaccard similarity for two keys to land in the same cluster. Lower ⇒ larger, looser clusters; higher ⇒ tighter, fewer clusters. Default 0.2 is a good middle ground. Use 0.5+ to find only the most strongly-linked pairs. |
| **`max_keys`** | Upper bound on event keys analysed. Keys are kept most-frequent-first; the cap retains the strongest signals when the window contains a long tail of rare events. Default 200 keeps the `O(n²)` matrix manageable. |

---

## 7. Complexity and scaling

For a window with `E` non-telemetry events spanning `n` distinct keys
and `B` distinct buckets:

| Phase | Cost |
|---|---|
| Fetch events from DB | dominated by ShardsManager queries — typically sub-second up to ~10⁶ records |
| Bucket assignment | `O(E)` |
| Per-bucket key dedup + pair enumeration | `O(B · k̄²)` where `k̄` is the average distinct keys per bucket |
| Pairwise Jaccard + union-find | `O(n²)` |
| Cluster bookkeeping (cohesion mean) | `O(Σ_C |C|² )` ≤ `O(n²)` |
| Causal ranking (when failure_key given) | `O(E)` extra |

The dominant terms are the bucket-pair enumeration `O(B · k̄²)` and the
pairwise Jaccard `O(n²)`. Both are well-behaved when the `max_keys` cap
holds: with `n ≤ 200`, `n² = 40 000` Jaccard lookups, each a single
hashmap probe — sub-millisecond.

Memory:

| Structure | Size |
|---|---|
| Bucket index (`HashMap<u64, HashSet<String>>`) | `O(E)` worst case |
| Frequencies | `O(n)` |
| Co-occurrence | `O(n²)` worst case (hashmap entries for every co-occurring pair) |
| Failure-bucket map | `O(B)` |
| Causal accumulator | `O(n)` |

In practice the co-occurrence map is much smaller than `n²` because most
key pairs never share a bucket — a dense one would imply *every* event
key fires in *every* bucket, which is operationally unrealistic.

---

## 8. Determinism guarantees

`RcaResult::analyze` is deterministic given:

- A deterministic event stream from the DB (which it gets — primary
  records have stable `(uuid, ts)` tuples and `primaries_get` returns
  them sorted).
- A deterministic key-ordering before union-find. The implementation
  sorts `frequencies.keys()` alphabetically (`keys.sort()`) before
  assigning index positions, so the union-find graph traversal hits
  pairs in the same order across runs.

Two paths could leak HashMap-iteration nondeterminism:

- Iterating `buckets.values()` in step 3 — but every operation inside
  is commutative (a bucket's pair contributions accumulate via `+=`,
  which is exact for integers), so iteration order does not affect
  the final `cooccurrence` and `frequencies` maps.
- Iterating `components.values()` in step 4 — but the result is
  immediately sorted by `(cohesion desc, support desc)`, and `id`
  assignment happens after the sort. Stable sort preserves
  insertion order; since the cohesion/support tiebreak rule almost
  always produces a strict total order on real corpora, the assignment
  is reproducible. (For pathologically symmetric inputs where two
  clusters tie on both metrics, the relative order may differ across
  runs — add a third tiebreaker on the smallest member name if
  byte-exact reproducibility matters.)

Time semantics:

- Events are bucketed by **integer division** of Unix seconds: `bucket
  = ts / bucket_secs`. This is reproducible across runs but **drifts
  with absolute time**: the same `duration` window run a minute later
  will use slightly different bucket boundaries. For comparison
  workflows where stability matters, use explicit `start_ts` /
  `end_ts` parameters rather than the relative-window form.

---

## 9. Worked examples

### Example A — co-occurrence clustering

A single Kubernetes incident produces, over a five-minute window:

```
service.api.down                @ 12:00:01, 12:01:14, 12:03:42
service.api.error_rate_high     @ 12:00:03, 12:01:16, 12:03:44
service.api.5xx_spike           @ 12:00:05, 12:01:18, 12:03:46
db.connection_pool_exhausted    @ 12:00:02, 12:01:13, 12:03:41
db.slow_query                   @ 12:00:00, 12:01:11, 12:03:39
unrelated.daily_backup_done     @ 12:00:30
unrelated.cron_health_ok        @ 12:01:00, 12:02:00, 12:03:00
```

With default config (`bucket_secs = 300`):

- All five `service.api.*` and `db.*` events fall in the same buckets
  (two buckets each, sub-second timestamp differences). Their pairwise
  Jaccard is 1.0 — they always co-occur.
- `unrelated.daily_backup_done` fires once and gets pruned by
  `min_support = 2`.
- `unrelated.cron_health_ok` fires three times in three different
  buckets (different from the failure cluster's two), so its Jaccard
  with the failure cluster is `0 / (3 + 2 - 0) = 0` — falls below the
  0.2 threshold, lands in its own cluster.

Output:

```
clusters: [
  { id: 0, support: 2, cohesion: 1.0, members: [
      "db.connection_pool_exhausted",
      "db.slow_query",
      "service.api.5xx_spike",
      "service.api.down",
      "service.api.error_rate_high",
  ]},
  { id: 1, support: 3, cohesion: 1.0, members: [
      "unrelated.cron_health_ok",
  ]},
]
```

The "service + db" cluster captures the entire incident in one row.

### Example B — causal ranking

Now invoke `RcaResult::analyze_failure("service.api.down", "1h", &cfg)`
on the same window. The pipeline computes lead-times relative to the
earliest `service.api.down` timestamp in each bucket:

```
First failure bucket (12:00:00–12:05:00):
  earliest service.api.down: 12:00:01

  db.slow_query                @ 12:00:00 → lead =  +1s
  db.connection_pool_exhausted @ 12:00:02 → lead = -1s
  service.api.error_rate_high  @ 12:00:03 → lead = -2s
  service.api.5xx_spike        @ 12:00:05 → lead = -4s

(similar pattern in the second failure bucket)
```

After averaging across both buckets:

| Candidate | avg_lead_secs | jaccard |
|---|---|---|
| `db.slow_query`                | +1.0  | 1.00 |
| `db.connection_pool_exhausted` | -1.0  | 1.00 |
| `service.api.error_rate_high`  | -2.0  | 1.00 |
| `service.api.5xx_spike`        | -4.0  | 1.00 |

The output, sorted by `avg_lead_secs` descending:

```
probable_causes: [
  { key: "db.slow_query", avg_lead_secs: 1.0, jaccard: 1.0, co_occurrence_count: 2 },
  { key: "db.connection_pool_exhausted", avg_lead_secs: -1.0, ... },
  { key: "service.api.error_rate_high",  avg_lead_secs: -2.0, ... },
  { key: "service.api.5xx_spike",        avg_lead_secs: -4.0, ... },
]
```

`db.slow_query` is the only positive-lead candidate — it's the leading
edge of the incident, not a consequence. Operationally, this is exactly
what you want surfaced first: the slow queries arrived before the API
went down, which means the next on-call action is to look at database
load.

The negative-lead entries are the cascading consequences. They share
perfect Jaccard with the failure (they always co-occur), but they
arrive *after* it — they're symptoms, not causes.

### Example C — disjoint windows

If the failure key never fires in the chosen window,
`probable_causes` is empty. The clustering output is unaffected — you
still get the co-occurrence picture of whatever did happen, which is
often useful as "negative space" context ("here's what *was* firing
when nothing failed").

---

## 10. Failure modes and edge cases

| Input | Behaviour |
|---|---|
| Empty window | `n_events = 0`, empty clusters, empty probable_causes |
| `failure_key` not observed | `probable_causes = []`; clusters still computed |
| Only telemetry records | `n_events = 0` after filtering; same as empty window |
| All events in one bucket | `cohesion = 1.0` for every cluster; no temporal information for ranking |
| `bucket_secs` larger than window | every event lands in the same bucket — see above |
| `bucket_secs` smaller than typical event timing jitter | events that fired "together" land in adjacent buckets; co-occurrence drops to 0; clusters fragment. **Tune `bucket_secs` to match your firing latency.** |
| `jaccard_threshold = 0.0` | every pair with any shared bucket gets unioned — one giant cluster |
| `jaccard_threshold = 1.0` | only pairs that always co-occur cluster — typically many singleton clusters |
| `max_keys` smaller than the number of event keys | least-frequent keys are dropped; the strongest signals always survive |
| Invalid humantime duration | returns `Err(...)` from `parse_window`; never panics |

The `.analyze*` methods return `Result<RcaResult>`; transient DB
errors propagate as `Err`. They never panic on user-supplied input.

---

## 11. References

- Jaccard, P. (1912). *The distribution of the flora in the alpine
  zone.* New Phytologist, 11(2), 37–50 — the original formulation of
  the Jaccard coefficient.
- Levandowsky, M., & Winter, D. (1971). *Distance between sets.* Nature,
  234(5323), 34–35 — the metric properties of the Jaccard distance
  `1 - J`.
- Tarjan, R. E. (1975). *Efficiency of a Good But Not Linear Set Union
  Algorithm.* Journal of the ACM, 22(2), 215–225 — the union-find with
  path compression and rank used by the cluster step.
- Tarjan, R. E., & van Leeuwen, J. (1984). *Worst-case analysis of set
  union algorithms.* Journal of the ACM, 31(2), 245–281.
- Hipp, J., Güntzer, U., & Nakhaeizadeh, G. (2000). *Algorithms for
  association rule mining — a general survey and comparison.* SIGKDD
  Explorations, 2(1), 58–64 — foundational treatment of co-occurrence
  in transactional data, including the bucketed-windows approach.
- Pearl, J. (2009). *Causality* (2nd ed.), Cambridge University Press —
  for the theoretical limits of inferring causation from temporal
  precedence alone, which is the standard caveat on RCA-style
  approaches.

## See also

- [`Documentation/tests/rca_test.md`](../tests/rca_test.md) —
  every test case and what it verifies for the telemetry-events RCA
  pipeline.
- [`Documentation/tests/shardsmanager_rca_templates_test.md`](../tests/shardsmanager_rca_templates_test.md)
  — the parallel test suite for the drain3-template variant
  (`bdslib::analysis::rca_templates`), which uses the same Jaccard +
  lead-time machinery on template UUIDs instead of event keys.
- [`Documentation/Algorithm/KNN.md`](KNN.md),
  [`Documentation/Algorithm/LSA.md`](LSA.md),
  [`Documentation/Algorithm/TEXTRANK.md`](TEXTRANK.md) — the text-based
  analysis algorithms. RCA Jaccard is the *temporal* analogue: it
  doesn't care what the events say, only when they fired together.
- `src/analysis/rca.rs` — the implementation itself, ~538 lines.
- `src/analysis/rca_templates.rs` — the same pipeline applied to
  drain3 template observations (template UUIDs replace event keys).
