//! Quick-reference panel content for Ctrl+B H.
//!
//! Two-layer composition:
//!
//! 1. **Static layer** — non-rebindable surface (arrow keys, F-keys,
//!    pane-focus chords, raw Enter/Esc behaviour, etc). These can't
//!    change at runtime, so they live as inline calls below.
//! 2. **Live keymap layer** — the meta-prefix (Ctrl+B …) and
//!    bund-prefix (Ctrl+Z …) chord tables are pulled from
//!    `KeyBindings::active()` so user overrides in HJSON or runtime
//!    `ink.key.*` mutations flow straight into Ctrl+B H. Disabled
//!    chords (`Action::None`) drop out; user-bound Bund lambdas
//!    surface as `λ <name>`.
//!
//! Returned entries own their strings — runtime synthesis from the
//! keymap means there's no usable lifetime that lets us keep static
//! slices everywhere.

use std::collections::HashSet;

use super::focus::Focus;
use super::keybind::{self, Action, BindingEntry};

#[derive(Debug, Clone)]
pub struct Entry {
    pub key: String,
    pub desc: String,
    /// Section heading (no description).
    pub is_header: bool,
}

fn entry(key: impl Into<String>, desc: impl Into<String>) -> Entry {
    Entry {
        key: key.into(),
        desc: desc.into(),
        is_header: false,
    }
}

fn header(label: impl Into<String>) -> Entry {
    Entry {
        key: label.into(),
        desc: String::new(),
        is_header: true,
    }
}

fn blank() -> Entry {
    Entry {
        key: String::new(),
        desc: String::new(),
        is_header: true,
    }
}

pub fn entries_for(focus: Focus) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    out.extend(global_entries());
    out.push(blank());
    match focus {
        Focus::Tree | Focus::SearchBar => {
            out.push(header("─── Tree ───"));
            out.extend(tree_entries());
        }
        Focus::Editor => {
            out.push(header("─── Editor ───"));
            out.extend(editor_entries());
        }
        Focus::Ai | Focus::AiPrompt => {
            out.push(header("─── AI ───"));
            out.extend(ai_entries());
        }
    }
    // Live keymap section — synthesised from KeyBindings::active().
    // Comes last so users who rebound something can see the actual
    // chord without scanning the static layer above.
    out.push(blank());
    out.push(header("─── Meta chords (live keymap) ───"));
    out.extend(live_chord_entries(keybind::Layer::MetaSub, focus));
    out.push(blank());
    out.push(header("─── Bund chords (live keymap) ───"));
    out.extend(live_chord_entries(keybind::Layer::BundSub, focus));
    out
}

fn global_entries() -> Vec<Entry> {
    vec![
        header("─── Global ───"),
        entry("Ctrl+Q", "Quit (autosaves dirty paragraph)"),
        entry("Tab / Shift+Tab", "Cycle Tree → Editor → AI"),
        entry("Ctrl+1/2/3/4/5", "Focus Editor/Tree/AI/Search/AI-prompt"),
        entry("Ctrl+T", "Focus Tree (Ctrl+2 alternative)"),
        entry("Ctrl+S", "Save current paragraph"),
        entry("Ctrl+/", "Focus Search bar"),
        entry("Ctrl+I", "Focus AI prompt"),
        entry("Ctrl+B", "Meta prefix (next key = action — see live section below)"),
        entry("Ctrl+Z", "Bund prefix (next key = bund action — see live section below)"),
        entry("F1", "Help-manual question (RAG over the Help book)"),
        entry("F7", "Grammar check the open paragraph (→ AI pane)"),
        entry("F9", "Cycle AI scope: None→Sel→Para→Sub→Chap→Book→None"),
        entry("F10", "Toggle inference: Local↔Full (Help locked to Local)"),
        entry("Esc", "Close overlay / cancel"),
    ]
}

fn tree_entries() -> Vec<Entry> {
    vec![
        entry("↑ / ↓ / Home / End", "Navigate"),
        entry("PageUp / PageDown", "Jump by 10"),
        entry("← / →", "Collapse / expand branch (← steps to parent)"),
        entry("Enter", "Open paragraph (autosaves the previous one)"),
        entry("F2", "Rename current node"),
        entry("F3", "File picker — load file or import directory tree"),
        blank(),
        header("─ Hierarchy edits ─"),
        entry("B / C / A / +", "Add book / chapter / subchapter / paragraph"),
        entry("V / S / P", "Insert chapter / subchapter / paragraph after current"),
        entry("D", "Delete branch at cursor (asks for confirmation)"),
        entry("-", "Delete paragraph at cursor"),
        blank(),
        header("─ Reorder ─"),
        entry("U", "Move current node up among siblings"),
        entry("J", "Move current node down among siblings"),
        blank(),
        header("─ Folding ─"),
        entry("← / →", "Collapse / expand cursor's branch"),
        entry("Z", "Collapse cursor's enclosing subchapter"),
        entry("X", "Collapse every expanded branch"),
        entry("q", "Quit (autosaves if dirty)"),
    ]
}

fn editor_entries() -> Vec<Entry> {
    vec![
        entry("arrows", "Move cursor"),
        entry("Ctrl+← / →", "Word back / forward"),
        entry("Ctrl+Home / End", "File top / bottom"),
        entry("Home / End", "Start / end of line"),
        entry("PageUp / PageDown", "By paragraph"),
        entry("Shift+arrows", "Extend linear selection"),
        entry("Ctrl+A", "Select all"),
        entry("Esc", "Clear in-buffer search (first press); cycle to Tree"),
        blank(),
        header("─ Clipboard ─"),
        entry("Ctrl+C", "Copy selection (system clipboard)"),
        entry("Ctrl+K", "Cut selection"),
        entry("Ctrl+P", "Paste at cursor"),
        blank(),
        header("─ Edit ─"),
        entry("Ctrl+U", "Undo"),
        entry("Ctrl+Y", "Redo"),
        entry("Ctrl+D", "Delete current line"),
        entry("Ctrl+E", "Delete cursor → end of line"),
        entry("Ctrl+W", "Delete cursor → start of line"),
        entry("Ctrl+Backspace", "Delete previous word"),
        blank(),
        header("─ Find / Replace (regex) ─"),
        entry("Ctrl+F", "Open Find (regex)"),
        entry("Ctrl+X", "Repeat — next match / replace+next"),
        entry("Ctrl+R", "Open Replace · or replace all (while in replace mode)"),
        blank(),
        header("─ Block selection ─"),
        entry("Alt+arrows", "Extend rectangular selection"),
        entry("Alt+C", "Copy rectangular block"),
        blank(),
        header("─ Files & snapshots ─"),
        entry("F3", "Load file → replace buffer (Ctrl+B F also toggles split)"),
        entry("F4 / Ctrl+F4", "Toggle split / accept split snapshot"),
        entry("F5", "Snapshot the current paragraph (== Ctrl+B N)"),
        entry(
            "F6",
            "Snapshot picker — ↑↓ navigate · Enter load · D / Del delete · Esc close",
        ),
        entry("Ctrl+H / Ctrl+J", "(split only) scroll lower pane up / down"),
    ]
}

fn ai_entries() -> Vec<Entry> {
    vec![
        header("─ AI pane (apply a finished inference) ─"),
        entry("r / R", "Replace editor selection with AI text"),
        entry("i / I", "Insert AI text at cursor"),
        entry("t / T", "Prepend AI text to top of paragraph"),
        entry("g / G", "Grammar apply: extract corrected text, overwrite buffer"),
        entry("b / B", "Append AI text to bottom"),
        entry("c / C", "Copy AI text to clipboard"),
        entry("Esc", "Bounce to AI prompt (mirror of AI prompt's Esc)"),
        entry("q / Q", "Quit (autosaves if dirty)"),
        blank(),
        header("─ AI prompt input ─"),
        entry("/", "Show prompt library picker (Tab / Enter to commit)"),
        entry("Help! …", "Help-manual question (same as F1, RAG over Help)"),
        entry("Enter", "Send (chat history is replayed automatically)"),
        entry("Esc", "Bounce to AI pane to read the answer"),
        blank(),
        header("─ Chat session ─"),
        entry("F9", "Cycle scope: None / Sel / Para / Sub / Chap / Book"),
        entry("F10", "Toggle inference: Local ↔ Full (Help locked to Local)"),
    ]
}

/// Synthesize one entry per active chord in the requested layer
/// (meta-sub or bund-sub) filtered by the current `focus`. Disabled
/// chords (`Action::None`) and entries whose action has no label
/// are dropped. Duplicates by action label collapse — the first
/// chord wins, so a binding table with both `Up` and `u` for
/// ReorderUp surfaces only as `Up`.
fn live_chord_entries(layer: keybind::Layer, focus: Focus) -> Vec<Entry> {
    let bindings = keybind::read();
    let table: &Vec<BindingEntry> = match layer {
        keybind::Layer::MetaSub => &bindings.meta_sub,
        keybind::Layer::BundSub => &bindings.bund_sub,
    };
    let prefix = match layer {
        keybind::Layer::MetaSub => bindings.meta_prefix.to_display_string(),
        keybind::Layer::BundSub => bindings
            .bund_prefix
            .map(|c| c.to_display_string())
            .unwrap_or_else(|| "(bund prefix disabled)".to_string()),
    };
    let mut out: Vec<Entry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for be in table.iter() {
        if !be.scope.matches(focus) {
            continue;
        }
        if matches!(be.action, Action::None) {
            continue;
        }
        let label = be.action.label();
        if label.is_empty() {
            continue;
        }
        // De-dupe by action label so two chords bound to the same
        // action show up once. The user can still see secondary
        // bindings via `ink.key.list` if they care.
        if !seen.insert(label.clone()) {
            continue;
        }
        let chord = format!("{} {}", prefix, be.chord.to_display_string());
        out.push(entry(chord, label));
    }
    if out.is_empty() {
        out.push(entry("—", "no chords active in this pane"));
    }
    out
}
