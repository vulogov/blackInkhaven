use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Lookback window, e.g. "1h", "30min"
    #[arg(short, long)]
    duration: Option<String>,

    /// Range start as Unix seconds (requires --end-ts)
    #[arg(long)]
    start_ts: Option<i64>,

    /// Range end as Unix seconds (requires --start-ts)
    #[arg(long)]
    end_ts: Option<i64>,
}

pub fn run(url: &str, _session: &str, args: Cmd) -> Result<Value> {
    let mut params = serde_json::Map::new();
    if let Some(d) = args.duration {
        params.insert("duration".into(), serde_json::json!(d));
    }
    if let Some(s) = args.start_ts {
        params.insert("start_ts".into(), serde_json::json!(s));
    }
    if let Some(e) = args.end_ts {
        params.insert("end_ts".into(), serde_json::json!(e));
    }
    crate::client::call(url, "v2/shards", Value::Object(params))
}
