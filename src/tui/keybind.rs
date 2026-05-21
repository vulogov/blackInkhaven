//! Chord-action binding table.
//!
//! Stage 1 of the rebindable-keys roadmap: extract every meta- and
//! bund-sub-chord from the hardcoded `match` arms in `app.rs` into
//! a data-driven `KeyBindings` struct. App dispatch becomes a
//! single table lookup followed by a `run_action` switch.
//!
//! ## What's here (Stage 1)
//!
//! * `Action` — one variant per reachable handler. Names are
//!   `snake_case` so they serialise to dotted strings in HJSON
//!   (`tree.add_chapter`, `bund.run_buffer`, …).
//! * `Scope` — pane filter on each binding entry.
//! * `BindingEntry` — `(chord, action, scope)` triple.
//! * `KeyBindings::defaults()` — produces the canonical table
//!   matching today's hardcoded chord layout exactly.
//! * `KeyBindings::resolve_*` — table lookups consulted by
//!   `handle_meta_action` / `handle_bund_action`.
//!
//! ## What's not here yet (Stage 2)
//!
//! * `ink.key.*` Bund stdlib for runtime rebinding.
//! * Auto-generated status-bar hint strings.
//! * Migration of F-keys (F1/F3/F4/F5/F6/F7) into the table.

use crossterm::event::KeyEvent;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};

use super::focus::Focus;
use super::keymap::KeyChord;

/// Which pane(s) a binding applies in. The first binding whose
/// scope matches the current focus wins, so narrow-scoped entries
/// (`Editor`) MUST come before broad ones (`Any`) in
/// `KeyBindings::defaults()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Active in any pane.
    Any,
    /// Editor pane only.
    Editor,
    /// Tree pane + the search bar (which lives above the tree).
    Tree,
    /// AI pane + the AI prompt input line.
    Ai,
}

impl Scope {
    pub fn matches(self, focus: Focus) -> bool {
        match self {
            Scope::Any => true,
            Scope::Editor => focus == Focus::Editor,
            Scope::Tree => matches!(focus, Focus::Tree | Focus::SearchBar),
            Scope::Ai => matches!(focus, Focus::Ai | Focus::AiPrompt),
        }
    }
}

/// Every user-reachable chord-action. New chord features add a
/// variant here + an arm in `App::run_action`. Variant names
/// serialise (via serde) to the canonical dotted form used in
/// HJSON `keys.bindings`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // ── Tree pane ─────────────────────────────────────────────
    #[serde(rename = "tree.add_book")]
    AddBook,
    #[serde(rename = "tree.add_chapter")]
    AddChapter,
    #[serde(rename = "tree.add_subchapter")]
    AddSubchapter,
    #[serde(rename = "tree.add_paragraph")]
    AddParagraph,
    #[serde(rename = "tree.delete_node")]
    DeleteNode,
    #[serde(rename = "tree.morph_type")]
    MorphType,
    #[serde(rename = "tree.reorder_up")]
    ReorderUp,
    #[serde(rename = "tree.reorder_down")]
    ReorderDown,

    // ── Editor pane ───────────────────────────────────────────
    #[serde(rename = "editor.save")]
    Save,
    #[serde(rename = "editor.create_snapshot")]
    CreateSnapshot,
    #[serde(rename = "editor.cycle_status")]
    CycleStatus,
    #[serde(rename = "editor.open_function_picker")]
    OpenFunctionPicker,
    #[serde(rename = "editor.rename_to_first_sentence")]
    RenameToFirstSentence,
    /// `P` in the editor — context-sensitive: image-picker when
    /// the cursor sits inside `#image(...)`, otherwise Places
    /// lexicon lookup.
    #[serde(rename = "editor.lookup_places_or_image")]
    LookupPlacesOrImage,
    #[serde(rename = "editor.lookup_characters")]
    LookupCharacters,
    #[serde(rename = "editor.lookup_notes")]
    LookupNotes,
    #[serde(rename = "editor.lookup_artefacts")]
    LookupArtefacts,
    #[serde(rename = "editor.open_quickref")]
    OpenQuickref,

    // ── Global meta ───────────────────────────────────────────
    #[serde(rename = "global.open_credits")]
    OpenCredits,
    #[serde(rename = "global.open_book_info")]
    OpenBookInfo,
    #[serde(rename = "global.open_llm_picker")]
    OpenLlmPicker,
    #[serde(rename = "global.toggle_sound")]
    ToggleSound,
    #[serde(rename = "global.schedule_assemble")]
    ScheduleAssemble,
    #[serde(rename = "global.schedule_build")]
    ScheduleBuild,
    #[serde(rename = "global.schedule_take")]
    ScheduleTake,
    #[serde(rename = "global.toggle_typewriter")]
    ToggleTypewriter,
    #[serde(rename = "global.toggle_ai_fullscreen")]
    ToggleAiFullscreen,
    #[serde(rename = "global.status_filter_ready")]
    StatusFilterReady,
    #[serde(rename = "global.status_filter_final")]
    StatusFilterFinal,
    #[serde(rename = "global.status_filter_third")]
    StatusFilterThird,
    #[serde(rename = "global.status_filter_second")]
    StatusFilterSecond,
    #[serde(rename = "global.status_filter_first")]
    StatusFilterFirst,
    #[serde(rename = "global.status_filter_napkin")]
    StatusFilterNapkin,
    #[serde(rename = "global.status_filter_none")]
    StatusFilterNone,

    // ── AI pane ───────────────────────────────────────────────
    #[serde(rename = "ai.clear_chat")]
    ClearChat,

    // ── Bund prefix ───────────────────────────────────────────
    #[serde(rename = "bund.run_buffer")]
    BundRunBuffer,
    #[serde(rename = "bund.new_script")]
    BundNewScript,
    #[serde(rename = "bund.open_eval_modal")]
    BundOpenEvalModal,

    /// Explicit "this chord does nothing" — overlay entries can
    /// set `action: "none"` to disable a default chord.
    #[serde(rename = "none")]
    None,

    /// Runtime-only: a Bund lambda registered under the given
    /// name via `ink.key.bind_lambda`. Dispatch routes to
    /// `scripting::hooks::fire(name, vec![])`. `#[serde(skip)]` —
    /// these can't appear in HJSON; they live only in memory and
    /// vanish on process exit.
    #[serde(skip)]
    BundLambda(Arc<str>),
}

impl Action {
    /// Short label used in the auto-generated status-bar meta
    /// hint ("add chapter", "morph-type", …). Returns `""` for
    /// `None` and the lambda name for `BundLambda`.
    pub fn label(&self) -> String {
        match self {
            Action::AddBook => "add book".into(),
            Action::AddChapter => "add chapter".into(),
            Action::AddSubchapter => "add subchapter".into(),
            Action::AddParagraph => "add paragraph".into(),
            Action::DeleteNode => "delete".into(),
            Action::MorphType => "morph-type".into(),
            Action::ReorderUp => "↑ reorder".into(),
            Action::ReorderDown => "↓ reorder".into(),

            Action::Save => "save".into(),
            Action::CreateSnapshot => "snapshot".into(),
            Action::CycleStatus => "status".into(),
            Action::OpenFunctionPicker => "func".into(),
            Action::RenameToFirstSentence => "retitle".into(),
            Action::LookupPlacesOrImage => "place/pic".into(),
            Action::LookupCharacters => "character".into(),
            Action::LookupNotes => "notes".into(),
            Action::LookupArtefacts => "artefacts".into(),
            Action::OpenQuickref => "help".into(),

            Action::OpenCredits => "credits".into(),
            Action::OpenBookInfo => "info".into(),
            Action::OpenLlmPicker => "LLM".into(),
            Action::ToggleSound => "sound".into(),
            Action::ScheduleAssemble => "assemble".into(),
            Action::ScheduleBuild => "build".into(),
            Action::ScheduleTake => "take".into(),
            Action::ToggleTypewriter => "typewriter".into(),
            Action::ToggleAiFullscreen => "AI-full".into(),
            Action::StatusFilterReady => "Ready".into(),
            Action::StatusFilterFinal => "Final".into(),
            Action::StatusFilterThird => "Third".into(),
            Action::StatusFilterSecond => "Second".into(),
            Action::StatusFilterFirst => "First".into(),
            Action::StatusFilterNapkin => "Napkin".into(),
            Action::StatusFilterNone => "None".into(),

            Action::ClearChat => "clear chat".into(),

            Action::BundRunBuffer => "run buffer".into(),
            Action::BundNewScript => "new script".into(),
            Action::BundOpenEvalModal => "eval".into(),

            Action::None => String::new(),
            Action::BundLambda(name) => format!("λ {name}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BindingEntry {
    pub chord: KeyChord,
    pub action: Action,
    pub scope: Scope,
}

/// Live binding table. Held in the process-wide `ACTIVE` slot
/// and consulted on every meta- / bund-sub-chord dispatch.
/// `ink.key.*` stdlib words mutate the same struct under the
/// shared RwLock.
#[derive(Debug, Clone)]
pub struct KeyBindings {
    /// Prefix chord that gates the meta sub-chord table (default
    /// `Ctrl+B`). Stored here so `ink.key.*` stdlib words can
    /// parse `"Ctrl+b m"` shorthand without taking a separate
    /// dependency on the App.
    pub meta_prefix: KeyChord,
    /// Same for the Bund sub-chord table (default `Ctrl+Z`).
    /// `None` when the user disabled it via empty config.
    pub bund_prefix: Option<KeyChord>,
    pub meta_sub: Vec<BindingEntry>,
    pub bund_sub: Vec<BindingEntry>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self::defaults()
    }
}

impl KeyBindings {
    /// The canonical chord layout — must reproduce the behaviour
    /// of the hardcoded match arms `app.rs` had before Stage 1.
    /// Narrow-scoped entries come BEFORE broad ones (`Any`) so
    /// pane-specific bindings beat global ones when both match.
    pub fn defaults() -> Self {
        Self {
            meta_prefix: KeyChord::parse("Ctrl+b").expect("default meta_prefix"),
            bund_prefix: Some(KeyChord::parse("Ctrl+z").expect("default bund_prefix")),
            meta_sub: vec![
                // ── Tree pane ─────────────────────────────────
                entry("c", Action::AddChapter, Scope::Tree),
                entry("s", Action::AddSubchapter, Scope::Tree),
                entry("p", Action::AddParagraph, Scope::Tree),
                entry("d", Action::DeleteNode, Scope::Tree),
                entry("m", Action::MorphType, Scope::Tree),
                entry("Up", Action::ReorderUp, Scope::Tree),
                entry("Down", Action::ReorderDown, Scope::Tree),
                // Reorder aliases used in the old keymap.
                entry("u", Action::ReorderUp, Scope::Tree),
                entry("j", Action::ReorderDown, Scope::Tree),

                // ── Editor pane ───────────────────────────────
                entry("s", Action::Save, Scope::Editor),
                entry("n", Action::CreateSnapshot, Scope::Editor),
                entry("r", Action::CycleStatus, Scope::Editor),
                entry("f", Action::OpenFunctionPicker, Scope::Editor),
                entry("t", Action::RenameToFirstSentence, Scope::Editor),
                entry("m", Action::MorphType, Scope::Editor),
                entry("p", Action::LookupPlacesOrImage, Scope::Editor),
                entry("c", Action::LookupCharacters, Scope::Editor),
                entry("g", Action::LookupNotes, Scope::Editor),
                entry("y", Action::LookupArtefacts, Scope::Editor),

                // ── AI pane ───────────────────────────────────
                entry("c", Action::ClearChat, Scope::Ai),

                // ── Global (Any) ──────────────────────────────
                // H is pane-aware-content but pane-agnostic-binding —
                // every pane gets a "quickref" overlay tailored to
                // the focused area.
                entry("h", Action::OpenQuickref, Scope::Any),
                entry("v", Action::OpenCredits, Scope::Any),
                entry("i", Action::OpenBookInfo, Scope::Any),
                entry("l", Action::OpenLlmPicker, Scope::Any),
                entry("e", Action::ToggleSound, Scope::Any),
                entry("a", Action::ScheduleAssemble, Scope::Any),
                entry("b", Action::ScheduleBuild, Scope::Any),
                entry("o", Action::ScheduleTake, Scope::Any),
                entry("w", Action::ToggleTypewriter, Scope::Any),
                entry("k", Action::ToggleAiFullscreen, Scope::Any),
                entry("1", Action::StatusFilterReady, Scope::Any),
                entry("2", Action::StatusFilterFinal, Scope::Any),
                entry("3", Action::StatusFilterThird, Scope::Any),
                entry("4", Action::StatusFilterSecond, Scope::Any),
                entry("5", Action::StatusFilterFirst, Scope::Any),
                entry("6", Action::StatusFilterNapkin, Scope::Any),
                entry("7", Action::StatusFilterNone, Scope::Any),
            ],
            bund_sub: vec![
                entry("r", Action::BundRunBuffer, Scope::Any),
                entry("n", Action::BundNewScript, Scope::Any),
                entry("e", Action::BundOpenEvalModal, Scope::Any),
            ],
        }
    }

    /// Resolve a meta sub-chord against the current focus. Returns
    /// `None` when no binding matches, `Some(Action::None)` when a
    /// binding was explicitly disabled by the user overlay.
    pub fn resolve_meta_sub(&self, ev: &KeyEvent, focus: Focus) -> Option<Action> {
        resolve_in(&self.meta_sub, ev, focus)
    }

    /// Same as `resolve_meta_sub` for chords after the bund_prefix.
    pub fn resolve_bund_sub(&self, ev: &KeyEvent, focus: Focus) -> Option<Action> {
        resolve_in(&self.bund_sub, ev, focus)
    }

    /// Apply a list of `(layer, entry)` overlay pairs on top of
    /// the existing table. Each new entry replaces any existing
    /// `(chord, scope)` match in the same layer and gets
    /// prepended so it wins resolution against the defaults.
    pub fn apply_overlay(&mut self, overlay: Vec<(Layer, BindingEntry)>) {
        for (layer, new) in overlay {
            let table = match layer {
                Layer::MetaSub => &mut self.meta_sub,
                Layer::BundSub => &mut self.bund_sub,
            };
            table.retain(|b| !(b.chord == new.chord && b.scope == new.scope));
            table.insert(0, new);
        }
    }

    /// Build a `KeyBindings` from `defaults()` overlaid with the
    /// parsed HJSON `keys.bindings` entries. Caller supplies the
    /// already-parsed meta + bund prefixes so the overlay parser
    /// can route `"Ctrl+b m"` → meta_sub table by prefix match.
    pub fn from_overrides(
        meta_prefix: KeyChord,
        bund_prefix: Option<KeyChord>,
        overrides: &[(String, String, Option<String>)],
    ) -> Result<Self, String> {
        let mut bindings = Self::defaults();
        bindings.meta_prefix = meta_prefix;
        bindings.bund_prefix = bund_prefix;
        let mut overlay: Vec<(Layer, BindingEntry)> = Vec::new();
        for (chord_str, action_str, scope_str) in overrides {
            let entry = parse_overlay(
                meta_prefix,
                bund_prefix.unwrap_or_else(disabled_chord_placeholder),
                chord_str,
                action_str,
                scope_str,
            )?;
            overlay.push(entry);
        }
        bindings.apply_overlay(overlay);
        Ok(bindings)
    }

    /// Add or replace a single binding. Used by `ink.key.bind` /
    /// `ink.key.bind_lambda`. Same `(chord, scope)` uniqueness
    /// semantics as the HJSON overlay: a new entry shadows any
    /// existing one with matching key.
    pub fn add(&mut self, layer: Layer, entry: BindingEntry) {
        let table = match layer {
            Layer::MetaSub => &mut self.meta_sub,
            Layer::BundSub => &mut self.bund_sub,
        };
        table.retain(|b| !(b.chord == entry.chord && b.scope == entry.scope));
        table.insert(0, entry);
    }

    /// Remove every entry whose `(chord, scope)` matches. Returns
    /// the number of entries removed (zero when nothing matched).
    pub fn remove(&mut self, layer: Layer, chord: &KeyChord, scope: Scope) -> usize {
        let table = match layer {
            Layer::MetaSub => &mut self.meta_sub,
            Layer::BundSub => &mut self.bund_sub,
        };
        let before = table.len();
        table.retain(|b| !(b.chord == *chord && b.scope == scope));
        before - table.len()
    }

    /// Parse a `"<prefix> <suffix>"` shorthand and return
    /// `(layer, suffix_chord)`. Used by `ink.key.*` stdlib words
    /// AND the HJSON overlay parser via `parse_overlay`.
    pub fn parse_sub_chord(&self, s: &str) -> Result<(Layer, KeyChord), String> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        let (prefix_str, suffix_str) = match parts.as_slice() {
            [single] => {
                return Err(format!(
                    "chord `{single}`: top-level (no-prefix) binding not yet supported \
                     — use `<meta_prefix> <key>` or `<bund_prefix> <key>`"
                ));
            }
            [prefix, suffix] => (*prefix, *suffix),
            _ => return Err(format!("chord `{s}`: expected `<prefix> <suffix>`")),
        };
        let prefix = KeyChord::parse(prefix_str)
            .map_err(|e| format!("chord `{s}` prefix: {e}"))?;
        let suffix = KeyChord::parse(suffix_str)
            .map_err(|e| format!("chord `{s}` suffix: {e}"))?;
        let layer = if prefix == self.meta_prefix {
            Layer::MetaSub
        } else if Some(prefix) == self.bund_prefix {
            Layer::BundSub
        } else {
            return Err(format!(
                "chord `{s}`: prefix `{prefix_str}` is not meta_prefix or bund_prefix"
            ));
        };
        if suffix == self.meta_prefix || Some(suffix) == self.bund_prefix {
            return Err(format!(
                "chord `{s}`: suffix collides with a prefix chord"
            ));
        }
        Ok((layer, suffix))
    }
}

impl KeyBindings {
    /// Build the status-bar hint string for the meta-prefix
    /// chord on the given focus. Iterates `meta_sub` in
    /// registration order, skipping disabled entries and
    /// deduplicating actions (so `Up` + `u` for ReorderUp
    /// surface as one entry).
    pub fn meta_hint(&self, focus: Focus) -> String {
        self.hint_for(&self.meta_sub, "META", focus)
    }

    /// Same for the bund-prefix chord.
    pub fn bund_hint(&self, focus: Focus) -> String {
        self.hint_for(&self.bund_sub, "BUND", focus)
    }

    fn hint_for(&self, table: &[BindingEntry], prefix: &str, focus: Focus) -> String {
        use std::collections::HashSet;
        let mut parts: Vec<String> = vec![prefix.to_string()];
        let mut seen: HashSet<String> = HashSet::new();
        for entry in table {
            if !entry.scope.matches(focus) {
                continue;
            }
            if matches!(entry.action, Action::None) {
                continue;
            }
            let label = entry.action.label();
            if label.is_empty() {
                continue;
            }
            // De-dupe by action label: a user who bound the same
            // action to two chords (e.g. ReorderUp on Up and u)
            // only sees the action once in the hint.
            if !seen.insert(label.clone()) {
                continue;
            }
            parts.push(format!("{} {}", entry.chord.to_display_string(), label));
        }
        parts.push("Esc cancel".into());
        parts.join(" · ")
    }
}

/// Placeholder chord matched by nothing real — used to satisfy
/// `parse_overlay`'s `bund_prefix` arg when the user disabled the
/// bund prefix via empty config.
fn disabled_chord_placeholder() -> KeyChord {
    KeyChord {
        code: crossterm::event::KeyCode::Null,
        modifiers: crossterm::event::KeyModifiers::NONE,
    }
}

/// Which sub-chord table the overlay entry targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    MetaSub,
    BundSub,
}

fn parse_overlay(
    meta_prefix: KeyChord,
    bund_prefix: KeyChord,
    chord: &str,
    action: &str,
    scope: &Option<String>,
) -> Result<(Layer, BindingEntry), String> {
    // Shorthand split: "Ctrl+b y" → ["Ctrl+b", "y"]. Trim runs of
    // whitespace so "Ctrl+b   y" also parses cleanly.
    let parts: Vec<&str> = chord.split_whitespace().collect();
    let (prefix_str, suffix_str) = match parts.as_slice() {
        [single] => {
            return Err(format!(
                "binding chord `{single}`: top-level (no-prefix) rebinding isn't supported \
                 in Stage 1 — use `<meta_prefix> <key>` or `<bund_prefix> <key>`"
            ));
        }
        [prefix, suffix] => (*prefix, *suffix),
        _ => {
            return Err(format!(
                "binding chord `{chord}`: expected `<prefix> <suffix>` (two tokens)"
            ));
        }
    };
    let prefix = KeyChord::parse(prefix_str)
        .map_err(|e| format!("binding chord `{chord}` prefix: {e}"))?;
    let suffix = KeyChord::parse(suffix_str)
        .map_err(|e| format!("binding chord `{chord}` suffix: {e}"))?;
    let layer = if prefix == meta_prefix {
        Layer::MetaSub
    } else if prefix == bund_prefix {
        Layer::BundSub
    } else {
        return Err(format!(
            "binding chord `{chord}`: prefix `{prefix_str}` is not meta_prefix or bund_prefix"
        ));
    };
    // Reject rebinding the prefixes themselves and the hard-quit
    // chord — those are configured via top-level `keys.*` slots,
    // not the bindings overlay.
    if suffix == meta_prefix || suffix == bund_prefix {
        return Err(format!(
            "binding chord `{chord}`: suffix collides with a prefix chord"
        ));
    }
    let scope = parse_scope(scope.as_deref())?;
    let action = parse_action(action)?;
    Ok((
        layer,
        BindingEntry {
            chord: suffix,
            action,
            scope,
        },
    ))
}

fn parse_scope(s: Option<&str>) -> Result<Scope, String> {
    match s {
        None | Some("any") => Ok(Scope::Any),
        Some("editor") => Ok(Scope::Editor),
        Some("tree") => Ok(Scope::Tree),
        Some("ai") => Ok(Scope::Ai),
        Some(other) => Err(format!(
            "scope `{other}`: expected one of any / editor / tree / ai"
        )),
    }
}

fn parse_action(s: &str) -> Result<Action, String> {
    // Round-trip via serde: variant rename attributes give us the
    // canonical dotted form. `serde_json::from_str` reads a JSON
    // string literal and matches it against the rename map.
    serde_json::from_str::<Action>(&format!("\"{s}\""))
        .map_err(|e| format!("action `{s}`: {e}"))
}

fn resolve_in(table: &[BindingEntry], ev: &KeyEvent, focus: Focus) -> Option<Action> {
    table
        .iter()
        .find(|b| b.scope.matches(focus) && b.chord.matches(ev))
        .map(|b| b.action.clone())
}

fn entry(chord: &str, action: Action, scope: Scope) -> BindingEntry {
    BindingEntry {
        chord: KeyChord::parse(chord).expect("invalid default chord — programmer error"),
        action,
        scope,
    }
}

// ── Shared active KeyBindings ────────────────────────────────────────
//
// App reads from this on every chord dispatch; `ink.key.*` Bund
// stdlib writes to it. Lazily initialised with `KeyBindings::defaults()`
// on first access — so CLI subcommands (`inkhaven bund`) that don't
// build an `App` still see a functioning binding table.
//
// `install` replaces the contents under the write lock, so TUI
// startup (which parses the HJSON overlay) wins over the lazy
// defaults whenever it runs.

static ACTIVE: LazyLock<RwLock<KeyBindings>> =
    LazyLock::new(|| RwLock::new(KeyBindings::defaults()));

/// Replace the active KeyBindings. Called by `App::new` after
/// applying the HJSON overlay. Cheap because the new value is
/// move-swapped under the write lock.
pub fn install(bindings: KeyBindings) {
    *ACTIVE.write() = bindings;
}

/// Read access. Lazy default-init means this never blocks on
/// missing installation — CLI smoke usage gets defaults.
pub fn read() -> RwLockReadGuard<'static, KeyBindings> {
    ACTIVE.read()
}

/// Write access for `ink.key.*` Bund stdlib words.
pub fn write() -> RwLockWriteGuard<'static, KeyBindings> {
    ACTIVE.write()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn ev(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn defaults_resolve_known_chords() {
        let k = KeyBindings::defaults();
        // Tree pane: C → add chapter
        assert_eq!(
            k.resolve_meta_sub(&ev('c'), Focus::Tree),
            Some(Action::AddChapter)
        );
        // Editor pane: C → character lookup (different action,
        // same key — scope discriminates).
        assert_eq!(
            k.resolve_meta_sub(&ev('c'), Focus::Editor),
            Some(Action::LookupCharacters)
        );
        // AI pane: C → clear chat
        assert_eq!(
            k.resolve_meta_sub(&ev('c'), Focus::Ai),
            Some(Action::ClearChat)
        );
        // V is global → open credits regardless of pane
        assert_eq!(
            k.resolve_meta_sub(&ev('v'), Focus::Tree),
            Some(Action::OpenCredits)
        );
        assert_eq!(
            k.resolve_meta_sub(&ev('v'), Focus::Editor),
            Some(Action::OpenCredits)
        );
    }

    #[test]
    fn pane_scope_beats_any() {
        let k = KeyBindings::defaults();
        // In editor, P → places-or-image (Editor scope), NOT add
        // paragraph (Tree scope). Both are listed; narrow scope
        // wins.
        assert_eq!(
            k.resolve_meta_sub(&ev('p'), Focus::Editor),
            Some(Action::LookupPlacesOrImage)
        );
        // In tree, P → add paragraph.
        assert_eq!(
            k.resolve_meta_sub(&ev('p'), Focus::Tree),
            Some(Action::AddParagraph)
        );
    }

    #[test]
    fn status_filter_digits() {
        let k = KeyBindings::defaults();
        for (c, expected) in [
            ('1', Action::StatusFilterReady),
            ('2', Action::StatusFilterFinal),
            ('3', Action::StatusFilterThird),
            ('4', Action::StatusFilterSecond),
            ('5', Action::StatusFilterFirst),
            ('6', Action::StatusFilterNapkin),
            ('7', Action::StatusFilterNone),
        ] {
            assert_eq!(
                k.resolve_meta_sub(&ev(c), Focus::Editor),
                Some(expected),
                "digit {c}"
            );
        }
    }

    #[test]
    fn bund_sub_known_chords() {
        let k = KeyBindings::defaults();
        assert_eq!(
            k.resolve_bund_sub(&ev('r'), Focus::Tree),
            Some(Action::BundRunBuffer)
        );
        assert_eq!(
            k.resolve_bund_sub(&ev('n'), Focus::Editor),
            Some(Action::BundNewScript)
        );
        assert_eq!(
            k.resolve_bund_sub(&ev('e'), Focus::Ai),
            Some(Action::BundOpenEvalModal)
        );
    }

    #[test]
    fn unknown_chord_is_none() {
        let k = KeyBindings::defaults();
        assert_eq!(k.resolve_meta_sub(&ev('z'), Focus::Editor), None);
    }
}
