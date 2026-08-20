//! LOCI — `inkhaven index-locorum`. Harvests every `@key[locus]` citation across
//! the manuscript, resolves each key to its source title from the Sources book,
//! and renders an Index Locorum (Markdown / Typst / JSON). The scholarly apparatus
//! of cited passages, for theology / classics / law.

use std::collections::HashMap;
use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::index_locorum::{self, LocusCitation};
use crate::project::ProjectLayout;
use crate::sources::BibEntry;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::{Store, SYSTEM_TAG_SOURCES};

pub fn run(
    project: &Path,
    book_name: Option<&str>,
    format: &str,
    out: Option<&Path>,
    strict: bool,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;

    let cites = gather_citations(&layout, &h, book_name)?;
    let (titles, declared) = collect_titles_and_schemes(&layout, &h);
    // Resolve a reference scheme for every cited key (built-in for scripture keys,
    // configured for the rest) and validate each locus against it.
    let keys: Vec<String> = {
        let mut ks: Vec<String> = cites.iter().map(|c| c.key.clone()).collect();
        ks.sort();
        ks.dedup();
        ks
    };
    let (schemes, scheme_errs) =
        index_locorum::resolve_schemes(&cfg.sources.ref_schemes, &declared, &keys);
    for (name, err) in &scheme_errs {
        eprintln!("index-locorum: reference scheme `{name}` has an invalid pattern: {err}");
    }
    let entries = index_locorum::build(&cites, &titles, &schemes);
    let heading = index_locorum::heading_for_language(&cfg.language);

    let rendered = match format.to_ascii_lowercase().as_str() {
        "md" | "markdown" => index_locorum::render_md(&entries, heading),
        "typ" | "typst" => index_locorum::render_typst(&entries, heading),
        "json" => index_locorum::render_json(&entries),
        other => {
            return Err(Error::Config(format!(
                "index-locorum: unknown --format `{other}` (use `md`, `typst`, or `json`)"
            )))
        }
    };

    match out {
        Some(p) => {
            std::fs::write(p, rendered.as_bytes()).map_err(Error::Io)?;
            let loci: usize = entries.iter().map(|e| e.loci.len()).sum();
            println!(
                "index-locorum: {} source(s), {loci} locus(es) → {}",
                entries.len(),
                p.display()
            );
        }
        None => print!("{rendered}"),
    }

    // Report malformed loci — a reference that does not match its source's scheme.
    let bad = index_locorum::malformed(&entries, &schemes);
    if !bad.is_empty() {
        eprintln!("\nindex-locorum: {} malformed locus(es):", bad.len());
        for m in &bad {
            eprintln!("  ⚠ @{}[{}] — expected {} ({})", m.key, m.locus, m.expected, m.title);
        }
        if strict {
            return Err(Error::Config(format!(
                "index-locorum: {} malformed locus(es) (--strict)",
                bad.len()
            )));
        }
    }
    Ok(())
}

/// Harvest `@key[locus]` citations from every manuscript paragraph, tagged with
/// its enclosing chapter. Reads **raw** prose (not flattened) so the `[…]`
/// supplement survives.
/// `pub(crate)` so the read-only `ink.locorum.*` Bund words can harvest the
/// same citations the CLI does (disk reads only — no write, no LLM).
pub(crate) fn gather_citations(
    layout: &ProjectLayout,
    h: &Hierarchy,
    book_name: Option<&str>,
) -> Result<Vec<LocusCitation>> {
    let books: Vec<&Node> = match book_name {
        Some(_) => vec![super::resolve_user_book(h, book_name, "index-locorum").map_err(Error::Store)?],
        None => h
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .collect(),
    };
    let mut cites = Vec::new();
    for book in &books {
        for chapter in h.children_of(Some(book.id)).into_iter().filter(|n| n.kind == NodeKind::Chapter) {
            for id in h.collect_subtree(chapter.id) {
                let Some(n) = h.get(id) else { continue };
                if n.kind != NodeKind::Paragraph {
                    continue;
                }
                let Some(rel) = n.file.as_ref() else { continue };
                let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else { continue };
                for (key, locus) in crate::sources::extract_cite_loci(&raw) {
                    cites.push(LocusCitation { key, locus, chapter: chapter.title.clone() });
                }
            }
        }
    }
    Ok(cites)
}

/// Map each Sources-book cite key to its title (for the index headings) and to the
/// reference-scheme name it declared (for validation). Reads all valid `BibEntry`
/// paragraphs regardless of `sources.all` scoping — the index locorum spans
/// whatever the manuscript actually cites.
pub(crate) fn collect_titles_and_schemes(
    layout: &ProjectLayout,
    h: &Hierarchy,
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut titles = HashMap::new();
    let mut declared = HashMap::new();
    let Some(sources_book) =
        h.iter().find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_SOURCES))
    else {
        return (titles, declared);
    };
    for id in h.collect_subtree(sources_book.id) {
        let Some(n) = h.get(id) else { continue };
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = &n.file else { continue };
        let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else { continue };
        let body = crate::typst_prose::strip_leading_heading(&raw);
        if let Some(e) = BibEntry::from_hjson(&body) {
            if e.is_valid() && !e.title.trim().is_empty() {
                titles.insert(e.key.clone(), e.title.clone());
            }
            if let Some(scheme) = e.scheme.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                declared.insert(e.key.clone(), scheme.to_string());
            }
        }
    }
    (titles, declared)
}
