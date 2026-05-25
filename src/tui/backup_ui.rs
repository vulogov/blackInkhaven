//! Backup orchestrators that animate the centered backup
//! splash while `crate::backup::create_backup` runs. The
//! draw + key-wait helpers live in [`super::splash`]; this
//! module owns the throttled progress callback, the
//! per-user backup-dir resolution, and the wait-for-key
//! gating. Extracted from `tui::app` in the 1.2.7 refactor.

use anyhow::Result;
use ratatui::Terminal;

use crate::config::Config;
use crate::project::ProjectLayout;

use super::splash::{draw_backup_splash, wait_for_any_key_on_backup_splash};

/// Check whether the project is overdue for a backup and run one if so,
/// streaming progress into the splash drawn over the alternate screen.
/// Returns `Ok(())` when no backup was required OR the backup succeeded;
/// `Err(_)` if the zip failed mid-flight.
pub(super) fn maybe_auto_backup<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    layout: &ProjectLayout,
    cfg: &Config,
) -> Result<()> {
    // Auto-backup opts out only via `max_age = 0s` now — `out_dir` empty
    // means "use the per-user default" (see `default_user_backup_dir`).
    let bcfg = &cfg.backup;
    if bcfg.max_age.as_secs() == 0 {
        return Ok(());
    }
    // If we already backed up recently, do nothing.
    let now = chrono::Utc::now();
    if let Some(state) = crate::backup::BackupState::load(&layout.root) {
        let age = now.signed_duration_since(state.last_at);
        if age.num_seconds() >= 0
            && (age.num_seconds() as u64) < bcfg.max_age.as_secs()
        {
            return Ok(());
        }
    }

    // Resolve the backup directory. Empty `out_dir` → per-user data
    // location; absolute path → used as-is; relative → resolved against
    // the project root (legacy override for users who explicitly want
    // backups inside the project).
    let out_dir = {
        let raw = bcfg.out_dir.trim();
        if raw.is_empty() {
            crate::store::default_user_backup_dir(&layout.root)
        } else {
            let p = std::path::PathBuf::from(raw);
            if p.is_absolute() {
                p
            } else {
                layout.root.join(p)
            }
        }
    };
    std::fs::create_dir_all(&out_dir).ok();
    let abs_project = std::fs::canonicalize(&layout.root)
        .unwrap_or_else(|_| layout.root.clone());
    let abs_out = std::fs::canonicalize(&out_dir).unwrap_or_else(|_| out_dir.clone());
    let skip = crate::cli::backup::skip_dirs_for(&abs_project, &abs_out);

    let project_display = layout.root.display().to_string();
    // First frame: 0/0 so the bar shows immediately even before file
    // enumeration completes.
    let _ = terminal.draw(|f| draw_backup_splash(f, &project_display, 0, 0, None));
    let mut last_redraw = std::time::Instant::now();
    // Track the most-recent progress numbers so the post-call
    // wait-for-key splash can keep the bar at 100% instead of
    // resetting it to 0/0.
    let mut last_progress: (usize, usize) = (0, 0);
    let backup_result = {
        let mut progress = |done: usize, total: usize| {
            last_progress = (done, total);
            // Throttle redraws to ~30Hz so a tiny project doesn't drown the
            // terminal in noise on a fast disk.
            if last_redraw.elapsed() < std::time::Duration::from_millis(33) {
                return;
            }
            last_redraw = std::time::Instant::now();
            let _ = terminal.draw(|f| {
                draw_backup_splash(f, &project_display, done, total, None)
            });
        };
        crate::backup::create_backup(&abs_project, &abs_out, &skip, Some(&mut progress))
    };
    let wait = cfg.backup.wait_for_key_after_backup;
    let (done_n, total_n) = last_progress;
    match backup_result {
        Ok(out_path) => {
            if wait {
                wait_for_any_key_on_backup_splash(
                    terminal,
                    &project_display,
                    done_n,
                    total_n,
                    Some(&out_path),
                );
            }
            Ok(())
        }
        Err(e) => {
            if wait {
                wait_for_any_key_on_backup_splash(
                    terminal,
                    &project_display,
                    done_n,
                    total_n,
                    None,
                );
            }
            Err(anyhow::Error::from(e))
        }
    }
}

/// Manual backup triggered by `Ctrl+B B` (uppercase). Unlike
/// `maybe_auto_backup`, this fires unconditionally — the
/// "we already backed up recently" cooldown is skipped because
/// the user explicitly asked for a fresh archive. The
/// `backup.wait_for_key_after_backup` toggle still applies.
pub(super) fn run_manual_backup<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    layout: &ProjectLayout,
    cfg: &Config,
) -> Result<std::path::PathBuf> {
    // Resolve the backup directory the same way `maybe_auto_backup`
    // does. Kept duplicated rather than refactored out so a single
    // future change to either path can be reasoned about in
    // isolation.
    let bcfg = &cfg.backup;
    let out_dir = {
        let raw = bcfg.out_dir.trim();
        if raw.is_empty() {
            crate::store::default_user_backup_dir(&layout.root)
        } else {
            let p = std::path::PathBuf::from(raw);
            if p.is_absolute() {
                p
            } else {
                layout.root.join(p)
            }
        }
    };
    std::fs::create_dir_all(&out_dir).ok();
    let abs_project = std::fs::canonicalize(&layout.root)
        .unwrap_or_else(|_| layout.root.clone());
    let abs_out = std::fs::canonicalize(&out_dir).unwrap_or_else(|_| out_dir.clone());
    let skip = crate::cli::backup::skip_dirs_for(&abs_project, &abs_out);

    let project_display = layout.root.display().to_string();
    let _ = terminal.draw(|f| draw_backup_splash(f, &project_display, 0, 0, None));
    let mut last_redraw = std::time::Instant::now();
    let mut last_progress: (usize, usize) = (0, 0);
    let backup_result = {
        let mut progress = |done: usize, total: usize| {
            last_progress = (done, total);
            if last_redraw.elapsed() < std::time::Duration::from_millis(33) {
                return;
            }
            last_redraw = std::time::Instant::now();
            let _ = terminal.draw(|f| {
                draw_backup_splash(f, &project_display, done, total, None)
            });
        };
        crate::backup::create_backup(&abs_project, &abs_out, &skip, Some(&mut progress))
    };
    let wait = cfg.backup.wait_for_key_after_backup;
    let (done_n, total_n) = last_progress;
    match backup_result {
        Ok(out_path) => {
            if wait {
                wait_for_any_key_on_backup_splash(
                    terminal,
                    &project_display,
                    done_n,
                    total_n,
                    Some(&out_path),
                );
            }
            Ok(out_path)
        }
        Err(e) => {
            if wait {
                wait_for_any_key_on_backup_splash(
                    terminal,
                    &project_display,
                    done_n,
                    total_n,
                    None,
                );
            }
            Err(anyhow::Error::from(e))
        }
    }
}
