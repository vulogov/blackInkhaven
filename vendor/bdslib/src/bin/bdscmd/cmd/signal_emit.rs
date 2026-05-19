use anyhow::{Context, Result};
use clap::Args;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Args)]
pub struct Cmd {
    /// Signal name / category
    #[arg(short, long)]
    name: String,

    /// Signal severity (e.g. info, warning, critical)
    #[arg(short, long)]
    severity: String,

    /// Unix-second timestamp (defaults to now)
    #[arg(short, long)]
    timestamp: Option<u64>,

    /// Additional metadata as a JSON object string (e.g. '{"host":"web01"}')
    #[arg(short, long)]
    metadata: Option<String>,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    let ts = args.timestamp.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    });

    let metadata: serde_json::Map<String, Value> = match args.metadata {
        Some(ref s) => {
            let v: Value = serde_json::from_str(s)
                .with_context(|| "--metadata must be a valid JSON object")?;
            match v {
                Value::Object(m) => m,
                _ => anyhow::bail!("--metadata must be a JSON object, not a scalar or array"),
            }
        }
        None => serde_json::Map::new(),
    };

    crate::client::call(
        url,
        "v2/signal.emit",
        serde_json::json!({
            "session":   session,
            "name":      args.name,
            "severity":  args.severity,
            "timestamp": ts,
            "metadata":  metadata,
        }),
    )
}
