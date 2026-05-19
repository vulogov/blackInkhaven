use anyhow::{Context, Result};
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// JSON array of documents, or NDJSON file path (reads from stdin if omitted)
    source: Option<String>,
}

pub fn run(url: &str, _session: &str, args: Cmd) -> Result<Value> {
    let raw = match args.source {
        Some(ref path) if path != "-" => {
            std::fs::read_to_string(path).with_context(|| format!("cannot read {path}"))?
        }
        _ => {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
                .context("failed to read from stdin")?;
            buf
        }
    };

    let docs: Vec<Value> = if raw.trim_start().starts_with('[') {
        serde_json::from_str(raw.trim()).context("invalid JSON array")?
    } else {
        raw.lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| {
                serde_json::from_str(l).with_context(|| format!("invalid JSON on line {}", i + 1))
            })
            .collect::<Result<Vec<_>>>()?
    };

    crate::client::call(url, "v2/add.batch", serde_json::json!({ "docs": docs }))
}
