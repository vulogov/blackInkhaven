# lsa_test.rs

**File:** `tests/lsa_test.rs`  
**Module:** `bdslib::analysis::lsa` — LSA extractive summarisation

Verifies the contract of `lsa_summary`, `lsa_summary_with`, and `lsa_rank`.

## Test functions

### Edge cases

| Test | What it verifies |
|---|---|
| `empty_input_returns_empty_string` | Empty input → empty summary, no panic |
| `single_input_is_returned_verbatim` | One input → returned unchanged |
| `two_inputs_returns_one_on_default_ratio` | Two inputs with default ratio 0.3 → at least one sentence selected |
| `stopword_only_inputs_do_not_panic` | All-stop-word inputs → no panic, fallback non-empty summary returned |
| `unicode_inputs_are_handled` | French-language inputs rank correctly — two "système de fichiers" sentences outrank an isolated "café" sentence |

### Core LSA behaviour

| Test | What it verifies |
|---|---|
| `repeated_topic_outranks_noise` | Four "disk failure" lines outrank two unrelated noise lines in the top-2 |
| `summary_preserves_original_input_order` | Top-k picks are emitted in the same order they appeared in the input |
| `max_sentences_caps_output_length` | `LsaConfig.max_sentences` upper-bounds the number of inputs in the summary (exactly 2 of 6) |
| `ratio_used_when_max_sentences_zero` | When `max_sentences == 0`, `ratio = 0.5` over 10 inputs selects exactly 5 |

### Ranking contract

| Test | What it verifies |
|---|---|
| `ranking_length_matches_input_count` | `lsa_rank` returns one entry per input |
| `ranking_scores_are_finite_and_descending` | Scores are finite, non-NaN, sorted descending |
| `ranking_indices_are_unique_and_in_bounds` | Each index appears exactly once and is within bounds |
| `duplicates_do_not_blow_up` | All-identical inputs run cleanly and return a non-empty summary |

### Config variations

| Test | What it verifies |
|---|---|
| `more_concepts_does_not_panic` | Requesting more concepts than useful (`n_concepts = 10`) doesn't panic |
| `single_concept_still_produces_output` | `n_concepts = 1` still produces a valid summary |

### Log / operational input

| Test | What it verifies |
|---|---|
| `log_fingerprint_clustering` | Recurring `503` and `429` patterns surface in the top-3; isolated `level=info` lines do not dominate |
| `lsa_and_textrank_give_consistent_dominant_topic` | Both LSA and TextRank rank a "disk failure" sentence first on the same corpus |

## Key properties verified

- **Determinism** — same inputs always produce the same ranking (no random seed used).
- **Order preservation** — the summary reads in the same order as the inputs were supplied.
- **Length contract** — `max_sentences` (when non-zero) is a hard upper bound; `ratio` is used otherwise.
- **Robustness** — empty input, single input, all-duplicates, stop-word-only inputs, and unicode all return without panicking.
- **Topic discovery** — repeated thematic content is reliably ranked above isolated noise regardless of IDF values or sentence length.
- **Algorithm consistency** — LSA and TextRank agree on the dominant topic in an unbalanced corpus.

## Run

```bash
cargo test --test lsa_test -- --show-output
```

Or a single test:

```bash
cargo test --test lsa_test repeated_topic_outranks_noise -- --show-output
```

## Notes

- LSA is a pure function over `&[String]` — no global state, no database, no embedding engine. Each test is independent and can run in parallel.
- The centred Gram matrix (off-diagonal cosine similarity, diagonal = 0) is the key design choice that prevents isolated sentences from scoring via their own self-similarity eigenvector. See the module-level rustdoc in `src/analysis/lsa.rs` for the full algorithm description.
