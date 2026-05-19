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

    /// Mean commonness above this classifies a fingerprint as noise
    #[arg(long, default_value_t = 0.85)]
    noise_threshold: f32,

    /// Cap on the `kept` array in the response (true total in `n_kept`)
    #[arg(long, default_value_t = 100)]
    max_kept: usize,

    /// Cap on the `removed` array in the response (true total in `n_removed`)
    #[arg(long, default_value_t = 100)]
    max_removed: usize,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/denoise.recent",
        serde_json::json!({
            "session":         session,
            "duration":        args.duration,
            "n":               args.n,
            "min_word_len":    args.min_word_len,
            "noise_threshold": args.noise_threshold,
            "max_kept":        args.max_kept,
            "max_removed":     args.max_removed,
        }),
    )
}
