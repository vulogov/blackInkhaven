//! `inkhaven backup --out <dir>` — zip the project into a dated archive.
//! Mirrors what the TUI does on its auto-backup-on-exit hook, but standalone
//! so users can take ad-hoc snapshots before risky operations.

use std::path::{Path, PathBuf};

use crate::backup;
use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;

pub fn run(project: &Path, out: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    // Read the config only to derive the canonical backup dir if `out` is
    // a relative path that needs resolving against the project. We do NOT
    // open the store: a backup is filesystem-level, and we don't want to
    // initialise duckdb/HNSW just to copy bytes.
    let _cfg = Config::load(&layout.config_path())?;

    // `out` may be relative — resolve against the cwd, not the project
    // root, so `inkhaven --project /foo backup --out .` lands in the
    // user's actual cwd. Resolve the project's own canonical path so we
    // can decide whether `out` lives inside it (and must be skip-listed).
    let abs_project = std::fs::canonicalize(&layout.root).map_err(crate::error::Error::Io)?;
    let abs_out = if out.is_absolute() {
        out.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(crate::error::Error::Io)?
            .join(out)
    };
    let skip = skip_dirs_for(&abs_project, &abs_out);

    let archive = backup::create_backup(&abs_project, &abs_out, &skip, None)?;
    eprintln!("wrote backup: {}", archive.display());
    Ok(())
}

/// Build the list of relative-or-absolute paths that the backup walker
/// should exclude. The backup output directory itself is the obvious
/// candidate when it sits inside the project — otherwise the zip would
/// recursively try to include its own grand-parent state. Returns paths in
/// the form `create_backup` checks against: project-relative if applicable,
/// absolute otherwise.
pub fn skip_dirs_for(abs_project: &Path, abs_out: &Path) -> Vec<PathBuf> {
    let mut skip: Vec<PathBuf> = Vec::new();
    if let Ok(rel) = abs_out.strip_prefix(abs_project) {
        if !rel.as_os_str().is_empty() {
            skip.push(rel.to_path_buf());
        }
    }
    skip.push(abs_out.to_path_buf());
    skip
}
