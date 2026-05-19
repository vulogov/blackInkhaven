//! `inkhaven import-help --documents-directory <PATH>`
//!
//! Walks the given directory and imports it under the Help system book:
//! subdirectories become chapters / subchapters / (flattened) and files
//! become paragraphs, mirroring the TUI's F3 directory-import semantics.
//!
//! Depth mapping (rooted at the Help book):
//!   Help (book)
//!   └── top-level dir → Chapter
//!       └── nested dir → Subchapter
//!           └── deeper dirs → flattened: their files become paragraphs of
//!               the enclosing subchapter (unless `unbounded_subchapters` is
//!               on, in which case Subchapter nests indefinitely).
//!
//! Files at the source root land directly under Help as paragraphs.
//!
//! Hidden entries (dotfiles) are skipped. The Help book's read-only flag
//! lives in the TUI editor — the store accepts writes here so importing
//! works.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::{InsertPosition, Store, SYSTEM_TAG_HELP};

#[derive(Default)]
struct Counts {
    branches: usize,
    paragraphs: usize,
}

pub fn run(project: &Path, documents_dir: &Path) -> Result<()> {
    if !documents_dir.is_dir() {
        return Err(Error::Store(format!(
            "--documents-directory `{}` is not a directory",
            documents_dir.display()
        )));
    }

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;

    // Locate the Help system book. `ensure_system_books` (called inside
    // `Store::open`) guarantees it exists, but we still go through the
    // hierarchy lookup so a hypothetical migration that loses the tag
    // surfaces a clear error rather than a silent failure.
    let hierarchy = Hierarchy::load(&store)?;
    let help = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_HELP)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(
                "Help system book not found — re-open the project to seed it".into(),
            )
        })?;

    // Wipe Help's existing contents before importing so repeated runs
    // don't accumulate stale chapters or duplicate paragraphs. The Help
    // book itself stays (it's a system book and other features depend on
    // its tagged identity); only its descendants are removed.
    let wiped = wipe_help_contents(&store, &hierarchy, help.id)?;
    if wiped > 0 {
        eprintln!("cleared {wiped} existing item(s) from Help");
    }
    // Reload the hierarchy so subsequent `import_*` paths see the now-empty
    // Help book instead of the stale snapshot they'd otherwise inherit
    // from the in-memory `hierarchy` we captured above.
    let _ = Hierarchy::load(&store)?;

    let mut counts = Counts::default();

    // Children of the source directory become either branches or paragraphs
    // directly under Help. Sorted dirs-first / alphabetical so the output
    // is deterministic.
    let entries = read_sorted_children(documents_dir);
    for entry in entries {
        let res = if entry.is_dir() {
            import_dir(&store, &cfg, &entry, help.id, &mut counts)
        } else {
            import_file(&store, &cfg, &entry, help.id, &mut counts)
        };
        if let Err(e) = res {
            eprintln!(
                "warning: {}: {e} — continuing with remaining entries",
                entry.display()
            );
        }
    }

    eprintln!(
        "imported {} branch(es) and {} paragraph(s) into Help from {}",
        counts.branches,
        counts.paragraphs,
        documents_dir.display()
    );
    Ok(())
}

/// Remove every direct child of the Help book (along with their entire
/// subtrees) so a fresh `import-help` produces a clean view. The Help book
/// itself is preserved — it's a system book whose identity (tag, protected
/// flag, fixed root position) other features rely on.
///
/// Returns the count of top-level subtrees that were wiped. Per-child
/// failures are logged to stderr but do not abort the wipe — partial
/// cleanup is better than aborting and leaving the project half-imported.
fn wipe_help_contents(store: &Store, hierarchy: &Hierarchy, help_id: Uuid) -> Result<usize> {
    let layout = store.project_root().to_path_buf();
    let direct_children: Vec<Uuid> = hierarchy
        .iter()
        .filter(|n| n.parent_id == Some(help_id))
        .map(|n| n.id)
        .collect();
    let mut wiped = 0usize;
    for child_id in direct_children {
        let Some(node) = hierarchy.get(child_id) else {
            continue;
        };
        // The subtree to delete: this child plus every descendant.
        let ids = hierarchy.collect_subtree(child_id);
        let fs_rel = match node.kind {
            NodeKind::Paragraph => node
                .file
                .as_ref()
                .map(std::path::PathBuf::from)
                .unwrap_or_default(),
            _ => {
                // Branch fs path: walk the hierarchy's `fs_path` against the
                // stored layout. We can't borrow `ProjectLayout` directly
                // from the store, so we reconstruct the relative path from
                // its absolute form below.
                let abs = layout.join(
                    hierarchy.fs_path(node, &crate::project::ProjectLayout::new(&layout)),
                );
                abs.strip_prefix(&layout)
                    .unwrap_or(&abs)
                    .to_path_buf()
            }
        };
        if let Err(e) = store.delete_subtree(&fs_rel, &ids) {
            eprintln!(
                "warning: couldn't fully wipe `{}` from Help: {e}",
                node.title
            );
            continue;
        }
        wiped += 1;
    }
    Ok(wiped)
}

/// Create a branch for `source` under `parent_id` and recurse into its
/// children. The branch's kind is determined by the parent's kind; when we
/// run out of legal depth we flatten files into the parent instead.
fn import_dir(
    store: &Store,
    cfg: &Config,
    source: &Path,
    parent_id: Uuid,
    counts: &mut Counts,
) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let parent = hierarchy
        .get(parent_id)
        .cloned()
        .ok_or_else(|| Error::Store(format!("import: parent {parent_id} vanished")))?;

    let kind = match next_branch_kind(&parent, cfg) {
        Some(k) => k,
        None => {
            // Depth limit hit — flatten all remaining files into `parent`.
            return flatten_files_into(store, cfg, source, parent_id, counts);
        }
    };

    let title = derive_branch_title(source);
    let created = store.create_node(
        cfg,
        &hierarchy,
        kind,
        &title,
        Some(&parent),
        None,
        InsertPosition::End,
    )?;
    counts.branches += 1;

    let children = read_sorted_children(source);
    let mut first_err: Option<Error> = None;
    for child in children {
        let res = if child.is_dir() {
            import_dir(store, cfg, &child, created.id, counts)
        } else {
            import_file(store, cfg, &child, created.id, counts)
        };
        if let Err(e) = res {
            eprintln!(
                "warning: {}: {e} — continuing with remaining entries",
                child.display()
            );
            if first_err.is_none() {
                first_err = Some(e);
            }
        }
    }
    match first_err {
        None => Ok(()),
        Some(e) => Err(e),
    }
}

fn import_file(
    store: &Store,
    cfg: &Config,
    file: &Path,
    parent_id: Uuid,
    counts: &mut Counts,
) -> Result<()> {
    let title = derive_paragraph_title(file);
    let bytes = std::fs::read(file).map_err(Error::Io)?;
    let hierarchy = Hierarchy::load(store)?;
    let parent = hierarchy
        .get(parent_id)
        .cloned()
        .ok_or_else(|| Error::Store(format!("import: parent {parent_id} vanished")))?;
    let created = store.create_node(
        cfg,
        &hierarchy,
        NodeKind::Paragraph,
        &title,
        Some(&parent),
        None,
        InsertPosition::End,
    )?;
    if let Some(rel) = &created.file {
        let abs = layout_root(store).join(rel);
        std::fs::write(&abs, &bytes).map_err(Error::Io)?;
        let mut node = created.clone();
        store.update_paragraph_content(&mut node, &bytes)?;
    }
    counts.paragraphs += 1;
    Ok(())
}

/// Walk `source` recursively and import every regular file as a paragraph
/// under `parent_id`. Used when we've hit the depth limit and can no longer
/// create deeper branches.
fn flatten_files_into(
    store: &Store,
    cfg: &Config,
    source: &Path,
    parent_id: Uuid,
    counts: &mut Counts,
) -> Result<()> {
    let mut first_err: Option<Error> = None;
    for entry in walkdir::WalkDir::new(source)
        .sort_by_file_name()
        .follow_links(false)
    {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("warning: walkdir: {e}");
                if first_err.is_none() {
                    first_err = Some(Error::Store(format!("walkdir: {e}")));
                }
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_str().unwrap_or("");
        if name.starts_with('.') {
            continue;
        }
        if let Err(e) = import_file(store, cfg, entry.path(), parent_id, counts) {
            eprintln!(
                "warning: {}: {e} — continuing with remaining files",
                entry.path().display()
            );
            if first_err.is_none() {
                first_err = Some(e);
            }
        }
    }
    match first_err {
        None => Ok(()),
        Some(e) => Err(e),
    }
}

fn next_branch_kind(parent: &Node, cfg: &Config) -> Option<NodeKind> {
    match parent.kind {
        NodeKind::Book => Some(NodeKind::Chapter),
        NodeKind::Chapter => Some(NodeKind::Subchapter),
        NodeKind::Subchapter => {
            if cfg.hierarchy.unbounded_subchapters {
                Some(NodeKind::Subchapter)
            } else {
                None
            }
        }
        NodeKind::Paragraph => None,
    }
}

fn read_sorted_children(source: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(source) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = rd
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|s| !s.starts_with('.'))
                .unwrap_or(true)
        })
        .collect();
    entries.sort_by(|a, b| {
        let a_dir = a.path().is_dir();
        let b_dir = b.path().is_dir();
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });
    entries.into_iter().map(|e| e.path()).collect()
}

fn derive_branch_title(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("imported");
    prettify_segment(name)
}

fn derive_paragraph_title(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported");
    prettify_segment(stem)
}

fn prettify_segment(raw: &str) -> String {
    let pretty: String = raw
        .replace('_', " ")
        .replace('-', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(c).collect::<String>(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if pretty.trim().is_empty() {
        raw.to_string()
    } else {
        pretty
    }
}

/// Accessor for the Store's project root. Lives here because the field is
/// private; we go through a tiny helper rather than expose it everywhere.
fn layout_root(store: &Store) -> PathBuf {
    store.project_root().to_path_buf()
}
