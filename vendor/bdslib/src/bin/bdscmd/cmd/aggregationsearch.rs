use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Plain-text query used for both telemetry vector search and document search
    #[arg(short, long)]
    query: String,

    /// Lookback window for the telemetry search, e.g. "1h", "30min", "7days"
    #[arg(short, long)]
    duration: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/aggregationsearch",
        serde_json::json!({
            "session": session,
            "duration": args.duration,
            "query": args.query,
        }),
    )
}
