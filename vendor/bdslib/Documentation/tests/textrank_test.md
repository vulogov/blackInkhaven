# textrank_test.rs

**File:** `tests/textrank_test.rs`  
**Module:** `bdslib::analysis::textrank` — extractive summarisation

Verifies the contract of `textrank_summary`, `textrank_summary_with`, and `textrank_rank`.

## Test functions

| Test | What it verifies |
|---|---|
| `empty_input_returns_empty_string` | Empty input → empty summary, no panic |
| `single_input_is_returned_verbatim` | One input → returned unchanged |
| `two_identical_inputs_return_at_least_one` | Duplicate inputs → still produce a usable summary |
| `central_topic_sentence_outranks_unrelated_noise` | A repeated topic ("system reboot") dominates the top-2; one-off noise sentences (Lisbon, lentil) are kept out of the top-2 |
| `summary_preserves_original_input_order` | Top-k picks are emitted in the same order they appeared in the input |
| `max_sentences_caps_output_length` | `TextRankConfig.max_sentences` upper-bounds the number of inputs in the summary |
| `ratio_used_when_max_sentences_zero` | When `max_sentences == 0`, `ratio` (e.g. `0.5` over 10 inputs) selects 5 inputs |
| `duplicates_do_not_blow_up` | All-duplicate inputs run cleanly and return a non-empty summary |
| `unicode_inputs_are_handled` | French-language inputs rank correctly (lowercase mapping, alphanumeric tokenisation) |
| `log_fingerprint_clustering` | Recurring failure patterns surface in the top-3; isolated `level=info` lines do not dominate |
| `ranking_lengths_match_input` | `textrank_rank` returns one entry per input, sorted by score desc, finite scores, unique indices |
| `stopword_only_inputs_do_not_panic` | Inputs containing only stop-words → no panic, fallback summary returned |

## Key properties verified

- **Determinism** — same inputs always produce the same ranking.
- **Order preservation** — the summary reads in the same order as the inputs were supplied.
- **Length contract** — `max_sentences` (when non-zero) is a hard upper bound; `ratio` is used otherwise.
- **Robustness** — empty input, single input, all-duplicates, stop-word-only inputs, and unicode all return without panicking.
- **Topic discovery** — repeated topical content is reliably ranked above one-off noise lines.

## Run

```bash
cargo test --test textrank_test -- --show-output
```

Or a single test:

```bash
cargo test --test textrank_test central_topic_sentence_outranks_unrelated_noise -- --show-output
```

## Notes

Unlike `lda_test`, `rca_test`, etc., this test file does not depend on the global `ShardsManager` — TextRank is a pure function over `&[String]`. Each test is independent and can run in parallel.
