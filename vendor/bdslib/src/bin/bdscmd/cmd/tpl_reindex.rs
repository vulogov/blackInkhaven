use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Lookback window for shards to reindex, e.g. "24h", "7days"
    #[arg(short, long, default_value = "24h")]
    duration: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/tpl.reindex",
        serde_json::json!({
            "session":  session,
            "duration": args.duration,
        }),
    )
}
