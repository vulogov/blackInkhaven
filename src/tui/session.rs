//! Persisted TUI session: what paragraph was open, where the cursors sat,
//! which tree branches were collapsed, which pane had focus. Stored in
//! `<project>/.session.json` and re-applied on next startup.
//!
//! Loaders ignore missing/corrupt files quietly — sessions are a UX nicety,
//! not a correctness requirement.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const SESSION_FILE: &str = ".session.json";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub tree: TreeSession,
    #[serde(default)]
    pub editor: Option<EditorSession>,
    /// One of "Tree", "Editor", "Ai", "SearchBar", "AiPrompt". Anything else
    /// is treated as "Tree" on restore.
    #[serde(default)]
    pub focus: String,
    /// Cursor/scroll positions per paragraph UUID. Updated whenever the
    /// editor loses focus, the user switches paragraphs, or the app exits —
    /// so re-opening any paragraph drops the cursor back where the user
    /// left it, even after a full restart.
    #[serde(default)]
    pub paragraph_cursors: HashMap<String, ParagraphCursor>,
    /// 1.2.7+ — visited-paragraph history (browser-style
    /// back/forward via Alt+Left / Alt+Right). UUIDs in
    /// visit order; cursor points at the current one.
    #[serde(default)]
    pub visited_history: Vec<String>,
    #[serde(default)]
    pub visited_cursor: usize,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct ParagraphCursor {
    #[serde(default)]
    pub cursor_row: usize,
    #[serde(default)]
    pub cursor_col: usize,
    #[serde(default)]
    pub scroll_row: usize,
    #[serde(default)]
    pub scroll_col: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TreeSession {
    /// UUID of the node the tree cursor was on.
    #[serde(default)]
    pub cursor_id: Option<String>,
    /// UUIDs of branches whose subtrees were collapsed.
    #[serde(default)]
    pub collapsed_nodes: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EditorSession {
    pub opened_id: String,
    #[serde(default)]
    pub cursor_row: usize,
    #[serde(default)]
    pub cursor_col: usize,
}

impl SessionState {
    pub fn load(project_root: &Path) -> Option<Self> {
        let path = project_root.join(SESSION_FILE);
        let raw = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = project_root.join(SESSION_FILE);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)
    }
}
