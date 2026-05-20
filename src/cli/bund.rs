//! `inkhaven bund <code>` — evaluate a Bund expression and print the
//! top of the workbench.
//!
//! Phase-0 smoke entry point. If the working directory (or
//! `--project`) is an initialised inkhaven project, the store is
//! opened — which auto-arms the scripting layer via
//! `Store::open` → `scripting::configure` — and `ink.*` words
//! become available. Otherwise the script runs against the bare
//! Adam VM (pure arithmetic / strings / control flow).
//!
//! The store-open path triggers fastembed model load on first use,
//! which is slow (~seconds). We avoid that for arithmetic-only
//! scripts by skipping the open when the path isn't an inkhaven
//! project.

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::Store;

pub fn run(code: &str, project: &Path) -> Result<()> {
    maybe_open_project(project);
    match crate::scripting::eval(code)? {
        Some(value) => println!("{}", crate::scripting::format_value(&value)),
        None => println!("(no result)"),
    }
    Ok(())
}

/// Open the project at `project` when it's a real inkhaven
/// directory. `Store::open` itself arms the scripting layer
/// (policy + active store) — we don't need to wire anything
/// else here.
fn maybe_open_project(project: &Path) {
    let layout = ProjectLayout::new(project);
    if layout.require_initialized().is_err() {
        return;
    }
    let cfg = match Config::load(&layout.config_path()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let _ = Store::open(layout, &cfg);
}
