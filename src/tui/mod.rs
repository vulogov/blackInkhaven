mod app;
mod bund_highlight;
mod file_picker;
mod focus;
mod highlight;
mod hjson_highlight;
mod input;
mod keymap;
mod lexicon;
mod markdown;
mod quickref;
mod sound;
mod theme;
mod typst_funcs;
mod search_replace;
mod search_results;
mod session;

use std::path::Path;

use anyhow::Result;

pub fn run(project: Option<&Path>) -> Result<()> {
    let project = project
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    app::run(&project)
}
