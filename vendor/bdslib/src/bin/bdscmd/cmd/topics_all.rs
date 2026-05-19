use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Lookback window, e.g. "1h"
    #[arg(short, long)]
    duration: String,

    /// Number of topics
    #[arg(long, default_value_t = 3)]
    k: usize,

    /// LDA alpha (document-topic prior)
    #[arg(long, default_value_t = 0.1)]
    alpha: f64,

    /// LDA beta (topic-word prior)
    #[arg(long, default_value_t = 0.01)]
    beta: f64,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Number of LDA iterations
    #[arg(long, default_value_t = 200)]
    iters: usize,

    /// Top N words per topic
    #[arg(long, default_value_t = 10)]
    top_n: usize,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/topics.all",
        serde_json::json!({
            "session":  session,
            "duration": args.duration,
            "k":        args.k,
            "alpha":    args.alpha,
            "beta":     args.beta,
            "seed":     args.seed,
            "iters":    args.iters,
            "top_n":    args.top_n,
        }),
    )
}
