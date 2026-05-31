pub(crate) mod app;
mod backup_ui;
mod bund_highlight;
// 1.2.12+ — exposed crate-wide so the CLI's
// `inkhaven export-concordance` subcommand can reuse
// the same builder + types the Ctrl+B Shift+L modal
// shows.
// 1.2.14+ Phase C.1 — inline comments on paragraph
// prose.  Sidecar JSON storage adjacent to the
// `.typ` file.
pub(crate) mod comments;
// 1.2.14+ Phase Q.2 — HJSON-driven snippet
// expansion for the editor.
pub(crate) mod snippets;
pub(crate) mod project_goal;
pub(crate) mod concordance;
mod credits;
mod diff_utils;
// 1.2.11+ — exposed crate-wide so the config-TUI's
// path widget can reuse the F3 file picker.
pub(crate) mod file_picker;
mod focus;
mod highlight;
mod hjson_edit;
mod hjson_highlight;
mod inference;
pub(crate) mod input;
pub(crate) mod keybind;
pub(crate) mod keymap;
mod lexicon;
mod lexicon_build;
mod markdown;
mod markdown_highlight;
mod modal;
mod pov_tracker;
mod say;
mod sentence_rhythm;
mod style_warnings;
mod quickref;
mod sound;
mod theme;
mod typst_funcs;
mod search_replace;
mod search_results;
mod session;
mod shell;
mod splash;
mod state;
mod status_helpers;
mod text_utils;
mod timeline_render;
pub(crate) mod timeline_state;

use std::path::Path;

use anyhow::Result;

pub fn run(project: Option<&Path>) -> Result<()> {
    let project = project
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    app::run(&project)
}
