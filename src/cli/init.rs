use std::io::{self, Write};
use std::path::Path;

use tracing::info;

use crate::config::{Config, DEFAULT_PROJECT_CONFIG, DEFAULT_PROMPTS};
use crate::error::{Error, Result};
use crate::project::{PROMPTS_FILE_DEFAULT, ProjectLayout};
use crate::store::Store;

/// Initialise a new project at `path`. If the directory already exists we
/// require explicit consent before wiping it — either the `--force` flag or
/// a `y` answer to the interactive prompt. After confirmation the entire
/// directory is removed and freshly re-created so the new database starts
/// from a clean slate (stale `metadata.db` + `vectors/` from a previous
/// install never trip up the schema).
pub fn run(path: &Path, force: bool) -> Result<()> {
    let layout = ProjectLayout::new(path);

    if path.exists() {
        // Either the user passed --force (non-interactive overwrite) or
        // we must ask. Anything else aborts cleanly.
        let confirmed = if force {
            true
        } else {
            confirm_overwrite(path)?
        };
        if !confirmed {
            return Err(Error::Store(format!(
                "init aborted — `{}` left untouched",
                path.display()
            )));
        }
        // Refuse to recursively delete the project root if the cwd lives
        // inside it (Mac/Linux happily wipes itself out of the cwd and
        // hands back an EINVAL on every subsequent operation).
        if let Ok(cwd) = std::env::current_dir() {
            if let Ok(abs_target) = std::fs::canonicalize(path) {
                if cwd.starts_with(&abs_target) {
                    return Err(Error::Store(format!(
                        "refusing to wipe `{}` — your current directory lives inside it",
                        abs_target.display()
                    )));
                }
            }
        }
        std::fs::remove_dir_all(path).map_err(Error::Io)?;
    }

    layout.create_layout()?;

    let config_path = layout.config_path();
    std::fs::write(&config_path, DEFAULT_PROJECT_CONFIG)?;
    info!(path = %config_path.display(), "wrote project config");

    let prompts_path = layout.root.join(PROMPTS_FILE_DEFAULT);
    std::fs::write(&prompts_path, DEFAULT_PROMPTS)?;
    info!(path = %prompts_path.display(), "wrote prompt library");

    // Round-trip parse the config to validate it.
    let cfg = Config::load(&config_path)?;

    // Open the document store. This creates `metadata.db` + `vecstore/`.
    // First-run embedding-model download (if needed) happens here.
    let _store = Store::open(layout.clone(), &cfg)?;

    eprintln!("Initialized inkhaven project at {}", layout.root.display());
    eprintln!("  config:    {}", layout.config_path().display());
    eprintln!("  prompts:   {}", layout.root.join(PROMPTS_FILE_DEFAULT).display());
    eprintln!("  store db:  {}", layout.metadata_db_path().display());
    eprintln!("  vecstore:  {}", layout.vecstore_path().display());
    eprintln!("  books:     {}", layout.books_path().display());
    Ok(())
}

/// Interactive y/N prompt on stderr. Returns true only when the user types
/// `y` / `yes` (case-insensitive). Any other input — including an empty
/// line, EOF, or `n` — returns false so we never wipe by accident.
fn confirm_overwrite(path: &Path) -> Result<bool> {
    eprint!(
        "Directory `{}` already exists. Remove it and re-initialise? [y/N] ",
        path.display()
    );
    io::stderr().flush().ok();
    let mut buf = String::new();
    if io::stdin().read_line(&mut buf).map_err(Error::Io)? == 0 {
        return Ok(false);
    }
    let answer = buf.trim().to_ascii_lowercase();
    Ok(matches!(answer.as_str(), "y" | "yes"))
}
