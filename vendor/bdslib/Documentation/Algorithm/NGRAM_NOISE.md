# N-gram noise removal (`bdslib::analysis::ngram::ngram_remove_noise`)

`ngram_remove_noise(&[String]) -> serde_json::Value` separates a
log corpus into **kept** (signal) and **removed** (noise) by
classifying each line on the **commonness** of its n-grams. A line
whose phrases are heavily-repeated across the rest of the corpus
is contributing nothing new and is removed; a line whose phrases
have low corpus-wide commonness is preserved as signal.

This is the **dual** of `ngram_anomaly`: same pipeline, same n-gram
extraction, same document-frequency primitive — only the per-line
score is inverted (commonness instead of rarity) and the output
shape is "kept vs removed" instead of "anomalies".

This document covers:

1. [What problem n-gram noise removal solves](#1-what-problem-n-gram-noise-removal-solves)
2. [The classical denoising idea](#2-the-classical-denoising-idea)
3. [How bdslib implements it](#3-how-bdslib-implements-it)
4. [The full pipeline, step by step](#4-the-full-pipeline-step-by-step)
5. [Output contract](#5-output-contract)
6. [Configuration knobs](#6-configuration-knobs)
7. [Complexity and scaling](#7-complexity-and-scaling)
8. [Determinism guarantees](#8-determinism-guarantees)
9. [Worked examples](#9-worked-examples)
10. [Failure modes and edge cases](#10-failure-modes-and-edge-cases)
11. [References](#11-references)

---

## 1. What problem n-gram noise removal solves

You have a batch of log lines and you want a **denoised version**:
the same corpus with the boring-and-repetitive lines removed,
leaving only the lines that actually say something distinctive. You
do not want to summarise (that's TextRank / LSA), you do not want
to cluster (that's k-NN), and you do not want a list of anomalies
(that's `ngram_anomaly`). You want to **keep most lines** and just
strip out the high-volume background.

This is the operational reality of most log streams:

- A monitoring agent emits 1,000 heartbeat lines per minute.
- A health-check probe pings every endpoint every second.
- A debug-level log writes every state transition.

Mixed with this firehose are the lines you actually care about: the
errors, the threshold crossings, the alert firings. You want a tool
that says *"here's the signal — skip the rest"*.

`ngram_remove_noise` answers this with a single threshold:

- For each line, compute the **mean commonness** of its n-grams.
- Line scores ≥ `noise_threshold` ⇒ noise (removed).
- Everything else ⇒ kept.

Practical use cases:

- **Pre-summarisation cleanup** — feed the kept array into TextRank
  / LSA so the summary reflects the signal, not the noise.
- **Dashboard "interesting events" tile** — show the operator just
  the unusual lines, not the heartbeats.
- **LLM context shrinkage** — denoise an hour of logs before
  passing them to a chat assistant; the same retrieval becomes
  cheaper and more accurate.

---

## 2. The classical denoising idea

Two independent classical lines feed into this:

- **Stop-word removal** (Salton & Buckley 1988): build a dictionary
  of frequent-everywhere words, drop them before any further text
  analysis. Universal in classical IR.
- **Burst / repetition filtering** (Kleinberg 2003, *Bursty and
  Hierarchical Structure in Streams*): identify bursts of nearly-
  identical events, collapse them into representative summaries.
  The dual problem: identify the *non*-burst lines that don't fit
  the burst pattern.

`ngram_remove_noise` generalises stop-word removal from individual
words to **n-gram phrases**, and from a hard-coded dictionary to a
**corpus-derived frequency cut**. The corpus *is* the dictionary —
no manual list, no language assumption, no training set.

The intuition is statistical: in a typical operational log corpus,
the n-gram document-frequency distribution is heavy-tailed. A small
number of n-grams (the "stop-phrases" of this corpus) appear in
almost every line; the long tail consists of rare n-grams unique
to a handful of lines. Lines composed entirely of head n-grams are
boring; lines with at least some tail n-grams carry information.

The classification is dual to anomaly detection in a precise sense:
for any line, `commonness + rarity = 1` per n-gram (since rarity
is `1 - commonness`). So `mean commonness` and `mean rarity` move
in lockstep, and the two thresholds carve the same axis from
opposite ends.

---

## 3. How bdslib implements it

`ngram_remove_noise` shares its full pipeline with `ngram_anomaly`
— see [NGRAM_ANOMALY.md § 3](NGRAM_ANOMALY.md#3-how-bdslib-implements-it)
for the shared internals. The only differences are:

| Aspect | `ngram_anomaly` | `ngram_remove_noise` |
|---|---|---|
| Per-line score | mean of `(1 - df[g] / N)` | mean of `df[g] / N` |
| Threshold direction | flag *above* `anomaly_threshold` | remove *above* `noise_threshold` |
| Output arrays | single `anomalies` | dual `kept` + `removed` |
| Per-line explanation | `novel_ngrams` | none (the line itself is the explanation) |

The same configuration tuning (no stop-word filter, document
frequency over term frequency, sorted-key summation for
determinism) applies — see the anomaly doc for the shared
rationale.

The dual nature also means the two endpoints can be chained: pass
your corpus through `ngram_remove_noise` to get the kept array,
then feed that into `ngram_anomaly` to find the anomalies *within
the signal*. This double pass is sometimes useful for very noisy
streams where the raw anomaly detector would be confused by the
noise floor.

---

## 4. The full pipeline, step by step

Given an input slice `logs: &[String]` of length `n_logs` and an
`NgramNoiseConfig`:

### Step 1 — Tokenise

Identical to `ngram_anomaly`: lowercase alphanumeric runs, drop
tokens shorter than `cfg.min_word_len`, no stop-word filter.

### Step 2 — Build n-grams

Identical: sliding window of length `n` joined by spaces; lines
too short for the window produce empty bags.

### Step 3 — Build the document-frequency table

Identical: `df[g]` = number of lines containing the n-gram `g`.

### Step 4 — Per-line commonness score

For each line, compute the **mean commonness** of its n-grams:

```rust
let mut commonness = vec![0.0f32; n_logs];
for (i, bag) in bags.iter().enumerate() {
    if bag.is_empty() { continue; }
    let mut sorted_grams: Vec<&str> = bag.iter().map(String::as_str).collect();
    sorted_grams.sort_unstable();    // determinism
    let mut sum = 0.0f32;
    for g in &sorted_grams {
        let f = *df.get(*g).unwrap_or(&1) as f32 / total_lines;
        sum += f;                    // commonness contribution
    }
    commonness[i] = sum / sorted_grams.len() as f32;
}
```

This is `per_line_score(invert=false)` in the implementation —
literally the same function as the anomaly path, with a single
boolean flag flipped.

Lines with no n-grams keep `commonness = 0` — they cannot be
classified as noise (we have no basis to remove them).

### Step 5 — Classify

Two-bucket partition:

```rust
let mut kept_idx:    Vec<usize> = Vec::new();
let mut removed_idx: Vec<usize> = Vec::new();
for i in 0..n_logs {
    if !bags[i].is_empty() && commonness[i] >= cfg.noise_threshold {
        removed_idx.push(i);
    } else {
        kept_idx.push(i);
    }
}
```

Two important behaviours:

- **Empty-bag lines are kept.** A line too short to produce any
  n-gram cannot be argued away as repetitive; the conservative
  choice is to keep it.
- **Equality with the threshold counts as noise.** `>=` not `>`.
  Operationally this matches user intuition ("threshold 0.85
  removes anything 85%+ common") and makes the threshold sweep
  from 0 → 1 monotonically widen the kept set.

### Step 6 — Sort and cap for the output

- **Kept array preserves input order.** This is the key UX choice:
  the kept array can be read sequentially as the denoised corpus,
  in the same order the original lines arrived.
- **Removed array is sorted by commonness descending**, with
  index ascending as the deterministic tiebreaker. The most
  noise-like lines appear first, so a UI showing only the top N
  removed lines shows what was *most aggressively* removed.

```rust
let true_n_kept    = kept_idx.len();
let true_n_removed = removed_idx.len();

kept_idx.truncate(cfg.max_kept);
removed_idx.sort_by(|&a, &b| {
    commonness[b].partial_cmp(&commonness[a])
        .unwrap_or(Ordering::Equal)
        .then(a.cmp(&b))
});
removed_idx.truncate(cfg.max_removed);
```

### Step 7 — Assemble the JSON

```rust
json!({
    "n_logs":          n_logs,
    "n":               n,
    "n_unique_ngrams": df.len(),
    "noise_threshold": cfg.noise_threshold,
    "n_kept":          true_n_kept,
    "n_removed":       true_n_removed,
    "kept":            kept_json,
    "removed":         removed_json,
})
```

Both `n_kept` and `n_removed` reflect the **true** counts. The
arrays themselves are bounded by `cfg.max_kept` / `cfg.max_removed`
so the JSON stays compact for very large corpora.

---

## 5. Output contract

```json
{
  "n_logs":          120,
  "n":               2,
  "n_unique_ngrams": 543,
  "noise_threshold": 0.85,
  "n_kept":          18,
  "n_removed":       102,
  "kept": [
    { "idx": 4,   "text": "ALERT memory pressure on node5",   "commonness": 0.21 },
    { "idx": 17,  "text": "ALERT disk failure detected",      "commonness": 0.18 }
  ],
  "removed": [
    { "idx": 0,   "text": "heartbeat ok node1 status nominal", "commonness": 0.91 },
    { "idx": 1,   "text": "heartbeat ok node2 status nominal", "commonness": 0.91 }
  ]
}
```

Field semantics:

| Field | Type | Description |
|---|---|---|
| `n_logs` | integer | Size of the input corpus |
| `n` | integer | Effective n-gram length (clamped to ≥ 1) |
| `n_unique_ngrams` | integer | Distinct n-gram count across the corpus |
| `noise_threshold` | number | Echoed from request |
| `n_kept` | integer | True total of kept lines (not capped) |
| `n_removed` | integer | True total of removed lines (not capped) |
| `kept[].idx` | integer | Index back into the input slice |
| `kept[].text` | string | The line itself |
| `kept[].commonness` | number | This line's commonness in `[0, 1]` |
| `removed[]` | (same fields as `kept[]`) | Lines classified as noise |

`n_kept + n_removed == n_logs` for every output (every line is in
exactly one bucket — see the test
`noise_kept_plus_removed_equals_n_logs`).

The `kept` array preserves input order; the `removed` array is
sorted by commonness descending.

---

## 6. Configuration knobs

```rust
pub struct NgramNoiseConfig {
    pub n:               usize,    // default: 2
    pub min_word_len:    usize,    // default: 2
    pub noise_threshold: f32,      // default: 0.85
    pub max_kept:        usize,    // default: 100
    pub max_removed:     usize,    // default: 100
}
```

| Knob | Effect |
|---|---|
| **`n`** | Same as the anomaly endpoint — `n=2` for general use, `n=3` for highly templated corpora where bigrams are too coarse, `n=1` to fall back to per-token frequency filtering (essentially "dynamic stop-word removal"). |
| **`min_word_len`** | Same — drops short tokens (digits, single-char identifiers) before n-gram construction. |
| **`noise_threshold`** | Higher ⇒ less aggressive denoising; only lines made of *very* repetitive n-grams are removed. The default `0.85` is intentionally strict — it removes only lines whose n-grams are present in 85%+ of the corpus on average. Lower to `0.5–0.7` for more aggressive denoising on heterogeneous corpora; raise to `0.95` to remove only the most blatant heartbeat-style traffic. |
| **`max_kept`** | JSON cap on `kept`. Default `100` is a reasonable display-tile cap. Set higher (or to `usize::MAX`) when you want the full denoised corpus to feed downstream. |
| **`max_removed`** | Same idea for `removed` — lets you sample the top-N noise patterns without bloating the JSON. |

---

## 7. Complexity and scaling

Identical to `ngram_anomaly` — see
[NGRAM_ANOMALY.md § 7](NGRAM_ANOMALY.md#7-complexity-and-scaling)
for the full table. The pipeline is linear in corpus size; the
only super-linear factor is per-line key sorting for determinism,
giving an overall `O(n_logs · L log L)` bound where `L` is the
average tokenised line length.

The two endpoints differ in cost only by a constant factor (a
single `1.0 -` vs no operation per n-gram). In practice you can
treat them as equivalent for sizing purposes.

---

## 8. Determinism guarantees

`ngram_remove_noise` is **byte-for-byte deterministic** given the
same `&[String]` input. The implementation guarantees this by:

1. Sorting n-gram keys before each per-line summation (eliminates
   HashMap-iteration nondeterminism in the floating-point sum).
2. Stable sort everywhere; index-ascending tiebreaker on the
   removed-array sort, so two lines with identical commonness
   always come out in the same order.
3. Kept-array order is the input order — trivially deterministic
   given a deterministic input.

Two processes loading the same corpus and config produce identical
JSON.

---

## 9. Worked examples

### Example A — heartbeat denoising

Input (10 lines):

```
heartbeat ok node1 status nominal
heartbeat ok node2 status nominal
heartbeat ok node3 status nominal
heartbeat ok node4 status nominal
heartbeat ok node5 status nominal
heartbeat ok node6 status nominal
heartbeat ok node7 status nominal
heartbeat ok node8 status nominal
ALERT memory pressure on node5 swap usage critical
ALERT disk failure detected on storage subsystem
```

The 8 heartbeat lines share three high-frequency bigrams:
`"heartbeat ok"`, `"status nominal"`, and `"ok node*"` (8 distinct
"ok node*" bigrams, each df=1/10). Their other shared bigrams have
df=8/10 → contribution 0.8 to commonness. So the heartbeat lines
have commonness ≈ 0.45 (mix of the 0.8 shared bigrams and the
0.1 unique node-id bigrams).

The two ALERT lines share only one common bigram (`"on node5"` is
in one of them and one heartbeat line). Most of their bigrams are
df=1/10. Commonness ≈ 0.10.

With the default threshold `0.85`, *nothing* is removed (heartbeat
commonness 0.45 < 0.85). To match the operational intent ("remove
the heartbeats"), use `noise_threshold = 0.4`:

```rust
let cfg = NgramNoiseConfig { noise_threshold: 0.4, ..NgramNoiseConfig::default() };
```

With this, all 8 heartbeats are classified as noise; the 2 ALERTs
are kept. Output:

```json
{ "n_kept": 2, "n_removed": 8, "kept": [ALERTs], "removed": [heartbeats] }
```

This is a recurring tuning point: the default 0.85 is **strict by
design** to avoid surprising removals on small or low-redundancy
corpora. For aggressive denoising of high-redundancy operational
streams, lower the threshold to match the actual noise commonness
of your stream (typically 0.3–0.6).

### Example B — chained with summarisation

A "denoise then summarise" pipeline:

```rust
let raw_logs: Vec<String> = fetch_recent_logs("1h");

// Step 1: denoise.
let noise_result = ngram_remove_noise_with(&raw_logs,
    &NgramNoiseConfig { noise_threshold: 0.5, ..Default::default() });

let kept: Vec<String> = noise_result["kept"]
    .as_array().unwrap()
    .iter()
    .map(|v| v["text"].as_str().unwrap().to_owned())
    .collect();

// Step 2: summarise the kept lines with TextRank.
let summary = textrank_summary(&kept);
```

The TextRank summary now reflects the signal lines, uncluttered by
the heartbeat noise. This is the canonical use case for the dual
endpoints: noise removal as a pre-processing step for downstream
NLP.

### Example C — duality demonstration

The two endpoints score the same line on opposite axes. For any
line: `mean rarity + mean commonness = 1`. So:

```rust
let logs = vec![
    "ping ok ping ok ping ok",
    "ping ok ping ok ping ok",
    "ping ok ping ok ping ok",
    "ping ok ping ok ping ok",
    "rare distinct unique line",
];
let anomaly = ngram_anomaly_with(&logs,
    &NgramAnomalyConfig { anomaly_threshold: 0.5, ..Default::default() });
let noise = ngram_remove_noise_with(&logs,
    &NgramNoiseConfig { noise_threshold: 0.5, ..Default::default() });
```

The results:

- `anomaly.anomalies` — one entry: `"rare distinct unique line"`,
  rarity ~0.8.
- `noise.kept` — one entry: `"rare distinct unique line"`,
  commonness ~0.2.
- `noise.removed` — four entries: the four `"ping ok ..."` lines,
  commonness ~0.8.

The unique line surfaces in `anomalies` *and* survives the
noise-removal cut. This is the operational definition of "the
signal in this corpus": a line that both endpoints agree is
distinctive.

---

## 10. Failure modes and edge cases

| Input | Behaviour |
|---|---|
| `[]` | Returns the documented empty shape (`n_logs=0`, empty arrays). |
| `[one_line]` | Single line: every n-gram has df=1/1=1.0 → commonness=1.0 → classified as noise. This is technically correct (the line is "100% repetitive of itself") but operationally vacuous; one-line corpora aren't a useful input shape. |
| All-stop-tokens / sub-`min_word_len` lines | Empty token bags → empty n-gram bags → all kept (cannot remove what we cannot score). |
| Lines too short for `n` | Cannot be classified as noise; land in `kept`. |
| `cfg.n = 0` | Internally bumped to 1. |
| `cfg.noise_threshold ≥ 1.0` | Almost nothing removed (only lines whose n-grams are *all* in every line). |
| `cfg.noise_threshold ≤ 0.0` | Every line with at least one n-gram removed; lines too short for an n-gram still kept. |
| Identical duplicate lines | Each line's n-grams have very high df → commonness near 1.0 → all removed. |
| Corpus with no repetition at all (every line uniquely worded) | Every n-gram has df=1/N (low) → low commonness → nothing removed. |
| Very large `n_logs` | Document-frequency map memory dominates; raise `min_word_len` to shrink the vocabulary. |

The function never panics on user-supplied input.

---

## 11. References

- Salton, G., & Buckley, C. (1988). *Term-weighting approaches in
  automatic text retrieval.* Information Processing & Management,
  24(5), 513–523 — the original treatment of stop-word removal as
  a frequency-based pre-processing step in IR. `ngram_remove_noise`
  generalises this to phrase-level frequency.
- Kleinberg, J. (2003). *Bursty and Hierarchical Structure in
  Streams.* Data Mining and Knowledge Discovery, 7(4), 373–397 —
  on identifying repetitive bursts in event streams; the
  complementary problem of "find the bursts" rather than "remove
  them".
- Manning, C. D., Raghavan, P., & Schütze, H. (2008). *Introduction
  to Information Retrieval.* Cambridge University Press — chapter
  6 (term weighting) and chapter 2 (the document-frequency primitive
  and its threshold-based usage).
- Yang, Y., & Pedersen, J. O. (1997). *A comparative study on
  feature selection in text categorization.* Proceedings of ICML
  '97 — empirical study of `df` thresholding as a feature-selection
  technique, the closest classical analog to this denoising
  approach.
- Forrest, S. et al. (1996). *A Sense of Self for Unix Processes.*
  IEEE Symposium on Security and Privacy — for the dual problem
  (anomaly via *low*-frequency n-grams), the original sequence-
  based anomaly detection paper. See the companion document
  [NGRAM_ANOMALY.md](NGRAM_ANOMALY.md).

## See also

- [`Documentation/Algorithm/NGRAM_ANOMALY.md`](NGRAM_ANOMALY.md) —
  the dual endpoint: same pipeline, scored by rarity, used to
  highlight anomalies rather than remove noise.
- [`Documentation/Algorithm/TEXTRANK.md`](TEXTRANK.md),
  [`Documentation/Algorithm/LSA.md`](LSA.md) — extractive
  summarisers that benefit from `ngram_remove_noise` as a
  pre-processing step on noisy operational streams.
- [`Documentation/tests/ngram_test.md`](../tests/ngram_test.md) —
  every test case and what it verifies.
- [`Documentation/examples/ngram_demo.md`](../examples/ngram_demo.md)
  — runnable demo walkthrough.
- `src/analysis/ngram.rs` — the implementation itself; both
  endpoints share an internal pipeline.
