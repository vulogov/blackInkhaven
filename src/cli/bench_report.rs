//! 1.2.18+ I.1.7 — criterion bench-result comparison +
//! CI regression gate.
//!
//! `inkhaven _bench-report` (hidden) walks two criterion
//! output trees — a `--baseline` (restored from the main
//! branch's last run) and a `--current` (this run) —
//! extracts each bench's median estimate, computes the
//! percent delta, prints a markdown table suitable for a
//! PR comment, and exits non-zero when any scenario
//! regresses past `--threshold` (default 20 %).
//!
//! ## Why median, not mean?
//!
//! criterion reports both; the median is robust to the
//! subprocess-spawn outliers our benches inevitably
//! produce on shared CI runners.
//!
//! ## A note on shared-runner noise
//!
//! Absolute wall-clock drifts between GitHub runners, so
//! a tight (e.g. 5 %) gate would flap.  The 20 % default
//! is deliberately loose: the regressions worth catching
//! here are *algorithmic* — re-introducing the eager
//! embedding load (I.1.4) or the O(n²) flatten (I.1.5)
//! shows up as a multiple, not a 20 % drift.  The gate
//! catches order-of-magnitude regressions reliably even
//! when the runner is noisy.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

/// One bench's median timing, keyed by its criterion id
/// (`<group>/<bench>`, e.g. `startup/cold_list`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BenchSample {
    pub id: String,
    pub median_ns: f64,
}

/// Per-bench comparison row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompareRow {
    pub id: String,
    pub baseline_ns: Option<f64>,
    pub current_ns: f64,
    /// Fractional delta `(current - baseline) / baseline`.
    /// `None` when the bench is new (no baseline).
    pub delta: Option<f64>,
    pub regressed: bool,
}

/// Full comparison result.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompareReport {
    pub rows: Vec<CompareRow>,
    pub threshold: f64,
}

impl CompareReport {
    pub fn any_regressed(&self) -> bool {
        self.rows.iter().any(|r| r.regressed)
    }
}

pub fn run(
    baseline: Option<&Path>,
    current: &Path,
    threshold: f64,
    markdown: bool,
) -> Result<()> {
    let current_samples = collect_samples(current)?;
    if current_samples.is_empty() {
        return Err(anyhow!(
            "no bench results found under {} — run `cargo bench` first",
            current.display(),
        ));
    }
    let baseline_samples = match baseline {
        Some(dir) if dir.exists() => collect_samples(dir)?,
        _ => Vec::new(),
    };

    let report = compare(&baseline_samples, &current_samples, threshold);

    if markdown {
        print!("{}", render_markdown(&report));
    } else {
        print!("{}", render_plain(&report));
    }

    if report.any_regressed() {
        // Exit code 2 — distinct from clap's usage-error
        // exit 1, so CI can tell "regression" from
        // "bad invocation".
        std::process::exit(2);
    }
    Ok(())
}

/// Walk `root` for `**/new/estimates.json` + build a
/// sample per bench.  Pure-ish (filesystem read only);
/// the parse + id-extraction are pulled into pure
/// helpers for testing.
fn collect_samples(root: &Path) -> Result<Vec<BenchSample>> {
    let mut out = Vec::new();
    collect_into(root, root, &mut out)?;
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn collect_into(
    root: &Path,
    dir: &Path,
    out: &mut Vec<BenchSample>,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_into(root, &path, out)?;
        } else if path.file_name().map(|n| n == "estimates.json").unwrap_or(false)
            && path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n == "new")
                .unwrap_or(false)
        {
            if let Some(id) = bench_id_from_path(root, &path) {
                let json = std::fs::read_to_string(&path)?;
                if let Some(median_ns) = parse_median_ns(&json) {
                    out.push(BenchSample { id, median_ns });
                }
            }
        }
    }
    Ok(())
}

/// Extract the criterion bench id (`<group>/<bench>`)
/// from a `.../<group>/<bench>/new/estimates.json` path,
/// relative to `root`.  Pure.
fn bench_id_from_path(root: &Path, estimates: &Path) -> Option<String> {
    // Strip the `/new/estimates.json` tail, then make the
    // remainder relative to root → `<group>/<bench>`.
    let bench_dir = estimates.parent()?.parent()?;
    let rel = bench_dir.strip_prefix(root).ok()?;
    let s = rel.to_string_lossy().replace('\\', "/");
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Pull `median.point_estimate` (nanoseconds) out of a
/// criterion `new/estimates.json` body.  Pure.
fn parse_median_ns(json: &str) -> Option<f64> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    value
        .get("median")?
        .get("point_estimate")?
        .as_f64()
}

/// Compare current vs baseline.  Pure — the heart of the
/// gate, fully unit-tested.
fn compare(
    baseline: &[BenchSample],
    current: &[BenchSample],
    threshold: f64,
) -> CompareReport {
    let mut rows = Vec::new();
    for cur in current {
        let base = baseline
            .iter()
            .find(|b| b.id == cur.id)
            .map(|b| b.median_ns);
        let delta = base.map(|b| {
            if b > 0.0 {
                (cur.median_ns - b) / b
            } else {
                0.0
            }
        });
        let regressed = delta.map(|d| d > threshold).unwrap_or(false);
        rows.push(CompareRow {
            id: cur.id.clone(),
            baseline_ns: base,
            current_ns: cur.median_ns,
            delta,
            regressed,
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    CompareReport { rows, threshold }
}

fn fmt_ms(ns: f64) -> String {
    format!("{:.2}ms", ns / 1_000_000.0)
}

fn fmt_delta(delta: Option<f64>) -> String {
    match delta {
        Some(d) => {
            let pct = d * 100.0;
            let arrow = if d > 0.0 { "▲" } else { "▼" };
            format!("{arrow} {pct:+.1}%")
        }
        None => "new".to_string(),
    }
}

fn render_markdown(report: &CompareReport) -> String {
    let mut out = String::new();
    let verdict = if report.any_regressed() {
        "❌ **regression detected**"
    } else {
        "✅ no regressions"
    };
    out.push_str(&format!(
        "### Bench report — {verdict}\n\n_threshold: {:.0}%_\n\n",
        report.threshold * 100.0,
    ));
    out.push_str("| bench | baseline | current | Δ | |\n");
    out.push_str("|-------|----------|---------|---|--|\n");
    for r in &report.rows {
        let base = r
            .baseline_ns
            .map(fmt_ms)
            .unwrap_or_else(|| "—".to_string());
        let mark = if r.regressed { "🔴" } else { "🟢" };
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            r.id,
            base,
            fmt_ms(r.current_ns),
            fmt_delta(r.delta),
            mark,
        ));
    }
    out
}

fn render_plain(report: &CompareReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "bench report (threshold {:.0}%):\n",
        report.threshold * 100.0,
    ));
    for r in &report.rows {
        let base = r
            .baseline_ns
            .map(fmt_ms)
            .unwrap_or_else(|| "—".to_string());
        let mark = if r.regressed { "REGRESSED" } else { "ok" };
        out.push_str(&format!(
            "  {:<28} base={:<10} cur={:<10} {:<10} [{mark}]\n",
            r.id,
            base,
            fmt_ms(r.current_ns),
            fmt_delta(r.delta),
        ));
    }
    out.push_str(&format!(
        "\nverdict: {}\n",
        if report.any_regressed() {
            "REGRESSION"
        } else {
            "ok"
        },
    ));
    out
}

/// Default criterion output directory.
pub(crate) fn default_criterion_dir() -> PathBuf {
    let target = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target"));
    target.join("criterion")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, ms: f64) -> BenchSample {
        BenchSample {
            id: id.to_string(),
            median_ns: ms * 1_000_000.0,
        }
    }

    // ── parse_median_ns ───────────────────────────────

    #[test]
    fn parse_median_extracts_point_estimate() {
        let json = r#"{
            "mean": { "point_estimate": 999.0 },
            "median": { "point_estimate": 483331777.15 }
        }"#;
        assert_eq!(parse_median_ns(json), Some(483331777.15));
    }

    #[test]
    fn parse_median_rejects_garbage() {
        assert_eq!(parse_median_ns("not json"), None);
    }

    #[test]
    fn parse_median_missing_field_is_none() {
        assert_eq!(parse_median_ns(r#"{"mean":{"point_estimate":1.0}}"#), None);
    }

    // ── bench_id_from_path ────────────────────────────

    #[test]
    fn bench_id_extracts_group_and_function() {
        let root = Path::new("/x/target/criterion");
        let est = Path::new(
            "/x/target/criterion/startup/cold_list/new/estimates.json",
        );
        assert_eq!(
            bench_id_from_path(root, est),
            Some("startup/cold_list".to_string()),
        );
    }

    #[test]
    fn bench_id_handles_single_level() {
        let root = Path::new("/c");
        let est = Path::new("/c/solo/new/estimates.json");
        assert_eq!(bench_id_from_path(root, est), Some("solo".to_string()));
    }

    // ── compare ───────────────────────────────────────

    #[test]
    fn compare_flags_regression_over_threshold() {
        let base = vec![sample("startup/cold_list", 100.0)];
        let cur = vec![sample("startup/cold_list", 130.0)]; // +30%
        let report = compare(&base, &cur, 0.20);
        assert!(report.any_regressed());
        let row = &report.rows[0];
        assert!((row.delta.unwrap() - 0.30).abs() < 1e-9);
        assert!(row.regressed);
    }

    #[test]
    fn compare_allows_within_threshold() {
        let base = vec![sample("search/rare", 100.0)];
        let cur = vec![sample("search/rare", 115.0)]; // +15%
        let report = compare(&base, &cur, 0.20);
        assert!(!report.any_regressed());
        assert!(!report.rows[0].regressed);
    }

    #[test]
    fn compare_improvement_is_not_a_regression() {
        let base = vec![sample("startup/cold_list", 500.0)];
        let cur = vec![sample("startup/cold_list", 36.0)]; // the I.1.4 win
        let report = compare(&base, &cur, 0.20);
        assert!(!report.any_regressed());
        let d = report.rows[0].delta.unwrap();
        assert!(d < 0.0, "improvement should be a negative delta");
    }

    #[test]
    fn compare_new_bench_has_no_baseline_no_regression() {
        let base = vec![];
        let cur = vec![sample("brand/new", 50.0)];
        let report = compare(&base, &cur, 0.20);
        assert!(!report.any_regressed());
        assert_eq!(report.rows[0].delta, None);
        assert_eq!(report.rows[0].baseline_ns, None);
    }

    #[test]
    fn compare_catches_algorithmic_regression() {
        // The real value of the gate: a 640x flatten
        // regression (O(n)→O(n²)) is unmissable.
        let base = vec![sample("tree/flatten", 0.05)];
        let cur = vec![sample("tree/flatten", 32.0)];
        let report = compare(&base, &cur, 0.20);
        assert!(report.any_regressed());
        assert!(report.rows[0].delta.unwrap() > 100.0);
    }

    #[test]
    fn compare_zero_baseline_does_not_divide_by_zero() {
        let base = vec![sample("weird/zero", 0.0)];
        let cur = vec![sample("weird/zero", 10.0)];
        let report = compare(&base, &cur, 0.20);
        // delta defined as 0.0 when baseline is 0 — no
        // panic, no spurious regression.
        assert_eq!(report.rows[0].delta, Some(0.0));
        assert!(!report.any_regressed());
    }

    // ── rendering ─────────────────────────────────────

    #[test]
    fn markdown_shows_regression_verdict() {
        let base = vec![sample("a/b", 100.0)];
        let cur = vec![sample("a/b", 200.0)];
        let report = compare(&base, &cur, 0.20);
        let md = render_markdown(&report);
        assert!(md.contains("regression detected"));
        assert!(md.contains("`a/b`"));
        assert!(md.contains("🔴"));
    }

    #[test]
    fn markdown_clean_verdict() {
        let base = vec![sample("a/b", 100.0)];
        let cur = vec![sample("a/b", 100.0)];
        let report = compare(&base, &cur, 0.20);
        let md = render_markdown(&report);
        assert!(md.contains("no regressions"));
        assert!(md.contains("🟢"));
    }

    #[test]
    fn plain_render_has_verdict_line() {
        let base = vec![sample("a/b", 100.0)];
        let cur = vec![sample("a/b", 130.0)];
        let report = compare(&base, &cur, 0.20);
        let plain = render_plain(&report);
        assert!(plain.contains("REGRESSION"));
        assert!(plain.contains("REGRESSED"));
    }

    // ── collect_samples (real fs) ─────────────────────

    #[test]
    fn collect_samples_walks_criterion_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let bench_new = root.join("startup").join("cold_list").join("new");
        std::fs::create_dir_all(&bench_new).unwrap();
        std::fs::write(
            bench_new.join("estimates.json"),
            r#"{"median":{"point_estimate":85000000.0}}"#,
        )
        .unwrap();
        // A `base/` dir must be ignored (only `new/`
        // counts).
        let bench_base = root.join("startup").join("cold_list").join("base");
        std::fs::create_dir_all(&bench_base).unwrap();
        std::fs::write(
            bench_base.join("estimates.json"),
            r#"{"median":{"point_estimate":999000000.0}}"#,
        )
        .unwrap();

        let samples = collect_samples(root).unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].id, "startup/cold_list");
        assert_eq!(samples[0].median_ns, 85000000.0);
    }
}
