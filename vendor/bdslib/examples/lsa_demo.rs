/// lsa_demo — Extractive summarisation via Latent Semantic Analysis (LSA).
///
/// Demonstrates the public API of `bdslib::analysis::lsa`, applied to three
/// progressively realistic inputs, followed by a comparison with TextRank on
/// the same corpus.
///
/// Algorithm sketch:
///   TF-IDF term–sentence matrix → centred Gram matrix (off-diagonal cosine
///   similarity) → truncated SVD via power iteration → Steinberger-Ježek score
///   √(Σₖ λₖ · v_k[j]²) per sentence.
///
/// Run with:
///
/// ```bash
/// cargo run --example lsa_demo
/// ```

use bdslib::{lsa_rank, lsa_summary, lsa_summary_with, textrank_rank, LsaConfig, TextRankConfig};

fn print_section(title: &str) {
    println!("\n=== {title} ===");
}

fn main() {
    // ── Section 1: Plain text passage ────────────────────────────────────────

    print_section("Section 1 — Plain text");

    let passage: Vec<String> = [
        "Distributed systems pose unique challenges.",
        "Network partitions can split nodes into groups that cannot communicate.",
        "Consensus algorithms help nodes agree despite partitions and failures.",
        "Raft and Paxos are widely used consensus algorithms in distributed systems.",
        "A well-designed distributed system tolerates node failures gracefully.",
        "Quorum-based protocols ensure that distributed systems remain consistent.",
        "Operators must monitor distributed systems for partition events.",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();

    let cfg = LsaConfig { max_sentences: 3, ..LsaConfig::default() };
    let summary = lsa_summary_with(&passage, &cfg);
    println!("LSA summary (top 3 sentences):\n  {summary}");

    println!("\nFull ranking:");
    for (rank, (idx, score)) in lsa_rank(&passage, &cfg).iter().enumerate() {
        println!("  #{:<2} score={:.4}  {}", rank + 1, score, passage[*idx]);
    }

    // ── Section 2: Operational log burst ────────────────────────────────────

    print_section("Section 2 — Log burst");

    let log_lines: Vec<String> = [
        "level=error code=503 msg=upstream_timeout service=auth",
        "level=error code=503 msg=upstream_timeout service=billing",
        "level=error code=503 msg=upstream_timeout service=catalog",
        "level=error code=503 msg=upstream_timeout service=auth",
        "level=info  msg=worker_started pid=4123",
        "level=warn  code=429 msg=rate_limit_exceeded service=auth",
        "level=info  msg=metrics_flushed count=12",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();

    let summary = lsa_summary(&log_lines);
    println!("Default summary (auto-sized):\n  {summary}");

    println!("\nFull ranking:");
    for (rank, (idx, score)) in lsa_rank(&log_lines, &LsaConfig::default()).iter().enumerate() {
        println!("  #{:<2} score={:.4}  {}", rank + 1, score, log_lines[*idx]);
    }

    // ── Section 3: Dominant-theme detection ──────────────────────────────────

    print_section("Section 3 — Dominant-theme detection");

    let mixed: Vec<String> = [
        "disk failure detected on node storage-1",
        "disk failure detected on node storage-2",
        "disk failure detected on node storage-3",
        "disk failure detected on node storage-4",
        "sunny day forecast for tomorrow",
        "local sports team won the championship",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();

    let lsa_top2: Vec<&str> = lsa_rank(&mixed, &LsaConfig::default())
        .iter()
        .take(2)
        .map(|(i, _)| mixed[*i].as_str())
        .collect();

    let tr_top2: Vec<&str> = textrank_rank(&mixed, &TextRankConfig::default())
        .iter()
        .take(2)
        .map(|(i, _)| mixed[*i].as_str())
        .collect();

    println!("LSA top-2:      {:?}", lsa_top2);
    println!("TextRank top-2: {:?}", tr_top2);
    println!("Both should surface disk-failure sentences despite the unrelated noise.");

    // ── Section 4: Config knobs ───────────────────────────────────────────────

    print_section("Section 4 — Config knobs");

    let inputs: Vec<String> = (0..10)
        .map(|i| format!("unique word{i} only{i} content{i}"))
        .collect();

    for ratio in [0.2_f32, 0.5, 1.0] {
        let cfg = LsaConfig { max_sentences: 0, ratio, ..LsaConfig::default() };
        let out = lsa_summary_with(&inputs, &cfg);
        let n = inputs.iter().filter(|s| out.contains(s.as_str())).count();
        println!("ratio={ratio:.1} → {n} sentences selected");
    }

    let cfg = LsaConfig { max_sentences: 3, ..LsaConfig::default() };
    let out = lsa_summary_with(&inputs, &cfg);
    let n = inputs.iter().filter(|s| out.contains(s.as_str())).count();
    println!("max_sentences=3 → {n} sentences selected");

    // ── Section 5: Edge cases ─────────────────────────────────────────────────

    print_section("Section 5 — Edge cases");

    let empty: Vec<String> = vec![];
    println!("empty input           → {:?}", lsa_summary(&empty));

    let one = vec!["only one input".to_owned()];
    println!("single input          → {:?}", lsa_summary(&one));

    let stops = vec![
        "the and is of".to_owned(),
        "to in on at".to_owned(),
        "by for with as".to_owned(),
    ];
    println!("stop-word-only inputs → {:?}", lsa_summary(&stops));

    let dups = vec![
        "watchdog reset the device".to_owned(),
        "watchdog reset the device".to_owned(),
        "watchdog reset the device".to_owned(),
    ];
    println!("identical duplicates  → {:?}", lsa_summary(&dups));

    println!("\nDone.");
}
