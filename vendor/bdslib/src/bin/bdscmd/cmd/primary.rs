use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// UUID of the primary record
    primary_id: String,
}

pub fn run(url: &str, _session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/primary",
        serde_json::json!({ "primary_id": args.primary_id }),
    )
}
