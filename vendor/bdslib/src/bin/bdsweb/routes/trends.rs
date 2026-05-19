use askama::Template;
use axum::{extract::{Query, State}, response::Html};
use serde::Deserialize;
use serde_json::json;

use crate::{client::{rpc, SESSION}, error::AppError, state::AppState};

#[derive(Deserialize, Default)]
pub struct Params {
    #[serde(default)]
    pub key: String,
    #[serde(default = "default_duration")]
    pub duration: String,
}
fn default_duration() -> String { "1h".to_owned() }

/// Inferred unit family for a telemetry key.  Drives both the Rust-side
/// stat formatting and the matching JS formatter used by the uPlot chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Bytes,
    Percent,       // value already on a 0..100 scale
    Ratio,         // value on a 0..1 scale; multiplied by 100 for display
    Milliseconds,
    Seconds,
    Count,
    Generic,
}

impl Unit {
    /// Stable identifier consumed by the JS chart formatter.
    fn js_kind(self) -> &'static str {
        match self {
            Unit::Bytes        => "bytes",
            Unit::Percent      => "percent",
            Unit::Ratio        => "ratio",
            Unit::Milliseconds => "ms",
            Unit::Seconds      => "secs",
            Unit::Count        => "count",
            Unit::Generic      => "generic",
        }
    }

    /// Short y-axis suffix, e.g. "%" or "bytes".  Empty for `Generic`/`Count`
    /// where the values speak for themselves.
    fn axis_suffix(self) -> &'static str {
        match self {
            Unit::Bytes        => " (bytes)",
            Unit::Percent      => " (%)",
            Unit::Ratio        => " (%)",
            Unit::Milliseconds => " (ms)",
            Unit::Seconds      => " (s)",
            _                  => "",
        }
    }
}

/// Best-effort unit inference from a telemetry key name.
///
/// Match order matters — narrower patterns are tested first so e.g.
/// `latency_ms` is detected as milliseconds before the "latency" or "ms"
/// substrings could pull it elsewhere.
fn guess_unit(key: &str) -> Unit {
    let k = key.to_lowercase();
    let k = k.as_str();

    // Bytes: any *_bytes / *.size / *_size key.
    if k.contains("byte") || k.ends_with(".size") || k.contains("_size") {
        return Unit::Bytes;
    }

    // Time first — these patterns embed unit suffixes that we don't want
    // confused with percentage substrings (e.g. "duration").
    if k.ends_with("_ms") || k.ends_with(".ms") || k.contains("latency_ms")
        || k.contains("duration_ms") || k.contains("_millis")
    {
        return Unit::Milliseconds;
    }
    if k.ends_with("_secs") || k.ends_with("_seconds") || k.ends_with(".secs")
        || k.ends_with(".seconds") || k.contains("latency_s") || k.contains("duration_s")
    {
        return Unit::Seconds;
    }

    // 0..1 fractions.
    if k.contains("ratio") || k.contains("fraction") || k.contains("frac_") || k.ends_with("_frac") {
        return Unit::Ratio;
    }

    // 0..100 percentages and utilisation.
    if k.contains("pct") || k.contains("percent") || k.contains("usage")
        || k.contains("util") || k.contains("io_wait") || k.contains("load")
    {
        return Unit::Percent;
    }

    // Counters.
    if k.contains("count") || k.contains("depth") || k.contains("connections")
        || k.contains("requests") || k.ends_with(".total") || k.ends_with("_total")
        || k.ends_with(".n") || k.ends_with("_n")
    {
        return Unit::Count;
    }

    Unit::Generic
}

#[derive(Debug, Default)]
pub struct TrendStats {
    pub n:           usize,
    pub min:         String,
    pub max:         String,
    pub mean:        String,
    pub median:      String,
    pub std_dev:     String,
    pub variability: String,
    pub anomalies:   usize,
    pub breakouts:   usize,
}

/// Format a single numeric stat using the guessed unit.
fn format_value(unit: Unit, v: f64) -> String {
    if !v.is_finite() {
        return "—".to_owned();
    }
    match unit {
        Unit::Bytes        => format_bytes(v),
        Unit::Percent      => format!("{v:.2}%"),
        Unit::Ratio        => format!("{:.2}%", v * 100.0),
        Unit::Milliseconds => format_ms(v),
        Unit::Seconds      => format_secs(v),
        Unit::Count        => format_count(v),
        Unit::Generic      => format!("{v:.4}"),
    }
}

/// Format bytes using IEC binary prefixes (KiB/MiB/GiB) — matches what most
/// memory and disk tooling reports.
fn format_bytes(v: f64) -> String {
    let abs = v.abs();
    let sign = if v < 0.0 { "-" } else { "" };
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut idx = 0;
    let mut val = abs;
    while val >= 1024.0 && idx < UNITS.len() - 1 {
        val /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{sign}{val:.0} {}", UNITS[idx])
    } else {
        format!("{sign}{val:.2} {}", UNITS[idx])
    }
}

fn format_ms(v: f64) -> String {
    let abs = v.abs();
    if abs < 1.0 {
        format!("{:.0} µs", v * 1000.0)
    } else if abs < 1000.0 {
        format!("{v:.2} ms")
    } else if abs < 60_000.0 {
        format!("{:.2} s", v / 1000.0)
    } else {
        format!("{:.1} min", v / 60_000.0)
    }
}

fn format_secs(v: f64) -> String {
    let abs = v.abs();
    if abs < 1.0 {
        format!("{:.0} ms", v * 1000.0)
    } else if abs < 60.0 {
        format!("{v:.2} s")
    } else if abs < 3600.0 {
        format!("{:.1} min", v / 60.0)
    } else {
        format!("{:.2} h", v / 3600.0)
    }
}

fn format_count(v: f64) -> String {
    let n = v.round() as i64;
    // Thousands separator without pulling a locale crate.
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.insert(0, ',');
        }
        out.insert(0, ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}

fn extract_stats(v: &serde_json::Value, unit: Unit) -> TrendStats {
    let val = |key: &str| -> String {
        v.get(key).and_then(|x| x.as_f64())
            .map(|f| format_value(unit, f))
            .unwrap_or_else(|| "—".to_owned())
    };
    // Variability is a coefficient of variation — dimensionless, render raw.
    let variability = v.get("variability").and_then(|x| x.as_f64())
        .map(|f| format!("{f:.4}"))
        .unwrap_or_else(|| "—".to_owned());
    TrendStats {
        n:           v.get("n").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
        min:         val("min"),
        max:         val("max"),
        mean:        val("mean"),
        median:      val("median"),
        std_dev:     val("std_dev"),
        variability,
        anomalies:   v.get("anomalies").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
        breakouts:   v.get("breakouts").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
    }
}

// ── Full page ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "trends.html")]
struct TrendsPage { key: String, duration: String }

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(TrendsPage { key: p.key, duration: p.duration }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/trends_data.html")]
struct TrendsData {
    key:             String,
    duration:        String,
    stats:           TrendStats,
    uplot_data_json: String,
    /// Breakout points as `[[ts, value], ...]` so the chart can overlay them
    /// as vertical markers without re-aligning to the main x-axis.
    breakouts_json:  String,
    has_data:        bool,
    /// Unit family inferred from the key — drives the JS chart formatter.
    unit_kind:       String,
    /// Suffix appended to the y-axis label, e.g. " (%)".
    unit_axis_suffix: String,
}

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    if p.key.is_empty() {
        return Ok(Html(TrendsData {
            key: p.key, duration: p.duration,
            stats: TrendStats::default(),
            uplot_data_json: "[[],[]]".to_owned(),
            breakouts_json:  "[]".to_owned(),
            has_data: false,
            unit_kind:        Unit::Generic.js_kind().to_owned(),
            unit_axis_suffix: Unit::Generic.axis_suffix().to_owned(),
        }.render()?));
    }

    let unit = guess_unit(&p.key);

    let (trend_v, telemetry_v) = tokio::try_join!(
        rpc(&state, "v2/trends", json!({
            "session":  SESSION,
            "key":      p.key,
            "duration": p.duration,
        })),
        rpc(&state, "v2/primaries.get.telemetry", json!({
            "session":  SESSION,
            "key":      p.key,
            "duration": p.duration,
        })),
    )?;

    let stats = extract_stats(&trend_v, unit);

    let results_arr = telemetry_v.get("results")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    let mut timestamps: Vec<f64> = Vec::with_capacity(results_arr.len());
    let mut values:     Vec<f64> = Vec::with_capacity(results_arr.len());
    for pt in &results_arr {
        if let (Some(ts), Some(val)) = (
            pt.get("timestamp").and_then(|x| x.as_u64()),
            pt.get("value").and_then(|x| x.as_f64()),
        ) {
            timestamps.push(ts as f64);
            values.push(val);
        }
    }

    let uplot_data_json = serde_json::to_string(&[&timestamps, &values])?;
    let has_data = !timestamps.is_empty();

    // Extract breakouts as [[ts, value], ...] for chart overlay markers.
    let breakouts: Vec<[f64; 2]> = trend_v
        .get("breakouts")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    let ts = p.get("timestamp").and_then(|x| x.as_u64())? as f64;
                    let v  = p.get("value").and_then(|x| x.as_f64())?;
                    Some([ts, v])
                })
                .collect()
        })
        .unwrap_or_default();
    let breakouts_json = serde_json::to_string(&breakouts)?;

    Ok(Html(TrendsData {
        key:              p.key,
        duration:         p.duration,
        stats,
        uplot_data_json,
        breakouts_json,
        has_data,
        unit_kind:        unit.js_kind().to_owned(),
        unit_axis_suffix: unit.axis_suffix().to_owned(),
    }.render()?))
}
