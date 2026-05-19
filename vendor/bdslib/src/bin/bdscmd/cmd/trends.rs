use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Key to analyse
    #[arg(short, long)]
    key: String,

    /// Lookback window, e.g. "1h"
    #[arg(short, long)]
    duration: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/trends",
        serde_json::json!({ "session": session, "key": args.key, "duration": args.duration }),
    )
}
