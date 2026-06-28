//! OUTLINE-1 (O-P6) — `inkhaven outline`: print the manuscript outline as an
//! indented text tree. The terminal counterpart to the full-screen Outline
//! pane (`Ctrl+2`). Each row shows the title, kind, and (for paragraphs) the
//! status + word count, plus the slash-separated slug path — which is exactly
//! what `inkhaven paragraph copy|move` takes as its `src` / `dest` arguments.

use std::path::Path;

use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

/// Render the manuscript outline as an indented text tree. Each node yields two
/// lines: a titled/glyphed row, then its slash-separated slug path (the path
/// the `paragraph` subcommands and `ink.outline.*` take). `filter` narrows to
/// rows whose title or slug-path contains the needle (case-insensitive). Shared
/// by the `inkhaven outline` CLI and the `ink.outline.print` Bund word.
pub fn render(h: &Hierarchy, filter: Option<&str>) -> String {
    let needle = filter.map(|s| s.to_lowercase());
    let mut out = String::new();
    for (node, depth) in h.flatten() {
        let mut full = node.path.clone();
        full.push(node.slug.clone());
        let full_path = full.join("/");

        if let Some(n) = needle.as_deref() {
            let hit = node.title.to_lowercase().contains(n)
                || full_path.to_lowercase().contains(n);
            if !hit {
                continue;
            }
        }

        let indent = "  ".repeat(depth);
        let glyph = match node.kind {
            NodeKind::Book => "📖",
            NodeKind::Chapter => "▸",
            NodeKind::Subchapter => "·",
            NodeKind::Paragraph => "¶",
            NodeKind::Image => "▣",
            NodeKind::Script => "λ",
        };
        let detail = if matches!(node.kind, NodeKind::Paragraph) {
            let status = node.status.as_deref().unwrap_or("—");
            let target = node
                .target_words
                .filter(|t| *t > 0)
                .map(|t| format!("/{t}"))
                .unwrap_or_default();
            format!("  [{status} · {}{} words]", node.word_count, target)
        } else {
            String::new()
        };
        out.push_str(&format!("{indent}{glyph} {}{detail}\n", node.title));
        out.push_str(&format!("{indent}   {full_path}\n"));
    }
    out
}

pub fn run(project: &Path, filter: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    print!("{}", render(&h, filter));
    Ok(())
}
