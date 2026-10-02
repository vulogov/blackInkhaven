//! WORLD-KEEP-2 (WK2-P4) — the whole `Ctrl+B W` → `C` compile as one function
//! that owns everything it touches, so it can run on a background thread.
//!
//! It compiles every layer, materializes them into the World book, and seeds
//! the proposal queue. It used to be the body of a TUI method running on the
//! UI thread, which froze the editor for as long as eleven chapters took to
//! write and re-embed. Nothing here borrows the app: it takes a cloned
//! [`Store`] (every store behind it is `Arc`-backed and pooled), reports
//! progress per layer, and checks a cancel flag between layers.
//!
//! Errors are the status-line strings the synchronous version produced, so the
//! two paths (worker, and the inline fallback when the job slot is busy) read
//! the same to the author.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::Config;
use crate::store::Store;
use crate::world::materialize as m;
use crate::world::types::WorldDefinition;

/// The `Err` payload of a compile the author cancelled between layers.
pub const CANCELLED: &str = "cancelled";

/// How many World-book chapters a full compile writes.
pub const LAYERS: usize = 11;

/// Compile `def`, materialize every layer into the World book, and seed the
/// proposal queue. Returns the number of proposals queued.
///
/// Layers are written in order and the run stops at the first failure (or at a
/// cancel), leaving the earlier layers in place — each leaf is written
/// atomically and a re-run is idempotent, so a partial compile is a valid
/// state, just an incomplete one.
pub fn compile_and_materialize(
    store: &Store,
    cfg: &Config,
    root: &Path,
    def: &WorldDefinition,
    progress: &mut dyn FnMut(String),
    cancel: &AtomicBool,
) -> std::result::Result<usize, String> {
    use crate::world::compile::*;

    progress("compiling the layers".into());
    // Geology: DEM if declared, else generated.
    let geo = match def.geology.as_ref().and_then(|g| g.dem.as_ref()) {
        Some(dem) => compile_geology_dem(def, &root.join(&dem.path)).map_err(|e| format!("world geology: {e}"))?,
        None => compile_geology(def),
    };
    let astro = compile_astronomy(&def.astronomy);
    let climate = compile_climate(def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);

    // The human half of the world, compiled up front so it materializes with
    // the physical layers and seeds the proposal queue below.
    let seed = def.seed_u64();
    let pol = compile_polities(&demo, &def.nations, seed);
    let capital_biomes: Vec<String> = pol
        .polities
        .iter()
        .map(|q| {
            demo.settlements
                .iter()
                .find(|s| (s.x, s.y) == q.capital_pos)
                .map(|s| s.biome.clone())
                .unwrap_or_default()
        })
        .collect();
    let cultures = compile_culture(&pol, &capital_biomes, &def.cultures, seed);
    let eco_declared = def.ecology.as_ref().map(|e| e.regions.as_slice()).unwrap_or(&[]);
    let eco = compile_ecology(&climate, eco_declared, seed);
    let trade = compile_trade(&pol, &geo, def.astronomy.planet.radius_earth);
    let magic = def.magic.clone().unwrap_or_default();

    // Sequential, stopping at the first failure.
    type Step<'a> = Box<dyn FnOnce() -> crate::error::Result<m::MaterializeReport> + 'a>;
    let steps: Vec<(&str, Step)> = vec![
        ("astronomy", Box::new(|| m::materialize_astronomy(store, cfg, &astro))),
        ("geology", Box::new(|| m::materialize_geology(store, cfg, &geo))),
        ("climate", Box::new(|| m::materialize_climate(store, cfg, &climate))),
        ("hydrology", Box::new(|| m::materialize_hydrology(store, cfg, &hydro))),
        ("demographics", Box::new(|| m::materialize_demographics(store, cfg, &demo))),
        ("polities", Box::new(|| m::materialize_polities(store, cfg, &pol))),
        ("culture", Box::new(|| m::materialize_culture(store, cfg, &cultures, &demo.role_archetypes, &capital_biomes))),
        ("ecology", Box::new(|| m::materialize_ecology(store, cfg, &eco))),
        ("trade", Box::new(|| m::materialize_trade(store, cfg, &pol, &trade))),
        ("magic", Box::new(|| m::materialize_magic(store, cfg, &magic))),
        ("setting", Box::new(|| m::materialize_setting(store, cfg, def))),
    ];
    debug_assert_eq!(steps.len(), LAYERS);
    for (i, (layer, step)) in steps.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.into());
        }
        progress(format!("{layer} ({}/{LAYERS})", i + 1));
        step().map_err(|e| format!("world materialize ({layer}): {e} — later layers skipped"))?;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED.into());
    }

    // Everything the world offers: Places, plus the culture-derived bridges
    // (Mythology symbols, realm rulers, and languages). Each kind clears only
    // its own pending set and skips sites the author already resolved.
    progress("proposals".into());
    let wlang = crate::world::i18n::WLang::of_config(&cfg.language);
    (|| -> crate::error::Result<usize> {
        use crate::error::Error;
        use crate::world::language_proposals::language_proposals;
        use crate::world::myth_proposals::myth_proposals;
        use crate::world::proposals::{place_proposals, PlaceProposal};
        use crate::world::ruler_proposals::ruler_proposals;
        use crate::world::storage::WorldStore;
        let ws = WorldStore::open_for_project(root).map_err(|e| Error::Store(format!("world store: {e}")))?;
        let resolved = ws.resolved_signatures(seed).map_err(|e| Error::Store(format!("{e}")))?;
        let batches: Vec<(&str, Vec<PlaceProposal>)> = vec![
            ("place", place_proposals(&demo, seed)),
            ("myth-%", myth_proposals(&cultures, seed)),
            ("character", ruler_proposals(&pol, &cultures, seed)),
            ("language", language_proposals(&pol, &cultures, seed)),
        ];
        let mut n = 0;
        for (like, mut batch) in batches {
            crate::world::i18n::localize_all(&mut batch, wlang);
            ws.clear_pending_kinds(like).map_err(|e| Error::Store(format!("{e}")))?;
            for p in batch {
                if !resolved.contains(&p.signature) {
                    ws.insert(&p, Some(seed)).map_err(|e| Error::Store(format!("{e}")))?;
                    n += 1;
                }
            }
        }
        Ok(n)
    })()
    .map_err(|e| format!("world proposals: {e}"))
}
