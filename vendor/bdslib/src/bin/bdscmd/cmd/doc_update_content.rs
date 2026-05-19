use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Document UUID
    #[arg(short, long)]
    id: String,

    /// Replacement content text
    #[arg(short, long)]
    content: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/doc.update.content",
        serde_json::json!({
            "session": session,
            "id": args.id,
            "content": args.content,
        }),
    )
}
