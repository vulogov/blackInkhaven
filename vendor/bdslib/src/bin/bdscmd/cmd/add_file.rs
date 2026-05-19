use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Absolute path to the NDJSON file on the server's filesystem
    path: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/add.file",
        serde_json::json!({ "session": session, "path": args.path }),
    )
}
