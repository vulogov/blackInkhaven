use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Lookback window, e.g. "1h", "30min", "7days"
    #[arg(short, long)]
    duration: String,

    /// N-gram length (2 = bigrams, 3 = trigrams, 1 = unigrams)
    #[arg(short, long, default_value_t = 2)]
    n: usize,

    /// Tokens shorter than this are dropped before n-gram construction
    #[arg(long, default_value_t = 2)]
    min_word_len: usize,

    /// Mean rarity above this flags a fingerprint as anomalous
    #[arg(long, default_value_t = 0.7)]
    anomaly_threshold: f32,

    /// Maximum anomalies in the response array (true total in `n_anomalies`)
    #[arg(long, default_value_t = 20)]
    max_anomalies: usize,

    /// Per-anomaly cap on the explanatory `novel_ngrams` array
    #[arg(long, default_value_t = 5)]
    max_novel_ngrams: usize,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/anomaly.recent",
        serde_json::json!({
            "session":           session,
            "duration":          args.duration,
            "n":                 args.n,
            "min_word_len":      args.min_word_len,
            "anomaly_threshold": args.anomaly_threshold,
            "max_anomalies":     args.max_anomalies,
            "max_novel_ngrams":  args.max_novel_ngrams,
        }),
    )
}
