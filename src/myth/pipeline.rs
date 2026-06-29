//! MYTH-1 (M-P4 / M-P6) — the deterministic scan pipeline. Refreshes the declared
//! inventory from the Mythology book, then per user-book chapter counts symbol
//! occurrences (density) and collects explicit motif occurrences. Zero-AI;
//! content-hash lazy. Reusing the CHAR-1 chapter walk.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use anyhow::Result;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::NodeKind;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::store::MythStore;
use super::{read_archetypes, read_motifs, read_symbols};

fn chapters_of<'a>(h: &'a Hierarchy, book: &Node) -> Vec<&'a Node> {
    h.children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .collect()
}

/// A chapter's prose paragraphs as `(node, stripped_text)` (Jinja excluded).
fn chapter_prose<'a>(layout: &ProjectLayout, h: &'a Hierarchy, chapter_id: uuid::Uuid) -> Vec<(&'a Node, String)> {
    let mut out = Vec::new();
    for id in h.collect_subtree(chapter_id) {
        let Some(p) = h.get(id) else { continue };
        if p.kind != NodeKind::Paragraph || p.content_type.as_deref() == Some("jinja") {
            continue;
        }
        if let Some(rel) = p.file.as_ref() {
            if let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) {
                out.push((p, crate::audiobook::typst_to_plain(&raw)));
            }
        }
    }
    out
}

fn hash_str(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Count occurrences of a vocabulary token in lowercased text. Single tokens
/// match on whole-word boundaries; multi-word phrases match as substrings.
fn count_token(text_lc: &str, token: &str) -> u32 {
    let t = token.trim().to_lowercase();
    if t.is_empty() {
        return 0;
    }
    if t.contains(' ') {
        // Overlapping-safe substring count.
        let mut n = 0u32;
        let mut from = 0usize;
        while let Some(pos) = text_lc[from..].find(&t) {
            n += 1;
            from += pos + t.len();
            if from >= text_lc.len() {
                break;
            }
        }
        n
    } else {
        text_lc.split(|c: char| !c.is_alphanumeric()).filter(|w| *w == t).count() as u32
    }
}

/// Refresh the declared inventory (symbols/motifs/archetypes) into the store,
/// keyed by the user book. Rebuilds the highlight vocab as a side effect.
pub(crate) fn refresh_inventory(
    store: &MythStore,
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &Node,
) -> Result<()> {
    let symbols = read_symbols(h, layout);
    let motifs = read_motifs(h, layout);
    let archetypes = read_archetypes(h, layout);
    store.replace_inventory(&book.slug, &symbols, &motifs, &archetypes)
}

/// Per-chapter symbol density scan. Returns the number of (symbol, chapter) cells
/// written. Content-hash lazy: an unchanged chapter is skipped unless forced.
pub(crate) fn run_density_scan(
    store: &MythStore,
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &Node,
    force: bool,
) -> Result<usize> {
    let symbols = store.symbols(&book.slug)?;
    if symbols.is_empty() {
        return Ok(0);
    }
    let now = chrono::Utc::now().to_rfc3339();
    let mut count = 0;
    for (idx, ch) in chapters_of(h, book).iter().enumerate() {
        let ord = (idx + 1) as u32;
        let text: String = chapter_prose(layout, h, ch.id)
            .iter()
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let lc = text.to_lowercase();
        let ph = hash_str(&lc);
        // Lazy: skip if this chapter's first symbol density row already matches
        // the prose hash (cheap proxy — the scan is per-chapter atomic).
        if !force && chapter_unchanged(store, &book.slug, ord, ph) {
            continue;
        }
        store.clear_density_chapter(&book.slug, ord)?;
        for s in &symbols {
            let total: u32 = s.vocabulary.iter().map(|v| count_token(&lc, v)).sum();
            store.upsert_density(&book.slug, &s.para_id, ord, total, ph, &now)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Whether a chapter's stored density rows already reflect `prose_hash` (so the
/// re-scan can be skipped). Reads one symbol's row for the chapter.
fn chapter_unchanged(store: &MythStore, book_slug: &str, chapter_ord: u32, prose_hash: u64) -> bool {
    // We don't expose a direct hash getter; cheap re-derivation: if any density
    // row exists for this chapter it carries the hash. The store keeps prose_hash
    // per row, so compare via the dedicated helper.
    store
        .density_chapter_hash(book_slug, chapter_ord)
        .ok()
        .flatten()
        .map(|h| h == prose_hash)
        .unwrap_or(false)
}

/// Collect explicit motif occurrences: a prose paragraph tagged `para:myth-motif`
/// that also carries a tag matching a declared motif's name. Replaces the
/// explicit-source rows. Returns the number collected.
pub(crate) fn collect_explicit_motifs(
    store: &MythStore,
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &Node,
) -> Result<usize> {
    let motifs = store.motifs(&book.slug)?;
    if motifs.is_empty() {
        return Ok(0);
    }
    // name (lowercased) → motif para_id
    let by_name: HashMap<String, String> =
        motifs.iter().map(|m| (m.name.trim().to_lowercase(), m.para_id.clone())).collect();
    store.clear_motif_occurrences(&book.slug, "explicit_tag")?;
    let now = chrono::Utc::now().to_rfc3339();
    let _ = layout;
    let mut count = 0;
    for (idx, ch) in chapters_of(h, book).iter().enumerate() {
        let ord = (idx + 1) as u32;
        for id in h.collect_subtree(ch.id) {
            let Some(p) = h.get(id) else { continue };
            if p.kind != NodeKind::Paragraph || !p.tags.iter().any(|t| t == "para:myth-motif") {
                continue;
            }
            // Find a tag naming a declared motif.
            if let Some(mp) = p.tags.iter().find_map(|t| by_name.get(&t.trim().to_lowercase())) {
                store.upsert_motif_occurrence(&book.slug, mp, ord, &id.to_string(), "explicit_tag", &now)?;
                count += 1;
            }
        }
    }
    Ok(count)
}

/// The user book's chapter count.
pub(super) fn chapter_count(h: &Hierarchy, book: &Node) -> u32 {
    chapters_of(h, book).len() as u32
}

/// Distinct chapter ordinals where `name` is mentioned (whole-word, or substring
/// for a multi-word name) — the archetype-presence scan (no continuity index
/// exists; this mirrors CHAR-1's per-chapter mention approach).
pub(super) fn character_mention_chapters(
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &Node,
    name: &str,
) -> Vec<u32> {
    let nm = name.trim().to_lowercase();
    if nm.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (idx, ch) in chapters_of(h, book).iter().enumerate() {
        let ord = (idx + 1) as u32;
        let lc: String = chapter_prose(layout, h, ch.id)
            .iter()
            .map(|(_, t)| t.to_lowercase())
            .collect::<Vec<_>>()
            .join("\n");
        let hit = if nm.contains(' ') {
            lc.contains(&nm)
        } else {
            lc.split(|c: char| !c.is_alphanumeric()).any(|w| w == nm)
        };
        if hit {
            out.push(ord);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_token_whole_word_and_phrase() {
        let lc = "the raven watched. ravens gathered. a white rose, a white rose bloomed.".to_lowercase();
        assert_eq!(count_token(&lc, "raven"), 1); // not "ravens"
        assert_eq!(count_token(&lc, "ravens"), 1);
        assert_eq!(count_token(&lc, "white rose"), 2);
        assert_eq!(count_token(&lc, "crow"), 0);
    }
}
