use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// UUID v7 of the template document to look up
    #[arg(short, long)]
    id: String,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/tpl.template_by_id",
        serde_json::json!({
            "session": session,
            "id":      args.id,
        }),
    )
}
