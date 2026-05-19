# N-gram anomaly detection (`bdslib::analysis::ngram::ngram_anomaly`)

`ngram_anomaly(&[String]) -> serde_json::Value` flags log lines whose
n-gram phrases are **statistically rare** in the corpus they belong
to. It's an unsupervised anomaly detector that runs on the same
input shape as `bdslib`'s other text-analysis algorithms (TextRank,
LSA, k-NN), but uses a fundamentally different signal: **phrase
structure** rather than word distributions or pairwise similarity.

This document covers:

1. [What problem n-gram anomaly detection solves](#1-what-problem-n-gram-anomaly-detection-solves)
2. [The classical n-gram anomaly idea](#2-the-classical-n-gram-anomaly-idea)
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

## 1. What problem n-gram anomaly detection solves

You have a batch of log lines and you want to know: *"which of these
lines is unusual?"* — without supervised training data, without an
explicit list of what counts as normal, and without committing to a
similarity metric over whole lines (which is what k-NN does).

**N-gram-based anomaly detection** answers this by looking at
sub-line phrase structure:

- A typical operational log line is a *template* with parameters:
  `"connection refused on port 5432"`, `"GET /api/v1/users 200 4ms"`.
- Templates repeat. Their constituent phrases (`"connection refused"`,
  `"on port"`, `"GET /api"`) appear in many lines.
- A genuinely unusual line uses **phrases that almost no other line
  uses**: `"unicorn detected"`, `"backup completed"`, `"manual
  intervention required"`.

Build the corpus-wide distribution of n-gram frequencies, then score
each line by how rare its n-grams are on average. High mean rarity ⇒
the line is using novel phrasing ⇒ candidate anomaly.

This complements k-NN's vocabulary-overlap-based anomaly detection.
A line can have a perfectly normal vocabulary (every word appears in
many other lines) and still be anomalous because of how the words
are *combined*. N-gram anomaly detection catches that; k-NN doesn't.

Practical use cases:

- **Operator triage** — "show me the 5 weirdest log lines from the
  last hour."
- **Drift detection** — when a service starts emitting a new template
  that wasn't in yesterday's corpus, its first few firings score as
  anomalies before the new template becomes baseline.
- **Outlier filtering** — exclude the rare lines from downstream
  summarisation so the summary reflects the typical traffic, not the
  edge cases.

---

## 2. The classical n-gram anomaly idea

The roots are in language modelling and intrusion detection:

- **Language modelling** (Shannon 1948, Markov chains): a sequence
  of tokens is "normal" if its n-gram transitions have high
  probability under a fitted model. Surprising sequences ⇒ low
  probability ⇒ unusual.
- **Sequence-anomaly intrusion detection** (Forrest et al. 1996,
  Stide system call sequences): build a database of "normal" n-grams
  from training data; flag any process whose call sequence contains
  too many out-of-database n-grams.
- **Statistical NLP** (Manning & Schütze 1999, ch. 6): smoothed
  n-gram language models with Laplace / Kneser-Ney smoothing for
  out-of-vocabulary handling.

bdslib's adaptation collapses the language-modelling machinery into
something much simpler:

- No probability estimation, no smoothing, no held-out test corpus.
- The n-gram **document frequency** (how many lines contain this
  n-gram) is the entire model.
- Per-line score is the mean of `(1 - df[g] / N)` across the line's
  n-grams.

The trade-off: the simple form gives no probability calibration —
rarity scores are not log-probabilities — but it is interpretable
("this line uses 5 phrases that fewer than 1% of other lines use"),
fast to compute (single pass + sort), and requires no training
corpus separate from the input batch.

---

## 3. How bdslib implements it

The implementation lives in `src/analysis/ngram.rs` alongside the
dual `ngram_remove_noise` endpoint. Both share an internal pipeline:

| Stage | Output | Used by |
|---|---|---|
| `tokenize` | `Vec<String>` of lowercase alphanumeric tokens | both |
| `build_ngrams` | sliding window of length `n` joined by spaces | both |
| `build_doc_frequency` | `HashMap<gram, usize>` of per-line counts | both |
| `per_line_score(invert=true)` | Vec<f32> of mean `(1 - df/N)` | anomaly |
| `per_line_score(invert=false)` | Vec<f32> of mean `df/N` | noise removal |

After scoring, anomaly detection thresholds the rarity vector,
sorts the matches, picks the top-K rarest n-grams per anomaly to
explain *why* the line was flagged, and returns the JSON.

Three implementation details matter:

1. **No stop-word filtering.** Unlike LSA/TextRank/k-NN, this module
   keeps all alphanumeric tokens (subject only to `min_word_len`).
   N-grams derive their signal from phrase structure, and
   stop-word phrases (`"the system"`, `"is the"`) carry meaningful
   template information.

2. **Document frequency, not term frequency.** A line that fires a
   given n-gram three times only counts as one document for that
   n-gram. This prevents long lines from artificially inflating the
   commonness of their n-grams.

3. **Sorted-key summation for determinism.** HashMap iteration is
   randomised by Rust's default `RandomState` and floating-point
   addition isn't associative, so per-line scores would otherwise
   vary in the last bit between runs and perturb the sort order at
   the threshold boundary. Sorting n-gram keys before summation makes
   the output byte-for-byte reproducible.

---

## 4. The full pipeline, step by step

Given an input slice `logs: &[String]` of length `n_logs` and an
`NgramAnomalyConfig`:

### Step 1 — Tokenise

Each input is normalised to lowercase, split on non-alphanumeric
boundaries, and filtered to drop tokens shorter than
`cfg.min_word_len`. **No stop-word list applies.**

```rust
fn tokenize(s: &str, min_word_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if !cur.is_empty() {
            if cur.len() >= min_word_len {
                out.push(cur.clone());
            }
            cur.clear();
        }
    };
    for c in s.chars() {
        if c.is_alphanumeric() {
            for lc in c.to_lowercase() { cur.push(lc); }
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}
```

Result: a `Vec<Vec<String>>` of one token sequence per input line.

### Step 2 — Build n-grams

For each token sequence, slide a window of length `n` across to
produce n-grams. Each n-gram is the windowed tokens joined by a
single space. Lines too short for the window (`tokens.len() < n`)
produce an **empty** n-gram bag.

```rust
fn build_ngrams(tokens: &[String], n: usize) -> Vec<String> {
    if tokens.len() < n { return Vec::new(); }
    let mut out = Vec::with_capacity(tokens.len() - n + 1);
    for i in 0..=(tokens.len() - n) {
        out.push(tokens[i..i + n].join(" "));
    }
    out
}
```

Why a space separator? Because tokens are pure alphanumeric runs
post-tokenisation, the space cannot collide with any token character
— so the joined string `"alpha beta"` is unambiguously the bigram
of `["alpha", "beta"]`.

### Step 3 — Build the document-frequency table

For each n-gram in the corpus, count **how many distinct lines
contain it**. A line counts at most once per n-gram — multiple
occurrences within the same line do not amplify the count.

```rust
fn build_doc_frequency(bags: &[Vec<String>]) -> HashMap<String, usize> {
    let mut df: HashMap<String, usize> = HashMap::new();
    for bag in bags {
        let mut seen: HashSet<&str> = HashSet::new();
        for g in bag {
            if seen.insert(g.as_str()) {
                *df.entry(g.clone()).or_insert(0) += 1;
            }
        }
    }
    df
}
```

Result: `df[g]` = number of lines containing the n-gram `g`.

### Step 4 — Per-line rarity score

For each line, compute `rarity[i] = mean(1 - df[g] / N)` across all
the line's n-grams (with `N = n_logs`). Higher means more uses of
n-grams that few other lines have.

```rust
let mut score = vec![0.0f32; n_logs];
for (i, bag) in bags.iter().enumerate() {
    if bag.is_empty() { continue; }
    let mut sorted_grams: Vec<&str> = bag.iter().map(String::as_str).collect();
    sorted_grams.sort_unstable();    // determinism
    let mut sum = 0.0f32;
    for g in &sorted_grams {
        let f = *df.get(*g).unwrap_or(&1) as f32 / total_lines;
        sum += 1.0 - f;              // rarity contribution
    }
    score[i] = sum / sorted_grams.len() as f32;
}
```

Lines with no n-grams (too short for the configured `n`) keep
`rarity = 0` — they carry no signal and cannot be anomalies.

### Step 5 — Threshold and sort

Pick lines whose rarity meets or exceeds `cfg.anomaly_threshold` AND
have at least one n-gram. Sort by rarity descending, with index
ascending as the deterministic tiebreaker.

```rust
let mut anomaly_idx: Vec<usize> = (0..n_logs)
    .filter(|&i| !bags[i].is_empty() && rarity[i] >= cfg.anomaly_threshold)
    .collect();
anomaly_idx.sort_by(|&a, &b| {
    rarity[b].partial_cmp(&rarity[a])
        .unwrap_or(std::cmp::Ordering::Equal)
        .then(a.cmp(&b))
});
let true_n_anomalies = anomaly_idx.len();
anomaly_idx.truncate(cfg.max_anomalies);
```

`true_n_anomalies` is reported back as `n_anomalies` in the JSON
even when the array itself is capped — callers always know the true
total.

### Step 6 — Render anomalies with explanatory novel n-grams

For each surfaced anomaly, list its top-K rarest **distinct**
n-grams (sorted by ascending document frequency, ties broken
alphabetically for reproducibility). This gives an operator a
direct answer to "why was this line flagged?":

```rust
let mut grams: Vec<(&str, f32)> = bags[i].iter()
    .map(|g| {
        let f = *df.get(g.as_str()).unwrap_or(&1) as f32 / total_lines;
        (g.as_str(), f)
    })
    .collect();
grams.sort_by(|a, b| {
    a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal)
        .then_with(|| a.0.cmp(b.0))
});

let mut seen = HashSet::new();
let novel: Vec<&str> = grams.iter()
    .filter_map(|(g, _)| if seen.insert(*g) { Some(*g) } else { None })
    .take(cfg.max_novel_ngrams)
    .collect();
```

### Step 7 — Assemble the JSON

```rust
json!({
    "n_logs":            n_logs,
    "n":                 n,
    "n_unique_ngrams":   df.len(),
    "anomaly_threshold": cfg.anomaly_threshold,
    "n_anomalies":       true_n_anomalies,
    "mean_rarity":       mean_rarity,
    "anomalies":         anomalies_json,
})
```

`mean_rarity` is reported as informational context — it lets a
caller see "is this corpus generally noisy or generally consistent?"
at a glance.

---

## 5. Output contract

```json
{
  "n_logs":            120,
  "n":                 2,
  "n_unique_ngrams":   543,
  "anomaly_threshold": 0.7,
  "n_anomalies":       7,
  "mean_rarity":       0.41,
  "anomalies": [
    {
      "idx":          84,
      "text":         "manual intervention required for stuck queue depth",
      "rarity":       0.93,
      "novel_ngrams": ["manual intervention", "intervention required", "stuck queue", "queue depth"]
    }
  ]
}
```

Field semantics:

| Field | Type | Description |
|---|---|---|
| `n_logs` | integer | Size of the input corpus |
| `n` | integer | The effective n-gram length (clamped to ≥ 1) |
| `n_unique_ngrams` | integer | Number of distinct n-grams in the corpus |
| `anomaly_threshold` | number | Echoed from request |
| `n_anomalies` | integer | True total of anomalies (not capped by `max_anomalies`) |
| `mean_rarity` | number | Informational — average rarity across non-empty lines |
| `anomalies[].idx` | integer | Index back into the input slice |
| `anomalies[].text` | string | The line itself |
| `anomalies[].rarity` | number | This line's rarity score in `[0, 1]` |
| `anomalies[].novel_ngrams` | array of string | Top-K rarest distinct n-grams from this line, ordered most-rare-first |

The `anomalies` array is bounded by `max_anomalies`; the true count
is always in `n_anomalies`. The `novel_ngrams` array is bounded by
`max_novel_ngrams` per anomaly.

---

## 6. Configuration knobs

```rust
pub struct NgramAnomalyConfig {
    pub n:                 usize,    // default: 2
    pub min_word_len:      usize,    // default: 2
    pub anomaly_threshold: f32,      // default: 0.7
    pub max_anomalies:     usize,    // default: 20
    pub max_novel_ngrams:  usize,    // default: 5
}
```

| Knob | Effect |
|---|---|
| **`n`** | Higher captures richer phrase structure (a trigram like `"out of memory"` is more discriminating than `"out of"`) but generates more *unique* n-grams per line, inflating per-line rarity even for normal text. `n=2` is the robust default. `n=1` reduces to rare-word detection (similar to TF-IDF outliers). `n=3` is good when most lines are highly templated. |
| **`min_word_len`** | Drops digits and short identifiers from tokenisation. `min_word_len=2` filters single-char tokens (most numeric ids); `min_word_len=3` filters short codes (`200`, `404`) too. |
| **`anomaly_threshold`** | Higher ⇒ stricter; only the most strikingly-novel lines are flagged. Default `0.7` works well on operational corpora where ~10–30% of n-grams are typically rare. |
| **`max_anomalies`** | JSON-array cap. Set higher for full-corpus dumps; lower for dashboard tiles. |
| **`max_novel_ngrams`** | Per-anomaly explanatory cap. `5` fits comfortably in a UI tile; `10+` for offline analysis where you want to see all the unusual phrases at a glance. |

---

## 7. Complexity and scaling

For `n_logs` lines with average tokenised length `L`:

| Phase | Cost |
|---|---|
| Tokenise | `O(n_logs · L)` |
| Build n-grams | `O(n_logs · L)` |
| Document-frequency build | `O(n_logs · L)` (each n-gram inserted once per line) |
| Per-line score | `O(n_logs · L · log L)` (sort step per line for determinism) |
| Filter + sort + render | `O(n_logs log n_logs)` |

The pipeline is **linear in corpus size** (the per-line sort
contributes the only super-linear factor and is on a per-line basis,
so the bound is `O(n_logs · L log L)`). On commodity hardware,
processing 10⁵ short log lines with `n=2` takes well under a second.

Memory:

| Structure | Size |
|---|---|
| Token bags | `O(n_logs · L)` |
| N-gram bags | `O(n_logs · L)` (almost the same — `L - n + 1 ≈ L`) |
| Document-frequency map | `O(unique_ngrams)` keys |
| Per-line score vector | `O(n_logs)` `f32` |

For very large corpora the document-frequency map dominates memory
because every unique n-gram is a heap-allocated `String`. If memory
is tight, raise `min_word_len` to shrink the vocabulary or use
`n=2` (smaller than `n=3`) to limit the unique-n-gram count.

---

## 8. Determinism guarantees

`ngram_anomaly` is **byte-for-byte deterministic** given the same
`&[String]` input. The implementation guarantees this by:

1. Sorting n-gram keys before each per-line summation. HashMap
   iteration is randomised by Rust's `RandomState`, and
   floating-point addition isn't associative, so unsorted summation
   would produce ULP-different scores across runs.
2. Sorting the global mean-rarity input deterministically before
   summing.
3. Stable sort everywhere — Rust's `sort_by` preserves insertion
   order for ties.
4. Index-ascending tiebreaker on the anomaly sort, so two lines with
   identical rarity always come out in the same order.
5. Alphabetical tiebreaker on the per-anomaly novel-n-grams sort,
   so the explanatory list is reproducible.

Two processes loading the same corpus and config produce identical
JSON, modulo `serde_json`'s map-key ordering (which is itself
deterministic in `serde_json` ≥ 1).

---

## 9. Worked examples

### Example A — outlier in a homogeneous error stream

Input:

```
ERROR upstream timeout service auth code 503
ERROR upstream timeout service billing code 503
ERROR upstream timeout service catalog code 503
ERROR upstream timeout service payment code 503
ERROR upstream timeout service auth code 503
INFO scheduled backup completed at 03:00 utc
```

With default `n=2` and threshold `0.7`:

- The 5 ERROR lines share the bigrams `"error upstream"`,
  `"upstream timeout"`, `"timeout service"`, `"service auth/billing/...", "code 503"`. Most of these have df=5/6.
- The INFO line has bigrams `"info scheduled"`, `"scheduled backup"`,
  `"backup completed"`, `"completed at"`, `"at 03"`, `"03 00"`,
  `"00 utc"` — every one with df=1/6.

Per-line rarities:
- ERROR lines: ~0.30 (most n-grams have df=5/6, contributing only
  0.17 to rarity each).
- INFO line: ~0.83 (every n-gram contributes ~1.0 - 0.17 = 0.83).

The INFO line clears the 0.7 threshold and is the only anomaly,
with explanatory `novel_ngrams = ["00 utc", "03 00", "at 03",
"backup completed", "completed at"]` (sorted by ascending df, ties
broken alphabetically).

This is the canonical "find the weird line in a sea of normal
errors" use case.

### Example B — emerging template (drift detection)

Suppose at hour 1 your stream contains 1000 instances of three
templates. At hour 2 a fourth template starts firing — first 5
instances, then 50, then becoming dominant.

- **First 5 firings of the new template** — its n-grams have
  df=5/N where N includes the 1000 baseline lines. Rarity ≈ 1.0.
  All 5 surface as anomalies.
- **At 50 firings** — df=50/(1000+50) ≈ 0.05, rarity ≈ 0.95.
  Still surface, but no longer the most novel.
- **At 500 firings** — df=500/1500 ≈ 0.33, rarity ≈ 0.67. Below
  the 0.7 threshold; the template is now considered baseline.

So `ngram_anomaly` automatically self-tunes to the corpus: a new
template stops being anomalous once it becomes common enough to be
"the new normal". This is the right behaviour for dashboards that
should foreground genuinely unusual events without becoming noisy.

### Example C — n=2 vs n=3 on near-duplicate lines

Input:

```
alpha beta gamma delta epsilon
alpha beta gamma delta epsilon
alpha beta gamma delta zeta       ← differs only in the last token
alpha beta gamma delta epsilon
alpha beta gamma delta eta        ← differs only in the last token
```

With `n=2` (default), the bigrams are mostly shared
(`"alpha beta"`, `"beta gamma"`, `"gamma delta"` appear in every
line). The two unique-tail lines have one novel bigram each
(`"delta zeta"`, `"delta eta"`) — diluted by 4 shared bigrams,
their rarity is ~0.18. Both fall well below 0.7. **Zero anomalies.**

With `n=3`, the trigrams `"gamma delta zeta"` and `"gamma delta eta"`
are unique (df=1/5 each). The "differing-tail" lines have 3
trigrams: `"alpha beta gamma"` (df=5/5), `"beta gamma delta"`
(df=5/5), and the unique tail trigram (df=1/5). Their rarity
becomes (0 + 0 + 0.8) / 3 ≈ 0.27. Lower threshold (0.0) surfaces
them.

`n=3` catches differences that `n=2` smooths over because longer
n-grams give finer-grained phrase identity. Use it when your
templates are mostly-identical-with-rare-tail-changes (typical of
log lines parameterised on a single field).

---

## 10. Failure modes and edge cases

| Input | Behaviour |
|---|---|
| `[]` | Returns the documented empty shape (`n_logs=0`, empty arrays). |
| `[one_line]` | Single line cannot be compared against anything; `mean_rarity` is well-defined but the anomaly threshold would only flag against an empty distribution. In practice the line is its own corpus and df=1 for every n-gram → rarity=0 → no anomalies. |
| All-stop-tokens / sub-`min_word_len` lines | Empty token bags → empty n-gram bags → all `rarity=0` → no anomalies. |
| Lines too short for `n` | The line is excluded from anomaly classification (cannot be an anomaly without n-grams). It still counts toward `n_logs`. |
| `cfg.n = 0` | Internally bumped to 1. |
| `cfg.anomaly_threshold ≥ 1.0` | Only impossible lines (rarity > 1) flag → effectively zero anomalies. |
| `cfg.anomaly_threshold ≤ 0.0` | Every line with at least one n-gram surfaces as an anomaly. |
| Identical duplicate lines | Each line's n-grams have df = number-of-duplicates / n_logs, often very high → rarity near 0 → no anomalies. Duplicates are not anomalous. |
| Very large `n_logs` | Document-frequency map memory is the bottleneck. Beyond a few million lines, raise `min_word_len` to shrink the vocabulary. |

The function never panics on user-supplied input.

---

## 11. References

- Shannon, C. E. (1948). *A Mathematical Theory of Communication.*
  Bell System Technical Journal, 27(3), 379–423 — the original
  treatment of n-gram language models and information measures.
- Forrest, S., Hofmeyr, S. A., Somayaji, A., & Longstaff, T. A.
  (1996). *A Sense of Self for Unix Processes.* IEEE Symposium on
  Security and Privacy — the Stide system call sequence anomaly
  detector, the canonical "anomalous if its n-grams aren't in the
  baseline" formulation.
- Manning, C. D., & Schütze, H. (1999). *Foundations of Statistical
  Natural Language Processing.* MIT Press — chapter 6 (n-gram
  language models with smoothing) is the textbook reference.
- Salton, G., & Buckley, C. (1988). *Term-weighting approaches in
  automatic text retrieval.* Information Processing & Management,
  24(5), 513–523 — the document-frequency primitive used here in
  its un-smoothed `df / N` form.
- Chandola, V., Banerjee, A., & Kumar, V. (2009). *Anomaly Detection:
  A Survey.* ACM Computing Surveys, 41(3) — surveys the family of
  unsupervised anomaly detectors that this implementation belongs
  to (sequence-based, frequency-based, classification-free).

## See also

- [`Documentation/Algorithm/NGRAM_NOISE.md`](NGRAM_NOISE.md) —
  the dual endpoint: same pipeline, scored by commonness instead of
  rarity, used to *remove* noise rather than highlight anomalies.
- [`Documentation/Algorithm/KNN.md`](KNN.md) — the alternative
  anomaly detector based on cosine similarity over TF-IDF vectors.
  k-NN catches **vocabulary-disjoint** outliers; n-gram catches
  **phrase-structure** outliers. They complement each other.
- [`Documentation/tests/ngram_test.md`](../tests/ngram_test.md) —
  every test case and what it verifies.
- [`Documentation/examples/ngram_demo.md`](../examples/ngram_demo.md)
  — runnable demo walkthrough.
- `src/analysis/ngram.rs` — the implementation itself.
