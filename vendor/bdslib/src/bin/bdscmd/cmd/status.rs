use anyhow::Result;
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {}

pub fn run(url: &str, _session: &str, _args: Cmd) -> Result<Value> {
    crate::client::call(url, "v2/status", serde_json::json!({}))
}
