use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// UUID v7 of the template to update
    #[arg(short, long)]
    id: String,

    /// New template name
    #[arg(short, long)]
    name: Option<String>,

    /// New template body text
    #[arg(short, long)]
    body: Option<String>,

    /// Replace tag list (may be repeated: --tag auth --tag login)
    #[arg(long = "tag")]
    tags: Vec<String>,

    /// New description
    #[arg(short, long)]
    description: Option<String>,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    let tags: Option<Vec<String>> = if args.tags.is_empty() { None } else { Some(args.tags) };
    crate::client::call(
        url,
        "v2/tpl.update",
        serde_json::json!({
            "session":     session,
            "id":          args.id,
            "name":        args.name,
            "body":        args.body,
            "tags":        tags,
            "description": args.description,
        }),
    )
}
