//! Tests for `bdslib::analysis::ngram`.
//!
//! Covers both endpoints — `ngram_anomaly` and `ngram_remove_noise` —
//! and their config knobs.  The two share an internal pipeline, so a
//! single test file with two test groups gives full coverage.
//!
//! Run with:
//! ```bash
//! cargo test --test ngram_test -- --show-output
//! ```

use bdslib::{
    ngram_anomaly, ngram_anomaly_with, ngram_remove_noise, ngram_remove_noise_with,
    NgramAnomalyConfig, NgramNoiseConfig,
};
use serde_json::Value as JsonValue;

fn s(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

fn ji(v: &JsonValue, key: &str) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(-1)
}
fn jf(v: &JsonValue, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(f64::NAN)
}
fn ja<'a>(v: &'a JsonValue, key: &str) -> Vec<&'a JsonValue> {
    v.get(key).and_then(|x| x.as_array()).map(|a| a.iter().collect()).unwrap_or_default()
}

// ── ngram_anomaly: shape / edge cases ───────────────────────────────────────

#[test]
fn anomaly_empty_input_returns_empty_shape() {
    let out = ngram_anomaly(&Vec::<String>::new());
    assert_eq!(ji(&out, "n_logs"), 0);
    assert_eq!(ji(&out, "n_anomalies"), 0);
    assert_eq!(ji(&out, "n_unique_ngrams"), 0);
    assert!(ja(&out, "anomalies").is_empty());
}

#[test]
fn anomaly_output_has_required_keys() {
    let logs = s(&["alpha beta gamma", "delta epsilon zeta"]);
    let out = ngram_anomaly(&logs);
    for k in &[
        "n_logs", "n", "n_unique_ngrams",
        "anomaly_threshold", "n_anomalies", "mean_rarity", "anomalies",
    ] {
        assert!(out.get(*k).is_some(), "missing key: {k}");
    }
}

#[test]
fn anomaly_lines_too_short_for_n_score_zero() {
    // Default n=2; "hi" tokenises to one token, which is too short for a bigram.
    let logs = s(&["hi", "alpha beta gamma", "alpha beta gamma"]);
    let cfg = NgramAnomalyConfig { anomaly_threshold: 0.0, ..NgramAnomalyConfig::default() };
    let out = ngram_anomaly_with(&logs, &cfg);

    // Even with threshold 0.0, the "hi" line cannot be flagged because it
    // has no scorable n-grams and thus no rarity signal.
    let anomalies = ja(&out, "anomalies");
    let texts: Vec<&str> = anomalies.iter()
        .filter_map(|a| a.get("text").and_then(|v| v.as_str()))
        .collect();
    assert!(!texts.contains(&"hi"), "line with no n-grams must not be anomaly: {texts:?}");
}

// ── ngram_anomaly: core behaviour ──────────────────────────────────────────

#[test]
fn anomaly_repeated_theme_has_no_anomalies() {
    let logs = s(&[
        "disk failure detected on storage node alpha",
        "disk failure detected on storage node beta",
        "disk failure detected on storage node gamma",
        "disk failure detected on storage node delta",
        "disk failure detected on storage node epsilon",
    ]);
    let out = ngram_anomaly(&logs);
    assert_eq!(ji(&out, "n_anomalies"), 0,
               "homogeneous corpus should have no anomalies; got {out}");
}

#[test]
fn anomaly_isolated_outlier_is_flagged() {
    let logs = s(&[
        "disk failure detected on storage node alpha",
        "disk failure detected on storage node beta",
        "disk failure detected on storage node gamma",
        "disk failure detected on storage node delta",
        "disk failure detected on storage node epsilon",
        "completely orthogonal weather forecast for tomorrow",   // outlier
    ]);
    let out = ngram_anomaly(&logs);

    assert!(ji(&out, "n_anomalies") >= 1, "expected the outlier to be flagged: {out}");

    // The outlier should appear in the top-1 anomaly slot.
    let anomalies = ja(&out, "anomalies");
    let top = anomalies.first().expect("at least one anomaly");
    let text = top.get("text").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        text.contains("weather"),
        "expected weather outlier as top anomaly, got: {text:?}"
    );
}

#[test]
fn anomaly_results_sorted_by_rarity_descending() {
    let logs = s(&[
        "alpha beta", "alpha beta", "alpha beta",
        "alpha beta gamma",       // slightly more novel
        "completely unrelated phrase here",  // very novel
    ]);
    let cfg = NgramAnomalyConfig { anomaly_threshold: 0.0, ..NgramAnomalyConfig::default() };
    let out = ngram_anomaly_with(&logs, &cfg);

    let anomalies = ja(&out, "anomalies");
    if anomalies.len() >= 2 {
        let mut prev = f64::INFINITY;
        for a in anomalies {
            let r = jf(a, "rarity");
            assert!(r <= prev + 1e-6,
                    "anomalies must be sorted by rarity descending; got {r} after {prev}");
            prev = r;
        }
    }
}

#[test]
fn anomaly_max_anomalies_caps_array() {
    // 6 unique anomalies in a tiny background — most will exceed threshold.
    let logs = s(&[
        "alpha beta", "alpha beta", "alpha beta", "alpha beta", "alpha beta",
        "unique one", "unique two", "unique three", "unique four", "unique five",
    ]);
    let cfg = NgramAnomalyConfig {
        anomaly_threshold: 0.0,
        max_anomalies: 3,
        ..NgramAnomalyConfig::default()
    };
    let out = ngram_anomaly_with(&logs, &cfg);

    assert!(ji(&out, "n_anomalies") >= 5, "true total must reflect all unique lines");
    assert_eq!(ja(&out, "anomalies").len(), 3, "anomalies array capped at max_anomalies");
}

#[test]
fn anomaly_lower_threshold_allows_more() {
    let logs = s(&[
        "alpha beta",
        "alpha beta",
        "alpha gamma delta",       // moderately novel
    ]);
    let strict   = NgramAnomalyConfig { anomaly_threshold: 0.95, ..NgramAnomalyConfig::default() };
    let lenient  = NgramAnomalyConfig { anomaly_threshold: 0.0,  ..NgramAnomalyConfig::default() };

    let strict_n  = ji(&ngram_anomaly_with(&logs, &strict),  "n_anomalies");
    let lenient_n = ji(&ngram_anomaly_with(&logs, &lenient), "n_anomalies");
    assert!(lenient_n >= strict_n,
            "a lower threshold cannot produce fewer anomalies (lenient={lenient_n}, strict={strict_n})");
}

#[test]
fn anomaly_novel_ngrams_capped() {
    let logs = s(&[
        "common phrase common phrase common phrase",
        "common phrase common phrase common phrase",
        "rare a rare b rare c rare d rare e rare f rare g",
    ]);
    let cfg = NgramAnomalyConfig {
        anomaly_threshold: 0.0,
        max_novel_ngrams: 3,
        ..NgramAnomalyConfig::default()
    };
    let out = ngram_anomaly_with(&logs, &cfg);

    let anomalies = ja(&out, "anomalies");
    assert!(!anomalies.is_empty());
    let novel = ja(anomalies[0], "novel_ngrams");
    assert!(novel.len() <= 3, "novel_ngrams capped to max_novel_ngrams; got {}", novel.len());
}

#[test]
fn anomaly_n_eq_3_does_not_panic() {
    let logs = s(&[
        "alpha beta gamma delta epsilon",
        "alpha beta gamma delta epsilon",
        "alpha beta gamma zeta eta",
    ]);
    let cfg = NgramAnomalyConfig { n: 3, ..NgramAnomalyConfig::default() };
    let _ = ngram_anomaly_with(&logs, &cfg);
}

#[test]
fn anomaly_identical_duplicates_have_no_rarity() {
    let logs = s(&[
        "the watchdog reset the device",
        "the watchdog reset the device",
        "the watchdog reset the device",
    ]);
    let out = ngram_anomaly(&logs);
    assert_eq!(ji(&out, "n_anomalies"), 0,
               "all-identical lines have rarity 0; nothing to flag");
}

#[test]
fn anomaly_deterministic_output() {
    let logs = s(&[
        "disk failure detected on storage alpha",
        "disk failure detected on storage beta",
        "weather forecast for tomorrow is sunny",
    ]);
    let a = ngram_anomaly(&logs);
    let b = ngram_anomaly(&logs);
    assert_eq!(a, b, "ngram_anomaly must be deterministic");
}

#[test]
fn anomaly_mean_rarity_in_unit_interval() {
    let logs = s(&[
        "alpha beta gamma",
        "alpha beta delta",
        "epsilon zeta eta theta",
    ]);
    let mr = jf(&ngram_anomaly(&logs), "mean_rarity");
    assert!(mr >= -1e-6 && mr <= 1.0 + 1e-6, "mean_rarity must be in [0, 1]; got {mr}");
}

// ── ngram_remove_noise: shape / edge cases ──────────────────────────────────

#[test]
fn noise_empty_input_returns_empty_shape() {
    let out = ngram_remove_noise(&Vec::<String>::new());
    assert_eq!(ji(&out, "n_logs"), 0);
    assert_eq!(ji(&out, "n_kept"), 0);
    assert_eq!(ji(&out, "n_removed"), 0);
    assert!(ja(&out, "kept").is_empty());
    assert!(ja(&out, "removed").is_empty());
}

#[test]
fn noise_output_has_required_keys() {
    let logs = s(&["alpha beta gamma", "delta epsilon zeta"]);
    let out = ngram_remove_noise(&logs);
    for k in &[
        "n_logs", "n", "n_unique_ngrams",
        "noise_threshold", "n_kept", "n_removed", "kept", "removed",
    ] {
        assert!(out.get(*k).is_some(), "missing key: {k}");
    }
}

#[test]
fn noise_lines_too_short_for_n_are_kept() {
    // "hi" cannot produce a bigram → cannot be classified as noise → kept.
    let logs = s(&["hi", "common common common", "common common common"]);
    let cfg = NgramNoiseConfig { noise_threshold: 0.0, ..NgramNoiseConfig::default() };
    let out = ngram_remove_noise_with(&logs, &cfg);

    let kept = ja(&out, "kept");
    let kept_texts: Vec<&str> = kept.iter()
        .filter_map(|k| k.get("text").and_then(|v| v.as_str()))
        .collect();
    assert!(kept_texts.contains(&"hi"), "line with no n-grams must be kept: {kept_texts:?}");
}

// ── ngram_remove_noise: core behaviour ──────────────────────────────────────

#[test]
fn noise_all_identical_corpus_is_all_removed() {
    let logs = s(&[
        "identical line identical line",
        "identical line identical line",
        "identical line identical line",
        "identical line identical line",
    ]);
    let out = ngram_remove_noise(&logs);
    assert_eq!(ji(&out, "n_removed"), 4, "every line should be removed: {out}");
    assert_eq!(ji(&out, "n_kept"), 0);
}

#[test]
fn noise_mixed_corpus_separates_signal_from_noise() {
    let logs = s(&[
        // Background noise — repeated heartbeat
        "heartbeat ok heartbeat ok",
        "heartbeat ok heartbeat ok",
        "heartbeat ok heartbeat ok",
        "heartbeat ok heartbeat ok",
        "heartbeat ok heartbeat ok",
        // Signal — unique events
        "disk failure detected storage node alpha",
        "service crash payment subsystem",
    ]);
    // The default threshold 0.85 is intentionally strict — with a 5/7
    // heartbeat ratio the commonness only reaches ~0.71, which the
    // default would *not* class as noise.  Use an explicit threshold
    // to demonstrate the separation behaviour on this corpus shape.
    let cfg = NgramNoiseConfig { noise_threshold: 0.5, ..NgramNoiseConfig::default() };
    let out = ngram_remove_noise_with(&logs, &cfg);

    assert!(ji(&out, "n_removed") >= 4, "heartbeat should be classified as noise: {out}");
    assert!(ji(&out, "n_kept")    >= 2, "disk + service lines should be kept as signal");

    let kept_texts: Vec<&str> = ja(&out, "kept").iter()
        .filter_map(|k| k.get("text").and_then(|v| v.as_str()))
        .collect();
    assert!(kept_texts.iter().any(|t| t.contains("disk failure")),
            "disk failure must survive denoising; got: {kept_texts:?}");
    assert!(kept_texts.iter().any(|t| t.contains("service crash")),
            "service crash must survive denoising; got: {kept_texts:?}");
}

#[test]
fn noise_kept_plus_removed_equals_n_logs() {
    let logs = s(&[
        "alpha beta",
        "alpha beta",
        "gamma delta",
        "epsilon zeta",
    ]);
    let out = ngram_remove_noise(&logs);
    assert_eq!(
        ji(&out, "n_kept") + ji(&out, "n_removed"),
        ji(&out, "n_logs"),
        "every line must be either kept or removed: {out}"
    );
}

#[test]
fn noise_higher_threshold_removes_fewer() {
    let logs = s(&[
        "common common",
        "common common",
        "alpha beta",
        "gamma delta",
    ]);
    let strict   = NgramNoiseConfig { noise_threshold: 0.99, ..NgramNoiseConfig::default() };
    let lenient  = NgramNoiseConfig { noise_threshold: 0.0,  ..NgramNoiseConfig::default() };

    let strict_n  = ji(&ngram_remove_noise_with(&logs, &strict),  "n_removed");
    let lenient_n = ji(&ngram_remove_noise_with(&logs, &lenient), "n_removed");
    assert!(strict_n <= lenient_n,
            "a higher threshold cannot remove more (strict={strict_n}, lenient={lenient_n})");
}

#[test]
fn noise_kept_preserves_input_order() {
    let logs = s(&[
        "alpha beta",       // unique → kept
        "common common",    // noise
        "gamma delta",      // unique → kept
        "common common",    // noise
        "epsilon zeta",     // unique → kept
    ]);
    let cfg = NgramNoiseConfig { noise_threshold: 0.5, ..NgramNoiseConfig::default() };
    let out = ngram_remove_noise_with(&logs, &cfg);

    let kept = ja(&out, "kept");
    let mut prev_idx: i64 = -1;
    for k in kept {
        let idx = ji(k, "idx");
        assert!(idx > prev_idx, "kept must preserve input order; got {idx} after {prev_idx}");
        prev_idx = idx;
    }
}

#[test]
fn noise_removed_sorted_by_commonness_descending() {
    let logs = s(&[
        // Two slightly different noisy patterns + one genuinely unique line
        "ping ok ping ok ping ok",       // most common pattern (replicated below)
        "ping ok ping ok ping ok",
        "ping ok ping ok ping ok",
        "ping ok ping ok ping ok",
        "tick tock tick tock",            // less common pattern
        "tick tock tick tock",
        "alpha beta gamma delta",         // unique
    ]);
    let cfg = NgramNoiseConfig { noise_threshold: 0.2, ..NgramNoiseConfig::default() };
    let out = ngram_remove_noise_with(&logs, &cfg);

    let removed = ja(&out, "removed");
    if removed.len() >= 2 {
        let mut prev = f64::INFINITY;
        for r in removed {
            let c = jf(r, "commonness");
            assert!(c <= prev + 1e-6,
                    "removed must be sorted by commonness desc; got {c} after {prev}");
            prev = c;
        }
    }
}

#[test]
fn noise_max_kept_and_max_removed_caps_arrays() {
    let mut logs: Vec<String> = (0..20).map(|i| format!("unique tag-{i} content")).collect();
    for _ in 0..20 {
        logs.push("noise noise noise noise".into());
    }
    // With 20/40 noise lines the "noise noise" bigram has commonness 0.5
    // — below the 0.85 default.  Use a threshold below 0.5 to make every
    // noise line removable so the cap behaviour is testable.
    let cfg = NgramNoiseConfig {
        noise_threshold: 0.4,
        max_kept: 3,
        max_removed: 4,
        ..NgramNoiseConfig::default()
    };
    let out = ngram_remove_noise_with(&logs, &cfg);

    assert!(ji(&out, "n_kept")    >= 20, "true n_kept reflects unique lines: {out}");
    assert!(ji(&out, "n_removed") >= 20, "true n_removed reflects noisy lines: {out}");
    assert_eq!(ja(&out, "kept").len(),    3);
    assert_eq!(ja(&out, "removed").len(), 4);
}

#[test]
fn noise_n_eq_1_unigram_works() {
    let logs = s(&[
        "alpha alpha alpha",
        "alpha alpha alpha",
        "completely orthogonal",
    ]);
    let cfg = NgramNoiseConfig { n: 1, ..NgramNoiseConfig::default() };
    let _ = ngram_remove_noise_with(&logs, &cfg);
}

#[test]
fn noise_deterministic_output() {
    let logs = s(&[
        "alpha beta gamma",
        "alpha beta gamma",
        "delta epsilon zeta",
    ]);
    let a = ngram_remove_noise(&logs);
    let b = ngram_remove_noise(&logs);
    assert_eq!(a, b, "ngram_remove_noise must be deterministic");
}

// ── duality between the two endpoints ──────────────────────────────────────

#[test]
fn anomaly_and_noise_are_dual_views() {
    // A line that's classified as noise should NOT also be flagged as
    // anomalous by the other endpoint with sensible defaults.
    let logs = s(&[
        "common common common",
        "common common common",
        "common common common",
        "common common common",
        "rare distinct unique line",
    ]);

    let noise   = ngram_remove_noise(&logs);
    let anomaly = ngram_anomaly(&logs);

    // The unique line should be in `kept` (denoised) AND in anomalies.
    let kept_texts: Vec<&str> = ja(&noise, "kept").iter()
        .filter_map(|k| k.get("text").and_then(|v| v.as_str())).collect();
    let anomaly_texts: Vec<&str> = ja(&anomaly, "anomalies").iter()
        .filter_map(|a| a.get("text").and_then(|v| v.as_str())).collect();

    assert!(kept_texts.iter().any(|t| t.contains("rare")),
            "unique line should survive denoising");
    assert!(anomaly_texts.iter().any(|t| t.contains("rare")),
            "unique line should also surface as anomaly");
}
