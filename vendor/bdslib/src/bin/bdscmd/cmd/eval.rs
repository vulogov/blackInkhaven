use anyhow::{Context, Result};
use clap::Args;
use serde_json::Value;

#[derive(Args)]
pub struct Cmd {
    /// Script source: path to a .bund file, "-" or omitted for stdin.
    ///
    /// When used as a shebang interpreter (#!/path/to/bdscmd eval), the kernel
    /// passes the script file path here automatically.
    source: Option<String>,

    /// BUND VM context name
    #[arg(short, long, default_value = "default")]
    context: String,
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
            std::fs::read_to_string(path).with_context(|| format!("cannot read script {path}"))?
        }
    };

    // Strip shebang line so scripts can begin with #!/path/to/bdscmd eval
    let script = if raw.starts_with("#!") {
        raw.splitn(2, '\n').nth(1).unwrap_or("").to_string()
    } else {
        raw
    };

    crate::client::call(
        url,
        "v2/eval",
        serde_json::json!({ "context": args.context, "script": script }),
    )
}
