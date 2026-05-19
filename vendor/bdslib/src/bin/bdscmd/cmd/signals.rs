use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Humantime lookback window (e.g. "30m", "1h", "24h")
    #[arg(short, long, default_value = "1h")]
    duration: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/signals",
        serde_json::json!({
            "session":  session,
            "duration": args.duration,
        }),
    )
}
