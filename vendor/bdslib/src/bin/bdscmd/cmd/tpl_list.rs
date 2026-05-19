use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Lookback window, e.g. "1h", "7days"
    #[arg(short, long, default_value = "1h")]
    duration: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/tpl.list",
        serde_json::json!({
            "session":  session,
            "duration": args.duration,
        }),
    )
}
