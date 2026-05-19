use crate::common::error::{err_msg, Result};
use crate::globals::get_db;
use anomaly_detection::AnomalyDetector;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── public output types ───────────────────────────────────────────────────────

/// A single observed data point (timestamp + extracted numeric value).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplePoint {
    /// Position of this point in the time-ordered sample array.
    pub index: usize,
    /// Unix timestamp of the document (seconds).
    pub timestamp: u64,
    /// Extracted numeric value.
    pub value: f64,
}

/// Statistical summary for a specific telemetry key over a time window.
///
/// Obtain via [`TelemetryTrend::query`] or [`TelemetryTrend::query_window`].
/// Both methods reach the global [`ShardsManager`](crate::ShardsManager) singleton
/// initialised by [`init_db`](crate::init_db).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryTrend {
    /// Metric key that was queried.
    pub key: String,
    /// Start of the queried window (Unix seconds, inclusive).
    pub start: u64,
    /// End of the queried window (Unix seconds, exclusive).
    pub end: u64,
    /// Number of samples collected.
    pub n: usize,
    /// Minimum observed value.  `NaN` when `n == 0`.
    pub min: f64,
    /// Maximum observed value.  `NaN` when `n == 0`.
    pub max: f64,
    /// Arithmetic mean.  `NaN` when `n == 0`.
    pub mean: f64,
    /// Statistical median.  `NaN` when `n == 0`.
    pub median: f64,
    /// Population standard deviation.  `NaN` when `n == 0`.
    pub std_dev: f64,
    /// Coefficient of variation (`std_dev / |mean|`).  `0` when `mean ≈ 0`.
    pub variability: f64,
    /// Data points flagged as statistical anomalies (S-H-ESD algorithm).
    ///
    /// Empty when fewer than 4 samples are available.
    pub anomalies: Vec<SamplePoint>,
    /// Data points where a significant distribution shift was detected.
    ///
    /// Empty when fewer than 4 samples are available.
    pub breakouts: Vec<SamplePoint>,
}

impl TelemetryTrend {
    /// Query telemetry for `key` in the absolute window `[start_secs, end_secs)`.
    ///
    /// Requires [`init_db`](crate::init_db) to have been called.
    pub fn query(key: &str, start_secs: u64, end_secs: u64) -> Result<Self> {
        let db = get_db()?;

        let start_ts = UNIX_EPOCH + Duration::from_secs(start_secs);
        let end_ts = UNIX_EPOCH + Duration::from_secs(end_secs);

        let mut points: Vec<(u64, f64)> = Vec::new();
        for info in db.cache().info().shards_in_range(start_ts, end_ts)? {
            let shard = db.cache().shard(info.start_time)?;
            for doc in shard.get_primaries_by_key(key)? {
                let ts = doc["timestamp"].as_u64().unwrap_or(0);
                if ts >= start_secs && ts < end_secs {
                    if let Some(val) = extract_value(&doc) {
                        points.push((ts, val));
                    }
                }
            }
        }

        points.sort_by_key(|(ts, _)| *ts);
        build(key, start_secs, end_secs, points)
    }

    /// Query telemetry for `key` in the lookback window `[now − duration, now)`.
    ///
    /// `duration` uses humantime notation (`"1h"`, `"30min"`, `"7days"`).
    /// Requires [`init_db`](crate::init_db) to have been called.
    pub fn query_window(key: &str, duration: &str) -> Result<Self> {
        let dur = humantime::parse_duration(duration)
            .map_err(|e| err_msg(format!("invalid duration '{duration}': {e}")))?;
        let now = SystemTime::now();
        let start = now.checked_sub(dur).unwrap_or(UNIX_EPOCH);
        let start_secs = start.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let end_secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        Self::query(key, start_secs, end_secs)
    }
}

// ── core computation ──────────────────────────────────────────────────────────

fn build(key: &str, start: u64, end: u64, points: Vec<(u64, f64)>) -> Result<TelemetryTrend> {
    let n = points.len();

    if n == 0 {
        return Ok(TelemetryTrend {
            key: key.to_string(),
            start,
            end,
            n: 0,
            min: f64::NAN,
            max: f64::NAN,
            mean: f64::NAN,
            median: f64::NAN,
            std_dev: f64::NAN,
            variability: f64::NAN,
            anomalies: vec![],
            breakouts: vec![],
        });
    }

    let values: Vec<f64> = points.iter().map(|(_, v)| *v).collect();

    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mean = values.iter().sum::<f64>() / n as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
    let std_dev = variance.sqrt();
    let variability = if mean.abs() > 1e-10 {
        std_dev / mean.abs()
    } else {
        0.0
    };
    let median = percentile_sorted(&values, 0.5);

    let anomalies = detect_anomalies(&points, &values);
    let breakouts = detect_breakouts(&points, &values);

    Ok(TelemetryTrend {
        key: key.to_string(),
        start,
        end,
        n,
        min,
        max,
        mean,
        median,
        std_dev,
        variability,
        anomalies,
        breakouts,
    })
}

// ── anomaly detection ─────────────────────────────────────────────────────────

/// Seasonal Hybrid ESD anomaly detection.
///
/// `period` defaults to `max(2, n / 4)` but is capped at `n / 2` so the
/// series always contains at least 2 complete periods.
fn detect_anomalies(points: &[(u64, f64)], values: &[f64]) -> Vec<SamplePoint> {
    let n = values.len();
    if n < 4 {
        return vec![];
    }
    let period = (n / 4).max(2).min(n / 2);
    let series: Vec<f32> = values.iter().map(|&v| v as f32).collect();

    match AnomalyDetector::params()
        .max_anoms(0.1)
        .fit(&series, period)
    {
        Ok(result) => result
            .anomalies()
            .iter()
            .filter_map(|&idx| {
                points.get(idx).map(|&(ts, v)| SamplePoint {
                    index: idx,
                    timestamp: ts,
                    value: v,
                })
            })
            .collect(),
        Err(_) => vec![],
    }
}

// ── breakout detection ────────────────────────────────────────────────────────

/// Energy-based multi-breakout (distribution shift) detection.
///
/// `min_size` is set to `max(2, n / 5)` so it scales with the series length,
/// with an upper cap of 30 (the library default).
fn detect_breakouts(points: &[(u64, f64)], values: &[f64]) -> Vec<SamplePoint> {
    let n = values.len();
    if n < 4 {
        return vec![];
    }
    let min_size = (n / 5).max(2).min(30);

    match breakout::multi().min_size(min_size).fit(values) {
        Ok(indices) => indices
            .iter()
            .filter_map(|&idx| {
                points.get(idx).map(|&(ts, v)| SamplePoint {
                    index: idx,
                    timestamp: ts,
                    value: v,
                })
            })
            .collect(),
        Err(_) => vec![],
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Extract the numeric measurement from a telemetry document.
///
/// Tries `data.value` first (structured telemetry), then falls back to `data`
/// itself when it is a bare number.
fn extract_value(doc: &JsonValue) -> Option<f64> {
    doc["data"]
        .as_f64()
        .or_else(|| doc["data"]["value"].as_f64())
}

/// Return the `p`-quantile of `values` (sorted copy; `p` in `[0, 1]`).
fn percentile_sorted(values: &[f64], p: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let pos = p * (n - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    let frac = pos - lo as f64;
    sorted[lo] + frac * (sorted[hi] - sorted[lo])
}
