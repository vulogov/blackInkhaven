//! Methods on `App` that drive the F6 swim-lane timeline view
//! (`Modal::TimelineView`) and its descent / scope navigation —
//! everything prefixed `timeline_*` in the original app.rs.
//! Pure state-transition methods; the data shapes themselves
//! (`TimelineViewState`, `TimelineEvent`, …) live in
//! `tui::timeline_state`, and the swim-lane painter lives in
//! `tui::app::render`. Extracted from `tui::app` in the 1.2.7
//! refactor, Phase 3 batch 2.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use uuid::Uuid;

use super::{handle_text_input_key, timeline_health_default_prompt};

use crate::ai::stream::spawn_chat_stream;
use crate::store::node::NodeKind;

use super::super::focus::Focus;
use super::super::inference::{Inference, InferenceStatus};
use super::super::input::TextInput;
use super::super::modal::Modal;
use super::super::session::TimelineViewSnapshot;
use super::super::timeline_state::{
    cycle_track, timeline_step_event_cursor, TimelineDescentChoice, TimelineDescentState,
    TimelineEvent, TimelineFocusLevel, TimelineViewState,
};

impl super::App {

    /// 1.2.7+ — snapshot the open swim-lane view's state
    /// into the per-book cache. Called from the Esc handler
    /// just before the timeline modal closes so the next
    /// open of the same book restores it.
    pub(super) fn timeline_capture_view_state(&mut self) {
        let Modal::TimelineView { state } = &self.modal else { return; };
        let snap = TimelineViewSnapshot {
            collapsed_tracks: state.collapsed_tracks.iter().cloned().collect(),
            expanded_track: state.expanded_track.clone(),
            track_highlight: state.track_highlight.clone(),
            ticks_per_cell: state.ticks_per_cell,
            scroll_ticks: state.scroll_ticks,
            cursor_ticks: state.cursor_ticks,
        };
        let book_id = state.book_id;
        self.timeline_views.insert(book_id, snap);
    }

    /// 1.2.7+ — apply a cached snapshot onto a freshly-opened
    /// `Modal::TimelineView` state. Skipped silently when no
    /// cache entry exists or when the cached zoom is
    /// non-positive (corrupt session). All-or-nothing — we
    /// keep auto-fit defaults when restoring fails.
    pub(super) fn timeline_restore_view_state(&mut self) {
        let Modal::TimelineView { state } = &mut self.modal else { return; };
        let Some(snap) = self.timeline_views.get(&state.book_id).cloned() else {
            return;
        };
        if snap.ticks_per_cell <= 0.0 {
            return;
        }
        state.collapsed_tracks = snap.collapsed_tracks.into_iter().collect();
        state.expanded_track = snap.expanded_track;
        state.track_highlight = snap.track_highlight;
        state.ticks_per_cell = snap.ticks_per_cell;
        state.scroll_ticks = snap.scroll_ticks;
        state.cursor_ticks = snap.cursor_ticks;
        // focus_level: keep the open-time default (Track).
        // Restoring Event focus from the previous session
        // would also require validating that
        // `expanded_track` still maps to events that exist
        // — too much for a UX nicety. The user re-Enters.
    }

    pub(super) fn timeline_view_handle_key(&mut self, key: KeyEvent) {
        // Descent picker captures keys when active.
        if self.timeline_descent_active() {
            self.timeline_descent_handle_key(key);
            return;
        }
        match key.code {
            // Scroll: ← / → shift the viewport by 1/6 of its
            // span (a "page-step" feels right at the default
            // ticks_per_cell). Shift+Left/Right page-jump
            // (full viewport width).
            KeyCode::Left => self.timeline_scroll(-1, false),
            KeyCode::Right => self.timeline_scroll(1, false),
            KeyCode::PageUp => self.timeline_scroll(-1, true),
            KeyCode::PageDown => self.timeline_scroll(1, true),
            // 1.2.6+ — Up/Down hop the cursor between events
            // chronologically. Pairs with Left/Right (viewport
            // scroll) and PgUp/PgDn (page scroll) to give the
            // user four distinct navigation modes.
            KeyCode::Up => self.timeline_step_cursor(-1),
            KeyCode::Down => self.timeline_step_cursor(1),
            // Zoom: + / =  zooms in (fewer ticks per cell),
            // - / _  zooms out (more ticks per cell). Each
            // press is a multiplicative step; keeps the
            // cursor tick fixed so the user can drill into
            // a specific event.
            KeyCode::Char('+') | KeyCode::Char('=') => self.timeline_zoom(0.66),
            KeyCode::Char('-') | KeyCode::Char('_') => self.timeline_zoom(1.5),
            KeyCode::Char('0') => self.timeline_reset_zoom(),
            // Cursor at center / Home / End — quick recenters.
            KeyCode::Home => self.timeline_jump_home(),
            KeyCode::End => self.timeline_jump_end(),
            // Descent-picker key dispatch routes here first.
            // When descent.is_some(), Up/Down/Enter/Esc are
            // captured by the picker. Otherwise they fall
            // through to the swim-lane handler above. We
            // re-handle a few above; this block catches what
            // the descent picker needs and the scope-nav chords.
            KeyCode::Char('u') | KeyCode::Char('U') => self.timeline_up_scope(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.timeline_open_descent(),
            KeyCode::Char('b') | KeyCode::Char('B') => self.timeline_jump_book_scope(),
            KeyCode::Char('p') | KeyCode::Char('P') => self.timeline_toggle_project(),
            // 1.2.7+ — tree-style nav. Tab cycles at the
            // current focus level (Track or Event). Shift+Tab
            // cycles backward. Enter descends; Backspace pops
            // up; Esc closes the modal.
            KeyCode::Tab => self.timeline_tab(false),
            KeyCode::BackTab => self.timeline_tab(true),
            KeyCode::Enter => self.timeline_enter(),
            KeyCode::Backspace => self.timeline_pop_to_track_focus(),
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.timeline_open_new_event_prompt()
            }
            // 1.2.6+ Phase 3 — AI health critique.
            //   y       — current scope, highlighted track only.
            //   Y       — current scope, all tracks.
            //   Ctrl+Y  — book scope, all tracks (widens regardless).
            KeyCode::Char('y') | KeyCode::Char('Y')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.timeline_start_health_critique(true, true);
            }
            KeyCode::Char('Y') => {
                self.timeline_start_health_critique(false, true);
            }
            KeyCode::Char('y') => {
                self.timeline_start_health_critique(false, false);
            }
            // 1.2.7+ — F12 mirrors the editor's "full AI
            // analysis" chord. In the timeline view it widens
            // to book scope + all tracks (same as Ctrl+Y) so
            // function-key users get the broadest consistency
            // audit without remembering scope letters.
            KeyCode::F(12) => {
                self.timeline_start_health_critique(true, true);
            }
            // 1.2.7+ — Space toggles collapse on the currently
            // highlighted track (Tab cycles). Collapsed tracks
            // render as a single dim header line; expanded
            // tracks show the full swim lane. Mirrors the
            // tree pane's ▾/▸ collapse model.
            KeyCode::Char(' ') => {
                self.timeline_toggle_collapse();
            }
            _ => {}
        }
    }

    /// 1.2.7+ — the effective track-key for an event,
    /// matching what `layout_swim_lanes` uses to build row
    /// labels. In project-overlay mode the key is prefixed
    /// with the event's book slug
    /// (`aerin-saga/main` vs bare `main`) so cross-book
    /// tracks don't collide. All track-aware helpers must
    /// agree on this key.
    pub(super) fn timeline_event_track_key(&self, e: &TimelineEvent) -> String {
        let raw = e
            .track
            .clone()
            .unwrap_or_else(|| self.cfg.timeline.default_track.clone());
        if e.book_prefix.is_empty() {
            raw
        } else {
            format!("{}/{}", e.book_prefix, raw)
        }
    }

    /// 1.2.7+ — collect the events of a given track in
    /// chronological order. Used by the tree-style nav to
    /// cycle events of the expanded track via Tab.
    pub(super) fn timeline_events_of_track(&self, label: &str) -> Vec<Uuid> {
        let Modal::TimelineView { state } = &self.modal else { return Vec::new(); };
        let mut hits: Vec<(i64, Uuid)> = state
            .events
            .iter()
            .filter(|e| !e.is_orphan && self.timeline_event_track_key(e) == label)
            .map(|e| (e.start_ticks, e.id))
            .collect();
        hits.sort_by_key(|(t, _)| *t);
        hits.into_iter().map(|(_, id)| id).collect()
    }

    /// 1.2.7+ — list of tracks visible in the swim lane, in
    /// the same order the render uses (default track first,
    /// then alphabetical). Skips the synthetic `orphan` row.
    /// Uses the book-prefixed key when project overlay is on.
    pub(super) fn timeline_visible_tracks(&self) -> Vec<String> {
        let Modal::TimelineView { state } = &self.modal else { return Vec::new(); };
        let default_track = self.cfg.timeline.default_track.clone();
        let mut tracks: Vec<String> = state
            .events
            .iter()
            .filter(|e| !e.is_orphan)
            .map(|e| self.timeline_event_track_key(e))
            .collect();
        tracks.sort();
        tracks.dedup();
        if let Some(i) = tracks.iter().position(|t| t == &default_track) {
            tracks.swap(0, i);
        }
        tracks
    }

    /// 1.2.7+ — Tab / Shift+Tab handler.
    /// * Track focus: cycle tracks (forward or backward).
    /// * Event focus: cycle events of the expanded track.
    pub(super) fn timeline_tab(&mut self, backward: bool) {
        // Pull the focus level out first to avoid borrow
        // tangles with the helpers below.
        let focus = {
            let Modal::TimelineView { state } = &self.modal else { return; };
            state.focus_level.clone()
        };
        match focus {
            TimelineFocusLevel::Track => self.timeline_tab_track(backward),
            TimelineFocusLevel::Event => self.timeline_tab_event(backward),
        }
    }

    pub(super) fn timeline_tab_track(&mut self, backward: bool) {
        let tracks = self.timeline_visible_tracks();
        if tracks.is_empty() {
            self.status = "timeline · no tracks to cycle".into();
            return;
        }
        let Modal::TimelineView { state } = &mut self.modal else { return; };
        let current_idx = state
            .track_highlight
            .as_ref()
            .and_then(|h| tracks.iter().position(|t| t == h));
        let next_idx = match (current_idx, backward) {
            (None, false) => 0,
            (None, true) => tracks.len() - 1,
            (Some(i), false) => (i + 1) % tracks.len(),
            (Some(i), true) => (i + tracks.len() - 1) % tracks.len(),
        };
        let next_label = tracks[next_idx].clone();
        state.track_highlight = Some(next_label.clone());
        self.status = format!(
            "timeline · track `{next_label}` highlighted — Enter to expand · Space to collapse"
        );
    }

    pub(super) fn timeline_tab_event(&mut self, backward: bool) {
        // Pull the expanded track + current event out, then
        // compute next via the events_of_track helper.
        let (track_label, current_event) = {
            let Modal::TimelineView { state } = &self.modal else { return; };
            (
                state.expanded_track.clone(),
                state.selected_event_id,
            )
        };
        let Some(label) = track_label else {
            // Shouldn't happen — Event focus implies
            // expanded_track is set — but recover safely.
            self.timeline_pop_to_track_focus();
            return;
        };
        let events = self.timeline_events_of_track(&label);
        if events.is_empty() {
            self.status = format!("timeline · `{label}` has no events");
            self.timeline_pop_to_track_focus();
            return;
        }
        let current_idx = current_event.and_then(|id| events.iter().position(|e| *e == id));
        let next_idx = match (current_idx, backward) {
            (None, false) => 0,
            (None, true) => events.len() - 1,
            (Some(i), false) => (i + 1) % events.len(),
            (Some(i), true) => (i + events.len() - 1) % events.len(),
        };
        let next_id = events[next_idx];
        // Use the existing select-by-id flow (sets cursor +
        // pans viewport).
        self.timeline_select_event_by_id(next_id);
    }

    /// 1.2.7+ — Enter handler.
    /// * Track focus: expand the highlighted track and drop
    ///   into Event focus (first event of that track).
    /// * Event focus: open the linked-paragraphs picker
    ///   (existing `timeline_open_event_under_cursor`).
    pub(super) fn timeline_enter(&mut self) {
        let focus = {
            let Modal::TimelineView { state } = &self.modal else { return; };
            state.focus_level.clone()
        };
        match focus {
            TimelineFocusLevel::Track => {
                let highlight = {
                    let Modal::TimelineView { state } = &self.modal else { return; };
                    state.track_highlight.clone()
                };
                let Some(label) = highlight else {
                    self.status =
                        "timeline · Tab to highlight a track, then Enter to expand its events".into();
                    return;
                };
                let events = self.timeline_events_of_track(&label);
                let first = events.first().copied();
                {
                    let Modal::TimelineView { state } = &mut self.modal else { return; };
                    state.expanded_track = Some(label.clone());
                    state.focus_level = TimelineFocusLevel::Event;
                }
                if let Some(id) = first {
                    self.timeline_select_event_by_id(id);
                }
                let n = events.len();
                self.status = format!(
                    "timeline · expanded `{label}` ({n} event{plural}) · Tab cycles events · Enter opens linked ¶ · Backspace pops up",
                    plural = if n == 1 { "" } else { "s" }
                );
            }
            TimelineFocusLevel::Event => {
                self.timeline_open_event_under_cursor();
            }
        }
    }

    /// Helper used by Tab-cycle-events and Enter-on-track to
    /// stamp the selection + pan the viewport in one place.
    pub(super) fn timeline_select_event_by_id(&mut self, id: Uuid) {
        let (start_ticks, end_ticks) = {
            let Modal::TimelineView { state } = &self.modal else { return; };
            let ev = state.events.iter().find(|e| e.id == id);
            match ev {
                Some(e) => (e.start_ticks, e.end_ticks),
                None => return,
            }
        };
        let Modal::TimelineView { state } = &mut self.modal else { return; };
        state.selected_event_id = Some(id);
        state.cursor_ticks = start_ticks;
        // Same auto-pan rule as `timeline_step_cursor`.
        let term_w = crossterm::terminal::size()
            .map(|(w, _)| w as usize)
            .unwrap_or(80);
        let content_w = term_w.saturating_sub(16).max(40) as f64;
        let visible_ticks = (content_w * state.ticks_per_cell) as i64;
        let span_end = end_ticks.unwrap_or(start_ticks);
        let span_width = (span_end - start_ticks).abs();
        if span_width >= visible_ticks {
            state.scroll_ticks =
                start_ticks.saturating_sub((visible_ticks - span_width) / 2);
        } else {
            let margin = (visible_ticks / 7).max(2);
            let left = state.scroll_ticks;
            let right = state.scroll_ticks + visible_ticks;
            if start_ticks < left + margin {
                state.scroll_ticks = start_ticks.saturating_sub(margin);
            } else if span_end > right - margin {
                state.scroll_ticks = span_end.saturating_sub(visible_ticks - margin);
            }
        }
    }

    /// 1.2.7+ — Backspace / Esc-at-Event handler. Drops back
    /// to Track focus, clears event selection but keeps the
    /// track highlight + the expanded sub-rows visible.
    pub(super) fn timeline_pop_to_track_focus(&mut self) {
        let Modal::TimelineView { state } = &mut self.modal else { return; };
        if state.focus_level == TimelineFocusLevel::Event {
            state.focus_level = TimelineFocusLevel::Track;
            // Clear selection so the swim-lane highlight goes
            // away; expansion stays so the user can re-enter
            // it with Enter.
            state.selected_event_id = None;
            let label = state
                .expanded_track
                .clone()
                .unwrap_or_else(|| "?".into());
            self.status = format!(
                "timeline · back to track focus (`{label}` still expanded — Enter re-enters)"
            );
        }
    }

    /// 1.2.7+ — flip the highlighted track between expanded
    /// (▾) and collapsed (▸). When no track is highlighted,
    /// status hint nudges the user toward Tab. Orphan row is
    /// not collapsible — it's already a one-liner.
    pub(super) fn timeline_toggle_collapse(&mut self) {
        let Modal::TimelineView { state } = &mut self.modal else { return; };
        let Some(label) = state.track_highlight.clone() else {
            self.status =
                "timeline · Tab to highlight a track, then Space to collapse / expand".into();
            return;
        };
        let was_collapsed = state.collapsed_tracks.contains(&label);
        if was_collapsed {
            state.collapsed_tracks.remove(&label);
            self.status = format!("timeline · expanded `{label}`");
        } else {
            state.collapsed_tracks.insert(label.clone());
            self.status = format!("timeline · collapsed `{label}`");
        }
    }

    /// Cycle `track_highlight` through the tracks that
    /// appear in the current event snapshot. None → first
    /// track → next → … → None.
    pub(super) fn timeline_cycle_track(&mut self) {
        let Modal::TimelineView { state } = &mut self.modal else { return; };
        let default_track = self.cfg.timeline.default_track.clone();
        let mut tracks: Vec<String> = state
            .events
            .iter()
            .filter(|e| !e.is_orphan)
            .map(|e| e.track.clone().unwrap_or_else(|| default_track.clone()))
            .collect();
        tracks.sort();
        tracks.dedup();
        let next = cycle_track(state.track_highlight.as_deref(), &tracks);
        state.track_highlight = next.clone();
        self.status = match next {
            Some(t) => format!("timeline · track highlight: `{t}`"),
            None => "timeline · track highlight cleared".into(),
        };
    }

    /// Find the event closest to `cursor_ticks` (preferring
    /// the highlighted track) and open the LinkPicker over
    /// its `linked_paragraphs` — the scenes / manuscript
    /// paragraphs the event is anchored to.
    ///
    /// 1.2.7+ behaviour change: Enter used to open the
    /// event paragraph body itself. That surface still lives
    /// behind the `Ctrl+V e` picker. Enter in the timeline
    /// view now takes the user to the *content* the event
    /// references — the typical follow-up from "I see this
    /// event on the swim lane, take me to the scene it
    /// anchors". Zero / single / many linked paragraphs
    /// each handled with the right shortcut.
    pub(super) fn timeline_open_event_under_cursor(&mut self) {
        let Modal::TimelineView { state } = &self.modal else { return; };
        // 1.2.7+ — when ↑/↓ has explicitly selected an event,
        // route Enter to THAT event so the highlight on the
        // swim lane matches the picker that opens. Falls back
        // to the nearest-by-tick search for cold opens (e.g.
        // first Enter after opening the timeline).
        let best: Option<(Uuid, i64)> = if let Some(id) = state.selected_event_id {
            state
                .events
                .iter()
                .find(|e| e.id == id)
                .map(|e| (e.id, 0))
        } else {
            let cursor = state.cursor_ticks;
            let highlight = state.track_highlight.clone();
            let mut best: Option<(Uuid, i64)> = None;
            for ev in &state.events {
                // Track filter is a preference, not a hard
                // requirement — if no on-track event is close,
                // we still pick the absolute nearest.
                let on_highlight = match (&highlight, &ev.track) {
                    (Some(h), Some(t)) => h == t,
                    (Some(_), None) => false,
                    (None, _) => true,
                };
                let distance = (ev.start_ticks - cursor).abs();
                let weight = if on_highlight { distance } else { distance + 1_000_000 };
                match best {
                    None => best = Some((ev.id, weight)),
                    Some((_, w)) if weight < w => best = Some((ev.id, weight)),
                    _ => {}
                }
            }
            best
        };
        let Some((event_id, _)) = best else {
            self.status = "timeline · no events to open".into();
            return;
        };
        let event_title = self
            .hierarchy
            .get(event_id)
            .map(|n| n.title.clone())
            .unwrap_or_else(|| "<event>".into());
        // Pull the linked paragraphs from the event node.
        // Empty / single / many → three different paths.
        let entries = self.collect_link_entries(event_id);
        match entries.len() {
            0 => {
                self.status = format!(
                    "timeline · `{event_title}` has no linked paragraphs — Ctrl+V A on the event ¶ to link a scene"
                );
            }
            1 => {
                // Single hit — open it directly. Status
                // notes which event we routed through so
                // the user can audit later.
                let id = entries[0].id;
                let target_title = entries[0].title.clone();
                self.modal = Modal::None;
                if let Err(e) = self.open_paragraph_by_uuid(id) {
                    self.status =
                        format!("timeline · couldn't open `{target_title}`: {e}");
                } else if !self.status.starts_with("orphan event") {
                    self.status = format!(
                        "timeline · `{event_title}` → `{target_title}`"
                    );
                }
            }
            _ => {
                let count = entries.len();
                self.modal = Modal::LinkPicker {
                    owner: event_id,
                    entries,
                    cursor: 0,
                    scroll: 0,
                };
                self.status = format!(
                    "timeline · `{event_title}` links to {count} paragraph(s) · ↑↓ select · Enter opens · Esc closes"
                );
            }
        }
    }

    pub(super) fn timeline_new_event_prompt_handle_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Enter) {
            let taken = std::mem::replace(&mut self.modal, Modal::None);
            if let Modal::TimelineNewEventPrompt {
                input,
                book_id,
                cursor_ticks,
                track,
                return_to,
            } = taken
            {
                let title = input.as_str().trim().to_string();
                let mut underlying = *return_to;
                if title.is_empty() {
                    self.modal = underlying;
                    self.status = "new event: empty title — cancelled".into();
                    return;
                }
                // Create the event via the same path the CLI
                // uses. Errors surface in the status bar and
                // the timeline view re-opens with whatever
                // state survived.
                match self.create_event_at_cursor(book_id, &title, cursor_ticks, track.as_deref()) {
                    Ok(()) => {
                        // Refresh the timeline state's events.
                        if let Modal::TimelineView { state } = &mut underlying {
                            // Rebuild the snapshot in-place.
                            let book_id = state.book_id;
                            let project = state.project_overlay;
                            let scope_id = state.scope_id;
                            let all = self.collect_book_events(book_id, project);
                            let filtered: Vec<TimelineEvent> = if scope_id == book_id || project {
                                all
                            } else {
                                let subtree: std::collections::HashSet<Uuid> = self
                                    .hierarchy
                                    .collect_subtree(scope_id)
                                    .into_iter()
                                    .collect();
                                all.into_iter()
                                    .filter(|ev| {
                                        subtree.contains(&ev.id)
                                            || ev
                                                .linked_paragraphs
                                                .iter()
                                                .any(|p| subtree.contains(p))
                                    })
                                    .collect()
                            };
                            if let Modal::TimelineView { state } = &mut underlying {
                                state.events = filtered;
                                // Land the cursor on the new event.
                                state.cursor_ticks = cursor_ticks;
                            }
                        }
                        self.modal = underlying;
                        self.status = format!("event `{title}` added at cursor");
                    }
                    Err(e) => {
                        self.modal = underlying;
                        self.status = format!("new event: {e}");
                    }
                }
            }
            return;
        }
        if let Modal::TimelineNewEventPrompt { input, .. } = &mut self.modal {
            handle_text_input_key(input, key);
        }
    }

    /// 1.2.6+ Phase 3 — kick off the timeline health
    /// critique. `widen_to_book` ignores the current
    /// sub-scope and uses the whole book's event set;
    /// `widen_to_all_tracks` ignores `track_highlight`.
    pub(super) fn timeline_start_health_critique(
        &mut self,
        widen_to_book: bool,
        widen_to_all_tracks: bool,
    ) {
        let (book_id, project, scope_id, track_highlight, scope_events) =
            match &self.modal {
                Modal::TimelineView { state } => (
                    state.book_id,
                    state.project_overlay,
                    state.scope_id,
                    state.track_highlight.clone(),
                    state.events.clone(),
                ),
                _ => return,
            };
        // Build the event set for the critique. When
        // widen_to_book is true we sidestep the scope filter
        // and grab everything in the book (or project).
        let critique_events: Vec<TimelineEvent> = if widen_to_book {
            self.collect_book_events(book_id, project)
        } else {
            scope_events
        };
        if critique_events.is_empty() {
            self.status =
                "timeline critique: no events in this scope".into();
            return;
        }
        let track_filter: Option<String> = if widen_to_all_tracks {
            None
        } else {
            track_highlight.clone()
        };
        let crumb = if widen_to_book {
            self.hierarchy
                .get(book_id)
                .map(|n| n.title.clone())
                .unwrap_or_else(|| "(book)".into())
        } else {
            let snapshot = TimelineViewState {
                book_id,
                scope_id,
                nav_history: Vec::new(),
                events: Vec::new(),
                track_highlight: None,
                ticks_per_cell: 1.0,
                scroll_ticks: 0,
                cursor_ticks: 0,
                selected_event_id: None,
                collapsed_tracks: std::collections::HashSet::new(),
                expanded_track: None,
                focus_level: TimelineFocusLevel::Track,
                project_overlay: project,
                descent: None,
            };
            self.timeline_scope_crumb(&snapshot)
        };
        let calendar = crate::timeline::Calendar::from_config(
            self.cfg.timeline.calendar.clone(),
        );
        let payload_body = crate::timeline::critique::build_health_payload(
            &critique_events,
            &calendar,
            &self.hierarchy,
            &crumb,
            track_filter.as_deref(),
            &self.cfg.timeline.default_track,
        );
        let template = self.resolve_prompt_template("timeline-health", || {
            timeline_health_default_prompt().to_string()
        });
        let rendered = self.render_template(&template);
        let prompt_text = format!("{rendered}\n\n{payload_body}");

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("timeline critique: {e}");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            None,
            Vec::new(),
            prompt_text,
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.pending_chat_user_msg = None;
        // Close the modal so the AI pane is visible.
        self.modal = Modal::None;
        self.change_focus(Focus::Ai);
        let scope_label = if widen_to_book {
            "book"
        } else if widen_to_all_tracks {
            "scope · all tracks"
        } else {
            "scope · current track"
        };
        self.status = format!(
            "timeline critique ({scope_label}) · {n} events → {provider}…",
            n = critique_events.len(),
        );
    }

    pub(super) fn timeline_edit_event_prompt_handle_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Enter) {
            self.commit_edit_event_metadata();
            return;
        }
        if let Modal::TimelineEditEventPrompt { input, .. } = &mut self.modal {
            handle_text_input_key(input, key);
        }
    }

    pub(super) fn timeline_open_new_event_prompt(&mut self) {
        let Modal::TimelineView { state } = &self.modal else { return; };
        let cursor = state.cursor_ticks;
        let calendar = crate::timeline::Calendar::from_config(
            self.cfg.timeline.calendar.clone(),
        );
        let formatted = calendar.format(
            crate::timeline::TimelinePoint::from_ticks(cursor),
            crate::timeline::Precision::Day,
        );
        let track = state.track_highlight.clone();
        let book_id = state.book_id;
        // Stash the timeline state in a closure-callable
        // place via a NewEventPrompt sub-modal — return_to
        // pattern mirrors TagAddPrompt.
        let return_to = std::mem::replace(&mut self.modal, Modal::None);
        self.modal = Modal::TimelineNewEventPrompt {
            input: TextInput::new(),
            book_id,
            cursor_ticks: cursor,
            track,
            return_to: Box::new(return_to),
        };
        self.status = format!(
            "new event @ {formatted}: type title, Enter commits, Esc cancels"
        );
    }

    pub(super) fn timeline_descent_active(&self) -> bool {
        matches!(
            &self.modal,
            Modal::TimelineView { state } if state.descent.is_some()
        )
    }

    pub(super) fn timeline_up_scope(&mut self) {
        let (project, scope_id) = match &self.modal {
            Modal::TimelineView { state } => (state.project_overlay, state.scope_id),
            _ => return,
        };
        if project {
            self.status =
                "timeline · already at project scope (Ctrl+P to toggle off)".into();
            return;
        }
        let Some(parent_id) =
            self.hierarchy.get(scope_id).and_then(|n| n.parent_id)
        else {
            self.status =
                "timeline · at book root (Ctrl+P widens to project)".into();
            return;
        };
        // Walk up until we hit a Chapter / Subchapter / Book.
        let mut cur = parent_id;
        let mut target: Option<Uuid> = None;
        loop {
            let Some(n) = self.hierarchy.get(cur) else { break };
            if matches!(n.kind, NodeKind::Book | NodeKind::Chapter | NodeKind::Subchapter) {
                target = Some(cur);
                break;
            }
            match n.parent_id {
                Some(p) => cur = p,
                None => break,
            }
        }
        let Some(new_scope) = target else {
            self.status = "timeline · no parent scope to climb to".into();
            return;
        };
        if let Modal::TimelineView { state } = &mut self.modal {
            state.nav_history.push(state.scope_id);
            state.scope_id = new_scope;
        }
        self.timeline_refresh_after_scope_change();
        let crumb = match &self.modal {
            Modal::TimelineView { state } => self.timeline_scope_crumb(state),
            _ => String::new(),
        };
        self.status = format!("timeline · up-scope · {crumb}");
    }

    pub(super) fn timeline_open_descent(&mut self) {
        // Extract everything we need from the modal first to
        // avoid holding a &mut self.modal while we touch
        // self.hierarchy.
        let (project, scope_id, book_id, events_total, event_links): (
            bool,
            Uuid,
            Uuid,
            usize,
            std::collections::HashSet<Uuid>,
        ) = match &self.modal {
            Modal::TimelineView { state } => (
                state.project_overlay,
                state.scope_id,
                state.book_id,
                state.events.len(),
                state
                    .events
                    .iter()
                    .flat_map(|e| e.linked_paragraphs.iter().copied())
                    .collect(),
            ),
            _ => return,
        };
        if project {
            self.status =
                "timeline · descent disabled in project overlay (Ctrl+P off to drill in)"
                    .into();
            return;
        }
        let children = self.hierarchy.children_of(Some(scope_id));
        let mut choices: Vec<TimelineDescentChoice> = children
            .into_iter()
            .filter(|n| matches!(n.kind, NodeKind::Chapter | NodeKind::Subchapter))
            .map(|n| {
                let descendants = self.hierarchy.collect_subtree(n.id);
                let mut count = 0usize;
                for d in &descendants {
                    if event_links.contains(d) {
                        count += 1;
                    }
                    if let Some(node) = self.hierarchy.get(*d) {
                        if node.event.is_some() {
                            count += 1;
                        }
                    }
                }
                TimelineDescentChoice {
                    id: n.id,
                    title: n.title.clone(),
                    event_count: count,
                }
            })
            .collect();
        if let Some(timeline_chapter) = self.hierarchy.iter().find(|n| {
            n.parent_id == Some(book_id)
                && n.system_tag.as_deref()
                    == Some(crate::store::SYSTEM_TAG_BOOK_TIMELINE)
        }) {
            if scope_id == book_id
                && !choices.iter().any(|c| c.id == timeline_chapter.id)
            {
                choices.push(TimelineDescentChoice {
                    id: timeline_chapter.id,
                    title: format!("{} (system)", timeline_chapter.title),
                    event_count: events_total,
                });
            }
        }
        if choices.is_empty() {
            self.status = "timeline · no sub-scopes here".into();
            return;
        }
        if let Modal::TimelineView { state } = &mut self.modal {
            state.descent = Some(TimelineDescentState { choices, cursor: 0 });
        }
        self.status =
            "timeline · descend into … · ↑↓ select · Enter · Esc cancel".into();
    }

    pub(super) fn timeline_descent_handle_key(&mut self, key: KeyEvent) {
        let chosen: Option<TimelineDescentChoice> = match key.code {
            KeyCode::Up => {
                if let Modal::TimelineView { state } = &mut self.modal {
                    if let Some(d) = state.descent.as_mut() {
                        if d.cursor > 0 {
                            d.cursor -= 1;
                        }
                    }
                }
                return;
            }
            KeyCode::Down => {
                if let Modal::TimelineView { state } = &mut self.modal {
                    if let Some(d) = state.descent.as_mut() {
                        if d.cursor + 1 < d.choices.len() {
                            d.cursor += 1;
                        }
                    }
                }
                return;
            }
            KeyCode::Home => {
                if let Modal::TimelineView { state } = &mut self.modal {
                    if let Some(d) = state.descent.as_mut() {
                        d.cursor = 0;
                    }
                }
                return;
            }
            KeyCode::End => {
                if let Modal::TimelineView { state } = &mut self.modal {
                    if let Some(d) = state.descent.as_mut() {
                        d.cursor = d.choices.len().saturating_sub(1);
                    }
                }
                return;
            }
            KeyCode::Esc => {
                if let Modal::TimelineView { state } = &mut self.modal {
                    state.descent = None;
                }
                self.status = "timeline · descent cancelled".into();
                return;
            }
            KeyCode::Enter => {
                if let Modal::TimelineView { state } = &mut self.modal {
                    let pick = state
                        .descent
                        .as_ref()
                        .and_then(|d| d.choices.get(d.cursor).cloned());
                    state.descent = None;
                    pick
                } else {
                    None
                }
            }
            _ => return,
        };
        let Some(choice) = chosen else { return };
        if let Modal::TimelineView { state } = &mut self.modal {
            state.nav_history.push(state.scope_id);
            state.scope_id = choice.id;
        }
        self.timeline_refresh_after_scope_change();
        let crumb = match &self.modal {
            Modal::TimelineView { state } => self.timeline_scope_crumb(state),
            _ => String::new(),
        };
        self.status = format!(
            "timeline · descended into `{}` · {crumb}",
            choice.title
        );
    }

    pub(super) fn timeline_jump_book_scope(&mut self) {
        let (scope_eq_book, project) = match &self.modal {
            Modal::TimelineView { state } => {
                (state.scope_id == state.book_id, state.project_overlay)
            }
            _ => return,
        };
        if scope_eq_book && !project {
            self.status = "timeline · already at book scope".into();
            return;
        }
        if let Modal::TimelineView { state } = &mut self.modal {
            state.nav_history.push(state.scope_id);
            state.scope_id = state.book_id;
            state.project_overlay = false;
        }
        self.timeline_refresh_after_scope_change();
        let crumb = match &self.modal {
            Modal::TimelineView { state } => self.timeline_scope_crumb(state),
            _ => String::new(),
        };
        self.status = format!("timeline · book scope · {crumb}");
    }

    pub(super) fn timeline_toggle_project(&mut self) {
        let user_book_count = self
            .hierarchy
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .count();
        if user_book_count < 2 {
            self.status =
                "timeline · only one user book; project overlay needs ≥2".into();
            return;
        }
        let new_overlay = match &self.modal {
            Modal::TimelineView { state } => !state.project_overlay,
            _ => return,
        };
        if let Modal::TimelineView { state } = &mut self.modal {
            if new_overlay {
                state.nav_history.push(state.scope_id);
            }
            state.project_overlay = new_overlay;
        }
        self.timeline_refresh_after_scope_change();
        self.status = if new_overlay {
            "timeline · project overlay ON · tracks prefixed with book slug · Ctrl+P toggles".into()
        } else {
            "timeline · project overlay OFF · book scope".into()
        };
    }

    /// Rebuild the event snapshot after any scope or project-
    /// overlay change. Keeps cursor / scroll positions
    /// reasonable.
    pub(super) fn timeline_refresh_after_scope_change(&mut self) {
        let (book_id, scope_id, project) = match &self.modal {
            Modal::TimelineView { state } => {
                (state.book_id, state.scope_id, state.project_overlay)
            }
            _ => return,
        };
        let all = self.collect_book_events(book_id, project);
        let filtered: Vec<TimelineEvent> = if scope_id == book_id || project {
            all
        } else {
            let subtree: std::collections::HashSet<Uuid> = self
                .hierarchy
                .collect_subtree(scope_id)
                .into_iter()
                .collect();
            all.into_iter()
                .filter(|ev| {
                    if subtree.contains(&ev.id) {
                        return true;
                    }
                    ev.linked_paragraphs.iter().any(|p| subtree.contains(p))
                })
                .collect()
        };
        if let Modal::TimelineView { state } = &mut self.modal {
            state.events = filtered;
            if let Some(first) = state.events.first() {
                if !state.events.iter().any(|e| e.start_ticks == state.cursor_ticks) {
                    state.cursor_ticks = first.start_ticks;
                    state.scroll_ticks = first.start_ticks.saturating_sub(20);
                }
            }
        }
    }

    pub(super) fn timeline_scroll(&mut self, dir: i64, page: bool) {
        let Modal::TimelineView { state } = &mut self.modal else {
            return;
        };
        // Determine inner pane width to scale page steps.
        // We don't know the modal width here; approximate
        // with a sensible page = 60 cells, step = 10 cells.
        let cells = if page { 60.0 } else { 10.0 };
        let delta_ticks = (cells * state.ticks_per_cell * dir as f64).round() as i64;
        state.scroll_ticks = state.scroll_ticks.saturating_add(delta_ticks);
        state.cursor_ticks = state.cursor_ticks.saturating_add(delta_ticks);
    }

    /// 1.2.6+ — Up/Down arrows: hop the timeline cursor to the
    /// previous / next event in chronological order, and pan
    /// the viewport just enough to keep the new cursor on
    /// screen. Direction: -1 = previous, +1 = next.
    ///
    /// 1.2.7+ — also stamps `selected_event_id` so the
    /// render highlights the entire event span (start → end),
    /// and pans so both endpoints land inside the visible
    /// viewport (zooms out if the event is wider than the
    /// available space).
    pub(super) fn timeline_step_cursor(&mut self, direction: i64) {
        let Modal::TimelineView { state } = &mut self.modal else { return; };
        let Some((event_id, target)) = timeline_step_event_cursor(
            &state.events,
            state.cursor_ticks,
            direction,
        ) else {
            self.status = if direction > 0 {
                "timeline · already at the last event".into()
            } else {
                "timeline · already at the first event".into()
            };
            return;
        };
        // Stamp the selection so the render can highlight the
        // whole event span and the link-picker (Enter) knows
        // which event to query.
        state.selected_event_id = Some(event_id);
        state.cursor_ticks = target;
        // Auto-pan so the WHOLE selected event sits in the
        // viewport — start + end both visible. Falls back to
        // cursor-centred behaviour when the event has no end.
        let term_w = crossterm::terminal::size()
            .map(|(w, _)| w as usize)
            .unwrap_or(80);
        let content_w = term_w.saturating_sub(16).max(40) as f64;
        let visible_ticks = (content_w * state.ticks_per_cell) as i64;
        let span_end = state
            .events
            .iter()
            .find(|e| e.id == event_id)
            .and_then(|e| e.end_ticks)
            .unwrap_or(target);
        let span_width = (span_end - target).abs();
        if span_width >= visible_ticks {
            // Event spans more than one screen — centre it.
            state.scroll_ticks = target
                .saturating_sub((visible_ticks - span_width) / 2);
        } else {
            // Pan with a 15% margin on each edge so the event
            // doesn't kiss the border.
            let margin = (visible_ticks / 7).max(2);
            let left = state.scroll_ticks;
            let right = state.scroll_ticks + visible_ticks;
            if target < left + margin {
                state.scroll_ticks = target.saturating_sub(margin);
            } else if span_end > right - margin {
                state.scroll_ticks = span_end
                    .saturating_sub(visible_ticks - margin);
            }
        }
        self.status = format!(
            "timeline · cursor → tick {target} · Enter opens linked paragraphs"
        );
    }

    pub(super) fn timeline_zoom(&mut self, factor: f64) {
        let Modal::TimelineView { state } = &mut self.modal else {
            return;
        };
        let new = (state.ticks_per_cell * factor).clamp(0.05, 1000.0);
        if (new - state.ticks_per_cell).abs() < f64::EPSILON {
            return;
        }
        // Keep the cursor's screen column stable through the
        // zoom — recompute scroll_ticks so cursor_ticks lands
        // at the same column count.
        let approx_col = ((state.cursor_ticks - state.scroll_ticks) as f64
            / state.ticks_per_cell)
            .round();
        let new_scroll =
            state.cursor_ticks - (approx_col * new).round() as i64;
        state.ticks_per_cell = new;
        state.scroll_ticks = new_scroll;
        self.status = format!(
            "timeline view · zoom {z:.2}× ({ticks_per_cell:.3} ticks/cell)",
            z = 1.0 / new,
            ticks_per_cell = new,
        );
    }

    pub(super) fn timeline_reset_zoom(&mut self) {
        let Modal::TimelineView { state } = &mut self.modal else {
            return;
        };
        state.ticks_per_cell = 1.0;
        state.scroll_ticks = state.cursor_ticks.saturating_sub(20);
        self.status = "timeline view · zoom 1.00× (reset)".into();
    }

    pub(super) fn timeline_jump_home(&mut self) {
        let Modal::TimelineView { state } = &mut self.modal else {
            return;
        };
        if let Some(first) = state.events.first() {
            state.cursor_ticks = first.start_ticks;
            state.scroll_ticks = first.start_ticks.saturating_sub(10);
        }
    }

    pub(super) fn timeline_jump_end(&mut self) {
        let Modal::TimelineView { state } = &mut self.modal else {
            return;
        };
        if let Some(last) = state.events.last() {
            state.cursor_ticks = last.start_ticks;
            state.scroll_ticks = last.start_ticks.saturating_sub(30);
        }
    }

    /// Human-readable breadcrumb for the scope crumb shown in
    /// the modal header + status bar.
    pub(super) fn timeline_scope_crumb(&self, state: &TimelineViewState) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut cur_id = state.scope_id;
        loop {
            let Some(node) = self.hierarchy.get(cur_id) else {
                break;
            };
            parts.push(node.title.clone());
            match node.parent_id {
                Some(p) => cur_id = p,
                None => break,
            }
        }
        parts.reverse();
        if parts.is_empty() {
            "(scope?)".into()
        } else {
            parts.join(" ▸ ")
        }
    }

}
