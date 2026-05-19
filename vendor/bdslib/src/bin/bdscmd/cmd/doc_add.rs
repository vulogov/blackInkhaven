use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// JSON metadata object (inline JSON string)
    #[arg(short, long)]
    metadata: String,

    /// Document content text
    #[arg(short, long)]
    content: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    let metadata: Value = serde_json::from_str(&args.metadata)
        .map_err(|e| anyhow::anyhow!("invalid JSON metadata: {e}"))?;
    crate::client::call(
        url,
        "v2/doc.add",
        serde_json::json!({
            "session": session,
            "metadata": metadata,
            "content": args.content,
        }),
    )
}
