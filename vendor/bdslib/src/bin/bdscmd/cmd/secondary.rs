use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// UUID of the secondary record
    secondary_id: String,
}

pub fn run(url: &str, _session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/secondary",
        serde_json::json!({ "secondary_id": args.secondary_id }),
    )
}
