use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Semantic search query
    #[arg(short, long)]
    query: String,

    /// Lookback window, e.g. "1h"
    #[arg(short, long)]
    duration: String,

    /// Maximum number of results
    #[arg(short, long, default_value_t = 10)]
    limit: usize,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/search.get",
        serde_json::json!({
            "session": session,
            "query": args.query,
            "duration": args.duration,
            "limit": args.limit,
        }),
    )
}
