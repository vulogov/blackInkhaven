/// Tests for `bdslib::analysis::latentdirichletallocation`.
///
/// All assertions run in a single function because `OnceLock<ShardsManager>`
/// cannot be reset between test runs in the same process.
///
/// Ordering:
///   error paths (before init_db)
///   → init_db
///   → empty corpus
///   → log-entry corpus (string-rich data — good LDA signal)
///   → mixed key corpus
///   → LdaConfig overrides
use bdslib::{init_db, LdaConfig, TopicSummary};
use std::time::{SystemTime, UNIX_EPOCH};

fn write_config(dir: &tempfile::TempDir) -> String {
    let db_path = dir.path().join("db");
    std::fs::create_dir_all(&db_path).unwrap();
    let cfg = dir.path().join("bds.hjson");
    std::fs::write(
        &cfg,
        format!(
            "{{\n  dbpath: \"{}\"\n  shard_duration: \"1h\"\n  pool_size: 2\n}}\n",
            db_path.display()
        ),
    )
    .unwrap();
    cfg.to_str().unwrap().to_string()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn ingest(docs: Vec<serde_json::Value>) {
    bdslib::get_db().unwrap().add_batch(docs).unwrap();
}

#[test]
fn test_lda_lifecycle() {
    let t0 = now();

    // ── 1. before init_db → error ─────────────────────────────────────────────
    let err = TopicSummary::query("any.key", t0 - 3600, t0, LdaConfig::default())
        .err()
        .unwrap()
        .to_string();
    assert!(
        err.contains("not initialized"),
        "expected not-initialized error, got: {err}"
    );

    let err = TopicSummary::query_window("any.key", "1h", LdaConfig::default())
        .err()
        .unwrap()
        .to_string();
    assert!(
        err.contains("not initialized"),
        "expected not-initialized error before init, got: {err}"
    );

    // ── 2. init_db ────────────────────────────────────────────────────────────
    let dir = tempfile::TempDir::new().unwrap();
    init_db(Some(&write_config(&dir))).expect("init_db should succeed");

    // ── 3. empty corpus ───────────────────────────────────────────────────────
    let t = TopicSummary::query("no.such.key", t0 - 3600, t0 + 1, LdaConfig::default())
        .expect("empty-corpus query should not error");
    assert_eq!(t.n_docs, 0);
    assert_eq!(t.n_topics, 0);
    assert!(t.keywords.is_empty(), "keywords should be empty for n=0");

    // ── 4. syslog corpus — string-rich ────────────────────────────────────────
    // Ingest hand-crafted syslog-like documents; each has unique (key, data)
    // so the dedup layer stores them all as individual records.
    let syslog_start = t0 - 1800;
    let syslog_docs: Vec<serde_json::Value> = [
        ("sshd", "session opened for user admin"),
        ("sshd", "authentication failure for root from remote host"),
        ("sshd", "invalid user guest connection refused"),
        ("kernel", "out of memory oom killer invoked process killed"),
        ("kernel", "disk read error sector corrupted filesystem check"),
        ("nginx", "access log get request path api version data"),
        ("nginx", "upstream connection timeout backend server error"),
        ("cron", "scheduled job backup database completed successfully"),
        ("cron", "periodic task cleanup temp files executed"),
        ("systemd", "service unit restarted after failure dependency"),
        ("systemd", "started network manager connection established"),
        ("postfix", "mail delivery error bounce message user unknown"),
    ]
    .iter()
    .enumerate()
    .map(|(i, (prog, msg))| {
        serde_json::json!({
            "timestamp": syslog_start + i as u64,
            "key": "syslog",
            "data": {
                "program": prog,
                "message": msg,
                "pid": 1000 + i,
                "host": "server-01"
            }
        })
    })
    .collect();

    ingest(syslog_docs);

    let t = TopicSummary::query("syslog", syslog_start, syslog_start + 20, LdaConfig::default())
        .expect("syslog query should succeed");

    assert_eq!(t.n_docs, 12, "should find all 12 syslog docs");
    assert!(t.n_topics >= 1);
    assert!(
        !t.keywords.is_empty(),
        "keywords should not be empty for a rich corpus"
    );

    // Keywords must be sorted and comma-separated.
    let kws: Vec<&str> = t.keywords.split(", ").collect();
    assert!(kws.len() >= 2, "should extract at least a few keywords");
    let sorted_check = kws.windows(2).all(|w| w[0] <= w[1]);
    assert!(sorted_check, "keywords must be sorted alphabetically");

    // Deduplication: no keyword appears twice.
    let unique: std::collections::HashSet<&str> = kws.iter().copied().collect();
    assert_eq!(kws.len(), unique.len(), "each keyword must appear once");

    println!("syslog keywords: {}", t.keywords);

    // ── 5. query_window round-trip ────────────────────────────────────────────
    let tw = TopicSummary::query_window("syslog", "1h", LdaConfig::default())
        .expect("query_window should succeed");
    assert_eq!(tw.n_docs, 12);
    assert!(!tw.keywords.is_empty());

    // ── 6. config overrides ───────────────────────────────────────────────────
    // k=2, fewer topics → keyword set may differ but must still be valid.
    let cfg2 = LdaConfig { k: 2, top_n: 5, iters: 50, ..LdaConfig::default() };
    let t2 = TopicSummary::query("syslog", syslog_start, syslog_start + 20, cfg2)
        .expect("k=2 query should succeed");
    assert_eq!(t2.n_topics, 2);
    assert!(!t2.keywords.is_empty());

    // k=1 edge case
    let cfg1 = LdaConfig { k: 1, top_n: 3, ..LdaConfig::default() };
    let t1 = TopicSummary::query("syslog", syslog_start, syslog_start + 20, cfg1)
        .expect("k=1 query should succeed");
    assert_eq!(t1.n_topics, 1);
    assert!(!t1.keywords.is_empty());

    // ── 7. k > n_docs is clamped to n_docs ───────────────────────────────────
    let single_start = t0 - 1500;
    ingest(vec![serde_json::json!({
        "timestamp": single_start,
        "key": "demo.single",
        "data": { "message": "only one document here nothing else" }
    })]);

    let cfg_big_k = LdaConfig { k: 10, ..LdaConfig::default() };
    let ts = TopicSummary::query("demo.single", single_start, single_start + 1, cfg_big_k)
        .expect("k clamping should not error");
    assert_eq!(ts.n_docs, 1);
    assert_eq!(ts.n_topics, 1, "k should be clamped to n_docs=1");

    // ── 8. numeric-only data produces empty or minimal keywords ──────────────
    // Numeric leaf values are skipped; only the key name contributes text.
    let num_start = t0 - 1200;
    let num_docs: Vec<serde_json::Value> = (0..8)
        .map(|i| {
            serde_json::json!({
                "timestamp": num_start + i as u64,
                "key": "cpu.usage",
                "data": { "value": 50.0 + i as f64, "unit": "percent" }
            })
        })
        .collect();
    ingest(num_docs);

    let tn = TopicSummary::query("cpu.usage", num_start, num_start + 8, LdaConfig::default())
        .expect("numeric-data query should not error");
    // "unit" and "percent" are string leaves, plus key parts "cpu" / "usage" → some keywords
    assert!(tn.n_docs >= 1);
    // keywords may be sparse but must still satisfy the sorted/unique invariant.
    if !tn.keywords.is_empty() {
        let kws2: Vec<&str> = tn.keywords.split(", ").collect();
        let sorted2 = kws2.windows(2).all(|w| w[0] <= w[1]);
        assert!(sorted2, "keywords from numeric docs must also be sorted");
    }
    println!("cpu.usage keywords: {}", tn.keywords);
}
