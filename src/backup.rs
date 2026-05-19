//! Project zip backup / restore + last-backup timestamp tracking.
//!
//! Used by the `inkhaven backup` / `inkhaven restore` CLI subcommands and
//! by the TUI's exit hook (auto-backup when `BackupConfig::max_age` is
//! exceeded). Skips the alternate-screen log file, prior backup zips that
//! happen to live under the project root, and any directory the user has
//! configured as the backup output target (avoiding a recursive zip of
//! zips). Everything else under the project root is shipped verbatim so a
//! `restore` produces an identical working tree.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::{Error, Result};

/// Sentinel filename stored alongside the project that records when the
/// last backup happened. Kept inside the project (not in the home dir) so
/// it travels with the project across machines.
pub const LAST_BACKUP_FILE: &str = ".inkhaven-backup.json";

/// Lightweight "we backed up at this RFC-3339 timestamp" record. Missing
/// or corrupt file → returns `None` and callers treat it as "never".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupState {
    pub last_at: chrono::DateTime<chrono::Utc>,
}

impl BackupState {
    pub fn load(project_root: &Path) -> Option<Self> {
        let path = project_root.join(LAST_BACKUP_FILE);
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }
    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = project_root.join(LAST_BACKUP_FILE);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }
}

/// Build the deterministic backup filename for a given timestamp. The
/// caller chose the format `blackinkhaven_YYYYDDMM_HHMMSS.zip` explicitly
/// — note this is *not* ISO-style ordering (YYYYMMDD), so files for the
/// same year won't sort chronologically on filename alone. They will still
/// sort within a single month.
pub fn backup_filename(now: chrono::DateTime<chrono::Utc>) -> String {
    now.format("blackinkhaven_%Y%d%m_%H%M%S.zip").to_string()
}

/// Optional progress callback fired while the backup walker enumerates and
/// streams files into the zip. `done` and `total` are file counts, not
/// bytes — gives the splash-screen progress bar something sensible to draw.
pub type ProgressFn<'a> = Option<&'a mut dyn FnMut(usize, usize)>;

/// Zip `project_root` into `<out_dir>/blackinkhaven_YYYYDDMM_HHMMSS.zip`.
/// Returns the path of the newly created archive.
///
/// `skip_dirs` lists directories (relative to `project_root`) whose
/// contents are excluded — typically the backup output directory itself
/// when it lives inside the project. The runtime log file
/// (`.inkhaven.log`) is always skipped.
pub fn create_backup(
    project_root: &Path,
    out_dir: &Path,
    skip_dirs: &[PathBuf],
    progress: ProgressFn<'_>,
) -> Result<PathBuf> {
    if !project_root.is_dir() {
        return Err(Error::Store(format!(
            "backup: project root `{}` is not a directory",
            project_root.display()
        )));
    }
    std::fs::create_dir_all(out_dir).map_err(Error::Io)?;

    let now = Utc::now();
    let filename = backup_filename(now);
    let out_path = out_dir.join(&filename);

    // First pass: enumerate files so the progress callback gets a sensible
    // denominator. walkdir is cheap to traverse twice at literary scale.
    let mut to_include: Vec<PathBuf> = Vec::new();
    for entry in walkdir::WalkDir::new(project_root)
        .sort_by_file_name()
        .follow_links(false)
    {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("warning: walking {}: {e}", project_root.display());
                continue;
            }
        };
        if entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        let rel = match path.strip_prefix(project_root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if rel.as_os_str().is_empty() {
            continue;
        }
        if rel.ends_with(".inkhaven.log") {
            continue;
        }
        if skip_dirs
            .iter()
            .any(|skip| rel.starts_with(skip) || path.starts_with(skip))
        {
            continue;
        }
        to_include.push(path.to_path_buf());
    }

    let total = to_include.len();
    let file = std::fs::File::create(&out_path).map_err(Error::Io)?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    let mut buf = Vec::with_capacity(8 * 1024);
    let mut done = 0usize;
    let mut progress = progress;
    for path in to_include {
        let rel = path
            .strip_prefix(project_root)
            .map_err(|e| Error::Store(format!("backup: strip prefix: {e}")))?;
        let zip_name = rel
            .to_str()
            .ok_or_else(|| Error::Store(format!("backup: non-UTF8 path: {}", rel.display())))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        zip.start_file(&zip_name, opts)
            .map_err(|e| Error::Store(format!("backup: zip start {zip_name}: {e}")))?;
        let mut src = std::fs::File::open(&path).map_err(Error::Io)?;
        buf.clear();
        src.read_to_end(&mut buf).map_err(Error::Io)?;
        zip.write_all(&buf).map_err(Error::Io)?;
        done += 1;
        if let Some(cb) = progress.as_deref_mut() {
            cb(done, total);
        }
    }
    zip.finish()
        .map_err(|e| Error::Store(format!("backup: zip finalise: {e}")))?;

    // Record the backup timestamp so the TUI's exit hook can decide
    // whether the next session is overdue for another snapshot.
    let _ = (BackupState { last_at: now }).save(project_root);

    Ok(out_path)
}

/// Restore a backup zip into `dest`. Creates `dest` if missing. Refuses to
/// proceed if `dest` already contains an `inkhaven.hjson` (suggests an
/// existing project) so the user doesn't accidentally clobber live work.
pub fn restore_backup(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive).map_err(Error::Io)?;
    let mut zip = ZipArchive::new(file)
        .map_err(|e| Error::Store(format!("restore: open zip: {e}")))?;

    // Sanity: a real inkhaven backup will contain `inkhaven.hjson` at the
    // archive root. Refuse arbitrary zips so we don't splatter random
    // content into the destination directory.
    let has_marker = (0..zip.len()).any(|i| {
        zip.by_index(i)
            .map(|f| f.name() == "inkhaven.hjson")
            .unwrap_or(false)
    });
    if !has_marker {
        return Err(Error::Store(
            "restore: archive does not look like an inkhaven backup (no `inkhaven.hjson` at root)"
                .into(),
        ));
    }
    if dest.join("inkhaven.hjson").exists() {
        return Err(Error::Store(format!(
            "restore: `{}` already contains an inkhaven project — pick a fresh directory",
            dest.display()
        )));
    }
    std::fs::create_dir_all(dest).map_err(Error::Io)?;

    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| Error::Store(format!("restore: read entry: {e}")))?;
        let name = entry
            .enclosed_name()
            .ok_or_else(|| Error::Store(format!("restore: unsafe entry: {}", entry.name())))?
            .to_path_buf();
        let out_path = dest.join(&name);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(Error::Io)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        let mut out_file = std::fs::File::create(&out_path).map_err(Error::Io)?;
        std::io::copy(&mut entry, &mut out_file).map_err(Error::Io)?;
    }
    Ok(())
}
