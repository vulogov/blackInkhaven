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
const RING_SPACING: f64 = 220.0;
/// Maximum half-width of a node shape — used both as the
/// drawn-width heuristic and as the boundary-stop distance when
/// truncating edge endpoints.
const NODE_HALF_W: f64 = 70.0;
/// Maximum half-height. Most shapes are wider than tall; this
/// gates label vertical-centering.
const NODE_HALF_H: f64 = 22.0;
/// Margin around the laid-out graph before SVG viewBox is
/// computed. Big enough to cover label overflow on the outer
/// ring nodes.
const SVG_MARGIN: f64 = 100.0;

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
    label: String,
    shape: ShapeKind,
    fill: &'static str,
    /// Position in laid-out SVG user-space (before the
    /// margin translation that `render_svg` applies).
    x: f64,
    y: f64,
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

    // Place structural nodes via twopi.
    let mut positions: HashMap<Uuid, (f64, f64)> = HashMap::new();
    place_radial(
        book.id,
        0.0,
        TAU,
        0,
        &structural_children,
        &leaves,
        &mut positions,
    );

    // Lexicon nodes — every paragraph-kind node under
    // Characters / Places / Artefacts system books. Only
    // ones actually mentioned land on the graph.
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
    let lex_ring = (max_depth as f64 + 1.0) * RING_SPACING;
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
        let label = sanitize_label(&n.title);
        nodes.push(GraphNode {
            id: n.id,
            label,
            shape,
            fill,
            x,
            y,
        });
        node_lookup.insert(n.id);
        max_extent = max_extent.max(x.abs()).max(y.abs());
    }
    for lex in &lexicon {
        if !lex_targets.contains_key(&lex.id) {
            continue;
        }
        let Some(&(x, y)) = positions.get(&lex.id) else {
            continue;
        };
        let (shape, fill) = lexicon_shape_for(lex, hierarchy);
        nodes.push(GraphNode {
            id: lex.id,
            label: sanitize_label(&lex.title),
            shape,
            fill,
            x,
            y,
        });
        node_lookup.insert(lex.id);
        max_extent = max_extent.max(x.abs()).max(y.abs());
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
/// the origin at radius `RING_SPACING`; etc.
fn place_radial(
    node: Uuid,
    theta_start: f64,
    theta_end: f64,
    depth: usize,
    children: &HashMap<Uuid, Vec<Uuid>>,
    leaves: &HashMap<Uuid, usize>,
    out: &mut HashMap<Uuid, (f64, f64)>,
) {
    let theta_mid = (theta_start + theta_end) / 2.0;
    let radius = depth as f64 * RING_SPACING;
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
        place_radial(k, cursor, cursor + wedge, depth + 1, children, leaves, out);
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

/// Escape a label for SVG `<text>` and clamp long titles.
fn sanitize_label(s: &str) -> String {
    let mut t: String = s
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', " ");
    if t.chars().count() > 28 {
        t = t.chars().take(26).collect::<String>() + "…";
    }
    t
}

// ── SVG rendering ─────────────────────────────────────────────

fn render_svg(g: &Graph) -> String {
    // Translate the laid-out coordinates so everything is
    // positive within a (W, H) viewBox.
    let dim = g.extent + SVG_MARGIN;
    let total = (dim * 2.0) as u32;
    let cx = dim;
    let cy = dim;

    let position_of: HashMap<Uuid, (f64, f64)> = g
        .nodes
        .iter()
        .map(|n| (n.id, (n.x + cx, n.y + cy)))
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
        let (Some(&(ax, ay)), Some(&(bx, by))) =
            (position_of.get(&e.from), position_of.get(&e.to))
        else {
            continue;
        };
        let (x1, y1, x2, y2) = truncate_segment(ax, ay, bx, by);
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
        let shape_svg = shape_svg(n.shape, px, py, n.fill);
        s.push_str(&shape_svg);
        // Label centred on (px, py + 4) — `+4` because SVG y is
        // baseline-ish in default style; baseline-vertical
        // alignment varies across renderers, this nudge looks
        // right with resvg's text metrics.
        s.push_str(&format!(
            "<text x=\"{px:.1}\" y=\"{ly:.1}\" text-anchor=\"middle\" dominant-baseline=\"middle\">{label}</text>\n",
            ly = py,
            label = n.label,
        ));
    }

    s.push_str("</svg>\n");
    s
}

/// Push the line endpoints back so the visible segment starts
/// at each node's circumscribing-circle boundary, not at the
/// center. Approximation — exact shape-boundary clipping isn't
/// worth the complexity at this scale.
fn truncate_segment(ax: f64, ay: f64, bx: f64, by: f64) -> (f64, f64, f64, f64) {
    let dx = bx - ax;
    let dy = by - ay;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 2.0 * NODE_HALF_W {
        // Nodes overlap — return the raw segment.
        return (ax, ay, bx, by);
    }
    let nx = dx / len;
    let ny = dy / len;
    let r = NODE_HALF_W * 0.9; // small inset
    (ax + nx * r, ay + ny * r, bx - nx * r, by - ny * r)
}

/// Emit the SVG element for one shape, centred on `(x, y)`.
fn shape_svg(shape: ShapeKind, x: f64, y: f64, fill: &str) -> String {
    let stroke = "#444444";
    let sw = "1.2";
    let w = NODE_HALF_W;
    let h = NODE_HALF_H;
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
    fn sanitize_label_escapes_and_clamps() {
        let out = sanitize_label("a < b & c > d");
        assert!(out.contains("&lt;"));
        assert!(out.contains("&amp;"));
        assert!(out.contains("&gt;"));
        let long = "x".repeat(100);
        let out = sanitize_label(&long);
        assert!(out.chars().count() <= 28);
        assert!(out.ends_with('…'));
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
        place_radial(root, 0.0, TAU, 0, &children, &leaves, &mut positions);
        let &(rx, ry) = positions.get(&root).unwrap();
        assert!(rx.abs() < 1e-9 && ry.abs() < 1e-9);
        // Children should be on ring 1.
        let &(ax, ay) = positions.get(&a).unwrap();
        let &(bx, by) = positions.get(&b).unwrap();
        let ar = (ax * ax + ay * ay).sqrt();
        let br = (bx * bx + by * by).sqrt();
        assert!((ar - RING_SPACING).abs() < 1e-6);
        assert!((br - RING_SPACING).abs() < 1e-6);
    }

    /// `#[ignore]` smoke — exercises the full radial+SVG+resvg
    /// path on a hand-built mini-graph. Requires system fonts
    /// to render labels (the same env constraint other story-
    /// view smokes have).
    #[test]
    #[ignore]
    fn end_to_end_render_smoke() {
        let svg = render_svg(&Graph {
            nodes: vec![
                GraphNode {
                    id: Uuid::nil(),
                    label: "Root".into(),
                    shape: ShapeKind::Folder,
                    fill: "#fff7e6",
                    x: 0.0,
                    y: 0.0,
                },
                GraphNode {
                    id: Uuid::from_u128(1),
                    label: "Child A".into(),
                    shape: ShapeKind::Box,
                    fill: "#e6f4ff",
                    x: RING_SPACING,
                    y: 0.0,
                },
                GraphNode {
                    id: Uuid::from_u128(2),
                    label: "Child B".into(),
                    shape: ShapeKind::Ellipse,
                    fill: "#ffffff",
                    x: 0.0,
                    y: RING_SPACING,
                },
            ],
            edges: vec![
                GraphEdge {
                    from: Uuid::nil(),
                    to: Uuid::from_u128(1),
                    style: EdgeStyle::Structural,
                },
                GraphEdge {
                    from: Uuid::nil(),
                    to: Uuid::from_u128(2),
                    style: EdgeStyle::WikiLink,
                },
            ],
            extent: RING_SPACING,
        });
        let render = svg_to_png(&svg).expect("rasterise");
        assert!(render.width > 0 && render.height > 0);
        assert_eq!(
            &render.png_bytes[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        );
    }
}
