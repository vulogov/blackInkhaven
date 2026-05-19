use anyhow::{Context, Result};
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// JSON document to ingest (reads from stdin if omitted)
    doc: Option<String>,
}

pub fn run(url: &str, _session: &str, args: Cmd) -> Result<Value> {
    let raw = match args.doc {
        Some(s) => s,
        None => {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
                .context("failed to read doc from stdin")?;
            buf
        }
    };
    let doc: Value = serde_json::from_str(raw.trim()).context("invalid JSON document")?;
    crate::client::call(url, "v2/add", serde_json::json!({ "doc": doc }))
}
