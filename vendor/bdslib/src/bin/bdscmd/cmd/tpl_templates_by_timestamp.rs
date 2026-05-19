use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Range start as Unix seconds (inclusive)
    #[arg(short, long)]
    start_ts: u64,

    /// Range end as Unix seconds (inclusive)
    #[arg(short, long)]
    end_ts: u64,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/tpl.templates_by_timestamp",
        serde_json::json!({
            "session":  session,
            "start_ts": args.start_ts,
            "end_ts":   args.end_ts,
        }),
    )
}
