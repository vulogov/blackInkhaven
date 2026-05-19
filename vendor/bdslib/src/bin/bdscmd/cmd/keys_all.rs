use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Lookback window, e.g. "1h"
    #[arg(short, long)]
    duration: String,

    /// Key glob pattern (default: "*")
    #[arg(short, long, default_value = "*")]
    key: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/keys.all",
        serde_json::json!({ "session": session, "duration": args.duration, "key": args.key }),
    )
}
