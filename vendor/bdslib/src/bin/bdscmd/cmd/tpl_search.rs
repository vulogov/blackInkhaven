use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Search query string
    #[arg(short, long)]
    query: String,

    /// Lookback window for shards to search, e.g. "1h", "7days"
    #[arg(short, long, default_value = "1h")]
    duration: String,

    /// Maximum number of results to return
    #[arg(short, long, default_value_t = 10)]
    limit: usize,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/tpl.search",
        serde_json::json!({
            "session":  session,
            "duration": args.duration,
            "query":    args.query,
            "limit":    args.limit,
        }),
    )
}
