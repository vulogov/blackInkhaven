use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Human-readable template name
    #[arg(short, long)]
    name: String,

    /// Template body text (drain3 pattern, e.g. "user <*> logged in from <*>")
    #[arg(short, long)]
    body: String,

    /// Unix timestamp (seconds); defaults to current wall-clock time
    #[arg(short, long)]
    timestamp: Option<u64>,

    /// Tags (may be repeated: --tag auth --tag login)
    #[arg(long = "tag")]
    tags: Vec<String>,

    /// Optional description
    #[arg(short, long, default_value = "")]
    description: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/tpl.add",
        serde_json::json!({
            "session":     session,
            "name":        args.name,
            "body":        args.body,
            "timestamp":   args.timestamp,
            "tags":        args.tags,
            "description": args.description,
        }),
    )
}
