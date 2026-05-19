use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Lookback window, e.g. "1h", "30min", "7days"
    #[arg(short, long)]
    duration: String,

    /// Neighbours per node in the k-NN graph
    #[arg(short, long, default_value_t = 5)]
    k: usize,

    /// Tokens shorter than this are dropped before TF-IDF
    #[arg(long, default_value_t = 2)]
    min_word_len: usize,

    /// Max cosine similarity to nearest neighbour above which a fingerprint is
    /// **not** flagged as anomalous (lower = more isolated = more anomalous)
    #[arg(long, default_value_t = 0.2)]
    anomaly_threshold: f32,

    /// Cap on members listed per cluster (true total reported in `size`)
    #[arg(long, default_value_t = 10)]
    max_cluster_members: usize,

    /// Cap on the `anomalies` array in the response
    #[arg(long, default_value_t = 20)]
    max_anomalies: usize,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/knn",
        serde_json::json!({
            "session":             session,
            "duration":            args.duration,
            "k":                   args.k,
            "min_word_len":        args.min_word_len,
            "anomaly_threshold":   args.anomaly_threshold,
            "max_cluster_members": args.max_cluster_members,
            "max_anomalies":       args.max_anomalies,
        }),
    )
}
