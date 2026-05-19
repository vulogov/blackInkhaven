use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// UUIDv7 of the queue to pop from
    #[arg(short, long)]
    id: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/results.pull",
        serde_json::json!({ "session": session, "id": args.id }),
    )
}
