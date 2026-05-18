use std::path::Path;

use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

pub fn run(project: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let h = Hierarchy::load(&store)?;

    if h.is_empty() {
        eprintln!("(empty project — add a book with `inkhaven add book <title>`)");
        return Ok(());
    }

    let roots = h.children_of(None);
    for (i, root) in roots.iter().enumerate() {
        let last = i + 1 == roots.len();
        print_node(&h, root, "", last);
    }
    Ok(())
}

fn print_node(h: &Hierarchy, node: &Node, indent: &str, last: bool) {
    let branch = if last { "└─ " } else { "├─ " };
    let leaf_marker = match node.kind {
        crate::store::NodeKind::Paragraph => "¶ ",
        _ => "",
    };
    println!(
        "{indent}{branch}{leaf}{title}  [{kind}, {slug}]",
        leaf = leaf_marker,
        title = node.title,
        kind = node.kind.as_str(),
        slug = node.slug,
    );

    let child_indent = format!("{indent}{}", if last { "   " } else { "│  " });
    let children = h.children_of(Some(node.id));
    for (i, c) in children.iter().enumerate() {
        let cl = i + 1 == children.len();
        print_node(h, c, &child_indent, cl);
    }
}
