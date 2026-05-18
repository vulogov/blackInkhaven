use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

pub fn run(
    project: &Path,
    kind: NodeKind,
    title: &str,
    parent_path: Option<&str>,
    slug_override: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

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

    let node = store.create_node(&cfg, &hierarchy, kind, title, parent, slug_override)?;

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
