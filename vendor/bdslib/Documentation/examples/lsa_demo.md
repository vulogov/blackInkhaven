# lsa_demo.rs

**File:** `examples/lsa_demo.rs`

Demonstrates `bdslib::analysis::lsa`: extractive summarisation of any list of strings — sentences, log lines, or JSON fingerprints — using Latent Semantic Analysis (Steinberger & Ježek 2004).

## What it demonstrates

| Function | Description |
|---|---|
| `lsa_summary(&[String])` | Summary using the default config (~30% of inputs, top-ranked) |
| `lsa_summary_with(&[String], &LsaConfig)` | Summary with caller-controlled length and tuning |
| `lsa_rank(&[String], &LsaConfig)` | Full ranked list of `(input_index, score)` pairs |

## Sections

| # | Topic | Inputs | Behaviour shown |
|---|---|---|---|
| 1 | Plain text passage | 7 sentences about distributed systems | Top-3 sentences picked; full ranking printed with scores |
| 2 | Operational log burst | 7 log lines, 4 recurring `code=503` errors | Auto-sizing surfaces the recurring error pattern in the top-3 |
| 3 | Dominant-theme detection | 4 disk-failure lines + 2 noise lines | LSA and TextRank both rank disk-failure sentences first |
| 4 | Config knobs | 10 unique inputs | `ratio` (0.2/0.5/1.0) and `max_sentences` control output length |
| 5 | Edge cases | Empty, single, stop-word-only, all-duplicate inputs | No panics; sensible fallback values |

## LsaConfig

| Field | Default | Description |
|---|---|---|
| `max_sentences` | `0` (auto) | Hard cap on summary length; `0` defers to `ratio` |
| `ratio` | `0.3` | Fraction of inputs kept when `max_sentences == 0` |
| `min_word_len` | `2` | Tokens shorter than this are dropped before scoring |
| `n_concepts` | `3` | Number of LSA concepts (singular vectors) to extract |
| `power_iters` | `50` | Power-iteration steps per eigenvector |

## How it works

1. **Tokenise** — each input is split into lowercase alphanumeric tokens; stop-words and tokens shorter than `min_word_len` are dropped.
2. **TF-IDF** — smoothed `log((N+1)/(df+1)) + 1` IDF × normalized TF for every (term, sentence) pair.
3. **Centred Gram matrix** — off-diagonal cosine similarities only (diagonal = 0). Keeps isolated sentences in the null space so they cannot score via self-similarity.
4. **Truncated SVD** — top-`n_concepts` eigenpairs extracted by power iteration with Gram–Schmidt deflation.
5. **Steinberger-Ježek score** — `score[j] = √(Σₖ λₖ · v_k[j]²)`, where (λₖ, v_k) are the eigenpairs. Sentences that contribute strongly to many important latent concepts score highest.
6. **Pick top-k** — by score, then re-sorted into original input order so the summary reads naturally.

## Run

```bash
cargo run --example lsa_demo
```
