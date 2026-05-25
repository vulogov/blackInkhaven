//! Manual-backup orchestration on `App` (Ctrl+B Shift+B chord).
//! Thin wrappers — the heavy lifting lives in
//! `tui::backup_ui::run_manual_backup` (extracted in Phase 1).
//! These methods just (a) latch a "do it next tick" flag from
//! the key handler and (b) drain that flag from the run loop's
//! tick. Extracted from `tui::app` in the 1.2.7 refactor,
//! Phase 3 batch 5.

use ratatui::Terminal;

use crate::project::ProjectLayout;

use super::super::backup_ui::run_manual_backup;

impl super::App {

    /// Ctrl+B Shift+B — schedule an immediate project backup. The
    /// next main-loop tick picks up the flag and runs
    /// `run_pending_backup_now` against the live `terminal`.
    pub(super) fn schedule_backup_now(&mut self) {
        self.pending_backup_now = true;
        self.status = "Backup: zipping the project…".into();
    }

    /// Drain the `pending_backup_now` flag — runs the manual
    /// backup with its own splash. Honours
    /// `backup.wait_for_key_after_backup`. Status bar carries the
    /// final outcome.
    pub(super) fn run_pending_backup_now<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) {
        let layout = ProjectLayout::new(self.store.project_root());
        match run_manual_backup(terminal, &layout, &self.cfg) {
            Ok(path) => {
                self.status = format!("Backup OK · {}", path.display());
            }
            Err(e) => {
                self.status = format!("Backup failed: {e:#}");
            }
        }
    }

}
