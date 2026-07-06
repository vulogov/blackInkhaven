//! WORLD-13 — the single place a world proposal becomes a book entry. Both the
//! CLI (`realworld proposals accept`) and the TUI (`Ctrl+B W → P`) route through
//! [`commit_proposal`], so every proposal kind (Place · Mythology · Character ·
//! language) is committed the same way from either surface — no duplicated,
//! drifting accept logic.
//!
//! Each committer takes an already-open [`Store`], so the TUI reuses its live
//! store instead of opening a second handle.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::store::hierarchy::Hierarchy;
use crate::store::{InsertPosition, NodeKind, Store};
use crate::store::{SYSTEM_TAG_CHARACTERS, SYSTEM_TAG_LANGUAGES, SYSTEM_TAG_MYTHOLOGY, SYSTEM_TAG_PLACES};
use crate::world::proposals::{PlaceLink, PlaceProposal};
use crate::world::storage::WorldStore;
use uuid::Uuid;

/// Commit an accepted proposal into its target book, dispatching on `kind`.
/// Returns the human label of the book it landed in (for the status line). For a
/// Place, also records the `world_place_links` cross-reference via `ws`.
pub(crate) fn commit_proposal(
    store: &Store,
    cfg: &Config,
    ws: &WorldStore,
    p: &PlaceProposal,
) -> Result<&'static str> {
    if p.kind.starts_with("myth-") {
        commit_myth(store, cfg, p)?;
        Ok("Mythology")
    } else if p.kind == "character" {
        commit_character(store, cfg, p)?;
        Ok("Characters")
    } else if p.kind == "language" {
        commit_language(store, cfg, p)?;
        Ok("Language")
    } else {
        let place_id = commit_place(store, cfg, p)?;
        ws.insert_place_link(&PlaceLink::from_proposal(place_id, p))
            .map_err(|e| Error::Store(format!("place link: {e}")))?;
        Ok("Places")
    }
}

fn book<'a>(h: &'a Hierarchy, tag: &str, label: &str) -> Result<&'a crate::store::node::Node> {
    h.iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(tag))
        .ok_or_else(|| Error::Store(format!("{label} system book missing")))
}

/// Write a paragraph under `parent` with `body`, optional `tag` + hjson typing.
/// Returns the created node id.
fn write_paragraph(
    store: &Store,
    cfg: &Config,
    parent: &crate::store::node::Node,
    title: &str,
    body: &str,
    tag: Option<&str>,
    hjson: bool,
) -> Result<Uuid> {
    let h = Hierarchy::load(store)?;
    let mut node = store
        .create_node(cfg, &h, NodeKind::Paragraph, title, Some(parent), None, InsertPosition::End)
        .map_err(|e| Error::Store(format!("creating `{title}`: {e}")))?;
    if let Some(t) = tag {
        node.tags = vec![t.to_string()];
    }
    if hjson {
        node.content_type = Some("hjson".to_string());
    }
    if let Some(rel) = &node.file {
        std::fs::write(store.project_root().join(rel), body.as_bytes())
            .map_err(|e| Error::Store(format!("writing `{title}`: {e}")))?;
    }
    store
        .update_paragraph_content(&mut node, body.as_bytes())
        .map_err(|e| Error::Store(format!("saving `{title}`: {e}")))?;
    Ok(node.id)
}

/// A Place proposal → a prose paragraph under the Places book. Returns node id.
pub(crate) fn commit_place(store: &Store, cfg: &Config, p: &PlaceProposal) -> Result<Uuid> {
    let h = Hierarchy::load(store)?;
    let places = book(&h, SYSTEM_TAG_PLACES, "Places")?.clone();
    let pop = p.payload.get("population").and_then(|v| v.as_u64()).unwrap_or(0);
    let class = p.payload.get("class").and_then(|v| v.as_str()).unwrap_or("settlement");
    let basis = p.payload.get("basis").and_then(|v| v.as_str()).unwrap_or("").replace('_', " ");
    let biome = p.payload.get("biome").and_then(|v| v.as_str()).unwrap_or("").replace('_', " ");
    let prose = format!(
        "{} is a {} of roughly {} people, set at a {} in a {} zone.\n\n// world-compiler proposal {}\n",
        p.name, class, pop, basis, biome, p.signature
    );
    write_paragraph(store, cfg, &places, &p.name, &prose, None, false)
}

/// A Mythology proposal → a tagged `para:myth-*` HJSON paragraph.
pub(crate) fn commit_myth(store: &Store, cfg: &Config, p: &PlaceProposal) -> Result<()> {
    let h = Hierarchy::load(store)?;
    let myth = book(&h, SYSTEM_TAG_MYTHOLOGY, "Mythology")?.clone();
    let tag = p.payload.get("tag").and_then(|v| v.as_str()).unwrap_or("para:myth-symbol");
    let body = crate::world::myth_proposals::myth_entry_body(p);
    write_paragraph(store, cfg, &myth, &p.name, &body, Some(tag), true)?;
    Ok(())
}

/// A ruler proposal → a Character stub paragraph.
pub(crate) fn commit_character(store: &Store, cfg: &Config, p: &PlaceProposal) -> Result<()> {
    let h = Hierarchy::load(store)?;
    let chars = book(&h, SYSTEM_TAG_CHARACTERS, "Characters")?.clone();
    let body = crate::world::ruler_proposals::ruler_body(p);
    write_paragraph(store, cfg, &chars, &p.name, &body, None, false)?;
    Ok(())
}

/// A language proposal → a scaffolded language book seeded with the world brief.
pub(crate) fn commit_language(store: &Store, cfg: &Config, p: &PlaceProposal) -> Result<()> {
    let h = Hierarchy::load(store)?;
    let lang_book = book(&h, SYSTEM_TAG_LANGUAGES, "Language")?.clone();
    // Idempotent: an existing language of this name is not re-scaffolded.
    if h.children_of(Some(lang_book.id)).iter().any(|n| n.title.eq_ignore_ascii_case(&p.name)) {
        return Ok(());
    }
    let per_lang = store
        .create_node(cfg, &h, NodeKind::Book, &p.name, Some(&lang_book), None, InsertPosition::End)
        .map_err(|e| Error::Store(format!("creating language book: {e}")))?;
    crate::cli::language::scaffold_language_chapters(store, cfg, &per_lang, |_| {})
        .map_err(|e| Error::Store(format!("scaffolding language: {e}")))?;
    let h2 = Hierarchy::load(store)?;
    if let Some(meta) = h2
        .children_of(Some(per_lang.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Meta"))
    {
        let body = crate::world::language_proposals::language_brief_body(p);
        write_paragraph(store, cfg, &meta, "world-profile", &body, None, false)?;
    }
    Ok(())
}

/// Open the project's stores and commit a proposal by id — the CLI entry point
/// that also flips the proposal's status to `accepted`. Returns the book label.
pub(crate) fn accept_by_id(project: &Path, id: Uuid) -> Result<(&'static str, String)> {
    let layout = crate::project::ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let ws = WorldStore::open_for_project(project).map_err(|e| Error::Store(format!("world store: {e}")))?;
    let p = ws
        .get(id)
        .map_err(|e| Error::Store(format!("get proposal: {e}")))?
        .ok_or_else(|| Error::Config(format!("no proposal `{id}`")))?;
    if p.status == "accepted" {
        return Ok(("(already accepted)", p.name));
    }
    let label = commit_proposal(&store, &cfg, &ws, &p)?;
    ws.set_status(id, "accepted").map_err(|e| Error::Store(format!("accept: {e}")))?;
    Ok((label, p.name))
}
