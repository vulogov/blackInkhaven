# knn_test.rs

**File:** `tests/knn_test.rs`
**Module:** `bdslib::analysis::knn` — k-Nearest-Neighbour clustering and anomaly detection

Verifies the contract of `knn_summary` and `knn_summary_with`: the JSON output shape, the clustering and anomaly semantics, the config knobs, and end-to-end determinism.

## Test functions

### Shape / edge cases

| Test | What it verifies |
|---|---|
| `empty_input_returns_empty_shape` | Empty corpus → `n_logs=0`, empty `clusters`/`anomalies`/`representatives` arrays |
| `single_input_yields_one_trivial_cluster` | Single line → one cluster of size 1, no anomalies, the line is its own representative |
| `output_is_valid_json_object_with_required_keys` | The top-level value is a JSON object with every documented key present |
| `stopword_only_inputs_become_all_anomalies` | All-stop-word corpus has no scorable vocabulary → every line is an anomaly, no clusters |
| `unicode_inputs_are_handled` | French inputs cluster correctly: a "système" line wins over an isolated "café" line |

### Core algorithm behaviour

| Test | What it verifies |
|---|---|
| `repeated_theme_forms_a_cluster` | Five "disk failure" lines collapse into a single cluster of size 5 |
| `two_distinct_themes_form_two_clusters` | Disk-failure and network-timeout lines form two separate clusters with appropriate representatives |
| `isolated_inputs_are_classified_as_anomalies` | Lines sharing no vocabulary with the cluster surface in the anomaly list |
| `anomalies_are_sorted_most_isolated_first` | The `anomalies` array is sorted by `max_similarity` ascending (least-similar first) |
| `cluster_representative_has_highest_density` | The `representative` of every cluster has density ≥ every member's density |

### Config knobs

| Test | What it verifies |
|---|---|
| `k_is_clamped_when_larger_than_corpus` | `cfg.k = 50` on a 3-line corpus → reported `k` is in `[1, n-1]` |
| `max_cluster_members_caps_member_array` | Cluster `size` reflects the true count; `members` array is capped at `max_cluster_members` |
| `max_anomalies_caps_anomalies_array` | `n_anomalies` is the true total; `anomalies` array length is capped at `max_anomalies` |
| `lower_anomaly_threshold_keeps_more_lines_in_clusters` | Lower threshold ⇒ fewer-or-equal anomalies (monotonic in the threshold) |
| `min_word_len_filters_short_tokens` | Different `min_word_len` values run without panicking |

### Ranking / determinism

| Test | What it verifies |
|---|---|
| `cluster_sizes_are_descending` | Cluster `size` field is non-increasing across the `clusters` array |
| `deterministic_output_for_identical_input` | Two `knn_summary(&logs)` calls with identical input produce identical JSON (sorted-key summation eliminates HashMap-iteration nondeterminism) |
| `cluster_ids_are_dense_and_zero_based` | Cluster ids are `0..n_clusters` with no gaps |
| `density_is_bounded_in_unit_interval` | Every reported density is in `[0, 1]` (cosine-similarity range) |
| `representatives_index_back_to_clusters` | Each entry of `representatives` points at the correct cluster's `representative.idx` |
| `duplicates_do_not_blow_up` | Identical duplicate lines form a valid cluster without panicking |

## Key properties verified

- **Numeric exclusion of stop-words and short tokens** — the tokeniser is gated by `min_word_len` and `STOPWORDS`.
- **Bounded output** — `members` and `anomalies` arrays are never larger than the corresponding caps; the true counts are always reported separately.
- **Determinism** — same input → same JSON, byte-for-byte.
- **Density invariants** — every density value is bounded in `[0, 1]`; the cluster representative carries the maximum density of its members.
- **Anomaly cut** — high `anomaly_threshold` ⇒ more anomalies, low threshold ⇒ fewer anomalies (monotonic).
- **Clamping** — the reported `k` is always in `[1, n-1]` regardless of `cfg.k`.

## Run

```bash
cargo test --test knn_test -- --show-output
```

Or one specific test:

```bash
cargo test --test knn_test repeated_theme_forms_a_cluster -- --show-output
```

## Notes

- `knn_summary` is a pure function over `&[String]` — no global state, no database, no embedding engine. Each test is independent and can run in parallel.
- The cosine similarity dot products and L2 norms iterate sorted TF-IDF keys to keep floating-point summation reproducible across runs (HashMap iteration is randomised by Rust's default `RandomState`). Without that ordering, last-bit perturbations would propagate into densities and representative selection.
