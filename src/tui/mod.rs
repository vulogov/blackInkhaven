pub(crate) mod app;
mod bund_highlight;
mod diff_utils;
mod file_picker;
mod focus;
mod highlight;
mod hjson_edit;
mod hjson_highlight;
mod input;
pub(crate) mod keybind;
pub(crate) mod keymap;
mod lexicon;
mod markdown;
mod quickref;
mod sound;
mod theme;
mod typst_funcs;
mod search_replace;
mod search_results;
mod session;
mod status_helpers;
mod text_utils;
mod timeline_render;

use std::path::Path;

use anyhow::Result;

pub fn run(project: Option<&Path>) -> Result<()> {
    let project = project
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    app::run(&project)
}
