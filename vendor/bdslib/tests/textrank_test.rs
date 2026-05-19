//! Tests for `bdslib::analysis::textrank`.
//!
//! Each `#[test]` covers one behaviour in isolation; together they verify the
//! contract advertised in the module rustdoc.

use bdslib::{textrank_rank, textrank_summary, textrank_summary_with, TextRankConfig};

fn s(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

#[test]
fn empty_input_returns_empty_string() {
    let out = textrank_summary(&Vec::<String>::new());
    assert_eq!(out, "");
}

#[test]
fn single_input_is_returned_verbatim() {
    let inputs = s(&["The system is up."]);
    assert_eq!(textrank_summary(&inputs), "The system is up.");
}

#[test]
fn two_identical_inputs_return_at_least_one() {
    let inputs = s(&["disk usage at 95%", "disk usage at 95%"]);
    let out = textrank_summary(&inputs);
    assert!(out.contains("disk usage at 95%"), "got: {out:?}");
}

#[test]
fn central_topic_sentence_outranks_unrelated_noise() {
    // The "system reboot" topic is repeated under different phrasings, while
    // each noise sentence is a one-off.  TextRank should pull the top-ranked
    // results from the topic cluster.
    let inputs = s(&[
        "The database connection pool was exhausted under load.",
        "The system reboot was triggered by a kernel panic.",
        "Today is sunny in Lisbon.",
        "The system reboot left several services in a degraded state.",
        "A system reboot was initiated by the watchdog timer.",
        "The cafeteria is serving lentil soup today.",
    ]);

    let cfg = TextRankConfig::default();
    let ranking = textrank_rank(&inputs, &cfg);

    // The top-2 results should both belong to the "system reboot" cluster.
    for (rank_idx, (i, _)) in ranking.iter().take(2).enumerate() {
        let text = inputs[*i].to_lowercase();
        assert!(
            text.contains("system") && text.contains("reboot"),
            "rank #{} sentence should belong to the system/reboot cluster, got: {:?}",
            rank_idx + 1,
            inputs[*i]
        );
    }

    // The two unrelated one-off sentences must not appear in the top-2.
    let top_two_text: String = ranking.iter().take(2)
        .map(|(i, _)| inputs[*i].as_str()).collect::<Vec<_>>().join(" ").to_lowercase();
    assert!(!top_two_text.contains("lisbon"), "noise should not be top-ranked: {top_two_text}");
    assert!(!top_two_text.contains("lentil"), "noise should not be top-ranked: {top_two_text}");
}

#[test]
fn summary_preserves_original_input_order() {
    // Summary must read in the same order as the inputs were supplied.
    let inputs = s(&[
        "Alpha keepalive ping arrived.",
        "Beta heartbeat ping arrived.",
        "Gamma heartbeat ping arrived.",
        "Delta heartbeat ping arrived.",
    ]);
    let cfg = TextRankConfig { max_sentences: 3, ..TextRankConfig::default() };
    let summary = textrank_summary_with(&inputs, &cfg);

    // The picked sentences appear in the summary in their input order.
    let mut last_pos: i64 = -1;
    let mut found = 0;
    for sentence in &inputs {
        if let Some(pos) = summary.find(sentence.as_str()) {
            assert!(pos as i64 > last_pos, "out-of-order: {summary:?}");
            last_pos = pos as i64;
            found += 1;
        }
    }
    assert_eq!(found, 3, "expected exactly 3 sentences in the summary, got summary: {summary:?}");
}

#[test]
fn max_sentences_caps_output_length() {
    let inputs = s(&[
        "Connection refused on port 5432.",
        "Database connection refused on port 5432.",
        "Connection refused while attaching to port 5432 of postgres.",
        "Connection refused on the postgres listener at port 5432.",
        "User logged in.",
        "User logged out.",
    ]);
    let cfg = TextRankConfig { max_sentences: 2, ..TextRankConfig::default() };
    let out = textrank_summary_with(&inputs, &cfg);

    // Count how many of the original inputs appear in the summary.
    let n_present = inputs.iter().filter(|s| out.contains(s.as_str())).count();
    assert_eq!(n_present, 2, "max_sentences=2 should yield exactly 2 inputs; got: {out:?}");
}

#[test]
fn ratio_used_when_max_sentences_zero() {
    let inputs = s(&[
        "alpha alpha alpha",
        "beta beta beta",
        "gamma gamma gamma",
        "delta delta delta",
        "epsilon epsilon epsilon",
        "zeta zeta zeta",
        "eta eta eta",
        "theta theta theta",
        "iota iota iota",
        "kappa kappa kappa",
    ]);
    let cfg = TextRankConfig { max_sentences: 0, ratio: 0.5, ..TextRankConfig::default() };
    let out = textrank_summary_with(&inputs, &cfg);

    let n_present = inputs.iter().filter(|s| out.contains(s.as_str())).count();
    assert_eq!(n_present, 5, "ratio=0.5 over 10 inputs → 5 sentences; got: {out:?}");
}

#[test]
fn duplicates_do_not_blow_up() {
    let inputs = s(&[
        "the watchdog reset the device",
        "the watchdog reset the device",
        "the watchdog reset the device",
    ]);
    // Should run cleanly and return a non-empty summary.
    let out = textrank_summary(&inputs);
    assert!(!out.is_empty());
}

#[test]
fn unicode_inputs_are_handled() {
    let inputs = s(&[
        "système de fichiers en lecture seule",
        "le système de fichiers est passé en lecture seule",
        "café servi à la cantine",
    ]);
    let ranking = textrank_rank(&inputs, &TextRankConfig::default());
    let top_text = inputs[ranking[0].0].as_str().to_lowercase();
    assert!(
        top_text.contains("système") && top_text.contains("lecture"),
        "expected the system-related sentence first, got: {top_text:?}"
    );
}

#[test]
fn log_fingerprint_clustering() {
    // Simulates the future use case: clustered log fingerprints.  Two
    // recurring failure patterns plus an unrelated one-off.  The summary
    // should surface the recurring patterns.
    let inputs = s(&[
        "level=error code=503 msg=upstream timeout service=auth",
        "level=error code=503 msg=upstream timeout service=billing",
        "level=error code=503 msg=upstream timeout service=catalog",
        "level=info  msg=worker started pid=4123",
        "level=warn  code=429 msg=rate limit exceeded service=auth",
        "level=warn  code=429 msg=rate limit exceeded service=billing",
        "level=info  msg=metrics flushed count=12",
    ]);

    let cfg = TextRankConfig { max_sentences: 3, ..TextRankConfig::default() };
    let summary = textrank_summary_with(&inputs, &cfg);

    // Either the upstream-timeout cluster or the rate-limit cluster should be
    // represented in the top-3.
    let lower = summary.to_lowercase();
    assert!(
        lower.contains("upstream timeout") || lower.contains("rate limit"),
        "summary should surface a recurring failure pattern, got: {summary:?}"
    );
    // Single-occurrence info lines shouldn't dominate the summary.
    assert!(
        !(lower.contains("worker started") && lower.contains("metrics flushed")),
        "summary should not pick both isolated info lines, got: {summary:?}"
    );
}

#[test]
fn ranking_lengths_match_input() {
    let inputs = s(&[
        "alpha alpha alpha",
        "beta gamma delta",
        "alpha beta gamma",
        "delta epsilon zeta",
    ]);
    let cfg = TextRankConfig::default();
    let ranking = textrank_rank(&inputs, &cfg);
    assert_eq!(ranking.len(), inputs.len());

    // Scores must be finite and in descending order.
    for w in ranking.windows(2) {
        assert!(w[0].1 >= w[1].1, "ranking not sorted: {ranking:?}");
        assert!(w[0].1.is_finite() && w[1].1.is_finite());
    }
    // All indices must be unique and in [0, n).
    let mut indices: Vec<usize> = ranking.iter().map(|(i, _)| *i).collect();
    indices.sort_unstable();
    indices.dedup();
    assert_eq!(indices.len(), inputs.len());
}

#[test]
fn stopword_only_inputs_do_not_panic() {
    // Each input contains nothing but stop-words; tokenisation drops them all,
    // similarity is zero everywhere, PageRank should still return scores and
    // the summary should fall back to a non-empty string.
    let inputs = s(&[
        "the and is of",
        "to in on at",
        "by for with as",
    ]);
    let out = textrank_summary(&inputs);
    assert!(!out.is_empty(), "expected fallback summary for stop-word-only inputs");
}
