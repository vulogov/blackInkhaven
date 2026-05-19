/// Tests for `bdslib::analysis::telemetrytrend`.
///
/// All tests share one init_db call because OnceLock cannot be reset.
/// Ordering:
///   error paths (get_db not ready)
///   → init_db with a valid config
///   → ingest known telemetry via Generator
///   → TelemetryTrend::query / query_window correctness
use bdslib::common::generator::Generator;
use bdslib::{init_db, SamplePoint, TelemetryTrend};

// ── config helper ─────────────────────────────────────────────────────────────

fn write_config(dir: &tempfile::TempDir) -> String {
    let db_path = dir.path().join("db");
    std::fs::create_dir_all(&db_path).unwrap();
    let config_path = dir.path().join("bds.hjson");
    std::fs::write(
        &config_path,
        format!(
            "{{\n  dbpath: \"{}\"\n  shard_duration: \"1h\"\n  pool_size: 2\n}}\n",
            db_path.display()
        ),
    )
    .unwrap();
    config_path.to_str().unwrap().to_string()
}

// ── synthetic trend helpers ───────────────────────────────────────────────────

/// Return a deterministic ascending ramp of `n` values for `key`.
fn make_trend_docs(key: &str, n: usize, start_secs: u64) -> Vec<serde_json::Value> {
    (0..n)
        .map(|i| {
            serde_json::json!({
                "timestamp": start_secs + i as u64,
                "key": key,
                "data": { "value": i as f64 * 1.0 }
            })
        })
        .collect()
}

/// Return docs with a clear outlier at position `outlier_idx`.
///
/// Each "normal" point has a small unique perturbation so deduplication does
/// not collapse them all into a single primary record.
fn make_docs_with_outlier(
    key: &str,
    n: usize,
    start_secs: u64,
    outlier_idx: usize,
    outlier_value: f64,
) -> Vec<serde_json::Value> {
    (0..n)
        .map(|i| {
            // Unique value per point avoids the (key, data) exact-match dedup.
            let v = if i == outlier_idx {
                outlier_value
            } else {
                5.0 + (i as f64) * 0.001
            };
            serde_json::json!({
                "timestamp": start_secs + i as u64,
                "key": key,
                "data": { "value": v }
            })
        })
        .collect()
}

// ── tests ──────────────────────────────────────────────────────────────────────

#[test]
fn test_telemetrytrend_lifecycle() {
    // ── 1. query before init returns error ───────────────────────────────────
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let err = TelemetryTrend::query("cpu.usage", now - 3600, now)
        .err()
        .unwrap()
        .to_string();
    assert!(
        err.contains("not initialized"),
        "expected not-initialized error, got: {err}"
    );

    let err = TelemetryTrend::query_window("cpu.usage", "1h")
        .err()
        .unwrap()
        .to_string();
    assert!(
        err.contains("not initialized"),
        "expected not-initialized error before init, got: {err}"
    );

    // ── 2. init_db ────────────────────────────────────────────────────────────
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = write_config(&dir);
    init_db(Some(&config_path)).expect("init_db should succeed");

    // ── 3. empty result for unknown key ───────────────────────────────────────
    let trend = TelemetryTrend::query("no.such.key", now - 3600, now + 1)
        .expect("query should succeed even for empty result");
    assert_eq!(trend.n, 0, "no docs for unknown key");
    assert!(trend.min.is_nan());
    assert!(trend.max.is_nan());
    assert!(trend.mean.is_nan());
    assert!(trend.anomalies.is_empty());
    assert!(trend.breakouts.is_empty());

    // ── 4. ingest deterministic ramp ─────────────────────────────────────────
    // 100 docs, value = 0..99, timestamps starting 30 min ago so they fall in
    // the current hour shard and within a 2h query window.
    let ramp_start = now - 1800;
    let ramp_key = "test.ramp";
    let ramp_docs = make_trend_docs(ramp_key, 100, ramp_start);

    use bdslib::get_db;
    get_db()
        .unwrap()
        .add_batch(ramp_docs)
        .expect("add_batch should succeed");

    // ── 5. TelemetryTrend::query basic stats ─────────────────────────────────
    let t = TelemetryTrend::query(ramp_key, ramp_start, ramp_start + 100)
        .expect("query ramp should succeed");

    assert_eq!(t.n, 100, "should find all 100 ramp docs");
    assert_eq!(t.key, ramp_key);
    assert!((t.min - 0.0).abs() < 1e-9, "min should be 0, got {}", t.min);
    assert!(
        (t.max - 99.0).abs() < 1e-9,
        "max should be 99, got {}",
        t.max
    );
    // arithmetic mean of 0..99 = 49.5
    assert!(
        (t.mean - 49.5).abs() < 1e-6,
        "mean should be 49.5, got {}",
        t.mean
    );
    // median of 0..99 (100 values) = 49.5
    assert!(
        (t.median - 49.5).abs() < 1e-6,
        "median should be 49.5, got {}",
        t.median
    );
    // population std dev of 0..99 = sqrt(8325/10) ≈ 28.866
    assert!(
        (t.std_dev - 28.8661).abs() < 0.001,
        "std_dev should be ~28.866, got {}",
        t.std_dev
    );
    // variability = std_dev / mean ≈ 0.584
    assert!(
        t.variability > 0.0,
        "variability should be positive, got {}",
        t.variability
    );

    // ── 6. query_window also works ────────────────────────────────────────────
    let tw = TelemetryTrend::query_window(ramp_key, "2h").expect("query_window should succeed");
    assert_eq!(tw.n, 100, "query_window should find the 100 ramp docs");

    // ── 7. ingest data with a known outlier ───────────────────────────────────
    let outlier_key = "test.outlier";
    let outlier_start = now - 900;
    let outlier_docs = make_docs_with_outlier(outlier_key, 60, outlier_start, 30, 1_000_000.0);
    get_db()
        .unwrap()
        .add_batch(outlier_docs)
        .expect("add_batch with outlier should succeed");

    let ot = TelemetryTrend::query(outlier_key, outlier_start, outlier_start + 60)
        .expect("query outlier should succeed");
    assert_eq!(ot.n, 60);
    assert!(
        (ot.max - 1_000_000.0).abs() < 1e-3,
        "max should be the outlier value"
    );
    // The S-H-ESD algorithm should flag the extreme outlier.
    assert!(
        !ot.anomalies.is_empty(),
        "expected at least one anomaly for extreme outlier series"
    );
    // The outlier should be among the flagged indices.
    let flagged: Vec<usize> = ot.anomalies.iter().map(|p| p.index).collect();
    assert!(
        flagged.contains(&30),
        "index 30 (outlier) should be flagged; got anomalies at: {flagged:?}"
    );

    // ── 8. SamplePoint fields are populated ───────────────────────────────────
    let pt: &SamplePoint = &ot.anomalies[0];
    assert!(pt.timestamp >= outlier_start, "anomaly timestamp should be within window");
    assert!(pt.value > 0.0, "anomaly value should be set");

    // ── 9. ingest data with a breakout ────────────────────────────────────────
    // 80 docs: first 40 near 10.0, last 40 near 100.0 → clear distribution shift.
    let bk_key = "test.breakout";
    let bk_start = now - 600;
    // Small unique perturbation per point keeps all 80 records out of the
    // exact-match dedup path while preserving the step-function shape.
    let bk_docs: Vec<serde_json::Value> = (0..80)
        .map(|i| {
            let base = if i < 40 { 10.0_f64 } else { 100.0_f64 };
            let v = base + (i as f64) * 0.001;
            serde_json::json!({
                "timestamp": bk_start + i as u64,
                "key": bk_key,
                "data": { "value": v }
            })
        })
        .collect();
    get_db()
        .unwrap()
        .add_batch(bk_docs)
        .expect("add_batch breakout should succeed");

    let bt = TelemetryTrend::query(bk_key, bk_start, bk_start + 80)
        .expect("query breakout should succeed");
    assert_eq!(bt.n, 80);
    assert!(
        !bt.breakouts.is_empty(),
        "expected at least one breakout for step-function series"
    );
    // Breakout should be near index 40 (within a few positions).
    let bk_indices: Vec<usize> = bt.breakouts.iter().map(|p| p.index).collect();
    assert!(
        bk_indices.iter().any(|&i| (30..50).contains(&i)),
        "breakout should be detected near the step at index 40; got: {bk_indices:?}"
    );

    // ── 10. Generator-based ingestion round-trip ──────────────────────────────
    let g = Generator::new();
    let gen_docs = g.telemetry("1h", 50);
    get_db()
        .unwrap()
        .add_batch(gen_docs)
        .expect("add_batch generator telemetry should succeed");

    // query_window should return at least the 50 generator docs.
    // We use a key that the generator actually emits.
    let gen_trend = TelemetryTrend::query_window("cpu.usage", "2h");
    // cpu.usage may or may not have been emitted; just verify no error.
    assert!(
        gen_trend.is_ok(),
        "query_window for generator data should not error"
    );
}
