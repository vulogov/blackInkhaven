//! 2.0 perf harness — `inkhaven _bench-render` (hidden). Opens the project and
//! times editor frame draws against a headless `TestBackend`, printing a stable,
//! parseable summary the criterion `render` bench reads. The internal timing
//! excludes process startup, so the reported per-frame cost is the real draw
//! cost — the metric the event-driven-redraw work will move.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;

pub fn run(project: &Path, frames: usize) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;

    let total = crate::tui::app::bench_render_frames(layout, cfg, store, frames)
        .map_err(|e| Error::Config(format!("render bench: {e}")))?;
    let avg = total / (frames.max(1) as u32);

    println!("render_frames: {frames}");
    println!("render_total_us: {}", total.as_micros());
    println!("render_avg_us: {}", avg.as_micros());
    Ok(())
}
