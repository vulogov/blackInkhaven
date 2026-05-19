use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// JSON query object (inline JSON string); embedded via json_fingerprint
    #[arg(short, long)]
    query: String,

    /// Maximum number of results
    #[arg(short, long, default_value_t = 10)]
    limit: usize,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    let query: Value = serde_json::from_str(&args.query)
        .map_err(|e| anyhow::anyhow!("invalid JSON query: {e}"))?;
    crate::client::call(
        url,
        "v2/doc.search.json",
        serde_json::json!({
            "session": session,
            "query": query,
            "limit": args.limit,
        }),
    )
}
