use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Lookback window, e.g. "1h", "30min", "7days"
    #[arg(short, long)]
    duration: String,

    /// Hard cap on summary length; 0 → derive from --ratio
    #[arg(long, default_value_t = 0)]
    max_sentences: usize,

    /// Fraction of inputs kept when --max-sentences is 0
    #[arg(long, default_value_t = 0.3)]
    ratio: f32,

    /// Tokens shorter than this are dropped before scoring
    #[arg(long, default_value_t = 2)]
    min_word_len: usize,

    /// PageRank damping factor
    #[arg(long, default_value_t = 0.85)]
    damping: f32,

    /// Maximum PageRank iterations
    #[arg(long, default_value_t = 30)]
    iters: usize,

    /// L1-norm change tolerance for PageRank early exit
    #[arg(long, default_value_t = 1e-4)]
    tolerance: f32,
}

pub fn run(url: &str, session: &str, args: Cmd) -> Result<Value> {
    crate::client::call(
        url,
        "v2/textrank.templates",
        serde_json::json!({
            "session":       session,
            "duration":      args.duration,
            "max_sentences": args.max_sentences,
            "ratio":         args.ratio,
            "min_word_len":  args.min_word_len,
            "damping":       args.damping,
            "iters":         args.iters,
            "tolerance":     args.tolerance,
        }),
    )
}
