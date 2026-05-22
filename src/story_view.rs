//! Ctrl+V W — story-view graph render (1.2.5+).
//!
//! Walks the current user book's hierarchy and builds a DOT
//! description of its shape:
//!
//! * **Solid edges** chain the structural skeleton:
//!     `book → chapter → subchapter → paragraph`.
//! * **Dashed edges** represent user-authored relationships:
//!     - `linked_paragraphs` (wiki-links from `Ctrl+V A` / `I`)
//!     - lexicon mentions: paragraphs whose body name-mentions a
//!       node in the `Characters` / `Places` / `Artefacts`
//!       system books get a dashed edge from the lexicon node
//!       to the paragraph.
//!
//! Different node kinds get different DOT shapes so the resulting
//! picture is readable at a glance.
//!
//! Pipeline: `String (DOT)` → `layout-rs` (layout + SVG export)
//! → `resvg` + `tiny-skia` (rasterise) → PNG bytes + a
//! `DynamicImage` for the floating ratatui-image modal.
//!
//! The render runs synchronously on the caller's thread — DOT
//! layout is fast on book-sized graphs (~50–1000 nodes typical;
//! hundreds of ms). The TUI's spinner-modal pattern is reused
//! around the call for visible feedback while it runs.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use layout::backends::svg::SVGWriter;
use layout::gv::{DotParser, GraphBuilder};
use resvg::{tiny_skia, usvg};
use uuid::Uuid;

use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::Store;

/// Process-wide `usvg::fontdb::Database` shared across every
/// story-view render. Loading system fonts is a one-shot
/// startup cost (~50–500 ms on a typical desktop); cache it so
/// repeated `Ctrl+V W` presses don't re-scan the disk.
///
/// The serif / sans-serif / cursive / fantasy / monospace
/// fallback families are set to the names usvg / resvg's
/// upstream CLI uses, so on any reasonable system at least one
/// font resolves for layout-rs's default `Times, serif` CSS.
static STORY_FONTDB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();

fn story_fontdb() -> Arc<usvg::fontdb::Database> {
    STORY_FONTDB
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            db.set_serif_family("Times New Roman");
            db.set_sans_serif_family("Arial");
            db.set_cursive_family("Comic Sans MS");
            db.set_fantasy_family("Impact");
            db.set_monospace_family("Courier New");
            Arc::new(db)
        })
        .clone()
}

/// Final output of the story-view pipeline.
pub struct StoryRender {
    pub width: u32,
    pub height: u32,
    pub png_bytes: Vec<u8>,
    pub image: image::DynamicImage,
}

/// Top-level entry. Returns Err with a human-readable cause when
/// any pipeline stage fails (DOT parse, SVG layout, raster).
pub fn build_story_png(
    store: &Store,
    hierarchy: &Hierarchy,
    book_id: Uuid,
) -> Result<StoryRender, String> {
    let book = hierarchy
        .get(book_id)
        .ok_or_else(|| format!("book {book_id} missing from hierarchy"))?
        .clone();
    if book.kind != NodeKind::Book {
        return Err(format!("`{}` is not a Book node", book.title));
    }
    let dot = build_dot(store, hierarchy, &book);
    dot_to_png(&dot).map(|(width, height, png_bytes, image)| StoryRender {
        width,
        height,
        png_bytes,
        image,
    })
}

/// Construct the DOT source for one user book. The graph
/// includes:
///
/// * Every node in `book`'s subtree (book + chapters +
///   subchapters + paragraphs / images / scripts / JSON).
/// * Lexicon nodes (Characters / Places / Artefacts) whose
///   title is mentioned by at least one paragraph in the book.
/// * Solid edges along the structural hierarchy.
/// * Dashed edges for `linked_paragraphs` (within the book) and
///   for lexicon mentions.
fn build_dot(
    store: &Store,
    hierarchy: &Hierarchy,
    book: &Node,
) -> String {
    let all = hierarchy.flatten();
    let in_book: HashSet<Uuid> = collect_subtree_ids(hierarchy, book.id);
    let book_nodes: Vec<&Node> = all
        .iter()
        .filter(|(n, _)| in_book.contains(&n.id))
        .map(|(n, _)| *n)
        .collect();

    // Lexicon nodes — every paragraph-kind node under
    // Characters / Places / Artefacts system books.
    let lexicon: Vec<&Node> = all
        .iter()
        .filter(|(n, _)| {
            n.kind == NodeKind::Paragraph
                && is_under_system_book(
                    hierarchy,
                    n,
                    &["characters", "places", "artefacts"],
                )
        })
        .map(|(n, _)| *n)
        .collect();

    // Collect paragraph bodies so we can scan them for lexicon
    // mentions. Skip non-paragraph kinds + paragraphs whose
    // content isn't typst-shaped (scripts, JSON blobs).
    let mut bodies: HashMap<Uuid, String> = HashMap::new();
    for n in &book_nodes {
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        let ct = n.content_type.as_deref().unwrap_or("typst");
        if ct != "typst" && ct != "" {
            continue;
        }
        if let Some(rel) = &n.file {
            let abs = store.project_root().join(rel);
            if let Ok(bytes) = std::fs::read(&abs) {
                if let Ok(text) = String::from_utf8(bytes) {
                    bodies.insert(n.id, text.to_lowercase());
                }
            }
        }
    }

    // Lexicon → paragraph mentions. Case-insensitive substring
    // — good enough for a first pass; word-boundary detection
    // can be a refinement later.
    let mut lex_mentions: Vec<(Uuid, Uuid)> = Vec::new();
    let mut mentioned_lex: HashSet<Uuid> = HashSet::new();
    for lex in &lexicon {
        let needle = lex.title.trim().to_lowercase();
        if needle.is_empty() {
            continue;
        }
        for (pid, body) in &bodies {
            if body.contains(&needle) {
                lex_mentions.push((lex.id, *pid));
                mentioned_lex.insert(lex.id);
            }
        }
    }

    // Now emit DOT. dot_id() makes a syntactically-safe identifier
    // from a Uuid (graphviz allows alnum + underscores).
    let mut dot = String::new();
    dot.push_str("digraph story {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  graph [splines=true, overlap=false, nodesep=0.5, ranksep=0.7];\n");
    dot.push_str("  node [fontname=\"Helvetica\", fontsize=11];\n");
    dot.push_str("  edge [fontname=\"Helvetica\", fontsize=9];\n");

    // Book + subtree nodes.
    for n in &book_nodes {
        let (shape, fill) = shape_for(n);
        let label = sanitize_label(&n.title);
        dot.push_str(&format!(
            "  {} [shape={shape}, style=\"filled\", fillcolor=\"{fill}\", label=\"{label}\"];\n",
            dot_id(n.id),
        ));
    }
    // Lexicon nodes — only the ones actually mentioned.
    for lex in &lexicon {
        if !mentioned_lex.contains(&lex.id) {
            continue;
        }
        let (shape, fill) = lexicon_shape_for(lex, hierarchy);
        let label = sanitize_label(&lex.title);
        dot.push_str(&format!(
            "  {} [shape={shape}, style=\"filled\", fillcolor=\"{fill}\", label=\"{label}\"];\n",
            dot_id(lex.id),
        ));
    }

    // Solid structural edges — every node's parent points down
    // into it. We walk the subtree and emit one edge per
    // parent-child pair (excluding the book's own parent).
    for n in &book_nodes {
        if n.id == book.id {
            continue;
        }
        if let Some(pid) = n.parent_id {
            if in_book.contains(&pid) {
                dot.push_str(&format!(
                    "  {} -> {} [style=\"solid\", color=\"#444444\"];\n",
                    dot_id(pid),
                    dot_id(n.id),
                ));
            }
        }
    }

    // Dashed user-link edges — Ctrl+V A / I outgoing links. Only
    // emit edges where both endpoints are inside the book (skip
    // dangling links to deleted paragraphs).
    for n in &book_nodes {
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        for target_id in &n.linked_paragraphs {
            if in_book.contains(target_id) {
                dot.push_str(&format!(
                    "  {} -> {} [style=\"dashed\", color=\"#7755aa\", label=\"link\"];\n",
                    dot_id(n.id),
                    dot_id(*target_id),
                ));
            }
        }
    }

    // Dashed lexicon edges — Characters / Places / Artefacts to
    // paragraphs that mention them.
    for (lex_id, para_id) in &lex_mentions {
        dot.push_str(&format!(
            "  {} -> {} [style=\"dashed\", color=\"#11883a\"];\n",
            dot_id(*lex_id),
            dot_id(*para_id),
        ));
    }

    dot.push_str("}\n");
    dot
}

/// Walk descendants of `root_id` and return every reachable id
/// (including the root itself).
fn collect_subtree_ids(hierarchy: &Hierarchy, root_id: Uuid) -> HashSet<Uuid> {
    let mut out = HashSet::new();
    let mut stack = vec![root_id];
    while let Some(id) = stack.pop() {
        if !out.insert(id) {
            continue;
        }
        for (n, _) in hierarchy.flatten() {
            if n.parent_id == Some(id) {
                stack.push(n.id);
            }
        }
    }
    out
}

fn is_under_system_book(hierarchy: &Hierarchy, n: &Node, tags: &[&str]) -> bool {
    hierarchy.ancestors(n).into_iter().any(|a| {
        a.kind == NodeKind::Book
            && a.system_tag
                .as_deref()
                .map(|t| {
                    let t = t.to_lowercase();
                    tags.iter().any(|want| t == *want)
                })
                .unwrap_or(false)
    })
}

/// Map a Node to (DOT shape, fill colour).
fn shape_for(n: &Node) -> (&'static str, &'static str) {
    match n.kind {
        NodeKind::Book => ("folder", "#fff7e6"),
        NodeKind::Chapter => ("box", "#e6f4ff"),
        NodeKind::Subchapter => ("octagon", "#e6fff4"),
        NodeKind::Paragraph => match n.content_type.as_deref() {
            Some("hjson") => ("note", "#fff0f5"),
            Some("bund") => ("parallelogram", "#fff5e0"),
            _ => ("ellipse", "#ffffff"),
        },
        NodeKind::Script => ("parallelogram", "#fff5e0"),
        NodeKind::Image => ("cds", "#e0e7ff"),
    }
}

fn lexicon_shape_for(n: &Node, hierarchy: &Hierarchy) -> (&'static str, &'static str) {
    // Tag is "Places"/"Characters"/"Artefacts" — case from system_tag.
    let book = hierarchy
        .ancestors(n)
        .into_iter()
        .find(|a| a.kind == NodeKind::Book);
    let tag = book
        .and_then(|b| b.system_tag.clone())
        .unwrap_or_default()
        .to_lowercase();
    match tag.as_str() {
        "characters" => ("egg", "#fffacd"),
        "places" => ("diamond", "#d0f0ff"),
        "artefacts" => ("hexagon", "#ffdab9"),
        _ => ("box", "#eeeeee"),
    }
}

/// Build a syntactically-safe DOT identifier from a UUID. We
/// prefix with `n_` so the identifier doesn't start with a
/// digit (graphviz rejects bare-digit identifiers as
/// statement markers in some contexts).
fn dot_id(id: Uuid) -> String {
    let mut s = String::with_capacity(34);
    s.push_str("n_");
    for ch in id.simple().to_string().chars() {
        s.push(ch);
    }
    s
}

/// Escape a label for a DOT `"…"` string. Keeps the label
/// single-line; long titles are truncated so the rendered
/// boxes stay readable.
fn sanitize_label(s: &str) -> String {
    let mut t: String = s.replace('\\', "\\\\").replace('"', "\\\"");
    if t.chars().count() > 40 {
        t = t.chars().take(37).collect::<String>() + "…";
    }
    // DOT label literal: escape newlines too.
    t.replace('\n', " ")
}

/// Run the DOT through layout-rs → SVG, then resvg → tiny-skia
/// pixmap → PNG. Returns `(width, height, png_bytes,
/// DynamicImage)`.
fn dot_to_png(
    dot: &str,
) -> Result<(u32, u32, Vec<u8>, image::DynamicImage), String> {
    // Phase 1: parse DOT → AST.
    let mut parser = DotParser::new(dot);
    let graph = parser
        .process()
        .map_err(|e| format!("dot parse: {e}"))?;
    // Phase 2: build VisualGraph from AST.
    let mut builder = GraphBuilder::new();
    builder.visit_graph(&graph);
    let mut vg = builder.get();
    // Phase 3: lay out + emit SVG.
    let mut svg = SVGWriter::new();
    vg.do_it(false, false, false, &mut svg);
    let svg_string = svg.finalize();

    // Phase 4: parse SVG → usvg::Tree. The fontdb is required
    // so layout-rs's text labels (`Times, serif`) actually
    // rasterise; without it text renders as invisible glyphs.
    let opts = usvg::Options {
        fontdb: story_fontdb(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&svg_string, &opts)
        .map_err(|e| format!("svg parse: {e}"))?;
    let int_size = tree.size().to_int_size();
    let (w, h) = (int_size.width(), int_size.height());
    if w == 0 || h == 0 {
        return Err("rendered SVG has zero size".into());
    }
    // Phase 5: rasterise. resvg writes into a tiny-skia Pixmap.
    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| format!("cannot allocate {w}×{h} pixmap"))?;
    // Default sRGB white background — DOT's default canvas.
    pixmap.fill(tiny_skia::Color::WHITE);
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    // Phase 6: encode + decode into the image crate's types so
    // ratatui-image can resize-protocol it.
    let png_bytes = pixmap
        .encode_png()
        .map_err(|e| format!("encode PNG: {e}"))?;
    let rgba = image::RgbaImage::from_raw(w, h, pixmap.data().to_vec())
        .ok_or_else(|| "pixmap dimensions disagree with buffer".to_owned())?;
    let dyn_img = image::DynamicImage::ImageRgba8(rgba);
    Ok((w, h, png_bytes, dyn_img))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_id_is_safe() {
        let id = Uuid::nil();
        let s = dot_id(id);
        assert!(s.starts_with("n_"));
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    #[test]
    fn sanitize_label_handles_quotes_and_long_titles() {
        let out = sanitize_label("Has \"quotes\" inside");
        assert!(out.contains("\\\""));
        let long = "x".repeat(100);
        let out = sanitize_label(&long);
        assert!(out.chars().count() <= 40);
        assert!(out.ends_with('…'));
    }

    /// `#[ignore]` smoke — exercises the full DOT → PNG path
    /// on a tiny hand-built graph. Requires no fonts (DOT
    /// labels are emitted as text; fontconfig may chime in
    /// depending on the host, hence the gate).
    #[test]
    #[ignore]
    fn end_to_end_render_smoke() {
        let dot = "digraph { a -> b [style=\"dashed\"]; b -> c; }";
        let (w, h, png, _img) = dot_to_png(dot).expect("render");
        assert!(w > 0 && h > 0);
        assert_eq!(&png[..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    }

    /// `#[ignore]` debug — dump the SVG layout-rs produces for
    /// a labelled graph so we can verify text actually lands
    /// on the canvas.
    #[test]
    #[ignore]
    fn dump_svg_with_labels() {
        use layout::backends::svg::SVGWriter;
        use layout::gv::{DotParser, GraphBuilder};

        let dot = r#"digraph {
            a [shape=box, label="The storm"];
            b [shape=ellipse, label="Bell tower"];
            c [shape=octagon, label="Three weeks"];
            a -> b;
            b -> c;
        }"#;
        let mut parser = DotParser::new(dot);
        let graph = parser.process().expect("parse");
        let mut builder = GraphBuilder::new();
        builder.visit_graph(&graph);
        let mut vg = builder.get();
        let mut svg = SVGWriter::new();
        vg.do_it(false, false, false, &mut svg);
        let s = svg.finalize();
        eprintln!("--- SVG dump start ---\n{s}\n--- SVG dump end ---");
        // Sanity — at least one of the labels must appear in the SVG text.
        assert!(s.contains("storm") || s.contains("Bell") || s.contains("Three"),
            "no label text found in SVG");
    }
}
