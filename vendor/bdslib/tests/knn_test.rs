//! Tests for `bdslib::analysis::knn`.
//!
//! Verifies the contract of `knn_summary` and `knn_summary_with`: TF-IDF
//! cosine k-NN, cluster discovery via mutual k-NN graph, anomaly
//! detection, and the JSON output shape.
//!
//! Run with:
//! ```bash
//! cargo test --test knn_test -- --show-output
//! ```

use bdslib::{knn_summary, knn_summary_with, KnnConfig};
use serde_json::Value as JsonValue;

fn s(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

/// Convenience: pull the integer at `key` (treating numeric JSON as i64).
fn ji(v: &JsonValue, key: &str) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(-1)
}

/// Convenience: pull the f64 at `key`.
fn jf(v: &JsonValue, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(f64::NAN)
}

/// Pull the array at `key`, or empty vec.
fn ja<'a>(v: &'a JsonValue, key: &str) -> Vec<&'a JsonValue> {
    v.get(key).and_then(|x| x.as_array()).map(|a| a.iter().collect()).unwrap_or_default()
}

// ── shape / edge cases ──────────────────────────────────────────────────────────

#[test]
fn empty_input_returns_empty_shape() {
    let out = knn_summary(&Vec::<String>::new());
    assert_eq!(ji(&out, "n_logs"), 0);
    assert_eq!(ji(&out, "n_clusters"), 0);
    assert_eq!(ji(&out, "n_anomalies"), 0);
    assert!(ja(&out, "clusters").is_empty());
    assert!(ja(&out, "anomalies").is_empty());
    assert!(ja(&out, "representatives").is_empty());
}

#[test]
fn single_input_yields_one_trivial_cluster() {
    let logs = s(&["the only log line"]);
    let out = knn_summary(&logs);
    assert_eq!(ji(&out, "n_logs"), 1);
    assert_eq!(ji(&out, "n_clusters"), 1);
    assert_eq!(ji(&out, "n_anomalies"), 0);

    let clusters = ja(&out, "clusters");
    assert_eq!(clusters.len(), 1);
    assert_eq!(ji(clusters[0], "size"), 1);
    let rep = clusters[0].get("representative").unwrap();
    assert_eq!(rep.get("text").unwrap().as_str().unwrap(), "the only log line");
}

#[test]
fn output_is_valid_json_object_with_required_keys() {
    let logs = s(&["alpha beta", "gamma delta"]);
    let out = knn_summary(&logs);
    assert!(out.is_object(), "top-level must be a JSON object");
    for k in &[
        "n_logs", "k", "anomaly_threshold",
        "n_clusters", "n_anomalies",
        "clusters", "anomalies", "representatives",
    ] {
        assert!(out.get(*k).is_some(), "missing key: {k}");
    }
}

#[test]
fn stopword_only_inputs_become_all_anomalies() {
    let logs = s(&[
        "the and is of",
        "to in on at",
        "by for with as",
    ]);
    let out = knn_summary(&logs);
    assert_eq!(ji(&out, "n_logs"), 3);
    assert_eq!(ji(&out, "n_clusters"), 0);
    assert_eq!(ji(&out, "n_anomalies"), 3);
    assert!(ja(&out, "clusters").is_empty());
    assert_eq!(ja(&out, "anomalies").len(), 3);
}

#[test]
fn unicode_inputs_are_handled() {
    let logs = s(&[
        "système de fichiers en lecture seule",
        "le système de fichiers est passé en lecture seule",
        "système de fichiers monté en lecture seule",
        "café servi à la cantine",
    ]);
    let out = knn_summary(&logs);
    let n_clusters = ji(&out, "n_clusters");
    assert!(n_clusters >= 1, "expected at least one cluster, got {n_clusters}");

    // The cluster representative should be a "système" line, not the café noise.
    let clusters = ja(&out, "clusters");
    let rep = clusters[0].get("representative").unwrap();
    let rep_text = rep.get("text").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        rep_text.to_lowercase().contains("système"),
        "expected 'système' representative, got: {rep_text:?}"
    );
}

// ── core algorithm behaviour ────────────────────────────────────────────────────

#[test]
fn repeated_theme_forms_a_cluster() {
    let logs = s(&[
        "disk failure detected on storage node 1",
        "disk failure detected on storage node 2",
        "disk failure detected on storage node 3",
        "disk failure detected on storage node 4",
        "disk failure detected on storage node 5",
    ]);
    let out = knn_summary(&logs);

    assert_eq!(ji(&out, "n_clusters"), 1, "all five lines share vocabulary");
    let clusters = ja(&out, "clusters");
    assert_eq!(ji(clusters[0], "size"), 5);

    let rep_text = clusters[0]["representative"]["text"].as_str().unwrap();
    assert!(rep_text.contains("disk failure"), "got: {rep_text:?}");
}

#[test]
fn two_distinct_themes_form_two_clusters() {
    let logs = s(&[
        // Disk-failure cluster
        "disk failure storage node alpha",
        "disk failure storage node beta",
        "disk failure storage node gamma",
        "disk failure storage node delta",
        // Network cluster
        "network timeout upstream auth",
        "network timeout upstream billing",
        "network timeout upstream catalog",
        "network timeout upstream payment",
    ]);
    let cfg = KnnConfig { k: 3, ..KnnConfig::default() };
    let out = knn_summary_with(&logs, &cfg);

    assert_eq!(ji(&out, "n_clusters"), 2, "expected 2 clusters, got {out}");

    // Both clusters should be size-4. Representatives carry one signature each.
    let clusters = ja(&out, "clusters");
    assert_eq!(clusters.len(), 2);
    assert_eq!(ji(clusters[0], "size"), 4);
    assert_eq!(ji(clusters[1], "size"), 4);

    let r0 = clusters[0]["representative"]["text"].as_str().unwrap();
    let r1 = clusters[1]["representative"]["text"].as_str().unwrap();
    let pair = format!("{r0}|{r1}");
    assert!(
        pair.contains("disk failure") && pair.contains("network timeout"),
        "expected one disk and one network rep, got: {pair:?}"
    );
}

#[test]
fn isolated_inputs_are_classified_as_anomalies() {
    let logs = s(&[
        // Cluster of similar lines (top-1 sim well above threshold)
        "disk failure storage node alpha",
        "disk failure storage node beta",
        "disk failure storage node gamma",
        "disk failure storage node delta",
        // Lone outliers — share no vocabulary with the cluster
        "weekly newsletter delivered today",
        "ferry timetable updated for spring",
    ]);
    let cfg = KnnConfig { k: 2, anomaly_threshold: 0.2, ..KnnConfig::default() };
    let out = knn_summary_with(&logs, &cfg);

    let n_anomalies = ji(&out, "n_anomalies");
    assert!(
        n_anomalies >= 1,
        "expected at least one anomaly for vocabulary-disjoint lines, got: {out}"
    );

    let anomalies = ja(&out, "anomalies");
    let texts: Vec<&str> = anomalies
        .iter()
        .filter_map(|a| a.get("text").and_then(|v| v.as_str()))
        .collect();
    let any_outlier = texts.iter().any(|t| t.contains("newsletter") || t.contains("ferry"));
    assert!(any_outlier, "expected at least one of the outliers in anomalies, got: {texts:?}");
}

#[test]
fn anomalies_are_sorted_most_isolated_first() {
    let logs = s(&[
        // Tight cluster
        "alpha alpha alpha",
        "alpha alpha alpha",
        "alpha alpha alpha",
        // Slight overlap (low but non-zero similarity)
        "alpha mostly different content here",
        // Total outlier
        "completely orthogonal log line",
    ]);
    let cfg = KnnConfig { k: 2, anomaly_threshold: 0.99, ..KnnConfig::default() };
    let out = knn_summary_with(&logs, &cfg);

    let anomalies = ja(&out, "anomalies");
    if anomalies.len() >= 2 {
        let mut prev = -1.0_f64;
        let mut first = true;
        for a in anomalies {
            let sim = jf(a, "max_similarity");
            if !first {
                assert!(
                    sim >= prev,
                    "anomalies must be sorted by max_similarity ascending, got prev={prev} sim={sim}"
                );
            }
            prev = sim;
            first = false;
        }
    }
}

#[test]
fn cluster_representative_has_highest_density() {
    let logs = s(&[
        "alpha beta gamma delta epsilon",
        "alpha beta gamma delta epsilon",
        "alpha beta gamma delta epsilon",
        "alpha beta gamma delta epsilon",
        "alpha beta only partial overlap",
    ]);
    let out = knn_summary(&logs);

    let clusters = ja(&out, "clusters");
    assert!(!clusters.is_empty());
    let cluster = clusters[0];

    let rep_density = jf(cluster.get("representative").unwrap(), "density");
    let members = ja(cluster, "members");
    for m in members {
        let d = jf(m, "density");
        assert!(
            rep_density + 1e-6 >= d,
            "representative density {rep_density} should be >= every member density (got {d})"
        );
    }
}

// ── config knobs ────────────────────────────────────────────────────────────────

#[test]
fn k_is_clamped_when_larger_than_corpus() {
    let logs = s(&["a a a", "a a b", "a b c"]);
    let cfg = KnnConfig { k: 50, ..KnnConfig::default() };
    let out = knn_summary_with(&logs, &cfg);
    let k_eff = ji(&out, "k");
    assert!(
        k_eff >= 1 && k_eff <= 2,
        "k must be clamped to n - 1 = 2, got {k_eff}"
    );
}

#[test]
fn max_cluster_members_caps_member_array() {
    let logs: Vec<String> = (0..30)
        .map(|i| format!("disk failure storage node {i} sector data"))
        .collect();
    let cfg = KnnConfig { k: 5, max_cluster_members: 4, ..KnnConfig::default() };
    let out = knn_summary_with(&logs, &cfg);

    let clusters = ja(&out, "clusters");
    assert_eq!(ji(clusters[0], "size"), 30, "size reflects true count");
    assert_eq!(
        ja(clusters[0], "members").len(),
        4,
        "members array capped to max_cluster_members"
    );
}

#[test]
fn max_anomalies_caps_anomalies_array() {
    let mut logs: Vec<String> = (0..20)
        .map(|i| format!("orthogonal line {i} unique-token-{i}"))
        .collect();
    // Add a tiny cluster so the anomalies-only path doesn't trigger.
    logs.push("shared shared shared shared".into());
    logs.push("shared shared shared shared".into());

    let cfg = KnnConfig { k: 3, anomaly_threshold: 0.99, max_anomalies: 5, ..KnnConfig::default() };
    let out = knn_summary_with(&logs, &cfg);

    let n_anomalies = ji(&out, "n_anomalies");
    let shown = ja(&out, "anomalies").len();
    assert!(n_anomalies as usize >= shown, "n_anomalies must be the true total");
    assert!(shown <= 5, "shown anomalies must be capped at max_anomalies, got {shown}");
}

#[test]
fn lower_anomaly_threshold_keeps_more_lines_in_clusters() {
    let logs = s(&[
        "alpha alpha alpha alpha",
        "alpha alpha alpha alpha",
        "alpha mostly different",
    ]);
    let strict = KnnConfig { k: 2, anomaly_threshold: 0.5, ..KnnConfig::default() };
    let lenient = KnnConfig { k: 2, anomaly_threshold: 0.05, ..KnnConfig::default() };

    let strict_out  = knn_summary_with(&logs, &strict);
    let lenient_out = knn_summary_with(&logs, &lenient);

    let strict_n  = ji(&strict_out,  "n_anomalies");
    let lenient_n = ji(&lenient_out, "n_anomalies");
    assert!(
        lenient_n <= strict_n,
        "a lower threshold cannot produce more anomalies (lenient={lenient_n}, strict={strict_n})"
    );
}

#[test]
fn min_word_len_filters_short_tokens() {
    // With min_word_len=3, "to" / "go" are dropped — but "alpha" / "beta" survive.
    let logs = s(&[
        "to go alpha alpha alpha",
        "to go alpha alpha alpha",
        "to go beta beta beta",
    ]);
    let cfg_default = KnnConfig::default();
    let cfg_strict  = KnnConfig { min_word_len: 4, ..KnnConfig::default() };

    let _ = knn_summary_with(&logs, &cfg_default);
    let _ = knn_summary_with(&logs, &cfg_strict);
    // Both invocations must succeed without panicking — main contract under
    // varying tokenisation aggressiveness.
}

// ── ranking / determinism ───────────────────────────────────────────────────────

#[test]
fn cluster_sizes_are_descending() {
    let logs = s(&[
        // Big cluster (5)
        "alpha alpha alpha alpha alpha", "alpha alpha alpha alpha alpha",
        "alpha alpha alpha alpha alpha", "alpha alpha alpha alpha alpha",
        "alpha alpha alpha alpha alpha",
        // Smaller cluster (3)
        "beta beta beta beta beta", "beta beta beta beta beta", "beta beta beta beta beta",
        // Smallest cluster (2)
        "gamma gamma gamma gamma", "gamma gamma gamma gamma",
    ]);
    let cfg = KnnConfig { k: 2, ..KnnConfig::default() };
    let out = knn_summary_with(&logs, &cfg);

    let clusters = ja(&out, "clusters");
    if clusters.len() >= 2 {
        let mut prev = i64::MAX;
        for c in clusters {
            let sz = ji(c, "size");
            assert!(sz <= prev, "cluster sizes must be non-increasing, got {sz} > {prev}");
            prev = sz;
        }
    }
}

#[test]
fn deterministic_output_for_identical_input() {
    let logs = s(&[
        "disk failure storage node alpha",
        "disk failure storage node beta",
        "disk failure storage node gamma",
        "network timeout upstream service",
        "network timeout upstream backend",
    ]);
    let a = knn_summary(&logs);
    let b = knn_summary(&logs);
    assert_eq!(a, b, "knn_summary must be deterministic");
}

#[test]
fn cluster_ids_are_dense_and_zero_based() {
    let logs = s(&[
        "alpha alpha alpha",
        "alpha alpha alpha",
        "beta beta beta",
        "beta beta beta",
        "gamma gamma gamma",
        "gamma gamma gamma",
    ]);
    let cfg = KnnConfig { k: 1, ..KnnConfig::default() };
    let out = knn_summary_with(&logs, &cfg);

    let clusters = ja(&out, "clusters");
    for (i, c) in clusters.iter().enumerate() {
        assert_eq!(ji(c, "id") as usize, i, "cluster ids must be dense 0..n_clusters");
    }
}

#[test]
fn density_is_bounded_in_unit_interval() {
    let logs = s(&[
        "alpha alpha alpha",
        "alpha alpha alpha",
        "beta beta beta",
        "gamma gamma gamma",
    ]);
    let out = knn_summary(&logs);
    for c in ja(&out, "clusters") {
        let d = jf(c.get("representative").unwrap(), "density");
        assert!(d >= -1e-6 && d <= 1.0 + 1e-6, "density must be in [0, 1], got {d}");
        for m in ja(c, "members") {
            let d = jf(m, "density");
            assert!(d >= -1e-6 && d <= 1.0 + 1e-6, "member density out of range: {d}");
        }
    }
}

#[test]
fn representatives_index_back_to_clusters() {
    let logs = s(&[
        "alpha alpha alpha",
        "alpha alpha alpha",
        "beta beta beta",
        "beta beta beta",
    ]);
    let cfg = KnnConfig { k: 1, ..KnnConfig::default() };
    let out = knn_summary_with(&logs, &cfg);

    let clusters = ja(&out, "clusters");
    let reps = ja(&out, "representatives");
    assert_eq!(reps.len(), clusters.len());

    for r in reps {
        let cid = ji(r, "cluster") as usize;
        assert!(cid < clusters.len(), "representative cluster id out of range: {cid}");
        let r_idx = ji(r, "idx");
        let c_rep_idx = ji(clusters[cid].get("representative").unwrap(), "idx");
        assert_eq!(r_idx, c_rep_idx, "representatives entry must match cluster.representative.idx");
    }
}

#[test]
fn duplicates_do_not_blow_up() {
    let logs = s(&[
        "the watchdog reset the device",
        "the watchdog reset the device",
        "the watchdog reset the device",
    ]);
    let out = knn_summary(&logs);
    assert_eq!(ji(&out, "n_logs"), 3);
    assert!(ji(&out, "n_clusters") >= 1, "duplicates should form at least one cluster");
}
