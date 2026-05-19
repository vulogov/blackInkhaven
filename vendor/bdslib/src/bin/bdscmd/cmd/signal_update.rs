use anyhow::{Context, Result};
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Signal UUID to update
    #[arg(short, long)]
    id: String,

    /// New metadata as a JSON object string
    #[arg(short, long)]
    metadata: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    let metadata: Value = serde_json::from_str(&args.metadata)
        .with_context(|| "metadata must be a valid JSON object")?;
    crate::client::call(
        url,
        "v2/signal.update",
        serde_json::json!({
            "session":  session,
            "id":       args.id,
            "metadata": metadata,
        }),
    )
}
