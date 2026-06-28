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
// 1.2.16+ Phase A.2 — manuscript intelligence
// dashboard (Ctrl+V Shift+J).
pub(crate) mod journal;
pub(crate) mod concordance;
mod conlang_hub;
mod credits;
mod diff_utils;
// 1.2.11+ — exposed crate-wide so the config-TUI's
// path widget can reuse the F3 file picker.
pub(crate) mod file_picker;
mod focus;
mod highlight;
mod hjson_edit;
mod hjson_highlight;
mod jinja_highlight;
mod inference;
pub(crate) mod input;
pub(crate) mod keybind;
pub(crate) mod keymap;
pub(crate) mod palette;
mod lexicon;
mod lexicon_build;
mod markdown;
mod markdown_highlight;
mod modal;
mod pov_tracker;
// 1.2.18+ R.3 — reading-time computation for the
// status-bar chip + R.4 reader-pace preview.
mod reading_time;
// 1.2.18+ R.4 — reader-pace preview teleprompter.
mod reader_pace;
mod say;
// 1.2.17+ T.1 — engine abstraction over the macOS `say`
// backend (preserved as `TtsEngine::System`) and the
// upcoming Piper neural backend (`TtsEngine::Piper`).
pub(crate) mod tts;
// 1.2.17+ T.1 stub — Piper backend type.  Full
// implementation lands across T.2–T.5.
pub(crate) mod piper;
// 1.2.17+ T.6 — pure state model for the
// `Ctrl+B Shift+V` voice picker modal.
mod voice_picker;
mod sentence_rhythm;
pub(crate) mod style_warnings;
mod echo_overlay;
mod quickref;
mod sound;
mod theme;
mod typst_funcs;
mod search_replace;
mod search_results;
// OUTLINE-1 — built across O-P0…O-P8; the renderer/keys (O-P1/O-P2) consume the
// state, so allow dead_code until then.
#[allow(dead_code)]
mod outline;
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
