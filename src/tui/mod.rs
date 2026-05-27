pub(crate) mod app;
mod backup_ui;
mod bund_highlight;
mod concordance;
mod credits;
mod diff_utils;
mod file_picker;
mod focus;
mod highlight;
mod hjson_edit;
mod hjson_highlight;
mod inference;
mod input;
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
