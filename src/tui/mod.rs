mod app;
mod focus;
mod highlight;
mod input;
mod keymap;
mod search_results;

use std::path::Path;

use anyhow::Result;

pub fn run(project: Option<&Path>) -> Result<()> {
    let project = project
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    app::run(&project)
}
