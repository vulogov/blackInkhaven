//! OUTLINE-1 (O-P6) — `inkhaven paragraph copy|move`: cross-parent paragraph
//! relocation from the terminal, the CLI counterpart to the Outline/Tree
//! `y`/`m`/`f` clipboard. `src` and `dest` are slash-separated slug paths (as
//! printed by `inkhaven outline`). The paragraph lands as the last child of
//! `dest`'s effective parent — INTO `dest` when it's a branch, alongside it
//! when it's itself a paragraph.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

pub fn copy(project: &Path, src_path: &str, dest_path: &str) -> Result<()> {
    run(project, src_path, dest_path, false)
}

pub fn move_(project: &Path, src_path: &str, dest_path: &str) -> Result<()> {
    run(project, src_path, dest_path, true)
}

fn run(project: &Path, src_path: &str, dest_path: &str, is_move: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;

    let src = h
        .find_by_path(src_path)
        .ok_or_else(|| Error::Store(format!("source not found: `{src_path}`")))?;
    if src.kind != NodeKind::Paragraph {
        return Err(Error::Store(format!(
            "`{src_path}` is a {} — only paragraphs can be copied/moved",
            src.kind.as_str()
        )));
    }
    let src_id = src.id;

    let dest = h
        .find_by_path(dest_path)
        .ok_or_else(|| Error::Store(format!("destination not found: `{dest_path}`")))?;
    // Effective parent: append INTO a branch, or alongside a paragraph.
    let dest_parent = if dest.kind == NodeKind::Paragraph {
        dest.parent_id
    } else {
        Some(dest.id)
    };

    if is_move {
        store.move_node_to_parent(&cfg, &h, src_id, dest_parent)?;
        eprintln!("moved `{src_path}` → under `{dest_path}`");
    } else {
        let new_id = store.copy_paragraph_to_parent(&cfg, &h, src_id, dest_parent)?;
        eprintln!(
            "copied `{src_path}` → under `{dest_path}` (new uuid {})",
            new_id.simple()
        );
    }
    Ok(())
}
