use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

pub fn run(project: &Path, node_path: &str, yes: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;

    let node = h
        .find_by_path(node_path)
        .ok_or_else(|| Error::Store(format!("node not found: `{node_path}`")))?;
    if node.protected {
        return Err(Error::Store(format!(
            "`{}` is a system book — it can't be deleted",
            node.title
        )));
    }
    if let Some(help_anc) = h.ancestors(node).into_iter().find(|a| {
        a.protected && a.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_HELP)
    }) {
        return Err(Error::Store(format!(
            "`{}` lives inside the read-only Help book (`{}`)",
            node.title, help_anc.title
        )));
    }
    let ids = h.collect_subtree(node.id);
    let descendant_count = ids.len().saturating_sub(1);

    if !yes {
        return Err(Error::Store(format!(
            "would delete `{}` ({}) and {} descendant(s); pass --yes to confirm",
            node_path,
            node.kind.as_str(),
            descendant_count
        )));
    }

    let fs_rel = match node.kind {
        NodeKind::Paragraph => node
            .file
            .as_ref()
            .map(std::path::PathBuf::from)
            .unwrap_or_default(),
        _ => h.fs_path(node, &layout),
    };

    store.delete_subtree(&fs_rel, &ids)?;

    eprintln!(
        "deleted {} `{}` ({} other node{} removed)",
        node.kind.as_str(),
        node.title,
        descendant_count,
        if descendant_count == 1 { "" } else { "s" }
    );
    Ok(())
}
