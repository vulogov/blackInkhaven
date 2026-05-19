use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Document UUID
    #[arg(short, long)]
    id: String,

    /// Replacement JSON metadata object (inline JSON string)
    #[arg(short, long)]
    metadata: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    let metadata: Value = serde_json::from_str(&args.metadata)
        .map_err(|e| anyhow::anyhow!("invalid JSON metadata: {e}"))?;
    crate::client::call(
        url,
        "v2/doc.update.metadata",
        serde_json::json!({
            "session": session,
            "id": args.id,
            "metadata": metadata,
        }),
    )
}
