/// textrank_demo — Extractive summarisation via TextRank.
///
/// Walks through the public API of `bdslib::analysis::textrank`, applied to
/// three progressively realistic inputs:
///
///   1. A short paragraph about distributed systems       — classic text.
///   2. A burst of synthetic log lines                    — operational noise.
///   3. A list of JSON fingerprints                       — the future use
///                                                          case for clustering
///                                                          log entries.
///
/// Run with:
///
/// ```bash
/// cargo run --example textrank_demo
/// ```

use bdslib::{textrank_rank, textrank_summary, textrank_summary_with, TextRankConfig};

fn print_section(title: &str) {
    println!("\n=== {title} ===");
}

fn main() {
    // ── Section 1: A short text passage ──────────────────────────────────────

    print_section("Section 1 — Plain text");

    let passage: Vec<String> = [
        "Distributed systems pose unique challenges.",
        "Network partitions can split nodes into groups that cannot communicate.",
        "Consensus algorithms help nodes agree despite partitions and failures.",
        "Raft and Paxos are widely used consensus algorithms in distributed systems.",
        "A well-designed distributed system tolerates node failures gracefully.",
        "Quorum-based protocols ensure that distributed systems remain consistent.",
        "Operators must monitor distributed systems for partition events.",
    ].iter().map(|s| (*s).to_owned()).collect();

    let cfg = TextRankConfig { max_sentences: 3, ..TextRankConfig::default() };
    let summary = textrank_summary_with(&passage, &cfg);
    println!("Summary (top 3 sentences):\n  {summary}");

    println!("\nFull ranking:");
    for (rank, (idx, score)) in textrank_rank(&passage, &cfg).iter().enumerate() {
        println!("  #{:<2} score={:.4}  {}", rank + 1, score, passage[*idx]);
    }

    // ── Section 2: Synthetic log burst ───────────────────────────────────────

    print_section("Section 2 — Log burst");

    let log_lines: Vec<String> = [
        "2026-05-08T10:00:01 ERROR upstream timeout service=auth code=503",
        "2026-05-08T10:00:02 ERROR upstream timeout service=billing code=503",
        "2026-05-08T10:00:04 ERROR upstream timeout service=catalog code=503",
        "2026-05-08T10:00:05 ERROR upstream timeout service=auth code=503",
        "2026-05-08T10:00:07 INFO  worker started pid=4123",
        "2026-05-08T10:00:08 WARN  rate limit exceeded service=auth code=429",
        "2026-05-08T10:00:09 INFO  metrics flushed count=12",
    ].iter().map(|s| (*s).to_owned()).collect();

    let summary = textrank_summary(&log_lines);
    println!("Default summary (auto sizing):\n{summary}");

    // ── Section 3: JSON fingerprints (future use case) ───────────────────────

    print_section("Section 3 — JSON fingerprints");

    // These look like the output of `bdslib::common::jsonfingerprint::json_fingerprint`
    // applied to a cluster of structurally similar log records.
    let fingerprints: Vec<String> = [
        r#"event=login user=alice ip=10.0.0.1 result=success"#,
        r#"event=login user=alice ip=10.0.0.1 result=success"#,
        r#"event=login user=bob   ip=10.0.0.2 result=failure reason=bad-password"#,
        r#"event=login user=bob   ip=10.0.0.2 result=failure reason=bad-password"#,
        r#"event=login user=bob   ip=10.0.0.2 result=failure reason=bad-password"#,
        r#"event=heartbeat node=worker-3 status=ok"#,
        r#"event=heartbeat node=worker-7 status=ok"#,
    ].iter().map(|s| (*s).to_owned()).collect();

    let cfg = TextRankConfig { max_sentences: 2, ..TextRankConfig::default() };
    let summary = textrank_summary_with(&fingerprints, &cfg);
    println!("Top-2 fingerprints summarising the cluster:\n{summary}");

    println!("\nDetailed scores:");
    for (rank, (idx, score)) in textrank_rank(&fingerprints, &cfg).iter().enumerate() {
        println!("  #{:<2} score={:.4}  {}", rank + 1, score, fingerprints[*idx]);
    }

    // ── Section 4: Edge cases ────────────────────────────────────────────────

    print_section("Section 4 — Edge cases");

    let empty: Vec<String> = vec![];
    println!("empty input → {:?}", textrank_summary(&empty));

    let one = vec!["only one input".to_owned()];
    println!("single input → {:?}", textrank_summary(&one));

    let stops = vec![
        "the and is of".to_owned(),
        "to in on at".to_owned(),
    ];
    println!("stop-word-only inputs → {:?}", textrank_summary(&stops));

    println!("\nDone.");
}
