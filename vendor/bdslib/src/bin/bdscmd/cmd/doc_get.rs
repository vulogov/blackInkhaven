use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Document UUID
    #[arg(short, long)]
    id: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/doc.get",
        serde_json::json!({
            "session": session,
            "id": args.id,
        }),
    )
}
