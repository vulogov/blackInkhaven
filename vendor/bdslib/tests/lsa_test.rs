//! Tests for `bdslib::analysis::lsa`.
//!
//! Each `#[test]` covers one behaviour in isolation; together they verify
//! the contract advertised in the module rustdoc and the Steinberger-Ježek
//! scoring semantics.
//!
//! Run with:
//! ```bash
//! cargo test --test lsa_test -- --show-output
//! ```

use bdslib::{lsa_rank, lsa_summary, lsa_summary_with, LsaConfig};

fn s(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

// ── edge cases ────────────────────────────────────────────────────────────────

#[test]
fn empty_input_returns_empty_string() {
    assert_eq!(lsa_summary(&Vec::<String>::new()), "");
}

#[test]
fn single_input_is_returned_verbatim() {
    let inputs = s(&["The system is up."]);
    assert_eq!(lsa_summary(&inputs), "The system is up.");
}

#[test]
fn two_inputs_returns_one_on_default_ratio() {
    let inputs = s(&["alpha alpha", "beta beta"]);
    let out = lsa_summary(&inputs);
    // default ratio 0.3 rounds up to 1 out of 2
    assert!(!out.is_empty());
}

#[test]
fn stopword_only_inputs_do_not_panic() {
    let inputs = s(&["the and is of", "to in on at", "by for with as"]);
    let out = lsa_summary(&inputs);
    assert!(!out.is_empty(), "expected non-empty fallback for stop-word-only inputs");
}

#[test]
fn unicode_inputs_are_handled() {
    let inputs = s(&[
        "système de fichiers en lecture seule",
        "le système de fichiers est passé en lecture seule",
        "café servi à la cantine",
    ]);
    let ranking = lsa_rank(&inputs, &LsaConfig::default());
    // The two "système de fichiers" sentences share vocabulary and should rank higher.
    let top_idx = ranking[0].0;
    let top_text = inputs[top_idx].to_lowercase();
    assert!(
        top_text.contains("système"),
        "expected a 'système' sentence at rank 1, got: {top_text:?}"
    );
}

// ── core LSA behaviour ────────────────────────────────────────────────────────

#[test]
fn repeated_topic_outranks_noise() {
    // The "disk failure" theme appears 4×; unrelated noise 2×.
    // LSA should promote the dominant theme.
    let inputs = s(&[
        "disk failure detected on node storage-1",
        "disk failure detected on node storage-2",
        "weather forecast for tomorrow is sunny",
        "disk failure detected on node storage-3",
        "disk failure detected on node storage-4",
        "local sports team won the championship",
    ]);

    let ranking = lsa_rank(&inputs, &LsaConfig::default());
    let top_2_texts: Vec<&str> = ranking
        .iter()
        .take(2)
        .map(|(i, _)| inputs[*i].as_str())
        .collect();

    for text in &top_2_texts {
        assert!(
            text.contains("disk failure"),
            "top-2 should contain disk-failure sentences, got: {text:?}"
        );
    }
}

#[test]
fn summary_preserves_original_input_order() {
    let inputs = s(&[
        "Alpha keepalive ping arrived.",
        "Beta heartbeat ping arrived.",
        "Gamma heartbeat ping arrived.",
        "Delta heartbeat ping arrived.",
    ]);
    let cfg = LsaConfig { max_sentences: 3, ..LsaConfig::default() };
    let summary = lsa_summary_with(&inputs, &cfg);

    // Every picked sentence must appear in the summary in its input order.
    let mut last_pos: i64 = -1;
    let mut found = 0usize;
    for sentence in &inputs {
        if let Some(pos) = summary.find(sentence.as_str()) {
            assert!(
                pos as i64 > last_pos,
                "out-of-order sentence detected: {summary:?}"
            );
            last_pos = pos as i64;
            found += 1;
        }
    }
    assert_eq!(found, 3, "expected exactly 3 sentences in summary: {summary:?}");
}

#[test]
fn max_sentences_caps_output_length() {
    let inputs = s(&[
        "Connection refused on port 5432.",
        "Database connection refused on port 5432.",
        "Connection refused while attaching to postgres port 5432.",
        "Connection refused on the postgres listener at port 5432.",
        "User logged in.",
        "User logged out.",
    ]);
    let cfg = LsaConfig { max_sentences: 2, ..LsaConfig::default() };
    let out = lsa_summary_with(&inputs, &cfg);
    let n_present = inputs.iter().filter(|s| out.contains(s.as_str())).count();
    assert_eq!(
        n_present, 2,
        "max_sentences=2 should yield exactly 2 inputs; got: {out:?}"
    );
}

#[test]
fn ratio_used_when_max_sentences_zero() {
    let inputs: Vec<String> = (0..10)
        .map(|i| format!("unique word{i} only{i} content{i}"))
        .collect();
    let cfg = LsaConfig { max_sentences: 0, ratio: 0.5, ..LsaConfig::default() };
    let out = lsa_summary_with(&inputs, &cfg);
    let n_present = inputs.iter().filter(|s| out.contains(s.as_str())).count();
    assert_eq!(
        n_present, 5,
        "ratio=0.5 over 10 inputs → 5 sentences; got: {out:?}"
    );
}

// ── ranking contract ──────────────────────────────────────────────────────────

#[test]
fn ranking_length_matches_input_count() {
    let inputs = s(&["alpha", "beta gamma", "gamma delta epsilon", "zeta"]);
    let ranking = lsa_rank(&inputs, &LsaConfig::default());
    assert_eq!(ranking.len(), inputs.len());
}

#[test]
fn ranking_scores_are_finite_and_descending() {
    let inputs = s(&[
        "network timeout on service auth",
        "network timeout on service billing",
        "user logged in successfully",
        "network timeout on service catalog",
        "database query completed",
    ]);
    let ranking = lsa_rank(&inputs, &LsaConfig::default());
    for w in ranking.windows(2) {
        assert!(
            w[0].1 >= w[1].1,
            "ranking not sorted: {ranking:?}"
        );
        assert!(w[0].1.is_finite(), "non-finite score: {:?}", w[0].1);
        assert!(w[1].1.is_finite(), "non-finite score: {:?}", w[1].1);
    }
}

#[test]
fn ranking_indices_are_unique_and_in_bounds() {
    let inputs = s(&["apple orange", "banana grape", "cherry plum", "fig date"]);
    let ranking = lsa_rank(&inputs, &LsaConfig::default());
    let mut indices: Vec<usize> = ranking.iter().map(|(i, _)| *i).collect();
    indices.sort_unstable();
    indices.dedup();
    assert_eq!(indices.len(), inputs.len());
    for i in &indices {
        assert!(*i < inputs.len());
    }
}

#[test]
fn duplicates_do_not_blow_up() {
    let inputs = s(&[
        "the watchdog reset the device",
        "the watchdog reset the device",
        "the watchdog reset the device",
    ]);
    let out = lsa_summary(&inputs);
    assert!(!out.is_empty());
}

// ── config variations ─────────────────────────────────────────────────────────

#[test]
fn more_concepts_does_not_panic() {
    let inputs = s(&[
        "cpu usage high on host web-1",
        "cpu usage high on host web-2",
        "memory pressure low on host db-1",
        "disk io spike on host storage-1",
        "network latency increased on host proxy-1",
    ]);
    // Request more concepts than useful — should not panic or error.
    let cfg = LsaConfig { n_concepts: 10, ..LsaConfig::default() };
    let out = lsa_summary_with(&inputs, &cfg);
    assert!(!out.is_empty());
}

#[test]
fn single_concept_still_produces_output() {
    let inputs = s(&[
        "authentication service restarted",
        "authentication service restarted due to OOM",
        "heartbeat from node-7",
    ]);
    let cfg = LsaConfig { n_concepts: 1, max_sentences: 2, ..LsaConfig::default() };
    let out = lsa_summary_with(&inputs, &cfg);
    assert!(!out.is_empty());
}

// ── log / operational input ───────────────────────────────────────────────────

#[test]
fn log_fingerprint_clustering() {
    let inputs = s(&[
        "level=error code=503 msg=upstream_timeout service=auth",
        "level=error code=503 msg=upstream_timeout service=billing",
        "level=error code=503 msg=upstream_timeout service=catalog",
        "level=info  msg=worker_started pid=4123",
        "level=warn  code=429 msg=rate_limit_exceeded service=auth",
        "level=warn  code=429 msg=rate_limit_exceeded service=billing",
        "level=info  msg=metrics_flushed count=12",
    ]);
    let cfg = LsaConfig { max_sentences: 3, ..LsaConfig::default() };
    let summary = lsa_summary_with(&inputs, &cfg);
    let lower = summary.to_lowercase();
    assert!(
        lower.contains("503") || lower.contains("upstream") || lower.contains("429"),
        "summary should surface a recurring failure pattern, got: {summary:?}"
    );
    // One-off info lines should not dominate.
    assert!(
        !(lower.contains("pid=4123") && lower.contains("count=12")),
        "both one-off info lines should not appear together in top-3, got: {summary:?}"
    );
}

#[test]
fn lsa_and_textrank_give_consistent_dominant_topic() {
    // Both algorithms should identify the dominant theme in an unbalanced corpus.
    use bdslib::{textrank_rank, TextRankConfig};

    let inputs = s(&[
        "disk failure on storage-1",
        "disk failure on storage-2",
        "disk failure on storage-3",
        "disk failure on storage-4",
        "sunny day today in the park",
    ]);

    let lsa_top = lsa_rank(&inputs, &LsaConfig::default())[0].0;
    let tr_top = textrank_rank(&inputs, &TextRankConfig::default())[0].0;

    // Both should put a "disk failure" sentence at rank 1.
    assert!(
        inputs[lsa_top].contains("disk failure"),
        "LSA rank-1 should be a disk-failure sentence, got: {:?}", inputs[lsa_top]
    );
    assert!(
        inputs[tr_top].contains("disk failure"),
        "TextRank rank-1 should be a disk-failure sentence, got: {:?}", inputs[tr_top]
    );
}
