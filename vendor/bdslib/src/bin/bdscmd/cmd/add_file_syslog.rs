use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Absolute path to the RFC 3164 syslog file on the server's filesystem
    path: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/add.file.syslog",
        serde_json::json!({ "session": session, "path": args.path }),
    )
}
