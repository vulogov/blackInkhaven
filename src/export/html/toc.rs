//! TDOC-4 — the site model + navigation, derived deterministically from the
//! hierarchy. One page per chapter; subchapters become `<h2>` sections within
//! their chapter page (and anchors in the nav).

use serde::Serialize;
use uuid::Uuid;

use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

use super::markdown_html::slugify;

/// A rendered page (one per chapter).
pub struct Chapter {
    pub title: String,
    /// File name, e.g. `ch01-the-tide.html`.
    pub file: String,
    /// Content nodes (subchapters + paragraphs) in DFS preorder.
    pub content: Vec<ContentNode>,
    /// Subchapter anchors, for the nav.
    pub sections: Vec<NavSection>,
}

pub struct ContentNode {
    pub id: Uuid,
    pub kind: NodeKind,
    /// For a subchapter: its heading anchor id.
    pub anchor: Option<String>,
    pub title: String,
}

/// One nav entry (chapter), with its subchapter anchors.
#[derive(Debug, Clone, Serialize)]
pub struct NavItem {
    pub title: String,
    pub href: String,
    pub current: bool,
    pub sections: Vec<NavSection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NavSection {
    pub title: String,
    pub href: String,
}

/// The whole site: chapters + any book-level front-matter paragraphs (index page).
pub struct SiteModel {
    pub chapters: Vec<Chapter>,
    pub front_matter: Vec<Uuid>,
}

/// Build the site model for a user book. `timeline_ids` are chapters to skip
/// (the book Timeline). Only paragraphs/subchapters/chapters are considered.
pub fn build(h: &Hierarchy, book_id: Uuid, timeline_ids: &std::collections::HashSet<Uuid>) -> SiteModel {
    let mut chapters = Vec::new();
    let mut front_matter = Vec::new();

    for (i, child) in h.children_of(Some(book_id)).into_iter().enumerate() {
        match child.kind {
            NodeKind::Paragraph => front_matter.push(child.id),
            NodeKind::Chapter => {
                if timeline_ids.contains(&child.id) {
                    continue;
                }
                let file = format!("ch{:02}-{}.html", i + 1, slugify(&child.title));
                let mut content = Vec::new();
                let mut sections = Vec::new();
                // DFS preorder of the chapter's subtree, skipping the chapter node itself.
                for id in h.collect_subtree(child.id) {
                    if id == child.id {
                        continue;
                    }
                    let Some(node) = h.get(id) else { continue };
                    match node.kind {
                        NodeKind::Subchapter => {
                            let anchor = slugify(&node.title);
                            sections.push(NavSection {
                                title: node.title.clone(),
                                href: format!("{file}#{anchor}"),
                            });
                            content.push(ContentNode {
                                id,
                                kind: NodeKind::Subchapter,
                                anchor: Some(anchor),
                                title: node.title.clone(),
                            });
                        }
                        NodeKind::Paragraph => {
                            if node.event.is_some() {
                                continue; // timeline event leaf
                            }
                            content.push(ContentNode {
                                id,
                                kind: NodeKind::Paragraph,
                                anchor: None,
                                title: node.title.clone(),
                            });
                        }
                        _ => {}
                    }
                }
                chapters.push(Chapter { title: child.title.clone(), file, content, sections });
            }
            _ => {}
        }
    }

    SiteModel { chapters, front_matter }
}

/// The nav list (one entry per chapter), with `current` marking the active page.
pub fn nav(chapters: &[Chapter], current_file: &str) -> Vec<NavItem> {
    chapters
        .iter()
        .map(|c| NavItem {
            title: c.title.clone(),
            href: c.file.clone(),
            current: c.file == current_file,
            sections: c.sections.clone(),
        })
        .collect()
}
