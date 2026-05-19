# Latent Dirichlet Allocation topic modelling (`bdslib::analysis::latentdirichletallocation`)

`TopicSummary::query_window(key, duration, cfg)` is bdslib's LDA-based
topic-modelling pass over a corpus of documents stored under a primary
key. It returns a sorted, deduplicated list of keywords that best
characterise the records observed in a time window — a one-glance
summary of "what was this key talking about during the last hour".

This document covers:

1. [What problem LDA solves here](#1-what-problem-lda-solves-here)
2. [The classical LDA algorithm](#2-the-classical-lda-algorithm)
3. [How bdslib uses LDA](#3-how-bdslib-uses-lda)
4. [The full pipeline, step by step](#4-the-full-pipeline-step-by-step)
5. [Output contract](#5-output-contract)
6. [Configuration knobs](#6-configuration-knobs)
7. [Complexity and scaling](#7-complexity-and-scaling)
8. [Determinism guarantees](#8-determinism-guarantees)
9. [Worked examples](#9-worked-examples)
10. [Failure modes and edge cases](#10-failure-modes-and-edge-cases)
11. [References](#11-references)

---

## 1. What problem LDA solves here

You have a stream of structured records under a single key —
`syslog`, `ingest_logs`, `app.audit`, `node.cpu` — and over the last
hour the system wrote anywhere from dozens to millions of them.
Looking at the raw records will tell you nothing useful: they're too
many, too noisy, too repetitive.

What you want is a **handful of keywords** that capture what's been
happening. Not a representative sentence (TextRank/LSA already give
you that), not a cluster membership (k-NN does), not a co-occurrence
graph (RCA Jaccard does) — keywords. Words like
`{ "auth", "failed", "ssh", "root", "ip" }` for an audit log under
attack, or `{ "cpu", "spike", "node", "core", "iowait" }` for a load
incident.

Topic modelling answers this question. LDA in particular gives you:

- Multiple topics in one pass (so a noisy hour with two unrelated
  themes still surfaces both keyword sets).
- A probabilistic per-topic word distribution, so the keywords
  returned are the ones the model is most confident about.
- A natural smoothing of word frequency vs. discriminative power, via
  the Dirichlet priors.

This module is the only `bdslib::analysis::*` algorithm that produces
**keywords** rather than ranked inputs or clusters. It pairs naturally
with the others — use LDA to find the right keywords, then feed them
into a vector search or a `v2/fulltext` query for drill-down.

---

## 2. The classical LDA algorithm

Latent Dirichlet Allocation (Blei, Ng, Jordan 2003) models each
document as a mixture of topics, and each topic as a distribution over
words. The generative story is:

```
For each topic k ∈ 1..K:
    φ_k ~ Dirichlet(β)            -- topic-word distribution
For each document d ∈ 1..D:
    θ_d ~ Dirichlet(α)            -- doc-topic distribution
    For each word position n ∈ 1..N_d:
        z_{d,n} ~ Categorical(θ_d)    -- pick a topic
        w_{d,n} ~ Categorical(φ_z)    -- pick a word from that topic
```

Two Dirichlet priors control everything:

- **`α`** (doc-topic prior). Smaller `α` ⇒ documents tend to be
  about a single topic (sparse `θ_d`). Larger `α` ⇒ documents mix
  many topics.
- **`β`** (topic-word prior). Smaller `β` ⇒ topics use a small,
  focused vocabulary (sparse `φ_k`). Larger `β` ⇒ topics share many
  words.

The inference task is the inverse: given the observed words in each
document, recover `θ`, `φ`, and the per-word topic assignments `z`.
There is no closed form — every LDA implementation uses one of three
families of approximations:

- **Collapsed Gibbs sampling** — integrate out `θ` and `φ`
  analytically, then iteratively resample each word's topic
  assignment conditional on every other assignment. Simple to
  implement; converges slowly but reliably. **This is what bdslib
  uses (via the `latentdirichletallocation` crate).**
- **Variational inference** — optimise a tractable lower bound on
  the marginal likelihood. Faster, but harder to tune.
- **Online / stochastic variational** — variational with mini-batches.
  Required for very large corpora.

For the operational use case here (single-key documents, hundreds to
tens of thousands per query), collapsed Gibbs is the right pick —
deterministic given a seed, no hyperparameter sensitivity beyond `α`
and `β`, and the convergence cost is bounded by `iters` regardless of
input shape.

---

## 3. How bdslib uses LDA

LDA proper is delegated to the external
[`latentdirichletallocation`](https://crates.io/crates/latentdirichletallocation)
crate. bdslib's job is everything around it:

- Locate the right documents — open every shard that overlaps the
  window, fetch primary records by key, filter by exact timestamp.
- Convert each record into a tokenisable text by **flattening its
  `data` subtree with `json_fingerprint`** and prepending the
  whitespace-normalised key name. This gives LDA both *field-name*
  context (e.g. `program`, `severity`) and *content* tokens (e.g.
  `sshd`, `error`), which keeps the model honest on heterogeneous
  schemas.
- Run LDA with the configured priors, training iterations, and seed.
- Collapse the per-topic top-`top_n` words into a single
  alphabetically-sorted, deduplicated keyword set.

The result is a `TopicSummary` — a value-object containing the key,
the window, the corpus size, the topic count actually used, and the
keyword string.

Three entry points cover the operational shapes:

| Method | Window | Scope |
|---|---|---|
| `TopicSummary::query(key, start_secs, end_secs, cfg)` | absolute Unix-seconds range | one key |
| `TopicSummary::query_window(key, "1h", cfg)` | humantime relative window | one key |
| `TopicSummary::query_all_keys("1h", cfg)` | humantime relative window | every key with primaries in the window |

The third form is `O(n_keys)` LDA passes — useful for periodic
"what's everything talking about right now" sweeps (the bdsweb logs
page and the JSON-RPC `v2/topics.all` endpoint use it).

---

## 4. The full pipeline, step by step

Given a key, a window, and an `LdaConfig`:

### Step 1 — Window resolution

For `query_window` and `query_all_keys`, parse the humantime string
and compute `[start, end)` in Unix seconds:

```rust
let dur   = humantime::parse_duration(duration)?;
let now   = SystemTime::now();
let start = now.checked_sub(dur).unwrap_or(UNIX_EPOCH);
let start_secs = start.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
let end_secs   = now  .duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
```

Invalid `duration` strings short-circuit with an `Err` — never panic.

### Step 2 — Locate overlapping shards

Walk the shard catalog and collect every shard whose interval
overlaps `[start, end)`:

```rust
for info in db.cache().info().shards_in_range(start_ts, end_ts)? {
    let shard = db.cache().shard(info.start_time)?;
    // …
}
```

This avoids opening shards that cannot possibly contain matching
records.

### Step 3 — Fetch and filter records

For each overlapping shard, pull every primary record stored under
`key` and keep only those whose `timestamp` actually falls inside the
window (the shard interval and the query window can have different
boundaries):

```rust
for doc in shard.get_primaries_by_key(key)? {
    let ts = doc["timestamp"].as_u64().unwrap_or(0);
    if ts >= start_secs && ts < end_secs {
        let text = doc_to_text(&doc);
        if !text.trim().is_empty() {
            texts.push(text);
        }
    }
}
```

Records that produce empty text are dropped silently (their `data`
was missing, all-numeric, or otherwise un-tokenisable).

### Step 4 — Convert documents to text

This is the only bdslib-specific design decision in the pipeline.
Each stored record looks like:

```json
{
  "key":       "syslog",
  "timestamp": 1745003724,
  "data": {
    "program":  "sshd",
    "host":     "server-01",
    "pid":      1234,
    "severity": "info",
    "message":  "session opened"
  }
}
```

`doc_to_text` produces:

```
"syslog  program: sshd  host: server-01  pid: 1234  severity: info  message: session opened"
```

The recipe:

```rust
fn doc_to_text(doc: &JsonValue) -> String {
    let key_part = doc["key"]
        .as_str()
        .map(|k| k.replace(['.', '_', '-'], " "))
        .unwrap_or_default();

    let data_fp = json_fingerprint(&doc["data"]);

    match (key_part.is_empty(), data_fp.is_empty()) {
        (true, _) => data_fp,
        (_, true) => key_part,
        _ => format!("{key_part}  {data_fp}"),
    }
}
```

Two important properties:

- **Key name normalisation.** Dots, underscores, and hyphens become
  spaces, so a key like `app.audit_v2` contributes the tokens `app`,
  `audit`, `v2` to LDA's vocabulary instead of one opaque atom.
- **`json_fingerprint` over `data`.** Every leaf — strings, numbers,
  booleans — gets emitted as `"field: value"`. This means LDA sees
  *both* the field name (`severity`) and the value (`info`), which is
  exactly the right granularity for log-style records: a topic
  consisting of `{ severity, error, sshd, program }` is more useful
  than one consisting of just `{ error, sshd }`.

### Step 5 — Build and train the LDA model

Clamp `k` so it never exceeds the corpus size:

```rust
let k = config.k.max(1).min(n_docs);
```

`k > n_docs` makes no sense (you can't have more topics than
documents) and the underlying solver may not handle it gracefully;
this clamp keeps the call site safe.

Construct the model and run `iters` rounds of collapsed Gibbs
sampling:

```rust
let doc_refs: Vec<&str> = texts.iter().map(String::as_str).collect();
let mut lda = Lda::from_documents(k, config.alpha, config.beta, &doc_refs, config.seed);
lda.train(config.iters);
```

The crate handles tokenisation and the vocabulary table internally —
it splits on whitespace, lowercases, and strips trivial punctuation.
Stop-words are *not* removed automatically; LDA tolerates them
because the Dirichlet priors push very-frequent-everywhere terms out
of every topic's top-N anyway. (If you need explicit stop-word
filtering, pre-process `texts` before constructing the model.)

### Step 6 — Extract and collapse top words

For each topic, take the top `top_n` words by per-topic probability:

```rust
let mut seen = HashSet::new();
let mut keywords: Vec<String> = lda
    .top_words(config.top_n)                                // Vec<Vec<(String, f64)>>
    .into_iter()
    .flat_map(|topic| topic.into_iter().map(|(word, _score)| word))
    .filter(|word| seen.insert(word.clone()))               // dedup across topics
    .collect();
keywords.sort_unstable();                                   // alphabetical
```

The deduplication is intentional: if two topics both surface
`error`, the keyword set should still contain `error` once. The
sort is also intentional — keyword summaries are consumed by humans
scanning a wall of text, and alphabetical order makes "did `auth`
appear?" trivially answerable.

### Step 7 — Assemble the result

```rust
TopicSummary {
    key:      key.to_string(),
    start:    start_secs,
    end:      end_secs,
    n_docs,
    n_topics: k,                           // post-clamp
    keywords: keywords.join(", "),
}
```

The `keywords` field is the comma-separated string for direct display;
`n_docs` and `n_topics` let consumers decide whether the summary
carries enough signal to act on.

---

## 5. Output contract

```rust
pub struct TopicSummary {
    pub key:      String,
    pub start:    u64,           // Unix seconds, inclusive
    pub end:      u64,           // Unix seconds, exclusive
    pub n_docs:   usize,         // documents actually used
    pub n_topics: usize,         // ≤ cfg.k, clamped to n_docs
    pub keywords: String,        // alphabetical, comma-separated, deduplicated
}
```

Edge-case guarantees:

| Input | Behaviour |
|---|---|
| Window contains zero matching records | `n_docs = 0`, `n_topics = 0`, `keywords = ""`. No error, no panic. |
| All matching records produce empty `doc_to_text` | Same as above — the model is never built. |
| `cfg.k > n_docs` | `n_topics = n_docs`. |
| `cfg.k = 0` | Internally bumped to 1. |
| Single document | `n_topics = 1`, the keyword set is `top_n` words from that single document's topic. |

The struct is `Serialize`/`Deserialize` so it round-trips cleanly
through the JSON-RPC layer (`v2/topics`, `v2/topics.all`).

---

## 6. Configuration knobs

```rust
pub struct LdaConfig {
    pub k:     usize,    // default: 3
    pub alpha: f64,      // default: 0.1
    pub beta:  f64,      // default: 0.01
    pub seed:  u64,      // default: 42
    pub iters: usize,    // default: 200
    pub top_n: usize,    // default: 10
}
```

| Knob | Effect |
|---|---|
| **`k`** | Number of topics. Higher ⇒ finer distinctions ("auth-failure topic" vs. "auth-success topic" vs. "rate-limit topic"); lower ⇒ broader merges. Clamped to `n_docs`. The default 3 is fine for typical operational corpora; raise to 5–8 for noisy multi-source streams. |
| **`alpha`** | Dirichlet prior on doc-topic distributions. Smaller ⇒ each document picks one dominant topic; larger ⇒ documents mix topics evenly. Default 0.1 (sparse) suits log records that usually concern one thing each. |
| **`beta`** | Dirichlet prior on topic-word distributions. Smaller ⇒ topics use a tight vocabulary; larger ⇒ topics share words. Default 0.01 (very sparse) suits the structured-fingerprint input where a few field/value tokens carry most of the meaning. |
| **`seed`** | RNG seed for collapsed Gibbs sampling. Identical seed + identical corpus ⇒ identical output. Change it to test stability under randomisation. |
| **`iters`** | Collapsed Gibbs iterations. More iterations ⇒ closer to the true posterior, slower runtime. Default 200 is a robust middle ground; cut to 100 for fast dashboards, raise to 500 for quality-sensitive offline analyses. |
| **`top_n`** | Words taken from each topic before collapsing. Default 10 produces compact summaries; raise for richer keyword sets at the cost of more noise from low-probability tail words. |

---

## 7. Complexity and scaling

For `D` documents, total token count `T` (sum of words across all
documents), `V` vocabulary size, `K` topics, and `I` Gibbs iterations:

| Phase | Cost |
|---|---|
| Locate overlapping shards | `O(n_shards)` catalog scan |
| Fetch records by key | dominated by ShardsManager queries — typically sub-second up to ~10⁵ records |
| `doc_to_text` per record | `O(json_fingerprint)` ≈ linear in record size |
| Vocabulary build (inside the LDA crate) | `O(T)` |
| Collapsed Gibbs sampling | `O(I · T · K)` |
| Top-N extraction | `O(K · V log V)` |
| Dedup + sort keywords | `O(K · top_n · log(K · top_n))` |

The dominant term is collapsed Gibbs sampling at `O(I · T · K)`. With
defaults (`I = 200`, `K = 3`), this is `600 · T` floating-point ops —
sub-second for `T` up to ~10⁶ tokens (≈ 100k records of typical log
shape). Beyond that, drop `iters` to 100 or pre-aggregate records by
template before LDA.

Memory:

| Structure | Size |
|---|---|
| `texts` Vec | `O(T)` bytes |
| Vocabulary table | `O(V)` |
| Per-document, per-word topic assignments | `O(T)` `usize` cells |
| Topic-word + doc-topic count tables | `O(K · V) + O(D · K)` |

For typical operational sizes everything fits comfortably in a few MB.

---

## 8. Determinism guarantees

`TopicSummary::query*` is **fully reproducible** given:

- A fixed seed (the `seed` field of `LdaConfig`).
- A deterministic corpus.

Both conditions are explicit in the design: the seed is exposed as a
config knob, and the corpus is fetched in deterministic order
(shard intervals walked in time order, records under each key in
storage order, timestamp filter applied identically each run).

The collapsed-Gibbs implementation in the
`latentdirichletallocation` crate is seeded from `cfg.seed` and uses a
deterministic PRNG; identical `(seed, corpus, k, alpha, beta, iters)`
produce identical posteriors and therefore identical keyword sets.

Two specific non-issues you might worry about:

- **Shard interleaving.** The catalog returns shards in time order;
  records inside a shard are returned in deterministic
  `(key, timestamp, uuid)` order. The order in which `texts` is
  populated is therefore reproducible.
- **Top-N tie-breaks.** When two words have the same per-topic
  probability, the underlying crate's tie-break is deterministic
  given the seed.

The only way to perturb the output is to change the corpus (new
records arrived in the window, or the window itself moved), which is
the desired behaviour: LDA reflects the corpus exactly, and a stable
corpus gives a stable summary.

---

## 9. Worked examples

### Example A — single key, focused stream

`key = "syslog"`, last hour, ~500 records all from one ssh-brute-force
incident.

```rust
let cfg = LdaConfig::default();   // k=3, alpha=0.1, beta=0.01
let summary = TopicSummary::query_window("syslog", "1h", cfg)?;
```

Typical output:

```
TopicSummary {
    key:      "syslog",
    n_docs:   500,
    n_topics: 3,
    keywords: "auth, failed, host, invalid, ip, port, root, sshd, user, …"
}
```

The three topics are mostly redundant (since the stream is so focused),
so the alphabetical dedup collapses them into a tight keyword set
that captures the entire incident.

### Example B — single key, multi-modal stream

`key = "ingest_logs"`, last 24 hours, mixing nginx access logs,
worker errors, and a daily backup job's stdout.

With `cfg.k = 5`:

```
keywords: "200, 304, 4xx, GET, POST, backup, completed, error, exception,
           failed, host, ip, latency, ms, path, pid, request, success,
           upstream, worker, …"
```

Three topics are distinguishable by eye:

- `{ GET, POST, 200, 304, path, host }` — access logs
- `{ error, exception, failed, worker, pid }` — worker errors
- `{ backup, completed, success, ms }` — daily job

But the union view still gives operators a useful pulse: "we had
HTTP traffic, worker errors, *and* a successful backup". For
finer separation, switch to `query_all_keys` so each schema has its
own summary, or pre-route logs by `key` before ingestion.

### Example C — every key in the window

`TopicSummary::query_all_keys("1h", cfg)` returns one summary per
key. Output (abbreviated):

```
[
  TopicSummary { key: "app.audit",   n_docs: 1240, keywords: "auth, failed, …" },
  TopicSummary { key: "node.cpu",    n_docs:  720, keywords: "cpu, idle, iowait, …" },
  TopicSummary { key: "node.memory", n_docs:  720, keywords: "available, free, used, …" },
  TopicSummary { key: "syslog",      n_docs: 4400, keywords: "kernel, message, sshd, …" },
]
```

Each row is computed independently — the `cfg` is reused but the
underlying LDA models are unrelated. This is the shape behind the
bdsweb dashboard "what's every key talking about right now"
section.

---

## 10. Failure modes and edge cases

| Input | Behaviour |
|---|---|
| Empty corpus | `TopicSummary { n_docs: 0, n_topics: 0, keywords: "" }`. No error. |
| All records have empty `data` and a numeric-only key part | All `texts` end up empty; same as empty corpus. |
| Window where `start ≥ end` | Vacuously empty corpus. |
| Invalid humantime duration | `Err` from `parse_duration`. |
| `cfg.k = 0` | Internally bumped to 1. |
| `cfg.k > n_docs` | Clamped to `n_docs`. |
| `cfg.iters = 0` | LDA runs zero iterations and returns the random-init posterior — keywords will look noisy. Avoid. |
| Very large `top_n` | Caps at the model's vocabulary size; no error, just diminishing returns past the long tail. |
| Single document | `n_topics = 1`. The single topic's top-`top_n` words are returned. |
| Records under the key all numeric (telemetry) | `json_fingerprint` flattens numeric leaves into `"value: 72.4"`-style tokens, so LDA still sees something — the topic will be dominated by field-name tokens (`value`, `unit`, …). For pure telemetry, prefer `v2/trends` or `bdslib::analysis::telemetrytrend` which model the actual numeric distribution. |

The function never panics on user-supplied input.

---

## 11. References

- Blei, D. M., Ng, A. Y., & Jordan, M. I. (2003). *Latent Dirichlet
  Allocation.* Journal of Machine Learning Research, 3, 993–1022 —
  the foundational LDA paper.
- Griffiths, T. L., & Steyvers, M. (2004). *Finding scientific topics.*
  Proceedings of the National Academy of Sciences, 101(suppl 1),
  5228–5235 — the collapsed Gibbs sampler used here.
- Heinrich, G. (2008). *Parameter estimation for text analysis.*
  Technical report, Fraunhofer IGD — the standard derivation of the
  collapsed Gibbs update equation, and a clean treatment of the
  Dirichlet priors `α` and `β`.
- Wallach, H. M., Mimno, D., & McCallum, A. (2009). *Rethinking LDA:
  Why priors matter.* Proceedings of NIPS 2009 — empirical guidance
  on choosing `α` and `β` (asymmetric priors, hyperparameter
  optimisation), informing the sparse defaults in this module.
- Hoffman, M. D., Bach, F., & Blei, D. M. (2010). *Online learning for
  Latent Dirichlet Allocation.* Proceedings of NIPS 2010 — the
  variational alternative to Gibbs sampling, useful when the corpus
  is too large for batch inference.
- Salton, G. (1989). *Automatic Text Processing.* Addison-Wesley —
  the canonical reference for vocabulary construction and
  bag-of-words representations.

## See also

- [`Documentation/tests/lda_test.md`](../tests/lda_test.md) —
  every test case and what it verifies.
- [`Documentation/Algorithm/TEXTRANK.md`](TEXTRANK.md),
  [`Documentation/Algorithm/LSA.md`](LSA.md) — the *extractive*
  algorithms. They pick representative inputs verbatim; LDA produces
  derived keywords that may not appear together in any single input.
- [`Documentation/Algorithm/KNN.md`](KNN.md),
  [`Documentation/Algorithm/RCA_JACCARD.md`](RCA_JACCARD.md) — the
  clustering algorithms. LDA finds *thematic* groupings of words; k-NN
  finds *similar inputs*; RCA finds *temporally-correlated event keys*.
  Together they cover every "what is this corpus about?" question.
- `src/analysis/latentdirichletallocation.rs` — the implementation
  itself, ~218 lines (most of the heavy lifting is in the external
  `latentdirichletallocation` crate).
- `crates.io/crates/latentdirichletallocation` — the Rust LDA solver
  bdslib delegates to.
