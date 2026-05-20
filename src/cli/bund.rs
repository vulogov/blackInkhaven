//! `inkhaven bund <code>` — evaluate a Bund expression and print the
//! top of the workbench.
//!
//! Phase-0 smoke entry point. If the working directory (or
//! `--project`) is an initialised inkhaven project, the store is
//! opened and registered with the scripting layer so `ink.*` words
//! work. Otherwise the script runs against the bare Adam VM (pure
//! arithmetic / strings / control flow).
//!
//! The store-open path triggers fastembed model load on first use,
//! which is slow (~seconds). We avoid that for arithmetic-only
//! scripts by skipping the open when the path isn't an inkhaven
//! project. Pure VM smoke tests stay fast.

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::Store;

pub fn run(code: &str, project: &Path) -> Result<()> {
    maybe_register_active_store(project);
    match crate::scripting::eval(code)? {
        Some(value) => println!("{}", crate::scripting::format_value(&value)),
        None => println!("(no result)"),
    }
    Ok(())
}

/// Best-effort attempt to open the project at `project` and install
/// its `Store` into the scripting layer. Failures (not initialised,
/// missing config, model load fails) are silent — the bund command
/// must remain usable for pure-VM experiments outside any project.
fn maybe_register_active_store(project: &Path) {
    let layout = ProjectLayout::new(project);
    if layout.require_initialized().is_err() {
        return;
    }
    let cfg = match Config::load(&layout.config_path()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let store = match Store::open(layout, &cfg) {
        Ok(s) => s,
        Err(_) => return,
    };
    crate::scripting::register_active_store(store);
}
