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

    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let h = Hierarchy::load(&store)?;

    if h.is_empty() {
        eprintln!("(empty project — add a book with `inkhaven add book <title>`)");
        return Ok(());
    }

    let roots = h.children_of(None);
    for (i, root) in roots.iter().enumerate() {
        let last = i + 1 == roots.len();
        print_node(&h, root, "", last, "");
    }
    eprintln!(
        "\nThe bracketed path is a slug path — pass it to commands that take \
         `--path` (e.g. `inkhaven inner-socrates check --path <path>`)."
    );
    Ok(())
}

fn print_node(h: &Hierarchy, node: &Node, indent: &str, last: bool, path_prefix: &str) {
    let branch = if last { "└─ " } else { "├─ " };
    let leaf_marker = match node.kind {
        crate::store::NodeKind::Paragraph => "¶ ",
        _ => "",
    };
    // The full slug path from the root — directly copy-pasteable into `--path`.
    let full_path = if path_prefix.is_empty() {
        node.slug.clone()
    } else {
        format!("{path_prefix}/{}", node.slug)
    };
    println!(
        "{indent}{branch}{leaf}{title}  [{kind}, {full_path}]",
        leaf = leaf_marker,
        title = node.title,
        kind = node.kind.as_str(),
    );

    let child_indent = format!("{indent}{}", if last { "   " } else { "│  " });
    let children = h.children_of(Some(node.id));
    for (i, c) in children.iter().enumerate() {
        let cl = i + 1 == children.len();
        print_node(h, c, &child_indent, cl, &full_path);
    }
}
