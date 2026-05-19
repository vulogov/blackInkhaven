# textrank_demo.rs

**File:** `examples/textrank_demo.rs`

Demonstrates `bdslib::analysis::textrank`: extractive summarisation of any list of strings — sentences, log lines, or JSON fingerprints — using the classic TextRank algorithm.

## What it demonstrates

| Function | Description |
|---|---|
| `textrank_summary(&[String])` | Summary using the default config (~30% of inputs, top-ranked) |
| `textrank_summary_with(&[String], &TextRankConfig)` | Summary with caller-controlled length and tuning |
| `textrank_rank(&[String], &TextRankConfig)` | Full ranked list of `(input_index, score)` pairs |

## Sections

| # | Topic | Inputs | Behaviour shown |
|---|---|---|---|
| 1 | Plain text passage | 7 sentences about distributed systems | Top-3 sentences picked; full ranking printed |
| 2 | Synthetic log burst | 7 log lines, 4 of them recurring `upstream timeout` errors | Default auto-sizing surfaces the recurring pattern |
| 3 | JSON fingerprints | 7 fingerprints with two recurring login patterns and isolated heartbeats | Top-2 picks the dominant cluster; isolated `event=heartbeat` lines rank lowest |
| 4 | Edge cases | Empty input, single input, stop-word-only inputs | No panics; sensible fallback values |

## TextRankConfig

| Field | Default | Description |
|---|---|---|
| `max_sentences` | `0` (auto) | Hard cap on summary length; `0` defers to `ratio` |
| `ratio` | `0.3` | Fraction of inputs kept when `max_sentences == 0` |
| `min_word_len` | `2` | Tokens shorter than this are dropped before scoring |
| `damping` | `0.85` | Standard PageRank damping factor |
| `iters` | `30` | Maximum PageRank iterations |
| `tolerance` | `1e-4` | Early-exit threshold on the L1-norm of score deltas |

## How it works

1. **Tokenise** — each input is split into lowercase alphanumeric tokens; stop-words and tokens shorter than `min_word_len` are dropped.
2. **Cosine similarity matrix** — pairwise cosine on term-frequency vectors of the bags of words.
3. **Row-normalise** — the similarity matrix is converted to a stochastic transition matrix; isolated rows (no overlap with anyone) are spread uniformly so PageRank stays well-defined.
4. **Weighted PageRank** — scored with damping `0.85` until either `iters` is reached or the L1-norm of deltas drops below `tolerance`.
5. **Pick top-k** — by score, then re-sorted into original input order so the summary reads naturally.

## Run

```bash
cargo run --example textrank_demo
```

## Future use

The "JSON fingerprints" section illustrates the planned integration: clustered log entries, summarised by their fingerprint patterns to give an operator a one-line picture of what the cluster is "about".
