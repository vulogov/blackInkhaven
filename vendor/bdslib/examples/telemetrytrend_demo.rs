/// telemetrytrend_demo — exercises TelemetryTrend against several synthetic
/// time series, each designed to highlight a different statistical feature.
///
/// Sections:
///   1. Setup       — init_db + ingestion helpers
///   2. Ramp        — monotone 0‥N series; validates min/max/mean/median/std_dev
///   3. Constant    — flat series; std_dev = 0, no anomalies or breakouts
///   4. Outlier     — steady baseline with one extreme spike
///   5. Breakout    — step-function: abrupt mean shift halfway through
///   6. Oscillating — sine-like values; variability and anomaly rate shown
///   7. Real keys   — Generator::telemetry() ingestion + query_window
///   8. Empty key   — TelemetryTrend on a key that was never ingested
use bdslib::common::generator::Generator;
use bdslib::{get_db, init_db, TelemetryTrend};
use std::time::{SystemTime, UNIX_EPOCH};

// ── display ───────────────────────────────────────────────────────────────────

fn section(title: &str) {
    println!("\n{}", "─".repeat(72));
    println!("  {title}");
    println!("{}", "─".repeat(72));
}

fn show_trend(t: &TelemetryTrend) {
    if t.n == 0 {
        println!("  n=0  (no samples found)");
        return;
    }
    println!("  key        : {}", t.key);
    println!("  window     : [{}, {})", t.start, t.end);
    println!("  n          : {}", t.n);
    println!("  min / max  : {:.4} / {:.4}", t.min, t.max);
    println!("  mean       : {:.4}", t.mean);
    println!("  median     : {:.4}", t.median);
    println!("  std_dev    : {:.4}", t.std_dev);
    println!("  variability: {:.4}  (CV = std_dev / |mean|)", t.variability);

    if t.anomalies.is_empty() {
        println!("  anomalies  : none");
    } else {
        println!("  anomalies  : {} flagged", t.anomalies.len());
        for p in t.anomalies.iter().take(5) {
            println!("    [{}]  ts={}  value={:.4}", p.index, p.timestamp, p.value);
        }
        if t.anomalies.len() > 5 {
            println!("    … and {} more", t.anomalies.len() - 5);
        }
    }

    if t.breakouts.is_empty() {
        println!("  breakouts  : none");
    } else {
        println!("  breakouts  : {} detected", t.breakouts.len());
        for p in &t.breakouts {
            println!("    [{}]  ts={}  value={:.4}", p.index, p.timestamp, p.value);
        }
    }
}

// ── ingestion helpers ─────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Ingest `docs` into the global DB, panic on error.
fn ingest(docs: Vec<serde_json::Value>) -> usize {
    let n = docs.len();
    get_db().unwrap().add_batch(docs).unwrap();
    n
}

/// Build `n` documents for `key`, assigning each point a unique value
/// produced by `f(index)`.  Timestamps are spaced 1 second apart from `start`.
fn make_series<F>(key: &str, start: u64, n: usize, f: F) -> Vec<serde_json::Value>
where
    F: Fn(usize) -> f64,
{
    (0..n)
        .map(|i| {
            serde_json::json!({
                "timestamp": start + i as u64,
                "key": key,
                "data": { "value": f(i) }
            })
        })
        .collect()
}

// ── config ────────────────────────────────────────────────────────────────────

fn write_config(dir: &tempfile::TempDir) -> String {
    let db_path = dir.path().join("db");
    std::fs::create_dir_all(&db_path).unwrap();
    let cfg = dir.path().join("bds.hjson");
    std::fs::write(
        &cfg,
        format!(
            "{{\n  dbpath: \"{}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n}}\n",
            db_path.display()
        ),
    )
    .unwrap();
    cfg.to_str().unwrap().to_string()
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── 1. Setup ──────────────────────────────────────────────────────────────
    section("1. Setup  (init_db + DB singleton)");

    let tmp = tempfile::TempDir::new()?;
    let cfg = write_config(&tmp);
    init_db(Some(&cfg))?;
    println!("  init_db OK  —  config: {cfg}");

    let now = now_secs();
    // All series start inside the current hour so they land in one shard.
    // We spread them across different negative offsets so windows don't overlap.
    let t_ramp  = now - 3500;
    let t_const = now - 3300;
    let t_out   = now - 3000;
    let t_bk    = now - 2700;
    let t_osc   = now - 2400;

    // ── 2. Ramp series ────────────────────────────────────────────────────────
    section("2. Ramp series  (0 … 99)");

    let n = ingest(make_series("demo.ramp", t_ramp, 100, |i| i as f64));
    println!("  ingested {n} docs");

    let t = TelemetryTrend::query("demo.ramp", t_ramp, t_ramp + 100)?;
    show_trend(&t);

    // Sanity checks printed inline:
    println!();
    println!("  expected min=0      got {:.1}", t.min);
    println!("  expected max=99     got {:.1}", t.max);
    println!("  expected mean=49.5  got {:.4}", t.mean);
    println!("  expected median=49.5 got {:.4}", t.median);
    println!("  expected std_dev≈28.87 got {:.4}", t.std_dev);

    // ── 3. Constant series ────────────────────────────────────────────────────
    section("3. Constant series  (all values = 42.0)");

    // Give each point a tiny unique epsilon so the dedup layer treats them as
    // distinct records; the variation is invisible in the trend output.
    let n = ingest(make_series("demo.const", t_const, 50, |i| 42.0 + i as f64 * 1e-6));
    println!("  ingested {n} docs");

    let t = TelemetryTrend::query("demo.const", t_const, t_const + 50)?;
    show_trend(&t);
    println!();
    println!("  std_dev close to 0 : {:.2e}  (expected ~0)", t.std_dev);
    println!("  variability        : {:.2e}  (expected ~0)", t.variability);

    // ── 4. Outlier series ─────────────────────────────────────────────────────
    section("4. Outlier series  (baseline 5.0, spike 50000.0 at index 40)");

    let n = ingest(make_series("demo.outlier", t_out, 80, |i| {
        if i == 40 {
            50_000.0
        } else {
            5.0 + i as f64 * 0.01
        }
    }));
    println!("  ingested {n} docs");

    let t = TelemetryTrend::query("demo.outlier", t_out, t_out + 80)?;
    show_trend(&t);

    let flagged: Vec<usize> = t.anomalies.iter().map(|p| p.index).collect();
    println!();
    if flagged.contains(&40) {
        println!("  spike at index 40 correctly flagged as anomaly");
    } else {
        println!("  anomalies detected at: {flagged:?}  (spike=40)");
    }

    // ── 5. Breakout series ────────────────────────────────────────────────────
    section("5. Breakout series  (mean 10 for first 50, mean 90 for last 50)");

    let n = ingest(make_series("demo.breakout", t_bk, 100, |i| {
        let base = if i < 50 { 10.0_f64 } else { 90.0_f64 };
        base + i as f64 * 0.001
    }));
    println!("  ingested {n} docs");

    let t = TelemetryTrend::query("demo.breakout", t_bk, t_bk + 100)?;
    show_trend(&t);

    let bk_indices: Vec<usize> = t.breakouts.iter().map(|p| p.index).collect();
    println!();
    if bk_indices.iter().any(|&i| (40..60).contains(&i)) {
        println!("  breakout correctly detected near the step at index 50");
    } else {
        println!("  breakout indices: {bk_indices:?}  (step at index 50)");
    }

    // ── 6. Oscillating series ─────────────────────────────────────────────────
    section("6. Oscillating series  (sine wave, amplitude 20, centre 50)");

    let n = ingest(make_series("demo.osc", t_osc, 100, |i| {
        // Full-period sine over the 100 samples; each value is unique.
        let angle = (i as f64) * std::f64::consts::TAU / 100.0;
        50.0 + 20.0 * angle.sin() + i as f64 * 1e-5
    }));
    println!("  ingested {n} docs");

    let t = TelemetryTrend::query("demo.osc", t_osc, t_osc + 100)?;
    show_trend(&t);
    println!();
    println!("  mean ≈ 50 (centre of sine):  {:.4}", t.mean);
    println!(
        "  variability = {:.4}  (non-zero because amplitude ≠ 0)",
        t.variability
    );

    // ── 7. Real keys via Generator ────────────────────────────────────────────
    section("7. Real metric keys  (Generator::telemetry, 2h window)");

    let g = Generator::new();
    let gen_docs = g.telemetry("2h", 300);
    println!("  generated {} telemetry docs (2h window)", gen_docs.len());

    // Collect the distinct keys so we can query each one.
    let mut key_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for d in &gen_docs {
        if let Some(k) = d["key"].as_str() {
            *key_counts.entry(k.to_string()).or_insert(0) += 1;
        }
    }
    ingest(gen_docs);

    println!("  distinct keys in batch ({} total):", key_counts.len());
    for (k, c) in &key_counts {
        println!("    {c:3}×  {k}");
    }

    // Query the top-3 keys by sample count.
    println!();
    let mut top: Vec<_> = key_counts.iter().collect();
    top.sort_by_key(|&(_, c)| std::cmp::Reverse(c));
    for (key, _) in top.iter().take(3) {
        match TelemetryTrend::query_window(key, "3h") {
            Ok(t) => {
                println!("  key = {key}");
                show_trend(&t);
                println!();
            }
            Err(e) => println!("  {key}: query error — {e}"),
        }
    }

    // ── 8. Empty key ──────────────────────────────────────────────────────────
    section("8. Empty result  (key never ingested)");

    let t = TelemetryTrend::query("no.such.metric", now - 7200, now + 1)?;
    show_trend(&t);
    println!();
    println!("  min is NaN  : {}", t.min.is_nan());
    println!("  mean is NaN : {}", t.mean.is_nan());
    println!("  anomalies   : {}", t.anomalies.len());
    println!("  breakouts   : {}", t.breakouts.len());

    println!();
    Ok(())
}
