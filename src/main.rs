mod ai;
mod assemble;
mod backup;
mod cli;
mod config;
mod config_tui;
mod crash;
mod health;
mod io_atomic;
mod prompts_tui;
mod error;
mod export;
mod grammar;
mod language_entry;
mod progress;
mod project;
mod scripting;
mod scrivener;
mod storage;
mod story_view;
mod store;
mod timeline;
mod tui;
mod typst_check;
mod typst_compile;
mod typst_inprocess;
mod typst_paragraph_render;
mod typst_world;

use clap::Parser;

fn main() {
    // Install the crash-report panic hook before
    // anything else.  Catches panics in CLI
    // subcommands, TUI startup, runtime init — every
    // code path.  TUI later registers its terminal-
    // restore closure via crash::set_terminal_restore.
    crash::install_panic_hook();

    let cli = cli::Cli::parse();

    // Tracing routing depends on the subcommand. TUI sessions must NOT
    // write to stderr — any log line printed mid-frame corrupts ratatui's
    // back-buffer (we'd see ghost panes or stray text inside the rendered
    // grid). Route TUI logs to a per-project file and keep CLI logs on
    // stderr where they're useful.
    // `Command::Config` and `Command::PromptsEditor` are
    // also TUIs (standalone HJSON / prompts editors) —
    // same stderr-quiet logging requirement as the main
    // editor.
    let is_tui = matches!(
        &cli.command,
        None | Some(cli::Command::Tui)
            | Some(cli::Command::Config)
            | Some(cli::Command::PromptsEditor)
    );
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("inkhaven=info,warn"));
    if is_tui {
        let log_path = tui_log_path(cli.project.as_deref());
        // Best-effort file open; fall back to stderr-less /dev/null if the
        // path can't be created (read-only project dir, full disk, etc.) —
        // logs are diagnostic, not load-bearing.
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .ok();
        if let Some(file) = file {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_ansi(false)
                .with_writer(std::sync::Mutex::new(file))
                .init();
        } else {
            // Last resort: drop logs entirely. We don't want stderr writes
            // bleeding into the TUI.
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(std::io::sink)
                .init();
        }
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
    }

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

    match cli.run() {
        Ok(()) => {}
        Err(e) => {
            // anyhow's `{:#}` chains causes without showing the backtrace.
            eprintln!("inkhaven: {e:#}");
            std::process::exit(1);
        }
    }
}

/// Where to write TUI session logs. Lives inside the project directory so
/// it's tied to the work being edited and easy to gitignore. Falls back to
/// the system temp dir if no `--project` was passed (the TUI will still try
/// to open `.` and may succeed).
fn tui_log_path(project: Option<&std::path::Path>) -> std::path::PathBuf {
    match project {
        Some(p) => p.join(".inkhaven.log"),
        None => std::env::current_dir()
            .unwrap_or_else(|_| std::env::temp_dir())
            .join(".inkhaven.log"),
    }
}
