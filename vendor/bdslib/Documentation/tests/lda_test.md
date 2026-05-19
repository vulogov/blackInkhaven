# lda_test.rs

**File:** `tests/lda_test.rs`  
**Module:** `bdslib::analysis::lda` — Latent Dirichlet Allocation topic modelling

Tests the LDA topic analysis pipeline over telemetry corpora.

## Test function

### `test_lda_lifecycle`

A single comprehensive test covering the full LDA lifecycle:

| Step | Description |
|---|---|
| 1 | Query before `init_db` returns `Err("not initialized")` |
| 2 | `init_db()` succeeds |
| 3 | Empty corpus returns `n_docs=0`, `n_topics=0`, empty keywords |
| 4 | Syslog corpus (12 unique string-rich documents) produces topics and keywords |
| 5 | Keywords are sorted, unique, and comma-space-separated |
| 6 | `query_window(key, start, end)` works on the same data |
| 7 | `k=2` config override produces exactly 2 topics with keywords |
| 8 | `k=1` config override produces 1 topic |
| 9 | `k > n_docs` is clamped to `n_docs` (e.g., k=10 with 1 doc → k=1) |
| 10 | Numeric-only data produces sparse or minimal keywords |
| 11 | All keyword lists are sorted, unique, and well-formatted |

## Key invariants

- **Keyword invariants**: all keywords are lowercase, alphabetically sorted, deduplicated, and separated by `", "` in the output
- **k clamping**: the number of topics is always ≤ `n_docs` to avoid degenerate LDA inputs
- **Empty corpus**: returns a well-formed result with zero counts rather than an error
- **Numeric corpus**: numeric-only data produces valid (possibly empty) keyword lists — no panic

## Notes

Like `globals_test.rs`, this uses a single `#[test]` function because the underlying `ShardsManager` is a `OnceLock` singleton. All sub-scenarios must run in sequence within one test to avoid init-order races.

The LDA implementation uses the full text of log messages as the corpus. Short or repetitive documents produce fewer distinct keywords than long, varied ones.
