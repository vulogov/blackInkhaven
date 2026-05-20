use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tracing::warn;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
#[allow(unused_imports)]
use crate::store::InsertPosition;

pub fn run(project: &Path, prune: bool, adopt: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;

    let mut updated = 0usize;
    let mut unchanged = 0usize;
    let mut missing_ids: Vec<Uuid> = Vec::new();
    let mut known_paths: HashSet<PathBuf> = HashSet::new();

    for node in h.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = node.file.as_ref() else {
            continue;
        };
        let abs = layout.root.join(rel);
        known_paths.insert(abs.clone());

        if !abs.is_file() {
            warn!(
                node = %node.slug,
                file = %abs.display(),
                "record points at a missing file"
            );
            missing_ids.push(node.id);
            continue;
        }

        let bytes = std::fs::read(&abs).map_err(Error::Io)?;
        let current = store.get_content(node.id)?;
        if current.as_deref() == Some(bytes.as_slice()) {
            unchanged += 1;
            continue;
        }

        let mut node = node.clone();
        store.update_paragraph_content(&mut node, &bytes)?;
        updated += 1;
    }

    let orphans = find_orphans(&layout, &known_paths)?;

    let pruned = if prune {
        prune_missing(&store, &missing_ids)?
    } else {
        0
    };
    let adopted = if adopt {
        adopt_orphans(&store, &h, &layout, &orphans)?
    } else {
        0
    };

    store.sync()?;

    eprintln!(
        "reindex: {updated} updated, {unchanged} unchanged, {} missing, {} orphan(s)",
        missing_ids.len(),
        orphans.len()
    );
    if prune {
        eprintln!("  pruned {pruned} missing record(s) from the store");
    } else if !missing_ids.is_empty() {
        eprintln!("  (re-run with --prune to remove records for missing files)");
    }
    if adopt {
        eprintln!("  adopted {adopted} orphan .typ file(s) into the hierarchy");
    } else if !orphans.is_empty() {
        eprintln!(
            "  (re-run with --adopt to auto-register orphan .typ files under their fs parent)"
        );
        for o in &orphans {
            eprintln!("    orphan: {}", o.display());
        }
    }
    Ok(())
}

fn find_orphans(layout: &ProjectLayout, known: &HashSet<PathBuf>) -> Result<Vec<PathBuf>> {
    let books = layout.books_path();
    if !books.is_dir() {
        return Ok(Vec::new());
    }

    let mut orphans = Vec::new();
    for entry in walkdir::WalkDir::new(&books).follow_links(false) {
        let entry = entry.map_err(|e| Error::Store(format!("walkdir: {e}")))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "typ") && !known.contains(path) {
            orphans.push(path.to_path_buf());
        }
    }
    orphans.sort();
    Ok(orphans)
}

fn prune_missing(store: &Store, ids: &[Uuid]) -> Result<usize> {
    let mut n = 0;
    for id in ids {
        if let Err(e) = store
            .raw()
            .delete_document(*id)
            .map_err(|e| Error::Store(format!("delete_document: {e}")))
        {
            warn!(uuid = %id, "prune failed: {e}");
        } else {
            n += 1;
        }
    }
    Ok(n)
}

/// Register every orphan `.typ` file as a paragraph under the deepest branch
/// node whose filesystem path matches the orphan's parent directory.
fn adopt_orphans(
    store: &Store,
    hierarchy: &Hierarchy,
    layout: &ProjectLayout,
    orphans: &[PathBuf],
) -> Result<usize> {
    // Build a map of (fs path → branch UUID) so each orphan can find its
    // logical parent in O(1) after a single pass through the hierarchy.
    let mut branches: HashMap<PathBuf, Uuid> = HashMap::new();
    for node in hierarchy.iter() {
        if node.kind == NodeKind::Paragraph {
            continue;
        }
        let abs = layout.root.join(hierarchy.fs_path(node, layout));
        branches.insert(abs, node.id);
    }

    // Track next available order per parent — orders persist across this
    // call so multiple orphans under the same branch don't collide.
    let mut next_order: HashMap<Uuid, u32> = HashMap::new();
    for node in hierarchy.iter() {
        let pid = node.parent_id.unwrap_or_default();
        let entry = next_order.entry(pid).or_insert(0);
        *entry = (*entry).max(node.order);
    }

    let mut adopted = 0usize;
    for orphan_abs in orphans {
        let parent_dir = match orphan_abs.parent() {
            Some(p) => p.to_path_buf(),
            None => {
                warn!(orphan = %orphan_abs.display(), "orphan has no parent dir; skipping");
                continue;
            }
        };
        let Some(&parent_id) = branches.get(&parent_dir) else {
            warn!(orphan = %orphan_abs.display(), "no branch in hierarchy matches the orphan's parent dir; skipping");
            continue;
        };
        let parent_node = hierarchy
            .get(parent_id)
            .expect("parent_id came from hierarchy");

        let (title, slug) = derive_title_and_slug(orphan_abs);

        // Make slug unique among existing siblings.
        let siblings = hierarchy.children_of(Some(parent_id));
        let mut final_slug = slug.clone();
        let mut n = 2;
        while siblings.iter().any(|s| s.slug == final_slug) {
            final_slug = format!("{slug}-{n}");
            n += 1;
        }

        let order_entry = next_order.entry(parent_id).or_insert(0);
        *order_entry += 1;
        let order = *order_entry;

        let path_chain = {
            let mut chain: Vec<String> = hierarchy
                .ancestors(parent_node)
                .into_iter()
                .map(|a| a.slug.clone())
                .collect();
            chain.push(parent_node.slug.clone());
            chain
        };

        let bytes = std::fs::read(orphan_abs).map_err(Error::Io)?;
        let word_count = std::str::from_utf8(&bytes)
            .map(|s| s.split_whitespace().count() as u64)
            .unwrap_or(0);
        let rel = orphan_abs
            .strip_prefix(&layout.root)
            .unwrap_or(orphan_abs)
            .to_string_lossy()
            .into_owned();

        let mut node = Node {
            id: Uuid::nil(),
            kind: NodeKind::Paragraph,
            title,
            slug: final_slug,
            path: path_chain,
            parent_id: Some(parent_id),
            order,
            file: Some(rel),
            word_count,
            modified_at: chrono::Utc::now(),
            protected: false,
            system_tag: None,
            image_ext: None,
            image_caption: None,
            image_alt: None,
            content_type: None,
            status: None,
        };

        store.put_node(&mut node, &bytes)?;
        adopted += 1;
    }
    Ok(adopted)
}

fn derive_title_and_slug(path: &Path) -> (String, String) {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("orphan");
    // Strip a leading `NN-` order prefix if present.
    let core = stem
        .split_once('-')
        .filter(|(prefix, _)| prefix.chars().all(|c| c.is_ascii_digit()))
        .map(|(_, rest)| rest)
        .unwrap_or(stem);
    let slug = slug::slugify(core);
    let title = core
        .replace('-', " ")
        .replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let title = if title.is_empty() {
        "Orphan".into()
    } else {
        title
    };
    let slug = if slug.is_empty() { "orphan".into() } else { slug };
    (title, slug)
}
