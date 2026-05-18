mod ai;
mod cli;
mod config;
mod error;
mod project;
mod store;
mod tui;

use clap::Parser;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("inkhaven=info,warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("inkhaven: could not start tokio runtime: {e}");
            std::process::exit(1);
        }
    };
    let _guard = rt.enter();

    let cli = cli::Cli::parse();
    match cli.run() {
        Ok(()) => {}
        Err(e) => {
            // anyhow's `{:#}` chains causes without showing the backtrace.
            eprintln!("inkhaven: {e:#}");
            std::process::exit(1);
        }
    }
}
