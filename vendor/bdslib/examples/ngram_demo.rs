/// ngram_demo — N-gram anomaly detection and noise removal.
///
/// Walks the public API of `bdslib::analysis::ngram`:
///
///   - `ngram_anomaly([logs])`        → flag lines using rare n-grams
///   - `ngram_remove_noise([logs])`   → split signal from repetitive noise
///
/// Sections:
///
///   1. Anomaly detection on a clear-outlier corpus.
///   2. Noise removal on a noisy heartbeat corpus.
///   3. The two endpoints applied to the same corpus side by side
///      (demonstrating that they're duals).
///   4. Config-knob effects: `n` (bigrams vs trigrams), thresholds.
///   5. Edge cases.
///   6. Full JSON output for one small corpus.
///
/// Run with:
///
/// ```bash
/// cargo run --example ngram_demo
/// ```

use bdslib::{
    ngram_anomaly, ngram_anomaly_with, ngram_remove_noise, ngram_remove_noise_with,
    NgramAnomalyConfig, NgramNoiseConfig,
};
use serde_json::Value as JsonValue;

fn print_section(title: &str) {
    println!("\n=== {title} ===");
}

fn pretty(v: &JsonValue) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

fn anomaly_brief(v: &JsonValue) {
    let n_logs       = v.get("n_logs").and_then(|x| x.as_u64()).unwrap_or(0);
    let n            = v.get("n").and_then(|x| x.as_u64()).unwrap_or(0);
    let n_anomalies  = v.get("n_anomalies").and_then(|x| x.as_u64()).unwrap_or(0);
    let mean_rarity  = v.get("mean_rarity").and_then(|x| x.as_f64()).unwrap_or(0.0);
    println!("  → n_logs={n_logs}  n={n}  anomalies={n_anomalies}  mean_rarity={mean_rarity:.3}");
    if let Some(arr) = v.get("anomalies").and_then(|x| x.as_array()) {
        for a in arr {
            let idx    = a.get("idx").and_then(|x| x.as_u64()).unwrap_or(0);
            let rarity = a.get("rarity").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let text   = a.get("text").and_then(|x| x.as_str()).unwrap_or("");
            let novel: Vec<&str> = a.get("novel_ngrams")
                .and_then(|x| x.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            println!("    idx={idx}  rarity={rarity:.3}  novel={novel:?}");
            println!("      text={text:?}");
        }
    }
}

fn noise_brief(v: &JsonValue) {
    let n_logs    = v.get("n_logs").and_then(|x| x.as_u64()).unwrap_or(0);
    let n         = v.get("n").and_then(|x| x.as_u64()).unwrap_or(0);
    let n_kept    = v.get("n_kept").and_then(|x| x.as_u64()).unwrap_or(0);
    let n_removed = v.get("n_removed").and_then(|x| x.as_u64()).unwrap_or(0);
    println!("  → n_logs={n_logs}  n={n}  kept={n_kept}  removed={n_removed}");
    println!("  Kept (signal):");
    if let Some(arr) = v.get("kept").and_then(|x| x.as_array()) {
        for k in arr.iter().take(5) {
            let idx        = k.get("idx").and_then(|x| x.as_u64()).unwrap_or(0);
            let commonness = k.get("commonness").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let text       = k.get("text").and_then(|x| x.as_str()).unwrap_or("");
            println!("    idx={idx}  commonness={commonness:.3}  text={text:?}");
        }
    }
    println!("  Removed (noise):");
    if let Some(arr) = v.get("removed").and_then(|x| x.as_array()) {
        for r in arr.iter().take(5) {
            let idx        = r.get("idx").and_then(|x| x.as_u64()).unwrap_or(0);
            let commonness = r.get("commonness").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let text       = r.get("text").and_then(|x| x.as_str()).unwrap_or("");
            println!("    idx={idx}  commonness={commonness:.3}  text={text:?}");
        }
    }
}

fn main() {
    // ── Section 1: anomaly detection on a clear-outlier corpus ──────────────

    print_section("Section 1 — anomaly detection: clear outlier in a tight cluster");

    let logs: Vec<String> = [
        "ERROR upstream timeout service auth code 503",
        "ERROR upstream timeout service billing code 503",
        "ERROR upstream timeout service catalog code 503",
        "ERROR upstream timeout service payment code 503",
        "ERROR upstream timeout service auth code 503",
        "INFO scheduled backup completed at 03:00 utc",      // outlier
    ].iter().map(|s| (*s).to_owned()).collect();

    let cfg = NgramAnomalyConfig { anomaly_threshold: 0.6, ..NgramAnomalyConfig::default() };
    anomaly_brief(&ngram_anomaly_with(&logs, &cfg));

    // ── Section 2: noise removal on a noisy heartbeat corpus ────────────────

    print_section("Section 2 — noise removal: heartbeat-buried signal");

    let logs: Vec<String> = [
        "heartbeat ok node1 status nominal",
        "heartbeat ok node2 status nominal",
        "heartbeat ok node3 status nominal",
        "heartbeat ok node4 status nominal",
        "heartbeat ok node5 status nominal",
        "heartbeat ok node6 status nominal",
        "heartbeat ok node7 status nominal",
        "heartbeat ok node8 status nominal",
        "ALERT memory pressure on node5 swap usage critical",   // signal
        "ALERT disk failure detected on storage subsystem",     // signal
    ].iter().map(|s| (*s).to_owned()).collect();

    // The heartbeat lines share the bigrams "heartbeat ok", "ok node*",
    // and "status nominal".  With 8 of 10 lines being heartbeats their
    // commonness sits around 0.45 — pick a threshold below that to
    // surface the noise/signal split this section is meant to demonstrate.
    let cfg = NgramNoiseConfig { noise_threshold: 0.4, ..NgramNoiseConfig::default() };
    noise_brief(&ngram_remove_noise_with(&logs, &cfg));

    // ── Section 3: duality — same corpus, both endpoints ────────────────────

    print_section("Section 3 — duality: a single corpus through both endpoints");

    let logs: Vec<String> = [
        "ping ok ping ok ping ok",
        "ping ok ping ok ping ok",
        "ping ok ping ok ping ok",
        "ping ok ping ok ping ok",
        "rare distinct unique line never seen before",
    ].iter().map(|s| (*s).to_owned()).collect();

    println!("Anomaly view:");
    anomaly_brief(&ngram_anomaly_with(
        &logs,
        &NgramAnomalyConfig { anomaly_threshold: 0.5, ..NgramAnomalyConfig::default() },
    ));
    println!("Noise-removal view:");
    noise_brief(&ngram_remove_noise_with(
        &logs,
        &NgramNoiseConfig { noise_threshold: 0.5, ..NgramNoiseConfig::default() },
    ));
    println!("Note: the unique line surfaces as the only anomaly AND survives denoising.");

    // ── Section 4: config knobs ─────────────────────────────────────────────

    print_section("Section 4 — config knobs: bigrams vs trigrams");

    let logs: Vec<String> = [
        "alpha beta gamma delta epsilon",
        "alpha beta gamma delta epsilon",
        "alpha beta gamma delta zeta",       // novel only at the end
        "alpha beta gamma delta epsilon",
        "alpha beta gamma delta eta",        // novel only at the end
    ].iter().map(|s| (*s).to_owned()).collect();

    println!("\n[n=2 bigrams, default]");
    anomaly_brief(&ngram_anomaly(&logs));

    println!("\n[n=3 trigrams — captures the trailing-token differences as more novel]");
    let cfg = NgramAnomalyConfig { n: 3, anomaly_threshold: 0.0, ..NgramAnomalyConfig::default() };
    anomaly_brief(&ngram_anomaly_with(&logs, &cfg));

    // ── Section 5: edge cases ───────────────────────────────────────────────

    print_section("Section 5 — edge cases");

    println!("Empty input (anomaly):");
    anomaly_brief(&ngram_anomaly(&Vec::<String>::new()));

    println!("\nSingle input (anomaly):");
    anomaly_brief(&ngram_anomaly(&vec!["the only line".into()]));

    println!("\nLine too short for n=2 (anomaly):");
    anomaly_brief(&ngram_anomaly(&vec![
        "hi".into(),
        "hello world greetings".into(),
        "hello world greetings".into(),
    ]));

    println!("\nAll-identical corpus (noise removal):");
    noise_brief(&ngram_remove_noise(&vec![
        "identical line".into(),
        "identical line".into(),
        "identical line".into(),
    ]));

    // ── Section 6: full JSON for a small corpus ─────────────────────────────

    print_section("Section 6 — full JSON output (small corpus)");

    let logs: Vec<String> = [
        "disk failure storage node 1",
        "disk failure storage node 2",
        "weekly newsletter delivered today",
    ].iter().map(|s| (*s).to_owned()).collect();

    println!("ngram_anomaly:");
    println!("{}", pretty(&ngram_anomaly(&logs)));

    println!("\nngram_remove_noise:");
    println!("{}", pretty(&ngram_remove_noise_with(
        &logs,
        &NgramNoiseConfig { noise_threshold: 0.5, ..NgramNoiseConfig::default() },
    )));

    println!("\nDone.");
}
