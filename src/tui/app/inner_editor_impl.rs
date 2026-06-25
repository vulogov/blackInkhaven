//! INNER_EDITOR-1 P4 — the TUI surface: the `Ctrl+V O` overview modal
//! (O = Observe; `Ctrl+B E` was taken by ToggleSound), the manual engage
//! (`E`), the ambient paragraph-pause auto-engage (opt-in via `A`), and the
//! off-thread engagement runner. The LLM call runs on a
//! background worker (`start_bg_job`) — exactly like WORLD-4's idle slow-track —
//! so the editor never blocks; the worker emits the (thresholded) observations
//! straight to the global Output store.

use crossterm::event::{KeyCode, KeyEvent};
use uuid::Uuid;

use super::super::focus::Focus;
use super::super::modal::Modal;
use super::{content_fingerprint, RightPane};

impl super::App {
    // ── the Ctrl+V O overview ────────────────────────────────────────────────

    pub(super) fn open_inner_editor_overview(&mut self) {
        let rows = self.build_inner_editor_rows();
        self.modal = Modal::InnerEditorOverview { rows, cursor: 0 };
        self.status = "Inner Editor · E engage ¶ · A ambient · F findings · Esc".into();
    }

    fn build_inner_editor_rows(&self) -> Vec<String> {
        use crate::inner_editor::InnerEditorStore;
        let ie = &self.cfg.inner_editor;
        let mut rows = vec![
            format!("Inner Editor — {}", if ie.enabled { "enabled" } else { "disabled" }),
            format!(
                "  tone {} · verbosity {} · praise {}",
                ie.persona.tone, ie.persona.verbosity, ie.persona.praise_frequency
            ),
            format!(
                "  genre-aware {} ({}) · belief stance {}",
                ie.persona.genre_aware,
                self.cfg.genre.as_deref().unwrap_or("none"),
                ie.persona.belief_stance_enabled
            ),
            format!(
                "Ambient auto-engage: {}",
                if self.ie_auto { "ON — runs on a writing pause" } else { "off (A toggles)" }
            ),
            String::new(),
        ];
        let root = self.store.project_root();
        if let Ok(s) = InnerEditorStore::open_for_project(root) {
            crate::dayclock::set_boundary(self.cfg.goals.day_boundary);
            let day = crate::dayclock::today_key();
            let used = s
                .llm_calls_today(&day, InnerEditorStore::ENGAGEMENT_SUB_BUDGET)
                .unwrap_or(0);
            let cap = ie.llm.editor_engagement.max_calls_per_day;
            rows.push(format!("Today: {used}/{cap} engagement call(s) (cap is informative)"));
            rows.push(String::new());
            let findings = s.list_findings().unwrap_or_default();
            if findings.is_empty() {
                rows.push("No observations yet.".into());
                rows.push("Press E to engage the open paragraph.".into());
            } else {
                rows.push(format!("Recent observations ({} shown):", findings.len().min(12)));
                for sf in findings.iter().take(12) {
                    rows.push(format!(
                        "  {} [{}] {}",
                        sf.finding.severity.label(),
                        sf.finding.category.label(),
                        sf.finding.observation
                    ));
                }
            }
        }
        rows.push(String::new());
        rows.push("The Editor observes craft; it never edits your prose.".into());
        rows
    }

    pub(super) fn inner_editor_overview_handle_key(&mut self, key: KeyEvent) -> bool {
        let n = match &self.modal {
            Modal::InnerEditorOverview { rows, .. } => rows.len(),
            _ => return false,
        };
        match key.code {
            KeyCode::Esc => {
                self.modal = Modal::None;
                self.status = "Inner Editor: closed".into();
            }
            KeyCode::Up => {
                if let Modal::InnerEditorOverview { cursor, .. } = &mut self.modal {
                    *cursor = cursor.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Modal::InnerEditorOverview { cursor, .. } = &mut self.modal {
                    if n > 0 {
                        *cursor = (*cursor + 1).min(n - 1);
                    }
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                self.modal = Modal::None;
                self.inner_editor_engage_open_paragraph(false);
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.toggle_inner_editor_auto();
                let rows = self.build_inner_editor_rows();
                if let Modal::InnerEditorOverview { rows: r, cursor } = &mut self.modal {
                    *r = rows;
                    *cursor = 0;
                }
            }
            KeyCode::Char('f') | KeyCode::Char('F') => self.inner_editor_jump_to_findings(),
            _ => {}
        }
        true
    }

    fn toggle_inner_editor_auto(&mut self) {
        self.ie_auto = !self.ie_auto;
        self.ie_last_fp = None;
        self.ie_needs_check = false;
        self.status = if self.ie_auto {
            "Inner Editor ambient auto-engage ON — runs on a writing pause (LLM cost)".into()
        } else {
            "Inner Editor ambient auto-engage off".into()
        };
    }

    /// Filter the Output pane to the Editor's findings and focus it.
    fn inner_editor_jump_to_findings(&mut self) {
        self.modal = Modal::None;
        self.output_filter.source = Some("inner-editor".into());
        self.right_pane = RightPane::Output;
        self.change_focus(Focus::Ai);
        self.status = "Inner Editor findings — Output filtered to inner-editor".into();
    }

    // ── engagement (manual + ambient share this) ─────────────────────────────

    /// Engage the Editor on the open paragraph. `ambient` only affects messaging
    /// (the manual path is explicit; the ambient path is quiet). Spawns the LLM
    /// call off-thread.
    pub(super) fn inner_editor_engage_open_paragraph(&mut self, ambient: bool) {
        if !self.cfg.inner_editor.enabled {
            if !ambient {
                self.status = "Inner Editor is disabled (inner_editor.enabled: false)".into();
            }
            return;
        }
        let Some(doc) = self.opened.as_ref() else {
            if !ambient {
                self.status = "Inner Editor: no paragraph open".into();
            }
            return;
        };
        let id = doc.id;
        let prose = doc.textarea.lines().join("\n");
        if prose.trim().is_empty() {
            if !ambient {
                self.status = "Inner Editor: the paragraph is empty".into();
            }
            return;
        }

        // Preceding context + enclosing chapter from the store (saved); the
        // current paragraph uses the live buffer above.
        let n = self.cfg.inner_editor.context.preceding_paragraphs;
        let (preceding, chapter_id) =
            crate::inner_editor::gather_context(&self.store, &self.hierarchy, id, n)
                .map(|g| (g.preceding, g.chapter_id))
                .unwrap_or_default();

        let want_lang = self.active_prompt_language();
        let system_override = self
            .resolve_prompt(crate::inner_editor::SYSTEM_PROMPT_NAME, &want_lang, || {
                crate::inner_editor::system_prompt(&want_lang).to_string()
            })
            .template;
        let threshold = self.cfg.inner_editor.output.severity_threshold.clone();
        let root = self.store.project_root().to_path_buf();

        self.ie_engage_para = Some(id);
        self.start_bg_job(
            super::BgJobKind::InnerEditorEngage,
            "inner editor",
            move |tx, _cancel| {
                let result = run_engagement(
                    root,
                    id,
                    chapter_id,
                    prose,
                    preceding,
                    want_lang,
                    system_override,
                    threshold,
                );
                let _ = tx.send(super::BgMsg::Done(result));
            },
        );
    }

    // ── ambient paragraph-pause trigger ──────────────────────────────────────

    /// The opt-in ambient: fingerprint the open paragraph each tick; a pause of
    /// `idle_threshold_seconds` (with no edit since, past the cooldown floor, and
    /// not the same version already engaged) fires one engagement in the
    /// background. Edits re-arm the idle timer (so it never fires mid-edit).
    pub(super) fn tick_inner_editor(&mut self) {
        if !self.cfg.inner_editor.enabled || !self.ie_auto {
            return;
        }
        let fp = self.opened.as_ref().map(content_fingerprint);
        if fp != self.ie_last_fp {
            self.ie_last_fp = fp;
            self.ie_activity_at = fp.map(|_| std::time::Instant::now());
            self.ie_needs_check = fp.is_some();
            return;
        }
        if !self.ie_needs_check || !matches!(self.modal, Modal::None) || self.bg_job.is_some() {
            return;
        }
        // Same content version already engaged → nothing to do.
        if fp.is_some() && fp == self.ie_last_engaged_fp {
            self.ie_needs_check = false;
            return;
        }
        // Cooldown floor since the last engagement.
        let cooldown = std::time::Duration::from_secs(self.cfg.inner_editor.engagement.cooldown_seconds);
        if let Some(last) = self.ie_last_engage_at {
            if last.elapsed() < cooldown {
                return;
            }
        }
        let idle = std::time::Duration::from_secs(self.cfg.inner_editor.engagement.idle_threshold_seconds);
        if let Some(t) = self.ie_activity_at {
            if t.elapsed() >= idle {
                self.ie_needs_check = false;
                self.ie_last_engaged_fp = fp;
                self.ie_last_engage_at = Some(std::time::Instant::now());
                self.inner_editor_engage_open_paragraph(true);
            }
        }
    }
}

/// The worker body (off-thread): run the engine, replace this paragraph's prior
/// Editor observations in Output, emit the kept findings that meet the visible
/// threshold. Returns the emitted count (as a string) or an error.
#[allow(clippy::too_many_arguments)]
fn run_engagement(
    project: std::path::PathBuf,
    paragraph_id: Uuid,
    chapter_id: Option<String>,
    prose: String,
    preceding: Vec<String>,
    language: String,
    system_override: String,
    threshold: String,
) -> std::result::Result<String, String> {
    use crate::inner_editor::output::{emit_finding, meets_threshold};
    let outcome = crate::inner_editor::engage(crate::inner_editor::EngageInput {
        project,
        paragraph_id: Some(paragraph_id),
        chapter_id,
        prose,
        preceding,
        language,
        snapshot_id: None,
        system_override: Some(system_override),
        force: false,
    })
    .map_err(|e| e.to_string())?;

    // Replace this paragraph's prior Editor observations.
    if let Some(s) = crate::pane::output::active() {
        if let Ok(msgs) = s.by_kind(crate::pane::output::kinds::INNER_EDITOR_OBSERVATION) {
            for m in msgs.iter().filter(|m| m.source_paragraph_id == Some(paragraph_id)) {
                let _ = s.dismiss(m.id);
            }
        }
    }
    let mut emitted = 0usize;
    for f in &outcome.findings {
        if meets_threshold(f.severity, &threshold) {
            emit_finding(f, Some(paragraph_id));
            emitted += 1;
        }
    }
    Ok(emitted.to_string())
}
