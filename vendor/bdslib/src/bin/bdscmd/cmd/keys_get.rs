use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Lookback window, e.g. "1h"
    #[arg(short, long)]
    duration: String,

    /// Key or glob pattern to look up
    #[arg(short, long)]
    key: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/keys.get",
        serde_json::json!({ "session": session, "duration": args.duration, "key": args.key }),
    )
}
