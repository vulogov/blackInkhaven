//! Static keybinding tables for the Ctrl+H quick reference. Layout is
//! produced at render time; entries here are just pairs of (key, action).

use super::focus::Focus;

#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub key: &'static str,
    pub desc: &'static str,
    /// Section heading (no description).
    pub is_header: bool,
}

const fn entry(key: &'static str, desc: &'static str) -> Entry {
    Entry {
        key,
        desc,
        is_header: false,
    }
}

const fn header(label: &'static str) -> Entry {
    Entry {
        key: label,
        desc: "",
        is_header: true,
    }
}

pub fn entries_for(focus: Focus) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    out.extend_from_slice(&global_entries());
    out.push(header("")); // blank spacer
    match focus {
        Focus::Tree | Focus::SearchBar => {
            out.push(header("─── Tree ───"));
            out.extend_from_slice(&tree_entries());
        }
        Focus::Editor => {
            out.push(header("─── Editor ───"));
            out.extend_from_slice(&editor_entries());
        }
        Focus::Ai | Focus::AiPrompt => {
            out.push(header("─── AI ───"));
            out.extend_from_slice(&ai_entries());
        }
    }
    out
}

fn global_entries() -> [Entry; 16] {
    [
        header("─── Global ───"),
        entry("Ctrl+Q", "Quit (autosaves dirty paragraph)"),
        entry("Tab / Shift+Tab", "Cycle Tree → Editor → AI"),
        entry("Ctrl+1/2/3/4/5", "Focus Editor/Tree/AI/Search/AI-prompt"),
        entry("Ctrl+T", "Focus Tree (Ctrl+2 alternative)"),
        entry("Ctrl+S", "Save current paragraph"),
        entry("Ctrl+/", "Focus Search bar"),
        entry("Ctrl+I", "Focus AI prompt"),
        entry("Ctrl+B", "Meta prefix (next key = action)"),
        entry("Ctrl+B H", "This Quick reference (works from every pane)"),
        entry("Ctrl+B V", "Version, author, and component credits"),
        entry("F1", "Help-manual question (RAG over the Help book)"),
        entry("F7", "Grammar check the open paragraph (→ AI pane)"),
        entry("F9", "Cycle AI scope: None→Sel→Para→Sub→Chap→Book→None"),
        entry("F10", "Toggle inference: Local↔Full (Help locked to Local)"),
        entry("Esc", "Close overlay / cancel"),
    ]
}

fn tree_entries() -> [Entry; 27] {
    [
        entry("↑ / ↓ / Home / End", "Navigate"),
        entry("PageUp / PageDown", "Jump by 10"),
        entry("← / →", "Collapse / expand branch (← steps to parent)"),
        entry("Enter", "Open paragraph (autosaves the previous one)"),
        entry("F2", "Rename current node"),
        entry("F3", "File picker (load file or import directory tree)"),
        header(""),
        header("─ Hierarchy edits ─"),
        entry("B / C / A / + ", "Add book / chapter / subchapter / paragraph"),
        entry("V / S / P", "Insert chapter/subchapter/paragraph after current"),
        entry("D", "Delete branch at cursor"),
        entry("-", "Delete paragraph at cursor"),
        header(""),
        header("─ Reorder ─"),
        entry("U", "Move current node up among siblings"),
        entry("J", "Move current node down among siblings"),
        header(""),
        header("─ Folding ─"),
        entry("← / →", "Collapse / expand cursor's branch"),
        entry("Z", "Collapse cursor's enclosing subchapter"),
        entry("X", "Collapse every expanded branch"),
        header(""),
        header("─ Meta prefix ─"),
        entry("Ctrl+B B/C/S/P", "Add book/chapter/subchapter/paragraph"),
        entry("Ctrl+B D", "Delete cursor's node"),
        entry("Ctrl+B ↑/↓", "Reorder within siblings (chord form of U/J)"),
        entry("q", "Quit (autosaves if dirty)"),
    ]
}

fn editor_entries() -> [Entry; 43] {
    [
        entry("arrows", "Move cursor"),
        entry("Ctrl+arrows", "Word back/forward / top / bottom"),
        entry("Home / End", "Start / end of line"),
        entry("PageUp / PageDown", "By paragraph"),
        entry("Shift+arrows", "Extend linear selection"),
        entry("Ctrl+A", "Select all"),
        header(""),
        header("─ Clipboard ─"),
        entry("Ctrl+C", "Copy selection (system clipboard)"),
        entry("Ctrl+K", "Cut selection"),
        entry("Ctrl+P", "Paste at cursor"),
        header(""),
        header("─ Edit ─"),
        entry("Ctrl+U", "Undo"),
        entry("Ctrl+Y", "Redo"),
        entry("Ctrl+D", "Delete current line"),
        entry("Ctrl+E", "Delete cursor → end of line"),
        entry("Ctrl+W", "Delete cursor → start of line"),
        entry("Ctrl+Backspace", "Delete previous word"),
        header(""),
        header("─ Find / Replace (regex) ─"),
        entry("Ctrl+F", "Open Find (regex)"),
        entry("Ctrl+X", "Repeat — next match / replace+next"),
        entry("Ctrl+R", "Open Replace · or replace all"),
        header(""),
        header("─ Block selection ─"),
        entry("Alt+arrows", "Extend rectangular selection"),
        entry("Alt+C", "Copy rectangular block"),
        header(""),
        header("─ Files & snapshots ─"),
        entry("F3", "Load file → replace buffer"),
        entry("F4 / Ctrl+F4", "Toggle split / accept snapshot"),
        entry("F5 / F6", "Create / list snapshots"),
        entry("Ctrl+H / Ctrl+J", "(split only) scroll lower pane up/down"),
        header(""),
        header("─ Editor meta (Ctrl+B …) ─"),
        entry("Ctrl+B S", "Save (alternative to Ctrl+S)"),
        entry("Ctrl+B N / R", "New snapshot / open snapshot history"),
        entry("Ctrl+B L / F", "Load file / toggle split-edit"),
        entry("Ctrl+B T", "Retitle paragraph from its first sentence"),
        entry("Ctrl+B P", "Place RAG — selection → Places book → AI pane"),
        entry("Ctrl+B C", "Character RAG — selection → Characters book → AI pane"),
        entry("Ctrl+B H", "Open this Quick reference"),
    ]
}

fn ai_entries() -> [Entry; 19] {
    [
        entry("r / R", "Replace editor selection with AI text"),
        entry("i / I", "Insert AI text at cursor"),
        entry("t / T", "Prepend AI text to top of paragraph"),
        entry("g / G", "Grammar check: replace buffer w/ corrected only"),
        entry("b / B", "Append AI text to bottom"),
        entry("c / C", "Copy AI text to clipboard"),
        entry("Esc", "Bounce to AI prompt (mirror of AI prompt's Esc)"),
        header(""),
        header("─ AI prompt input ─"),
        entry("/", "Show prompt library picker"),
        entry("Help! …", "Help-manual question (same as F1)"),
        entry("Enter", "Send (chat history is replayed automatically)"),
        entry("Esc", "Bounce to AI pane to read the answer"),
        header(""),
        header("─ Chat session ─"),
        entry("F9", "Cycle scope: None/Sel/Para/Sub/Chap/Book"),
        entry("F10", "Toggle inference: Local ↔ Full"),
        entry("Ctrl+B C", "Clear chat history + current inference"),
        entry("Ctrl+B H", "Open Quick reference"),
    ]
}
