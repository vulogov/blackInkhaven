use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Lookback window, e.g. "1h", "30min", "7days"
    #[arg(short, long)]
    duration: String,

    /// Failure template body to anchor the RCA (exact drain3 pattern including <*>)
    #[arg(short, long)]
    failure_body: Option<String>,

    /// Co-occurrence bucket size in seconds
    #[arg(long, default_value_t = 300)]
    bucket_secs: u64,

    /// Minimum distinct buckets a template must appear in to be included
    #[arg(long, default_value_t = 2)]
    min_support: usize,

    /// Jaccard similarity threshold for clustering
    #[arg(long, default_value_t = 0.2)]
    jaccard_threshold: f64,

    /// Maximum number of distinct template bodies to analyse
    #[arg(long, default_value_t = 200)]
    max_keys: usize,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/rca.templates",
        serde_json::json!({
            "session":           session,
            "duration":          args.duration,
            "failure_body":      args.failure_body,
            "bucket_secs":       args.bucket_secs,
            "min_support":       args.min_support,
            "jaccard_threshold": args.jaccard_threshold,
            "max_keys":          args.max_keys,
        }),
    )
}
