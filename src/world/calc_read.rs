//! RESRCH-4 (R4-D) — read **materialized World-book facts** for `/calc`.
//!
//! The WORLD-4 compiler materializes each layer as `content_type="hjson"`
//! paragraphs under `World / <Chapter> / …` (see `materialize.rs`). This reader
//! resolves a `Chapter/field[/idx|key…]` path against that book and returns the
//! JSON value at it — the "extract source data from World" half of RESRCH-4.
//!
//! Read-only (the `store_read` posture of the other `ink.*` words); it never
//! writes, recompiles, or touches the proposal queue. A path that doesn't
//! resolve returns `None` — the caller pushes `NODATA`, never a fabricated value.

use serde_json::Value as Json;

use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::{NodeKind, SYSTEM_TAG_WORLD};

/// All of one chapter's paragraph JSON objects merged into a single object, or
/// `None` when the World book / chapter is absent. The chapter head matches a
/// chapter by title or slug (case-insensitive), e.g. `Astronomy`.
pub fn chapter(store: &Store, chapter_head: &str) -> Option<Json> {
    let h = Hierarchy::load(store).ok()?;
    let book = h
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_WORLD))?;
    let head = chapter_head.trim();
    let chapter = h.children_of(Some(book.id)).into_iter().find(|n| {
        n.kind == NodeKind::Chapter
            && (n.title.eq_ignore_ascii_case(head) || n.slug.eq_ignore_ascii_case(head))
    });

    // Route 1 — read the materialized paragraph JSON of the chapter (if present).
    let mut map = serde_json::Map::new();
    if let Some(chapter) = &chapter {
        for pid in h.collect_subtree(chapter.id) {
            let Some(node) = h.get(pid) else { continue };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let Ok(Some(bytes)) = store.get_content(pid) else { continue };
            if let Ok(Json::Object(obj)) = serde_json::from_slice::<Json>(&bytes) {
                for (k, v) in obj {
                    map.insert(k, v);
                }
            }
        }
    }
    if !map.is_empty() {
        return Some(Json::Object(map));
    }
    // Route 2 — the chapter is absent or unmaterialized; recompile from world.hjson.
    recompile_chapter(store, head)
}

/// Route 2 — recompile a layer in memory from `world.hjson` and return its output
/// as a JSON object (same numbers materialization would write). Used when the
/// World book chapter isn't materialized; `None` when there's no `world.hjson` or
/// the chapter isn't a known layer.
fn recompile_chapter(store: &Store, chapter_head: &str) -> Option<Json> {
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_demographics, compile_geology, compile_hydrology,
    };
    use crate::world::types::WorldDefinition;

    let raw = std::fs::read_to_string(store.project_root().join("world.hjson")).ok()?;
    let def = WorldDefinition::from_hjson(&raw).ok()?;
    // The layer dependency chain: astronomy/geology from the definition; the rest
    // consume upstream outputs.
    let value = match chapter_head.to_ascii_lowercase().as_str() {
        "astronomy" => serde_json::to_value(compile_astronomy(&def.astronomy)),
        "geology" => serde_json::to_value(compile_geology(&def)),
        "climate" => {
            let astro = compile_astronomy(&def.astronomy);
            let geo = compile_geology(&def);
            serde_json::to_value(compile_climate(&def, &astro, &geo))
        }
        "hydrology" => {
            let astro = compile_astronomy(&def.astronomy);
            let geo = compile_geology(&def);
            let clim = compile_climate(&def, &astro, &geo);
            serde_json::to_value(compile_hydrology(&geo, &clim))
        }
        "demographics" => {
            let astro = compile_astronomy(&def.astronomy);
            let geo = compile_geology(&def);
            let clim = compile_climate(&def, &astro, &geo);
            let hydro = compile_hydrology(&geo, &clim);
            serde_json::to_value(compile_demographics(&clim, &hydro))
        }
        _ => return None,
    };
    match value.ok()? {
        obj @ Json::Object(_) => Some(obj),
        _ => None,
    }
}

/// Resolve a full `Chapter/field[/idx|key…]` path to a JSON value: the chapter
/// is the first segment, the rest index into that chapter's merged object
/// (object keys or array indices). `Chapter` alone returns the whole chapter.
pub fn lookup(store: &Store, path: &str) -> Option<Json> {
    let segs: Vec<&str> = path.split('/').map(str::trim).filter(|s| !s.is_empty()).collect();
    let (head, rest) = segs.split_first()?;
    let chapter = chapter(store, head)?;
    index_json(&chapter, rest)
}

/// Walk `path` segments into a JSON value — object key or (numeric) array index.
fn index_json<'a>(root: &'a Json, path: &[&str]) -> Option<Json> {
    let mut cur = root;
    for seg in path {
        cur = match cur {
            Json::Object(map) => map.get(*seg)?,
            Json::Array(arr) => {
                let i: usize = seg.parse().ok()?;
                arr.get(i)?
            }
            _ => return None,
        };
    }
    Some(cur.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_object_then_array_then_field() {
        let root = serde_json::json!({
            "year_length_planet_days": 412.3,
            "insolation_bands": [ { "annual_mean": 18.7 }, { "annual_mean": 9.1 } ],
        });
        assert_eq!(index_json(&root, &["year_length_planet_days"]).unwrap(), serde_json::json!(412.3));
        assert_eq!(index_json(&root, &["insolation_bands", "1", "annual_mean"]).unwrap(), serde_json::json!(9.1));
        assert!(index_json(&root, &["missing"]).is_none());
        assert!(index_json(&root, &["insolation_bands", "9"]).is_none());
        // whole-root request (empty path) returns the root.
        assert_eq!(index_json(&root, &[]).unwrap(), root);
    }
}
