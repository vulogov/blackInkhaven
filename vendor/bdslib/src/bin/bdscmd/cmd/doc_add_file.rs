use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Path to the text file to ingest
    #[arg(short, long)]
    path: String,

    /// Human-readable document name stored in metadata
    #[arg(short, long)]
    name: String,

    /// Maximum characters per chunk
    #[arg(short, long, default_value_t = 512)]
    slice: usize,

    /// Chunk overlap as a percentage of slice [0–99]
    #[arg(short, long, default_value_t = 20.0)]
    overlap: f32,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/doc.add.file",
        serde_json::json!({
            "session": session,
            "path": args.path,
            "name": args.name,
            "slice": args.slice,
            "overlap": args.overlap,
        }),
    )
}
