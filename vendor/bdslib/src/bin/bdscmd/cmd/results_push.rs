use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// UUIDv7 of the queue. Auto-created on first push.
    #[arg(short, long)]
    id: String,

    /// JSON value to enqueue. Pass valid JSON (object, array, number,
    /// string-with-quotes, true/false/null).  Use --raw to pass a plain string.
    #[arg(short, long, conflicts_with = "raw")]
    value: Option<String>,

    /// Raw string to enqueue (wrapped server-side as a JSON string value).
    #[arg(short, long, conflicts_with = "value")]
    raw: Option<String>,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    let payload: serde_json::Value = match (args.value, args.raw) {
        (Some(v), _) => serde_json::from_str(&v)
            .map_err(|e| anyhow::anyhow!("--value is not valid JSON: {e}"))?,
        (None, Some(r)) => serde_json::Value::String(r),
        (None, None) => return Err(anyhow::anyhow!("either --value (JSON) or --raw (string) is required")),
    };

    crate::client::call(
        url,
        "v2/results.push",
        serde_json::json!({
            "session": session,
            "id":      args.id,
            "value":   payload,
        }),
    )
}
