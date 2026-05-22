//! Ctrl+V W — story-view graph render (1.2.5+).
//!
//! Walks the current user book's hierarchy and lays it out as a
//! **twopi-style radial tree** with the book at the centre. Each
//! structural depth gets its own ring; wedges are sized in
//! proportion to subtree leaf counts so big chapters don't crowd
//! out small ones. Lexicon nodes (Characters / Places /
//! Artefacts) live on an outer ring at the angle of their first
//! mentioned paragraph, so the dashed-green relationship edges
//! flow naturally outward.
//!
//! * **Solid grey edges** chain the structural skeleton.
//! * **Dashed purple edges** are `linked_paragraphs` wiki-links
//!   (`Ctrl+V A` / `I`).
//! * **Dashed green edges** connect lexicon nodes to the
//!   paragraphs that mention them by title (case-insensitive
//!   substring on the body).
//!
//! Each node kind gets its own SVG shape (rectangle / ellipse /
//! octagon / hexagon / diamond / egg / folder / note /
//! parallelogram / chevron) — built by hand because `layout-rs`'s
//! Sugiyama DAG layout can't do radial. Bypassing layout-rs also
//! drops a 1k-LOC dep.
//!
//! Pipeline: hand-built SVG → `resvg` + `tiny-skia` (rasterise)
//! → PNG bytes + a `DynamicImage` for the floating modal.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::f64::consts::TAU;
use std::sync::{Arc, OnceLock};

use resvg::{tiny_skia, usvg};
use uuid::Uuid;

use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::Store;

// ── Layout constants ──────────────────────────────────────────

/// Distance between successive radial rings, in SVG user units.
/// The placer also bumps this against the largest node radius
/// it laid out so two rings can't crash into each other.
const RING_SPACING_BASE: f64 = 240.0;
/// Maximum margin around the laid-out graph before SVG viewBox
/// is computed. Big enough to cover label overflow on the outer
/// ring nodes.
const SVG_MARGIN: f64 = 120.0;

/// Approximate glyph-cell width at the 12 px sans-serif used
/// for labels. resvg doesn't expose font metrics to user code
/// at SVG-build time, so we estimate width = char-count ×
/// `CHAR_WIDTH`. Conservative: a 7 px-per-char assumption keeps
/// labels comfortably inside their boxes on every font I tested
/// (Helvetica / Arial / Liberation Sans / DejaVu Sans).
const CHAR_WIDTH: f64 = 7.2;
/// Vertical spacing between wrapped label lines, in SVG user
/// units. Matches `1.2em` for the 12 px text style.
const LINE_HEIGHT: f64 = 14.4;
/// Inner padding around the multi-line label inside a node's
/// bounding box. (`X` is left+right, `Y` is top+bottom.)
const PADDING_X: f64 = 18.0;
const PADDING_Y: f64 = 10.0;
/// Hard floor for the node bounding box — even a one-letter
/// label gets at least this much room so the shape doesn't
/// shrink into an unreadable dot.
const MIN_HALF_W: f64 = 36.0;
const MIN_HALF_H: f64 = 18.0;
/// Wrapping ceiling. Lines longer than this get wrapped on a
/// word boundary; titles taller than `MAX_LINES` get truncated
/// with `…`.
const WRAP_LINE_CHARS: usize = 22;
const MAX_LINES: usize = 4;

// ── Public API ────────────────────────────────────────────────

/// Final output of the story-view pipeline.
pub struct StoryRender {
    pub width: u32,
    pub height: u32,
    pub png_bytes: Vec<u8>,
    pub image: image::DynamicImage,
}

/// Top-level entry. Returns Err with a human-readable cause when
/// any pipeline stage fails (layout, SVG raster).
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
    let graph = build_graph(store, hierarchy, &book);
    let svg = render_svg(&graph);
    svg_to_png(&svg)
}

// ── Graph model ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeKind {
    Folder,        // book
    Box,           // chapter
    Octagon,       // subchapter
    Ellipse,       // typst paragraph
    Note,          // hjson paragraph
    Parallelogram, // bund script / Script node
    Cds,           // image
    Egg,           // character
    Diamond,       // place
    Hexagon,       // artefact
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeStyle {
    /// Solid grey — structural parent→child.
    Structural,
    /// Dashed purple — `linked_paragraphs` wiki-links.
    WikiLink,
    /// Dashed green — lexicon node → paragraph mention.
    Lexicon,
}

struct GraphNode {
    id: Uuid,
    /// Already-wrapped label lines (no SVG escaping yet — that
    /// happens at emit time so we can wrap on character count,
    /// not encoded length).
    label_lines: Vec<String>,
    shape: ShapeKind,
    fill: &'static str,
    /// Position in laid-out SVG user-space (before the
    /// margin translation that `render_svg` applies).
    x: f64,
    y: f64,
    /// Half-extent of the shape's axis-aligned bounding box.
    /// Per-node so shapes can grow to fit their wrapped label.
    /// Edge truncation uses these to nudge the segment endpoints
    /// out of the node's interior.
    half_w: f64,
    half_h: f64,
}

struct GraphEdge {
    from: Uuid,
    to: Uuid,
    style: EdgeStyle,
}

struct Graph {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    /// Extent — max |x| or |y| of any laid-out node. Used to
    /// compute the SVG viewBox.
    extent: f64,
}

// ── Build the graph ───────────────────────────────────────────

fn build_graph(store: &Store, hierarchy: &Hierarchy, book: &Node) -> Graph {
    let all = hierarchy.flatten();
    let in_book: HashSet<Uuid> = collect_subtree_ids(hierarchy, book.id);

    // Structural children map: parent_id -> [child ids, ordered].
    let mut structural: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for (n, _) in &all {
        if !in_book.contains(&n.id) {
            continue;
        }
        if let Some(pid) = n.parent_id {
            if in_book.contains(&pid) {
                structural.entry(pid).or_default().push(n.id);
            }
        }
    }
    // Sort children by `order` so siblings appear consistently.
    let mut order_of: HashMap<Uuid, u32> = HashMap::new();
    for (n, _) in &all {
        order_of.insert(n.id, n.order);
    }
    for kids in structural.values_mut() {
        kids.sort_by_key(|id| order_of.get(id).copied().unwrap_or(0));
    }

    let structural_children: HashMap<Uuid, Vec<Uuid>> =
        structural.into_iter().collect();

    // Compute leaf counts + depths via DFS rooted at the book.
    let mut leaves: HashMap<Uuid, usize> = HashMap::new();
    count_leaves(book.id, &structural_children, &mut leaves);
    let max_depth = max_depth_of(book.id, &structural_children);

    // Lexicon nodes — every paragraph-kind node under
    // Characters / Places / Artefacts system books. Only
    // ones actually mentioned land on the graph (filtering
    // happens further down once we have body scans).
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

    // ── Pre-size every candidate node ─────────────────────────
    // Wrap labels first so ring spacing can be tightened or
    // loosened against the widest box. Stored once and reused
    // when building the final GraphNode list further down.
    let mut sized: HashMap<Uuid, (Vec<String>, f64, f64)> = HashMap::new();
    for (n, _) in &all {
        if in_book.contains(&n.id) {
            let lines = wrap_label(&n.title);
            let (hw, hh) = size_for_label(&lines);
            sized.insert(n.id, (lines, hw, hh));
        }
    }
    for lex in &lexicon {
        let lines = wrap_label(&lex.title);
        let (hw, hh) = size_for_label(&lines);
        sized.insert(lex.id, (lines, hw, hh));
    }
    // Effective ring spacing — at least the base, but bumped to
    // cover the widest measured node plus a buffer so two rings
    // can never overlap.
    let max_half_w = sized
        .values()
        .map(|(_, hw, _)| *hw)
        .fold(0.0_f64, f64::max);
    let ring_spacing = RING_SPACING_BASE.max(2.0 * max_half_w + 60.0);

    // Place structural nodes via twopi.
    let mut positions: HashMap<Uuid, (f64, f64)> = HashMap::new();
    place_radial(
        book.id,
        0.0,
        TAU,
        0,
        &structural_children,
        &leaves,
        ring_spacing,
        &mut positions,
    );

    // Paragraph bodies — for lexicon-mention scanning. Skip
    // non-paragraph kinds + non-typst content types.
    let mut bodies: HashMap<Uuid, String> = HashMap::new();
    let book_nodes: Vec<&Node> = all
        .iter()
        .filter(|(n, _)| in_book.contains(&n.id))
        .map(|(n, _)| *n)
        .collect();
    for n in &book_nodes {
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        let ct = n.content_type.as_deref().unwrap_or("typst");
        if ct != "typst" && !ct.is_empty() {
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

    // Lexicon → paragraph mentions.
    let mut lex_targets: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for lex in &lexicon {
        let needle = lex.title.trim().to_lowercase();
        if needle.is_empty() {
            continue;
        }
        let mut hits: Vec<Uuid> = Vec::new();
        for (pid, body) in &bodies {
            if body.contains(&needle) {
                hits.push(*pid);
            }
        }
        if !hits.is_empty() {
            lex_targets.insert(lex.id, hits);
        }
    }

    // Place lexicon nodes on the ring just outside the
    // structural tree. Angle = first mentioned paragraph's
    // angle from origin. Multiple lexicon nodes that resolve
    // to the same angle nudge by a small offset so they don't
    // overlap.
    let lex_ring = (max_depth as f64 + 1.0) * ring_spacing;
    let mut angle_collisions: HashMap<i32, usize> = HashMap::new();
    for lex in &lexicon {
        let Some(targets) = lex_targets.get(&lex.id) else {
            continue;
        };
        let Some(&first) = targets.first() else { continue };
        let Some(&(tx, ty)) = positions.get(&first) else {
            continue;
        };
        // atan2 returns (-π, π]; promote into [0, TAU).
        let mut theta = ty.atan2(tx);
        if theta < 0.0 {
            theta += TAU;
        }
        // Nudge if two lexicon nodes round to the same 5-degree
        // bucket — keeps them from stacking.
        let bucket = (theta.to_degrees() / 5.0).round() as i32;
        let n_in_bucket = angle_collisions.entry(bucket).or_insert(0);
        let nudge = (*n_in_bucket as f64) * (8.0_f64.to_radians());
        *n_in_bucket += 1;
        theta += nudge;

        let (x, y) = (lex_ring * theta.cos(), lex_ring * theta.sin());
        positions.insert(lex.id, (x, y));
    }

    // Build the GraphNode list.
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut max_extent: f64 = 0.0;
    let mut node_lookup: HashSet<Uuid> = HashSet::new();
    for n in &book_nodes {
        let Some(&(x, y)) = positions.get(&n.id) else {
            continue;
        };
        let (shape, fill) = shape_for(n);
        let (label_lines, half_w, half_h) = sized
            .get(&n.id)
            .cloned()
            .unwrap_or_else(|| {
                let lines = wrap_label(&n.title);
                let (hw, hh) = size_for_label(&lines);
                (lines, hw, hh)
            });
        nodes.push(GraphNode {
            id: n.id,
            label_lines,
            shape,
            fill,
            x,
            y,
            half_w,
            half_h,
        });
        node_lookup.insert(n.id);
        max_extent = max_extent.max(x.abs() + half_w).max(y.abs() + half_h);
    }
    for lex in &lexicon {
        if !lex_targets.contains_key(&lex.id) {
            continue;
        }
        let Some(&(x, y)) = positions.get(&lex.id) else {
            continue;
        };
        let (shape, fill) = lexicon_shape_for(lex, hierarchy);
        let (label_lines, half_w, half_h) = sized
            .get(&lex.id)
            .cloned()
            .unwrap_or_else(|| {
                let lines = wrap_label(&lex.title);
                let (hw, hh) = size_for_label(&lines);
                (lines, hw, hh)
            });
        nodes.push(GraphNode {
            id: lex.id,
            label_lines,
            shape,
            fill,
            x,
            y,
            half_w,
            half_h,
        });
        node_lookup.insert(lex.id);
        max_extent = max_extent.max(x.abs() + half_w).max(y.abs() + half_h);
    }

    // Build the edge list.
    let mut edges: Vec<GraphEdge> = Vec::new();
    // Structural.
    for (parent, kids) in &structural_children {
        for child in kids {
            if node_lookup.contains(parent) && node_lookup.contains(child) {
                edges.push(GraphEdge {
                    from: *parent,
                    to: *child,
                    style: EdgeStyle::Structural,
                });
            }
        }
    }
    // Wiki-links.
    for n in &book_nodes {
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        for target in &n.linked_paragraphs {
            if node_lookup.contains(&n.id) && node_lookup.contains(target) {
                edges.push(GraphEdge {
                    from: n.id,
                    to: *target,
                    style: EdgeStyle::WikiLink,
                });
            }
        }
    }
    // Lexicon mentions.
    for (lex_id, targets) in &lex_targets {
        for t in targets {
            if node_lookup.contains(lex_id) && node_lookup.contains(t) {
                edges.push(GraphEdge {
                    from: *lex_id,
                    to: *t,
                    style: EdgeStyle::Lexicon,
                });
            }
        }
    }

    Graph {
        nodes,
        edges,
        extent: max_extent,
    }
}

fn count_leaves(
    n: Uuid,
    children: &HashMap<Uuid, Vec<Uuid>>,
    out: &mut HashMap<Uuid, usize>,
) -> usize {
    let kids = children.get(&n).cloned().unwrap_or_default();
    if kids.is_empty() {
        out.insert(n, 1);
        return 1;
    }
    let mut total = 0;
    for k in kids {
        total += count_leaves(k, children, out);
    }
    out.insert(n, total);
    total
}

fn max_depth_of(root: Uuid, children: &HashMap<Uuid, Vec<Uuid>>) -> usize {
    fn recurse(
        n: Uuid,
        depth: usize,
        children: &HashMap<Uuid, Vec<Uuid>>,
    ) -> usize {
        let kids = children.get(&n).cloned().unwrap_or_default();
        if kids.is_empty() {
            return depth;
        }
        kids.into_iter()
            .map(|k| recurse(k, depth + 1, children))
            .max()
            .unwrap_or(depth)
    }
    recurse(root, 0, children)
}

/// Twopi-style radial placement. Each subtree gets a wedge of
/// the parent's angular range proportional to its leaf count.
/// The root lands at the origin (depth 0); depth-1 nodes ring
/// the origin at radius `ring_spacing`; each successive depth
/// adds another `ring_spacing` of radius.
fn place_radial(
    node: Uuid,
    theta_start: f64,
    theta_end: f64,
    depth: usize,
    children: &HashMap<Uuid, Vec<Uuid>>,
    leaves: &HashMap<Uuid, usize>,
    ring_spacing: f64,
    out: &mut HashMap<Uuid, (f64, f64)>,
) {
    let theta_mid = (theta_start + theta_end) / 2.0;
    let radius = depth as f64 * ring_spacing;
    let (x, y) = if depth == 0 {
        (0.0, 0.0)
    } else {
        (radius * theta_mid.cos(), radius * theta_mid.sin())
    };
    out.insert(node, (x, y));

    let kids = children.get(&node).cloned().unwrap_or_default();
    if kids.is_empty() {
        return;
    }
    let total_leaves: usize = kids.iter().map(|c| *leaves.get(c).unwrap_or(&1)).sum();
    if total_leaves == 0 {
        return;
    }
    let span = theta_end - theta_start;
    let mut cursor = theta_start;
    for k in kids {
        let kl = *leaves.get(&k).unwrap_or(&1) as f64;
        let wedge = span * kl / total_leaves as f64;
        place_radial(
            k,
            cursor,
            cursor + wedge,
            depth + 1,
            children,
            leaves,
            ring_spacing,
            out,
        );
        cursor += wedge;
    }
}

// ── Shape mapping ─────────────────────────────────────────────

fn shape_for(n: &Node) -> (ShapeKind, &'static str) {
    match n.kind {
        NodeKind::Book => (ShapeKind::Folder, "#fff7e6"),
        NodeKind::Chapter => (ShapeKind::Box, "#e6f4ff"),
        NodeKind::Subchapter => (ShapeKind::Octagon, "#e6fff4"),
        NodeKind::Paragraph => match n.content_type.as_deref() {
            Some("hjson") => (ShapeKind::Note, "#fff0f5"),
            Some("bund") => (ShapeKind::Parallelogram, "#fff5e0"),
            _ => (ShapeKind::Ellipse, "#ffffff"),
        },
        NodeKind::Script => (ShapeKind::Parallelogram, "#fff5e0"),
        NodeKind::Image => (ShapeKind::Cds, "#e0e7ff"),
    }
}

fn lexicon_shape_for(n: &Node, hierarchy: &Hierarchy) -> (ShapeKind, &'static str) {
    let book = hierarchy
        .ancestors(n)
        .into_iter()
        .find(|a| a.kind == NodeKind::Book);
    let tag = book
        .and_then(|b| b.system_tag.clone())
        .unwrap_or_default()
        .to_lowercase();
    match tag.as_str() {
        "characters" => (ShapeKind::Egg, "#fffacd"),
        "places" => (ShapeKind::Diamond, "#d0f0ff"),
        "artefacts" => (ShapeKind::Hexagon, "#ffdab9"),
        _ => (ShapeKind::Box, "#eeeeee"),
    }
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

/// Word-wrap a raw title string into 1..=`MAX_LINES` lines of at
/// most `WRAP_LINE_CHARS` chars each (after whitespace
/// normalisation). Words longer than the line ceiling get broken
/// hard at the boundary; overflow past `MAX_LINES` collapses
/// into a single `…`-suffixed last line. No SVG escaping here
/// — that happens at emit time so the wrap math operates on
/// real character counts, not encoded length.
fn wrap_label(raw: &str) -> Vec<String> {
    let normalised: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalised.is_empty() {
        return vec!["(untitled)".to_string()];
    }
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in normalised.split(' ') {
        // Word longer than the cap — hard-break it.
        let mut remaining = word.to_string();
        while remaining.chars().count() > WRAP_LINE_CHARS {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let head: String = remaining.chars().take(WRAP_LINE_CHARS).collect();
            let tail: String = remaining.chars().skip(WRAP_LINE_CHARS).collect();
            lines.push(head);
            remaining = tail;
        }
        if remaining.is_empty() {
            continue;
        }
        let candidate_len = if current.is_empty() {
            remaining.chars().count()
        } else {
            current.chars().count() + 1 + remaining.chars().count()
        };
        if candidate_len > WRAP_LINE_CHARS && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current = remaining;
        } else if current.is_empty() {
            current = remaining;
        } else {
            current.push(' ');
            current.push_str(&remaining);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.len() > MAX_LINES {
        // Collapse the tail into the last visible line +
        // ellipsis. Prefer trimming the last kept line over
        // breaking mid-word.
        lines.truncate(MAX_LINES);
        if let Some(last) = lines.last_mut() {
            let max_with_ellipsis = WRAP_LINE_CHARS.saturating_sub(1);
            if last.chars().count() > max_with_ellipsis {
                *last = last.chars().take(max_with_ellipsis).collect::<String>();
            }
            last.push('…');
        }
    }
    lines
}

/// Given the wrapped label, compute the node's half-width and
/// half-height. Bounded by the minimums above so even short
/// names get a comfortably-readable box.
fn size_for_label(label_lines: &[String]) -> (f64, f64) {
    let widest_chars = label_lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(1);
    let text_w = widest_chars as f64 * CHAR_WIDTH;
    let text_h = label_lines.len() as f64 * LINE_HEIGHT;
    let half_w = ((text_w + PADDING_X) * 0.5).max(MIN_HALF_W);
    let half_h = ((text_h + PADDING_Y) * 0.5).max(MIN_HALF_H);
    (half_w, half_h)
}

/// XML-escape a single label line for `<tspan>` text content.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ── SVG rendering ─────────────────────────────────────────────

fn render_svg(g: &Graph) -> String {
    // Translate the laid-out coordinates so everything is
    // positive within a (W, H) viewBox.
    let dim = g.extent + SVG_MARGIN;
    let total = (dim * 2.0) as u32;
    let cx = dim;
    let cy = dim;

    // Per-node lookup: centre + half-extents. Edges use these
    // to truncate their endpoints back to the node boundary,
    // and rendering uses the centre to place shape + label.
    let node_box: HashMap<Uuid, (f64, f64, f64, f64)> = g
        .nodes
        .iter()
        .map(|n| (n.id, (n.x + cx, n.y + cy, n.half_w, n.half_h)))
        .collect();

    let mut s = String::new();
    s.push_str(&format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{total}" height="{total}" viewBox="0 0 {total} {total}">
<defs>
  <marker id="ah-grey" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
    <path d="M0,0 L10,5 L0,10 z" fill="#555555"/>
  </marker>
  <marker id="ah-purple" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
    <path d="M0,0 L10,5 L0,10 z" fill="#7755aa"/>
  </marker>
  <marker id="ah-green" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
    <path d="M0,0 L10,5 L0,10 z" fill="#11883a"/>
  </marker>
</defs>
<style>
  text {{ font-family: 'Helvetica', 'Arial', sans-serif; font-size: 12px; fill: #222222; }}
</style>
<rect width="{total}" height="{total}" fill="#ffffff"/>
"##,
    ));

    // Edges first so node fills cover them.
    for e in &g.edges {
        let (Some(&(ax, ay, ahw, ahh)), Some(&(bx, by, bhw, bhh))) =
            (node_box.get(&e.from), node_box.get(&e.to))
        else {
            continue;
        };
        let ar = (ahw * ahw + ahh * ahh).sqrt() * 0.85;
        let br = (bhw * bhw + bhh * bhh).sqrt() * 0.85;
        let (x1, y1, x2, y2) = truncate_segment(ax, ay, bx, by, ar, br);
        let (stroke, dash, marker) = match e.style {
            EdgeStyle::Structural => ("#555555", "", "url(#ah-grey)"),
            EdgeStyle::WikiLink => ("#7755aa", "6,4", "url(#ah-purple)"),
            EdgeStyle::Lexicon => ("#11883a", "5,3", "url(#ah-green)"),
        };
        s.push_str(&format!(
            "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"{stroke}\" stroke-width=\"1.4\" stroke-dasharray=\"{dash}\" marker-end=\"{marker}\"/>\n"
        ));
    }

    // Nodes on top.
    for n in &g.nodes {
        let (px, py) = (n.x + cx, n.y + cy);
        let shape_svg = shape_svg(n.shape, px, py, n.half_w, n.half_h, n.fill);
        s.push_str(&shape_svg);
        // Multi-line label centred vertically on `(px, py)`. The
        // first tspan sits one half-block above centre; each
        // subsequent line steps down by LINE_HEIGHT (1.2em at
        // 12 px). resvg honours `dominant-baseline="middle"` on
        // tspans so the visual centre lines up with `py`.
        let line_count = n.label_lines.len() as f64;
        let first_dy = -(line_count - 1.0) * 0.5 * LINE_HEIGHT;
        s.push_str(&format!(
            "<text x=\"{px:.1}\" y=\"{py:.1}\" text-anchor=\"middle\" dominant-baseline=\"middle\">"
        ));
        for (i, line) in n.label_lines.iter().enumerate() {
            let dy = if i == 0 { first_dy } else { LINE_HEIGHT };
            s.push_str(&format!(
                "<tspan x=\"{px:.1}\" dy=\"{dy:.1}\">{label}</tspan>",
                label = escape_xml(line),
            ));
        }
        s.push_str("</text>\n");
    }

    s.push_str("</svg>\n");
    s
}

/// Push the line endpoints back so the visible segment starts
/// at each node's circumscribing-circle boundary, not at the
/// centre. Per-node radii (`ar`, `br`) account for the
/// label-driven size variance — long-titled nodes are bigger
/// and need a wider inset.
fn truncate_segment(
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
    ar: f64,
    br: f64,
) -> (f64, f64, f64, f64) {
    let dx = bx - ax;
    let dy = by - ay;
    let len = (dx * dx + dy * dy).sqrt();
    if len < ar + br {
        // Nodes overlap — return the raw segment so something
        // visible still renders.
        return (ax, ay, bx, by);
    }
    let nx = dx / len;
    let ny = dy / len;
    (ax + nx * ar, ay + ny * ar, bx - nx * br, by - ny * br)
}

/// Emit the SVG element for one shape, centred on `(x, y)`,
/// sized to `(half_w, half_h)`.
fn shape_svg(shape: ShapeKind, x: f64, y: f64, w: f64, h: f64, fill: &str) -> String {
    let stroke = "#444444";
    let sw = "1.2";
    match shape {
        ShapeKind::Box => format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" ry=\"4\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>\n",
            x - w, y - h, 2.0 * w, 2.0 * h,
        ),
        ShapeKind::Ellipse => format!(
            "<ellipse cx=\"{x:.1}\" cy=\"{y:.1}\" rx=\"{}\" ry=\"{}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>\n",
            w, h,
        ),
        ShapeKind::Folder => {
            // Folder = rect with a tab notched on the top-left.
            let tab_w = w * 0.4;
            let tab_h = h * 0.4;
            format!(
                "<path d=\"M {tlx:.1},{tly:.1} L {tbx:.1},{tly:.1} L {tbx2:.1},{tlym:.1} L {trx:.1},{tlym:.1} L {trx:.1},{bry:.1} L {tlx:.1},{bry:.1} Z\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>\n",
                tlx = x - w,
                tly = y - h - tab_h,
                tbx = x - w + tab_w,
                tbx2 = x - w + tab_w + 8.0,
                tlym = y - h,
                trx = x + w,
                bry = y + h,
            )
        }
        ShapeKind::Octagon => polygon_points(x, y, w, h, &[
            (-0.6, -1.0), (0.6, -1.0), (1.0, -0.5), (1.0, 0.5),
            (0.6, 1.0), (-0.6, 1.0), (-1.0, 0.5), (-1.0, -0.5),
        ], fill, stroke, sw),
        ShapeKind::Hexagon => polygon_points(x, y, w, h, &[
            (-0.7, -1.0), (0.7, -1.0), (1.0, 0.0),
            (0.7, 1.0), (-0.7, 1.0), (-1.0, 0.0),
        ], fill, stroke, sw),
        ShapeKind::Diamond => polygon_points(x, y, w * 0.95, h * 1.5, &[
            (0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0),
        ], fill, stroke, sw),
        ShapeKind::Egg => {
            // Wider at bottom — approximate via two arcs.
            format!(
                "<path d=\"M {lx:.1},{cy:.1} C {lx:.1},{ty:.1} {rx:.1},{ty:.1} {rx:.1},{cy:.1} C {rx:.1},{by:.1} {lx:.1},{by:.1} {lx:.1},{cy:.1} Z\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>\n",
                lx = x - w * 0.7,
                rx = x + w * 0.7,
                ty = y - h * 1.7,
                cy = y - h * 0.2,
                by = y + h * 1.6,
            )
        }
        ShapeKind::Note => {
            // Rect with a folded top-right corner.
            let fold = h * 0.6;
            format!(
                "<path d=\"M {lx:.1},{ty:.1} L {rxf:.1},{ty:.1} L {rx:.1},{tyf:.1} L {rx:.1},{by:.1} L {lx:.1},{by:.1} Z\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>\n\
                 <path d=\"M {rxf:.1},{ty:.1} L {rxf:.1},{tyf:.1} L {rx:.1},{tyf:.1}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>\n",
                lx = x - w,
                rx = x + w,
                rxf = x + w - fold,
                ty = y - h,
                tyf = y - h + fold,
                by = y + h,
            )
        }
        ShapeKind::Parallelogram => polygon_points(x, y, w, h, &[
            (-0.7, -1.0), (1.0, -1.0), (0.7, 1.0), (-1.0, 1.0),
        ], fill, stroke, sw),
        ShapeKind::Cds => {
            // Chevron / flag — pentagonal arrow pointing right.
            polygon_points(x, y, w, h, &[
                (-1.0, -1.0), (0.6, -1.0), (1.0, 0.0),
                (0.6, 1.0), (-1.0, 1.0),
            ], fill, stroke, sw)
        }
    }
}

fn polygon_points(
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    norm: &[(f64, f64)],
    fill: &str,
    stroke: &str,
    sw: &str,
) -> String {
    let pts: Vec<String> = norm
        .iter()
        .map(|(nx, ny)| format!("{:.1},{:.1}", cx + nx * rx, cy + ny * ry))
        .collect();
    format!(
        "<polygon points=\"{}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>\n",
        pts.join(" "),
    )
}

// ── SVG → PNG (resvg + tiny-skia) ─────────────────────────────

/// Process-wide `usvg::fontdb::Database` shared across every
/// story-view render. Loading system fonts is a one-shot
/// startup cost (~50–500 ms on a typical desktop); cache it so
/// repeated `Ctrl+V W` presses don't re-scan the disk.
///
/// The serif / sans-serif / cursive / fantasy / monospace
/// fallback families are set to the names usvg / resvg's
/// upstream CLI uses, so on any reasonable system at least one
/// font resolves for our default `Helvetica, Arial, sans-serif`
/// label CSS.
static STORY_FONTDB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();

fn story_fontdb() -> Arc<usvg::fontdb::Database> {
    STORY_FONTDB
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            db.set_serif_family("Times New Roman");
            db.set_sans_serif_family("Helvetica");
            db.set_cursive_family("Comic Sans MS");
            db.set_fantasy_family("Impact");
            db.set_monospace_family("Courier New");
            Arc::new(db)
        })
        .clone()
}

fn svg_to_png(svg_string: &str) -> Result<StoryRender, String> {
    let opts = usvg::Options {
        fontdb: story_fontdb(),
        ..usvg::Options::default()
    };
    let tree =
        usvg::Tree::from_str(svg_string, &opts).map_err(|e| format!("svg parse: {e}"))?;
    let int_size = tree.size().to_int_size();
    let (w, h) = (int_size.width(), int_size.height());
    if w == 0 || h == 0 {
        return Err("rendered SVG has zero size".into());
    }
    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| format!("cannot allocate {w}×{h} pixmap"))?;
    pixmap.fill(tiny_skia::Color::WHITE);
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    let png_bytes = pixmap.encode_png().map_err(|e| format!("encode PNG: {e}"))?;
    let rgba = image::RgbaImage::from_raw(w, h, pixmap.data().to_vec())
        .ok_or_else(|| "pixmap dimensions disagree with buffer".to_owned())?;
    let image = image::DynamicImage::ImageRgba8(rgba);
    Ok(StoryRender {
        width: w,
        height: h,
        png_bytes,
        image,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_xml_handles_metas() {
        let out = escape_xml("a < b & c > d \"e\"");
        assert!(out.contains("&lt;"));
        assert!(out.contains("&amp;"));
        assert!(out.contains("&gt;"));
        assert!(out.contains("&quot;"));
    }

    #[test]
    fn wrap_short_label_stays_one_line() {
        let lines = wrap_label("The storm");
        assert_eq!(lines, vec!["The storm".to_string()]);
    }

    #[test]
    fn wrap_long_label_breaks_on_words() {
        let lines = wrap_label("Chapter three: the bell tower at dawn");
        assert!(lines.len() >= 2, "expected wrap, got {lines:?}");
        for line in &lines {
            assert!(
                line.chars().count() <= WRAP_LINE_CHARS,
                "line over cap: {line:?}",
            );
        }
    }

    #[test]
    fn wrap_extra_long_falls_back_to_ellipsis() {
        let lines = wrap_label(&"word ".repeat(60));
        assert!(lines.len() <= MAX_LINES);
        assert!(lines.last().unwrap().ends_with('…'));
    }

    #[test]
    fn size_grows_with_label() {
        let small = size_for_label(&vec!["x".to_string()]);
        let big = size_for_label(&vec![
            "Long line one".into(),
            "Long line two".into(),
            "Long line three".into(),
        ]);
        assert!(big.0 >= small.0, "wider label should not shrink");
        assert!(big.1 > small.1, "more lines should be taller");
    }

    #[test]
    fn radial_places_root_at_origin() {
        let mut children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        let root = Uuid::nil();
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        children.insert(root, vec![a, b]);
        let mut leaves = HashMap::new();
        count_leaves(root, &children, &mut leaves);
        let mut positions = HashMap::new();
        let ring = 220.0;
        place_radial(root, 0.0, TAU, 0, &children, &leaves, ring, &mut positions);
        let &(rx, ry) = positions.get(&root).unwrap();
        assert!(rx.abs() < 1e-9 && ry.abs() < 1e-9);
        // Children should be on ring 1.
        let &(ax, ay) = positions.get(&a).unwrap();
        let &(bx, by) = positions.get(&b).unwrap();
        let ar = (ax * ax + ay * ay).sqrt();
        let br = (bx * bx + by * by).sqrt();
        assert!((ar - ring).abs() < 1e-6);
        assert!((br - ring).abs() < 1e-6);
    }

    /// `#[ignore]` smoke — exercises the full SVG-build + resvg
    /// path on a hand-built mini-graph including a multi-line
    /// label. Requires system fonts.
    #[test]
    #[ignore]
    fn end_to_end_render_smoke() {
        let mk = |id: u128, x: f64, y: f64, lines: &[&str], shape, fill| {
            let label_lines: Vec<String> =
                lines.iter().map(|s| s.to_string()).collect();
            let (half_w, half_h) = size_for_label(&label_lines);
            GraphNode {
                id: Uuid::from_u128(id),
                label_lines,
                shape,
                fill,
                x,
                y,
                half_w,
                half_h,
            }
        };
        let ring = 220.0;
        let graph = Graph {
            nodes: vec![
                mk(0, 0.0, 0.0, &["Root book"], ShapeKind::Folder, "#fff7e6"),
                mk(
                    1,
                    ring,
                    0.0,
                    &["Chapter three:", "bell tower"],
                    ShapeKind::Box,
                    "#e6f4ff",
                ),
                mk(2, 0.0, ring, &["A"], ShapeKind::Ellipse, "#ffffff"),
            ],
            edges: vec![
                GraphEdge {
                    from: Uuid::from_u128(0),
                    to: Uuid::from_u128(1),
                    style: EdgeStyle::Structural,
                },
                GraphEdge {
                    from: Uuid::from_u128(0),
                    to: Uuid::from_u128(2),
                    style: EdgeStyle::WikiLink,
                },
            ],
            extent: ring + 80.0,
        };
        let svg = render_svg(&graph);
        let render = svg_to_png(&svg).expect("rasterise");
        assert!(render.width > 0 && render.height > 0);
        assert_eq!(
            &render.png_bytes[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        );
        // Multi-line text should show up as ≥2 tspans in the SVG
        // (regression-guard the wrapping path).
        let tspan_count = svg.matches("<tspan").count();
        assert!(tspan_count >= 2, "expected >=2 tspans, got {tspan_count}");
    }
}
