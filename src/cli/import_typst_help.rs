//! `inkhaven import-typst-help`
//!
//! One-shot import of inkhaven's curated Typst reference into the
//! Help system book. Creates / refreshes a `Typst reference` chapter
//! under Help with one paragraph per `##` section of the bundled
//! `assets/typst-help.md`. The chapter is wiped clean on every run
//! so it's safe to re-invoke after an inkhaven upgrade.
//!
//! The reference markdown is embedded at compile time via
//! `include_str!` — the command is fully offline, no network. The
//! same file is shipped as `assets/typst-help.md` so users can read
//! it directly on the filesystem if they prefer.
//!
//! Effect on F1: once imported, the Help-book RAG flow that powers
//! F1 sees these paragraphs and answers typst questions ("how do I
//! make a figure with a caption?") from grounded context.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::{InsertPosition, Store, SYSTEM_TAG_HELP};

/// Bundled curated Typst reference. Each `## …` is one Help
/// paragraph; the prose between two headings (or up to the first
/// heading / EOF) becomes the body.
const BUNDLED_REFERENCE: &str = include_str!("../../assets/typst-help.md");

const CHAPTER_TITLE: &str = "Typst reference";

pub fn run(project: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;

    let hierarchy = Hierarchy::load(&store)?;
    let help = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_HELP)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store("Help system book not found — re-open the project to seed it".into())
        })?;

    // Find or replace the "Typst reference" chapter directly under
    // Help. A re-run wipes the previous version so we don't double
    // up paragraphs.
    if let Some(existing) = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Chapter
                && n.parent_id == Some(help.id)
                && n.title == CHAPTER_TITLE
        })
        .cloned()
    {
        wipe_chapter(&store, &hierarchy, &existing)?;
    }

    // Re-create the chapter and populate it.
    let hierarchy = Hierarchy::load(&store)?;
    let chapter = store.create_node(
        &cfg,
        &hierarchy,
        NodeKind::Chapter,
        CHAPTER_TITLE,
        Some(&help),
        None,
        InsertPosition::End,
    )?;

    let sections = parse_reference(BUNDLED_REFERENCE);
    let total = sections.len();
    if total == 0 {
        return Err(Error::Store(
            "bundled typst-help.md has no `## …` sections to import".into(),
        ));
    }

    for (i, section) in sections.iter().enumerate() {
        // Reload the hierarchy on each create so create_node's
        // sibling-order resolution sees prior paragraphs.
        let h = Hierarchy::load(&store)?;
        let mut node = store.create_node(
            &cfg,
            &h,
            NodeKind::Paragraph,
            &section.title,
            Some(&chapter),
            None,
            InsertPosition::End,
        )?;
        let body = format!("= {}\n\n{}\n", section.title, section.body.trim());
        if let Some(rel) = node.file.clone() {
            let abs = store.project_root().join(&rel);
            std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
            store.update_paragraph_content(&mut node, body.as_bytes())?;
        }
        if (i + 1) % 5 == 0 || i + 1 == total {
            eprintln!("  imported {}/{} sections", i + 1, total);
        }
    }
    store.sync()?;
    eprintln!(
        "Typst reference imported under Help → `{CHAPTER_TITLE}` ({total} paragraph(s))."
    );
    Ok(())
}

struct ParsedSection<'a> {
    title: String,
    body: &'a str,
}

/// Walk the markdown line-by-line, splitting on `##` headers. Lines
/// before the first `##` (the top-level `# Typst overview` block) get
/// folded into a synthesised "Overview" paragraph so the user lands
/// on something useful when they open the chapter.
fn parse_reference(input: &str) -> Vec<ParsedSection<'_>> {
    let mut out: Vec<ParsedSection<'_>> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut body_start: usize = 0;
    let mut last_end: usize = 0;

    for (line_start, line) in line_offsets(input) {
        last_end = line_start + line.len();
        if let Some(rest) = line.strip_prefix("## ") {
            // New section. Flush the previous one if non-empty.
            let prev_body = &input[body_start..line_start];
            push_section(&mut out, current_title.take(), prev_body);
            current_title = Some(rest.trim().to_string());
            body_start = line_start + line.len();
        }
    }
    // Final section (or the trailing overview chunk if no `##`).
    let prev_body = &input[body_start..last_end];
    push_section(&mut out, current_title, prev_body);
    out
}

fn push_section<'a>(
    out: &mut Vec<ParsedSection<'a>>,
    title: Option<String>,
    body: &'a str,
) {
    let trimmed = body.trim_matches('\n');
    if trimmed.is_empty() && title.is_none() {
        return;
    }
    out.push(ParsedSection {
        title: title.unwrap_or_else(|| "Overview".into()),
        body: trimmed,
    });
}

/// Iterate `(byte_offset_at_line_start, line_without_terminator)` so
/// the parser can slice the original input by byte offset.
fn line_offsets(input: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut start = 0usize;
    let bytes = input.as_bytes();
    std::iter::from_fn(move || {
        if start >= bytes.len() {
            return None;
        }
        let mut end = start;
        while end < bytes.len() && bytes[end] != b'\n' {
            end += 1;
        }
        let line = &input[start..end];
        let next = if end < bytes.len() { end + 1 } else { end };
        let cur = start;
        start = next;
        Some((cur, line))
    })
}

fn wipe_chapter(store: &Store, hierarchy: &Hierarchy, chapter: &Node) -> Result<()> {
    let layout = store.project_root().to_path_buf();
    let ids = hierarchy.collect_subtree(chapter.id);
    let abs = layout.join(
        hierarchy.fs_path(chapter, &crate::project::ProjectLayout::new(&layout)),
    );
    let fs_rel = abs
        .strip_prefix(&layout)
        .unwrap_or(&abs)
        .to_path_buf();
    store.delete_subtree(&fs_rel, &ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reference_splits_on_double_hash() {
        let md = "# Top\nintro line\n\n## Alpha\nbody alpha\n\n## Beta\nbody beta\n";
        let sections = parse_reference(md);
        // Lines before the first `##` get folded into an "Overview" entry.
        assert!(sections.iter().any(|s| s.title == "Overview"));
        assert!(sections.iter().any(|s| s.title == "Alpha"));
        assert!(sections.iter().any(|s| s.title == "Beta"));
        let beta = sections.iter().find(|s| s.title == "Beta").unwrap();
        assert_eq!(beta.body.trim(), "body beta");
    }

    #[test]
    fn parse_reference_ignores_h3_and_lower() {
        let md = "## Real section\ncontent\n### A subheading inside body\nmore content\n";
        let sections = parse_reference(md);
        assert_eq!(sections.len(), 1);
        let s = &sections[0];
        assert_eq!(s.title, "Real section");
        // The `###` line ends up as part of the body, not a new section.
        assert!(s.body.contains("### A subheading inside body"));
    }

    #[test]
    fn parse_reference_bundled_has_many_sections() {
        let n = parse_reference(BUNDLED_REFERENCE).len();
        assert!(
            n >= 20,
            "bundled reference should ship plenty of sections, got {n}"
        );
    }
}
