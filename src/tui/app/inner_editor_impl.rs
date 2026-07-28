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
        self.status = "Inner Editor · E engage ¶ · C converse · A ambient · F findings · Esc".into();
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
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.modal = Modal::None;
                self.seed_editor_session();
                self.right_pane = RightPane::Ai;
                self.change_focus(Focus::AiPrompt);
            }
            _ => {}
        }
        true
    }

    // ── conversation mode (Ctrl+V O → C, or F9 → Editor scope) ───────────────

    /// Seed an Editor conversation about the open paragraph and make it the
    /// sticky `EditorConversation` AI scope (re-cycling F9 refreshes the seed).
    /// Shared by the `Ctrl+V O → C` chord and the F9 scope.
    pub(super) fn seed_editor_session(&mut self) {
        use crate::tui::inference::AiMode;
        if !self.cfg.inner_editor.enabled {
            self.status = "Inner Editor is disabled (inner_editor.enabled: false)".into();
            return;
        }
        let findings = self.collect_open_editor_findings();
        let (prologue, opening) = self.editor_conversation_seed(&findings);
        self.seed_editor_chat(prologue, opening);
        self.ai_mode = AiMode::EditorConversation;
        self.status = if findings.is_empty() {
            "Editor scope · ask about this paragraph's craft (F9 to exit)".into()
        } else {
            format!(
                "Editor scope · {} observation(s) queued (F9 to exit)",
                findings.len()
            )
        };
    }

    /// The open paragraph's current Editor findings (the latest engagement's —
    /// a re-engagement clears the prior set).
    fn collect_open_editor_findings(&self) -> Vec<crate::inner_editor::EditorFinding> {
        let Some(doc) = self.opened.as_ref() else {
            return Vec::new();
        };
        let id = doc.id;
        crate::inner_editor::InnerEditorStore::open_for_project(self.store.project_root())
            .ok()
            .map(|s| {
                s.list_findings()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|sf| sf.paragraph_id == Some(id))
                    .map(|sf| sf.finding)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Build the `(prologue, opening)` for an Editor conversation.
    fn editor_conversation_seed(
        &self,
        findings: &[crate::inner_editor::EditorFinding],
    ) -> (String, String) {
        let obs = if findings.is_empty() {
            "(no observations recorded on this paragraph yet)".to_string()
        } else {
            findings
                .iter()
                .map(|f| format!("- [{} · {}] {}", f.severity.label(), f.category.label(), f.observation))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let prologue = format!(
            "{EDITOR_SEED_MARKER}You are the Inner Editor — a thoughtful editor discussing your \
             observations about the author's open paragraph. You OBSERVE craft; you never \
             prescribe, never rewrite, never command (the words \"should\" / \"must\" / \"need \
             to\" are not in your vocabulary — only \"I notice\", \"you might consider\", \"this \
             could\"). When an observation implies a change, frame it conditionally. Praise must \
             be specific and earned. Discuss these observations one at a time, helping the author \
             think about whether each applies and what (if anything) they want to do — the choice \
             is theirs.\n\nObservations on the open paragraph:\n{obs}",
            EDITOR_SEED_MARKER = super::EDITOR_SEED_MARKER,
        );
        let opening = match findings.first() {
            Some(f) => format!("A couple of things I noticed. {}", f.observation),
            None => "I'm reading for craft — rhythm, texture, what's working and what could be \
                     sharper. What would you like to look at?"
                .to_string(),
        };
        (prologue, opening)
    }

    /// Seed the chat with the Editor prologue + opening, replacing any prior
    /// Editor seed (re-entering swaps rather than stacks).
    fn seed_editor_chat(&mut self, prologue: String, opening: String) {
        use crate::ai::stream::ChatTurn;
        if let Some(i) = self
            .chat_history
            .iter()
            .position(|t| matches!(t, ChatTurn::User(s) if s.starts_with(super::EDITOR_SEED_MARKER)))
        {
            if self.chat_history.get(i + 1).is_some_and(|t| matches!(t, ChatTurn::Assistant(_))) {
                self.chat_history.remove(i + 1);
            }
            self.chat_history.remove(i);
        }
        let mut seeded = vec![ChatTurn::User(prologue), ChatTurn::Assistant(opening)];
        seeded.append(&mut self.chat_history);
        self.chat_history = seeded;
        self.chat_history_scroll = 0;
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

    /// `i` on a selected `inner_editor_observation` Output row — declare that
    /// finding's category a deliberate choice (chapter-scoped), writing an intent
    /// ledger entry that suppresses future ones like it, then dismiss the row.
    /// (Routed from `handle_output_key`'s `i`, which otherwise records a Socratic
    /// intent.) Uses the chapter **id** so the scope matches the engine's
    /// `FindingContext.chapter_id`.
    pub(super) fn inner_editor_record_intent_action(&mut self) {
        let Some(m) = crate::pane::output::active()
            .and_then(|s| s.active().ok())
            .unwrap_or_default()
            .get(self.output_selected)
            .cloned()
        else {
            return;
        };
        if m.kind != crate::pane::output::kinds::INNER_EDITOR_OBSERVATION {
            return;
        }
        let Some(cat) = m
            .metadata
            .get("category")
            .and_then(|c| c.as_str())
            .and_then(crate::inner_editor::EditorCategory::from_id)
        else {
            return;
        };
        let chapter = self.inner_editor_chapter_of(m.source_paragraph_id);
        match crate::inner_editor::intent_declare::declare_intent(
            self.store.project_root(),
            cat,
            chapter.as_deref(),
            None,
        ) {
            Ok(()) => {
                self.status = format!("declared intent — {} won't be re-noted here", cat.label());
                if let Some(s) = crate::pane::output::active() {
                    let _ = s.dismiss(m.id);
                }
            }
            Err(e) => self.status = format!("Inner Editor intent failed: {e}"),
        }
    }

    /// COMPANIONS-1 — record a dismissal of the selected `inner_editor_observation`
    /// row (no-op for any other kind), feeding promotion-from-dismissal. Returns
    /// whether it set a promotion nudge (so the caller keeps it on the status
    /// line). Mirrors `record_socratic_dismissal`.
    pub(super) fn record_editor_dismissal(&mut self, m: &crate::pane::output::Message) -> bool {
        use crate::inner_editor::InnerEditorStore;
        if m.kind != crate::pane::output::kinds::INNER_EDITOR_OBSERVATION {
            return false;
        }
        let Some(cat) = m
            .metadata
            .get("category")
            .and_then(|c| c.as_str())
            .and_then(crate::inner_editor::EditorCategory::from_id)
        else {
            return false;
        };
        let chapter = self.inner_editor_chapter_of(m.source_paragraph_id).unwrap_or_default();
        let Ok(store) = InnerEditorStore::open_for_project(self.store.project_root()) else {
            return false;
        };
        let _ = store.record_dismissal(cat, &chapter);
        if let Ok(cands) = store.promotion_candidates(5) {
            if cands.iter().any(|c| c.category == cat && c.chapter_id == chapter) {
                self.status = format!(
                    "You've dismissed several ✎ {} observations — `inkhaven inner-editor suggestions` can declare it deliberate",
                    cat.label()
                );
                return true;
            }
        }
        false
    }

    /// The enclosing chapter's **id** (UUID string) for a paragraph — matches
    /// what the engine puts in `FindingContext.chapter_id`. `None` when unknown.
    fn inner_editor_chapter_of(&self, paragraph_id: Option<uuid::Uuid>) -> Option<String> {
        let pid = paragraph_id?;
        let node = self.hierarchy.get(pid)?;
        self.hierarchy
            .ancestors(node)
            .into_iter()
            .find(|a| a.kind == crate::store::node::NodeKind::Chapter)
            .map(|c| c.id.to_string())
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
        // STRUCT-1 — Jinja templates aren't prose; style analysis doesn't apply
        // (and the body is mostly Typst markup + Jinja tags).
        if doc.content_type.as_deref() == Some("jinja") {
            if !ambient {
                self.status = "Inner Editor: jinja template paragraphs are skipped \
                               (style analysis does not apply to templates)"
                    .into();
            }
            return;
        }
        // STRUCT-2 — non-prose structural paragraphs (code / admonition / math /
        // table) are skipped; `para:procedure` is prose and still runs.
        if self
            .hierarchy
            .get(doc.id)
            .is_some_and(super::is_structural_nonprose)
        {
            if !ambient {
                self.status = "Inner Editor: structural paragraphs are skipped \
                               (code / math / table — not prose)"
                    .into();
            }
            return;
        }
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
        // The paragraph's current (newest) snapshot, so a finding records which
        // draft it was made against (findings-over-time across F6 snapshots).
        let snapshot_id = self.store.list_snapshots(id).ok().and_then(|s| s.first().map(|x| x.id));

        // 1.8.34 (1b) — hand the worker the App-held store handles so it reuses
        // the one pool instead of opening a second instance across the LLM call
        // (inner_editor.db / inner_socrates.db, and the MAIN store for grounding).
        let ie_store = self.inner_editor_store.clone();
        let socrates_store = self.inner_socrates_store.clone();
        let main_store = self.store.clone();
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
                    snapshot_id,
                    ie_store,
                    socrates_store,
                    main_store,
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
    snapshot_id: Option<Uuid>,
    ie_store: Option<crate::inner_editor::InnerEditorStore>,
    socrates_store: Option<crate::inner_socrates::storage::InnerSocratesStore>,
    main_store: crate::store::Store,
) -> std::result::Result<String, String> {
    use crate::inner_editor::output::{emit_finding, meets_threshold};
    let outcome = crate::inner_editor::engage(crate::inner_editor::EngageInput {
        project,
        paragraph_id: Some(paragraph_id),
        chapter_id,
        prose,
        preceding,
        language,
        snapshot_id,
        system_override: Some(system_override),
        force: false,
        ie_store,
        socrates_store,
        store: Some(main_store),
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
