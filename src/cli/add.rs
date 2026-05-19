use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::InsertPosition;

pub fn run(
    project: &Path,
    kind: NodeKind,
    title: &str,
    parent_path: Option<&str>,
    slug_override: Option<&str>,
    after_path: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    // --after takes precedence and implies the parent.
    let (parent, position) = if let Some(after) = after_path {
        let anchor = hierarchy
            .find_by_path(after)
            .ok_or_else(|| Error::Store(format!("--after anchor not found: `{after}`")))?;
        if anchor.kind != kind {
            return Err(Error::Store(format!(
                "--after expects a sibling of kind `{}`, got `{}`",
                kind.as_str(),
                anchor.kind.as_str()
            )));
        }
        let parent_node = anchor
            .parent_id
            .and_then(|pid| hierarchy.get(pid));
        (parent_node, InsertPosition::After(anchor.id))
    } else {
        let parent = match (kind, parent_path) {
            (NodeKind::Book, None) => None,
            (NodeKind::Book, Some(_)) => {
                return Err(Error::Store(
                    "books are root nodes; do not pass --parent".into(),
                ));
            }
            (_, None) => {
                return Err(Error::Store(format!(
                    "--parent is required when adding a {}",
                    kind.as_str()
                )));
            }
            (_, Some(path)) => Some(
                hierarchy
                    .find_by_path(path)
                    .ok_or_else(|| Error::Store(format!("parent not found: `{path}`")))?,
            ),
        };
        // User-added Books at root slot ABOVE the system block (Notes,
        // Research, Prompts, Places, Characters, Help) — same behaviour as
        // the TUI's Tree-pane `B` shortcut.
        let position = if kind == NodeKind::Book && parent.is_none() {
            match hierarchy.iter().find(|n| {
                n.kind == NodeKind::Book
                    && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_NOTES)
            }) {
                Some(notes) => InsertPosition::Before(notes.id),
                None => InsertPosition::End,
            }
        } else {
            InsertPosition::End
        };
        (parent, position)
    };

    let node = store.create_node(
        &cfg,
        &hierarchy,
        kind,
        title,
        parent,
        slug_override,
        position,
    )?;

    let display_path = {
        let mut parts: Vec<String> = node.path.clone();
        parts.push(node.slug.clone());
        parts.join("/")
    };

    let rel = if let Some(file) = &node.file {
        file.clone()
    } else {
        // Branch — reconstruct directory path
        let mut p = std::path::PathBuf::from(crate::project::BOOKS_DIR);
        for slug in &node.path {
            // Walk hierarchy to get fs_name with order prefix. The freshly
            // loaded snapshot above doesn't include the new node yet, but its
            // ancestors are all there.
            if let Some(ancestor) = hierarchy
                .iter()
                .find(|n| n.slug == *slug && n.kind != NodeKind::Paragraph)
            {
                p.push(ancestor.fs_name());
            } else {
                p.push(slug);
            }
        }
        p.push(node.fs_name());
        p.to_string_lossy().into_owned()
    };

    eprintln!(
        "Added {} `{}` at {}",
        kind.as_str(),
        node.title,
        display_path
    );
    eprintln!("  uuid: {}", node.id);
    eprintln!("  file: {}", rel);
    Ok(())
}
