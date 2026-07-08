//! Multi-format book export.
//!
//! Phase 1 of the "more than PDF" track. Three pure-Rust converters
//! consume the same `combined` Typst source the existing
//! `cli::export::typst` path already builds:
//!
//! * `markdown` — typst-source → markdown converter
//!   ([`markdown::typst_to_markdown`]). Handles the subset inkhaven
//!   itself emits: `= Heading` levels, italics / bold, lists,
//!   `#image()`, `#raw(…)`, citations. Unknown Typst macros land
//!   verbatim inside fenced ``` ```typst ``` blocks so nothing
//!   silently disappears.
//! * `tex` — typst → LaTeX via the `tylax` crate
//!   ([`tex::typst_to_tex`]). Errors propagate as
//!   `anyhow::Error` so the CLI surfaces them at the call site.
//! * `epub` — markdown → minimal EPUB3 zip
//!   ([`epub::write_epub`]). One paragraph per `nav` entry, built
//!   from the same hierarchy walk the typst exporter uses.
//!
//! Everything in this module is **deterministic** given the same
//! project state — used both from `inkhaven export <fmt>` and from
//! the TUI's "Ctrl+B O extra formats" + "Ctrl+V" handlers, so the
//! same source-of-truth produces every artefact regardless of how
//! the user triggered it.

pub mod docx;
pub mod epub;
pub mod markdown;
pub mod tex;

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

/// Concatenate every paragraph's `.typ` file under `root_id` (or
/// the entire hierarchy if `root_id` is `None`) in DFS preorder.
/// Identical assembly rule the legacy `cli::export::typst` path
/// uses — moved here so every exporter starts from one canonical
/// source string.
///
/// Branch nodes don't emit anything themselves: paragraphs carry
/// their headings via the `= Title` template `inkhaven add
/// paragraph` writes. The user controls document structure by
/// ordering paragraphs.
pub fn assemble_typst_source(
    layout: &ProjectLayout,
    hierarchy: &Hierarchy,
    root_id: Option<uuid::Uuid>,
) -> Result<String> {
    assemble_typst_source_filtered(layout, hierarchy, root_id, None, None)
}

/// Same as [`assemble_typst_source`], but only emits paragraphs
/// whose status sits **at or above** `status_floor_idx` on the
/// canonical ladder `none → napkin → first → second → third →
/// final → ready` (0..=6). None means "no floor — include every
/// paragraph". A paragraph with no status set is treated as
/// index 0 (`none`); a `--status=napkin` floor still includes
/// them since 0 ≥ 0 is false → they're filtered out. Use
/// `--status=none` (or omit the flag) to include everything.
///
/// `tag_filter` (1.2.6+) is an additional predicate: when set,
/// only paragraphs carrying that tag (case-insensitive match)
/// are emitted. Combines with the status floor — both must
/// pass.
pub fn assemble_typst_source_filtered(
    layout: &ProjectLayout,
    hierarchy: &Hierarchy,
    root_id: Option<uuid::Uuid>,
    status_floor_idx: Option<usize>,
    tag_filter: Option<&str>,
) -> Result<String> {
    assemble_typst_source_profiled(layout, hierarchy, root_id, status_floor_idx, tag_filter, &[])
}

/// TDOC-3 — [`assemble_typst_source_filtered`] plus *single-sourcing*:
/// * **Profiles** — `profiles` is a list of `(dimension, value)` from `export
///   --profile`. A paragraph tagged `profile:<dim>:<v>` is emitted only when a
///   requested `(dim, value)` matches; a paragraph with no `profile:<dim>:*` tag
///   for a requested dimension is unconditional (always emitted). Dimensions not
///   requested are ignored (all their variants pass) — the DITA convention.
/// * **Variables** — `{{key}}` in a body is replaced by `docs.variables.key` from
///   config, in every export.
pub fn assemble_typst_source_profiled(
    layout: &ProjectLayout,
    hierarchy: &Hierarchy,
    root_id: Option<uuid::Uuid>,
    status_floor_idx: Option<usize>,
    tag_filter: Option<&str>,
    profiles: &[(String, String)],
) -> Result<String> {
    // TDOC-3 variables — loaded once; applied to every emitted body.
    let variables = crate::config::Config::load_layered(&layout.config_path())
        .map(|c| c.docs.variables)
        .unwrap_or_default();
    let tag_filter_norm =
        tag_filter.map(|t| t.trim().to_ascii_lowercase());
    let mut out = String::new();
    let candidates: Vec<&Node> = if let Some(root_id) = root_id {
        // Subtree mode — only the descendants of `root_id`, plus
        // the root itself if it carries content. `collect_subtree`
        // is DFS preorder, which matches our overall walk order.
        hierarchy
            .collect_subtree(root_id)
            .into_iter()
            .filter_map(|id| hierarchy.get(id))
            .collect()
    } else {
        hierarchy.flatten().into_iter().map(|(n, _)| n).collect()
    };

    // 1.2.6+: identify the Timeline chapter so its paragraphs
    // never land in the exported prose. The chapter carries the
    // `book_timeline` system_tag; we also belt-and-brace by
    // filtering individual event paragraphs (those with
    // `node.event.is_some()`) in case a stray event lives
    // elsewhere.
    let timeline_chapter_ids: std::collections::HashSet<uuid::Uuid> = hierarchy
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Chapter
                && n.system_tag.as_deref()
                    == Some(crate::store::SYSTEM_TAG_BOOK_TIMELINE)
        })
        .map(|n| n.id)
        .collect();

    for node in candidates {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        // 1.2.6+: skip event paragraphs AND any paragraph whose
        // parent is the Timeline chapter — timeline data is
        // metadata about the manuscript, not manuscript prose.
        if node.event.is_some() {
            continue;
        }
        if let Some(parent_id) = node.parent_id {
            if timeline_chapter_ids.contains(&parent_id) {
                continue;
            }
        }
        if let Some(floor) = status_floor_idx {
            let idx = status_ladder_index(node.status.as_deref());
            if idx < floor {
                continue;
            }
        }
        if let Some(needle) = tag_filter_norm.as_deref() {
            let has = node
                .tags
                .iter()
                .any(|t| t.to_ascii_lowercase() == needle);
            if !has {
                continue;
            }
        }
        // TDOC-3 — conditional (profiled) content.
        if !profile_matches(&node.tags, profiles) {
            continue;
        }
        let Some(rel) = node.file.as_ref() else {
            continue;
        };
        let abs = layout.root.join(rel);
        let mut body = std::fs::read_to_string(&abs)?;
        // TDOC-3 — resolve single-sourcing variables.
        if !variables.is_empty() {
            for (k, v) in &variables {
                body = body.replace(&format!("{{{{{k}}}}}"), v);
            }
        }
        if !out.is_empty() && !out.ends_with("\n\n") {
            if out.ends_with('\n') {
                out.push('\n');
            } else {
                out.push_str("\n\n");
            }
        }
        out.push_str(&body);
        if !body.ends_with('\n') {
            out.push('\n');
        }
    }
    Ok(out)
}

/// TDOC-3 — whether a node passes the requested profiles. For each requested
/// `(dimension, value)`: if the node carries any `profile:<dimension>:*` tag, at
/// least one must equal `value`; a node with no tag for that dimension is
/// unconditional. Dimensions not requested never exclude.
fn profile_matches(tags: &[String], profiles: &[(String, String)]) -> bool {
    for (dim, want) in profiles {
        let prefix = format!("profile:{}:", dim.to_ascii_lowercase());
        let vals: Vec<String> = tags
            .iter()
            .filter_map(|t| t.to_ascii_lowercase().strip_prefix(&prefix).map(str::to_string))
            .collect();
        if vals.is_empty() {
            continue; // unconditional for this dimension
        }
        if !vals.iter().any(|v| v == &want.to_ascii_lowercase()) {
            return false; // tagged for this dimension, but not the requested value
        }
    }
    true
}

/// Map a paragraph's `status` field to its ladder index. Unknown
/// values + None both collapse to 0 (`none`).
fn status_ladder_index(s: Option<&str>) -> usize {
    let Some(s) = s else { return 0 };
    match s.trim().to_ascii_lowercase().as_str() {
        "none" | "" => 0,
        "napkin" => 1,
        "first" => 2,
        "second" => 3,
        "third" => 4,
        "final" => 5,
        "ready" => 6,
        _ => 0,
    }
}

/// Format-tagged output bundle. The CLI writes whichever
/// variant matches the requested `--format`; the TUI's Ctrl+B O
/// extra-formats path writes all configured variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Artefact {
    Markdown(String),
    Tex(String),
    /// Pre-zipped EPUB bytes.
    Epub(Vec<u8>),
}

impl Artefact {
    /// File extension to append when the caller didn't supply
    /// one. Lowercase, dotless.
    pub fn extension(&self) -> &'static str {
        match self {
            Artefact::Markdown(_) => "md",
            Artefact::Tex(_) => "tex",
            Artefact::Epub(_) => "epub",
        }
    }

    /// Write to `path`, using the format-appropriate byte
    /// encoding (UTF-8 text for markdown/tex, raw bytes for epub).
    pub fn write_to(&self, path: &Path) -> Result<()> {
        match self {
            Artefact::Markdown(s) | Artefact::Tex(s) => {
                std::fs::write(path, s.as_bytes())?;
            }
            Artefact::Epub(bytes) => {
                std::fs::write(path, bytes)?;
            }
        }
        Ok(())
    }
}

/// Build `Artefact::Markdown` from a Typst source string.
pub fn build_markdown(combined: &str) -> Artefact {
    Artefact::Markdown(markdown::typst_to_markdown(combined))
}

/// Build `Artefact::Tex` from a Typst source string. The tylax
/// converter is best-effort: it returns whatever LaTeX it could
/// emit; unknown macros land verbatim. We don't second-guess —
/// the caller writes the bytes out and moves on.
pub fn build_tex(combined: &str) -> Artefact {
    Artefact::Tex(tex::typst_to_tex(combined))
}

/// Build `Artefact::Epub` from a markdown source string. `title`
/// shows up in the EPUB metadata + nav.
pub fn build_epub(markdown_src: &str, title: &str) -> Result<Artefact> {
    let bytes = epub::write_epub(markdown_src, title)?;
    Ok(Artefact::Epub(bytes))
}

/// Replace the file extension on `path` with the artefact's
/// canonical extension. Used by the Ctrl+B O extra-formats path
/// so every output lands next to the PDF with the same stem.
pub fn with_artefact_extension(path: &Path, artefact: &Artefact) -> PathBuf {
    path.with_extension(artefact.extension())
}

#[cfg(test)]
mod profile_tests {
    use super::profile_matches;

    fn tags(ts: &[&str]) -> Vec<String> {
        ts.iter().map(|s| s.to_string()).collect()
    }
    fn prof(ps: &[(&str, &str)]) -> Vec<(String, String)> {
        ps.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn tdoc3_profile_matching() {
        // No profiles requested → everything passes.
        assert!(profile_matches(&tags(&["profile:edition:enterprise"]), &[]));
        // Untagged for a requested dimension → unconditional (passes).
        assert!(profile_matches(&tags(&["para:code"]), &prof(&[("edition", "enterprise")])));
        // Tagged with the requested value → passes (case-insensitive).
        assert!(profile_matches(&tags(&["profile:edition:enterprise"]), &prof(&[("edition", "Enterprise")])));
        // Tagged for the dimension but a different value → excluded.
        assert!(!profile_matches(&tags(&["profile:edition:community"]), &prof(&[("edition", "enterprise")])));
        // Multiple values on one dimension → any match passes.
        assert!(profile_matches(
            &tags(&["profile:audience:beginner", "profile:audience:expert"]),
            &prof(&[("audience", "expert")])
        ));
        // A dimension the node isn't tagged for never excludes, even if another is checked.
        assert!(profile_matches(
            &tags(&["profile:edition:enterprise"]),
            &prof(&[("edition", "enterprise"), ("audience", "expert")])
        ));
    }
}
