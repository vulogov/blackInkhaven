use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Lookback window, e.g. "1h"
    #[arg(short, long)]
    duration: String,

    /// Failure key to anchor the RCA (optional; runs global RCA if omitted)
    #[arg(short, long)]
    failure_key: Option<String>,

    /// Co-occurrence bucket size in seconds
    #[arg(long, default_value_t = 300)]
    bucket_secs: u64,

    /// Minimum co-occurrence support count
    #[arg(long, default_value_t = 2)]
    min_support: usize,

    /// Jaccard similarity threshold
    #[arg(long, default_value_t = 0.2)]
    jaccard_threshold: f64,

    /// Maximum number of keys to consider
    #[arg(long, default_value_t = 200)]
    max_keys: usize,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/rca",
        serde_json::json!({
            "session":           session,
            "duration":          args.duration,
            "failure_key":       args.failure_key,
            "bucket_secs":       args.bucket_secs,
            "min_support":       args.min_support,
            "jaccard_threshold": args.jaccard_threshold,
            "max_keys":          args.max_keys,
        }),
    )
}
