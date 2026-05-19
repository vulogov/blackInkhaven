/// knn_demo — k-Nearest-Neighbour intelligence over a list of strings.
///
/// Walks through the public API of `bdslib::analysis::knn`, applied to four
/// progressively realistic inputs:
///
///   1. A two-themed log corpus  — clusters + anomalies appear together.
///   2. A pure-noise corpus      — every line is its own anomaly.
///   3. A dense same-template    — TF-IDF + k-NN graph collapses it cleanly.
///   4. Tunable `KnnConfig`      — what each knob does to the output.
///
/// Run with:
///
/// ```bash
/// cargo run --example knn_demo
/// ```

use bdslib::{knn_summary, knn_summary_with, KnnConfig};
use serde_json::Value as JsonValue;

fn print_section(title: &str) {
    println!("\n=== {title} ===");
}

fn pretty(v: &JsonValue) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

/// Single-line summary of the JSON outcome (useful when the full document
/// is too noisy for a quick eyeball).
fn summary_line(v: &JsonValue) {
    let n_logs       = v.get("n_logs").and_then(|x| x.as_u64()).unwrap_or(0);
    let k            = v.get("k").and_then(|x| x.as_u64()).unwrap_or(0);
    let n_clusters   = v.get("n_clusters").and_then(|x| x.as_u64()).unwrap_or(0);
    let n_anomalies  = v.get("n_anomalies").and_then(|x| x.as_u64()).unwrap_or(0);
    println!(
        "  → n_logs={n_logs} · k={k} · clusters={n_clusters} · anomalies={n_anomalies}"
    );
}

fn print_clusters_brief(v: &JsonValue) {
    if let Some(arr) = v.get("clusters").and_then(|x| x.as_array()) {
        for c in arr {
            let id   = c.get("id").and_then(|x| x.as_u64()).unwrap_or(0);
            let size = c.get("size").and_then(|x| x.as_u64()).unwrap_or(0);
            let rep  = c.get("representative")
                .and_then(|r| r.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            let dens = c.get("representative")
                .and_then(|r| r.get("density"))
                .and_then(|d| d.as_f64())
                .unwrap_or(0.0);
            println!(
                "    cluster #{id}  size={size}  rep_density={dens:.3}  rep={rep:?}"
            );
        }
    }
}

fn print_anomalies_brief(v: &JsonValue) {
    if let Some(arr) = v.get("anomalies").and_then(|x| x.as_array()) {
        for a in arr {
            let idx  = a.get("idx").and_then(|x| x.as_u64()).unwrap_or(0);
            let sim  = a.get("max_similarity").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let text = a.get("text").and_then(|x| x.as_str()).unwrap_or("");
            println!("    idx={idx}  max_sim={sim:.3}  text={text:?}");
        }
    }
}

fn main() {
    // ── Section 1: two themes + isolated outliers ───────────────────────────

    print_section("Section 1 — two themes + isolated outliers");

    let logs: Vec<String> = [
        // Cluster A — disk failures
        "ERROR disk failure detected on storage-1 sector 4096",
        "ERROR disk failure detected on storage-2 sector 8192",
        "ERROR disk failure detected on storage-3 sector 2048",
        "ERROR disk failure detected on storage-4 sector 1024",
        // Cluster B — network timeouts
        "WARN network timeout to upstream auth service",
        "WARN network timeout to upstream billing service",
        "WARN network timeout to upstream catalog service",
        "WARN network timeout to upstream payment service",
        // Anomalies — share no vocabulary with either theme
        "INFO scheduled backup completed successfully",
        "DEBUG metric flush count=12345 latency=4ms",
    ].iter().map(|s| (*s).to_owned()).collect();

    let result = knn_summary(&logs);
    summary_line(&result);
    println!("Clusters:");
    print_clusters_brief(&result);
    println!("Anomalies:");
    print_anomalies_brief(&result);

    // ── Section 2: pure noise corpus ────────────────────────────────────────

    print_section("Section 2 — pure noise (every line is its own outlier)");

    let logs: Vec<String> = [
        "alpha-event-1 done",
        "beta-event-2 done",
        "gamma-event-3 done",
        "delta-event-4 done",
    ].iter().map(|s| (*s).to_owned()).collect();

    // Strict threshold so all four are flagged as anomalies.
    let cfg = KnnConfig { k: 2, anomaly_threshold: 0.99, ..KnnConfig::default() };
    let result = knn_summary_with(&logs, &cfg);
    summary_line(&result);
    println!("Anomalies:");
    print_anomalies_brief(&result);

    // ── Section 3: dense same-template corpus ───────────────────────────────

    print_section("Section 3 — dense same-template corpus (30 lines, one cluster)");

    let logs: Vec<String> = (0..30)
        .map(|i| format!("disk failure storage node {i:03} sector {} data block", 4096 + i * 512))
        .collect();

    let result = knn_summary_with(&logs, &KnnConfig { k: 5, max_cluster_members: 4, ..KnnConfig::default() });
    summary_line(&result);
    println!("Clusters (members capped to 4 per cluster):");
    print_clusters_brief(&result);

    // ── Section 4: config knobs ────────────────────────────────────────────

    print_section("Section 4 — config knob effects");

    let logs: Vec<String> = [
        // Two clusters of three; one outlier
        "alpha alpha alpha shared content",
        "alpha alpha alpha shared content",
        "alpha alpha alpha shared content",
        "beta beta beta shared content",
        "beta beta beta shared content",
        "beta beta beta shared content",
        "completely orthogonal log line",
    ].iter().map(|s| (*s).to_owned()).collect();

    for (label, cfg) in [
        ("default",                       KnnConfig::default()),
        ("k=1",                           KnnConfig { k: 1, ..KnnConfig::default() }),
        ("strict anomaly_threshold=0.6",  KnnConfig { anomaly_threshold: 0.6, ..KnnConfig::default() }),
        ("lenient anomaly_threshold=0.0", KnnConfig { anomaly_threshold: 0.0, ..KnnConfig::default() }),
        ("max_cluster_members=1",         KnnConfig { max_cluster_members: 1, ..KnnConfig::default() }),
    ] {
        println!("\n[{label}]");
        let r = knn_summary_with(&logs, &cfg);
        summary_line(&r);
        print_clusters_brief(&r);
    }

    // ── Section 5: edge cases ──────────────────────────────────────────────

    print_section("Section 5 — edge cases");

    let empty: Vec<String> = vec![];
    let r = knn_summary(&empty);
    println!("empty input:");
    summary_line(&r);

    let one: Vec<String> = vec!["the only log line".into()];
    let r = knn_summary(&one);
    println!("\nsingle input:");
    summary_line(&r);
    print_clusters_brief(&r);

    let stops: Vec<String> = vec![
        "the and is of".into(),
        "to in on at".into(),
    ];
    let r = knn_summary(&stops);
    println!("\nstop-word-only inputs (all become anomalies):");
    summary_line(&r);

    // ── Section 6: full JSON for one example ───────────────────────────────

    print_section("Section 6 — full JSON output (small corpus)");

    let logs: Vec<String> = [
        "disk failure storage node 1",
        "disk failure storage node 2",
        "weekly newsletter delivered today",
    ].iter().map(|s| (*s).to_owned()).collect();
    let r = knn_summary(&logs);
    println!("{}", pretty(&r));

    println!("\nDone.");
}
