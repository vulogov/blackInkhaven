use anyhow::{Context, Result};
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Script source: path to a .bund file, "-" or omitted for stdin.
    ///
    /// Scripts beginning with a shebang line (`#!/...`) are supported;
    /// the shebang is stripped before submission.
    source: Option<String>,
}

pub fn run(url: &str, _session: &str, args: Cmd) -> Result<Value> {
    let raw = match args.source.as_deref() {
        None | Some("-") => {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
                .context("failed to read script from stdin")?;
            buf
        }
        Some(path) => {
            std::fs::read_to_string(path)
                .with_context(|| format!("cannot read script {path:?}"))?
        }
    };

    let script = if raw.starts_with("#!") {
        raw.splitn(2, '\n').nth(1).unwrap_or("").to_string()
    } else {
        raw
    };

    crate::client::call(
        url,
        "v2/eval.queued",
        serde_json::json!({ "script": script }),
    )
}
