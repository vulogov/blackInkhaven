# ngram_test.rs

**File:** `tests/ngram_test.rs`
**Module:** `bdslib::analysis::ngram` — n-gram anomaly detection and noise removal

Verifies the contract of `ngram_anomaly` / `ngram_anomaly_with` and `ngram_remove_noise` / `ngram_remove_noise_with`: JSON output shape, classification semantics, every config knob, and end-to-end determinism.

## Test functions

### Shape / edge cases — `ngram_anomaly`

| Test | What it verifies |
|---|---|
| `anomaly_empty_input_returns_empty_shape` | `n_logs=0`, empty `anomalies` array, `n_unique_ngrams=0` |
| `anomaly_output_has_required_keys` | Every documented key is present in the JSON object |
| `anomaly_lines_too_short_for_n_score_zero` | A line that produces no n-grams (too few tokens for `n`) is never flagged as an anomaly even with threshold 0.0 |

### Core algorithm — `ngram_anomaly`

| Test | What it verifies |
|---|---|
| `anomaly_repeated_theme_has_no_anomalies` | A homogeneous corpus produces zero anomalies — every line shares its n-grams with every other |
| `anomaly_isolated_outlier_is_flagged` | A vocabulary-disjoint outlier surfaces as the top anomaly |
| `anomaly_results_sorted_by_rarity_descending` | The `anomalies` array is sorted by `rarity` descending |
| `anomaly_max_anomalies_caps_array` | `n_anomalies` reports the true total; the array is bounded at `cfg.max_anomalies` |
| `anomaly_lower_threshold_allows_more` | Reducing `anomaly_threshold` cannot decrease the anomaly count (monotonic) |
| `anomaly_novel_ngrams_capped` | Each anomaly's `novel_ngrams` array is bounded at `cfg.max_novel_ngrams` |
| `anomaly_n_eq_3_does_not_panic` | Trigram mode (`n=3`) runs successfully on short corpora |
| `anomaly_identical_duplicates_have_no_rarity` | Byte-identical duplicates have zero rarity and produce no anomalies |
| `anomaly_deterministic_output` | Same input twice → same JSON, byte-for-byte (sorted-summation determinism) |
| `anomaly_mean_rarity_in_unit_interval` | The reported `mean_rarity` is in `[0, 1]` |

### Shape / edge cases — `ngram_remove_noise`

| Test | What it verifies |
|---|---|
| `noise_empty_input_returns_empty_shape` | `n_logs=0`, empty `kept`/`removed` arrays |
| `noise_output_has_required_keys` | Every documented key is present |
| `noise_lines_too_short_for_n_are_kept` | A line that produces no n-grams cannot be classified as noise — it lands in `kept` |

### Core algorithm — `ngram_remove_noise`

| Test | What it verifies |
|---|---|
| `noise_all_identical_corpus_is_all_removed` | Every line of an all-identical corpus has commonness 1.0 → all removed |
| `noise_mixed_corpus_separates_signal_from_noise` | Heartbeat-style repetition is removed; unique alert lines are kept |
| `noise_kept_plus_removed_equals_n_logs` | Every line is accounted for: `n_kept + n_removed == n_logs` |
| `noise_higher_threshold_removes_fewer` | Increasing `noise_threshold` cannot increase the removed count (monotonic) |
| `noise_kept_preserves_input_order` | `kept` array is in increasing `idx` order — readable as a denoised corpus |
| `noise_removed_sorted_by_commonness_descending` | `removed` array is sorted by `commonness` descending (most-noise-like first) |
| `noise_max_kept_and_max_removed_caps_arrays` | `n_kept`/`n_removed` report the true totals; arrays bounded at the caps |
| `noise_n_eq_1_unigram_works` | Unigram mode (`n=1`) runs successfully |
| `noise_deterministic_output` | Same input twice → same JSON |

### Duality

| Test | What it verifies |
|---|---|
| `anomaly_and_noise_are_dual_views` | A line classified as noise by one endpoint is *not* anomalous to the other; a unique line surfaces in `anomalies` AND survives in `kept` |

## Key properties verified

- **Determinism** — both endpoints are byte-for-byte reproducible across runs (sorted-key summation eliminates HashMap-iteration noise).
- **Threshold monotonicity** — looser threshold ⇒ at least as many anomalies / fewer-or-equal noise removals.
- **Bounded output** — every member array is capped by config; the `n_*` totals always reflect the truth.
- **Empty / too-short safety** — lines that yield no n-grams cannot trigger either classification.
- **Order semantics** — anomalies sorted by rarity desc; removed sorted by commonness desc; kept preserves input order.

## Run

```bash
cargo test --test ngram_test -- --show-output
```

Or one specific test:

```bash
cargo test --test ngram_test anomaly_isolated_outlier_is_flagged -- --show-output
```

## Notes

Both endpoints are pure functions over `&[String]` — no global state, no database, no embedding model. Each test is independent and runs in parallel.

The default `anomaly_threshold = 0.7` and `noise_threshold = 0.85` are intentionally strict so default-config calls produce conservative, high-confidence verdicts. Several tests pass explicit lower thresholds because their small corpora do not naturally hit the strict defaults — the test suite is exercising the *behaviour* of the classifier, not validating that the defaults are universally appropriate.
