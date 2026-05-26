//! Map a `BinderItem` to its inkhaven `NodeKind` + destination
//! book.
//!
//! Pure data — no I/O. The orchestrator (`import.rs`) calls
//! `classify(...)` for each binder node and dispatches based on
//! the result.

use crate::store::node::NodeKind;
use crate::scrivener::binder::BinderItem;

/// Decision for one binder item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classification {
    /// Becomes a user Book at the project root. Used for the
    /// Scrivener Draft / Manuscript folder.
    UserBook,
    /// Becomes a Chapter under whichever book is currently in
    /// scope. Triggered when a `Folder` appears one level
    /// below the Draft root.
    Chapter,
    /// Becomes a Subchapter (any deeper Folder).
    Subchapter,
    /// Becomes a Paragraph leaf. RTF body gets converted to
    /// Typst and stored as the paragraph content.
    Paragraph,
    /// Routed into the named system book (`places`, `characters`,
    /// `notes`, `research`, `artefacts`). Used when a top-level
    /// Folder has a recognisable name outside the Draft.
    SystemBook(&'static str),
    /// Skip this item but keep walking its children. Used for
    /// unknown / `Other`-type wrappers we don't want to drop the
    /// subtree underneath.
    SkipKeepChildren,
    /// Skip this item AND its subtree. Used for binary
    /// attachments etc.
    SkipSubtree,
}

/// Compute the classification for `item` given its `depth_in_draft`
/// (None = outside the Draft folder). The Draft root itself is
/// passed in with `depth_in_draft = Some(0)`; its immediate children
/// are at `1`; their children at `2`; etc.
pub fn classify(
    item: &BinderItem,
    depth_in_draft: Option<usize>,
) -> Classification {
    // Inside the Draft: kind drives the decision.
    if let Some(depth) = depth_in_draft {
        return match (item.kind.as_str(), depth) {
            ("DraftFolder", 0) => Classification::UserBook,
            ("Folder", 1) => Classification::Chapter,
            ("Folder", _) => Classification::Subchapter,
            ("Text", _) => Classification::Paragraph,
            // `Other` / unknown inside the Draft → preserve the
            // subtree by classifying as a Subchapter wrapper.
            // The user can clean up later.
            (_, _) => Classification::Subchapter,
        };
    }
    // Outside the Draft: route by title where possible.
    let title_lower = item.title.to_ascii_lowercase();
    let known_bucket = match title_lower.as_str() {
        "places" | "locations" | "settings" => Some("places"),
        "characters" | "cast" => Some("characters"),
        "notes" => Some("notes"),
        "research" => Some("notes"), // merge into Notes
        "artefacts" | "artifacts" | "items" => Some("artefacts"),
        _ => None,
    };
    if let Some(tag) = known_bucket {
        return Classification::SystemBook(tag);
    }
    // Unrecognised outside-Draft items: skip wrapper, keep
    // children — children might be useful documents that just
    // live under a custom organisational folder.
    if item.kind == "Folder" {
        Classification::SkipKeepChildren
    } else {
        Classification::SkipSubtree
    }
}

/// Once `classify` returns `Paragraph`, the orchestrator needs
/// to know which inkhaven `NodeKind` to create. The mapping is
/// trivially `NodeKind::Paragraph` today; broken out so future
/// special-casing (Scrivener "Text" with binary attachments →
/// `Image`?) lands here without spreading.
pub fn node_kind_for(c: &Classification) -> Option<NodeKind> {
    match c {
        Classification::UserBook | Classification::SystemBook(_) => {
            Some(NodeKind::Book)
        }
        Classification::Chapter => Some(NodeKind::Chapter),
        Classification::Subchapter => Some(NodeKind::Subchapter),
        Classification::Paragraph => Some(NodeKind::Paragraph),
        Classification::SkipKeepChildren | Classification::SkipSubtree => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrivener::binder::BinderItem;
    use uuid::Uuid;

    fn item(kind: &str, title: &str) -> BinderItem {
        BinderItem {
            uuid: Uuid::nil(),
            kind: kind.into(),
            title: title.into(),
            children: Vec::new(),
            keywords: Vec::new(),
            custom_meta: Vec::new(),
        }
    }

    #[test]
    fn draft_root_is_user_book() {
        let i = item("DraftFolder", "Manuscript");
        assert_eq!(classify(&i, Some(0)), Classification::UserBook);
    }

    #[test]
    fn folder_depth_drives_chapter_vs_subchapter() {
        let i = item("Folder", "Chapter 1");
        assert_eq!(classify(&i, Some(1)), Classification::Chapter);
        assert_eq!(classify(&i, Some(2)), Classification::Subchapter);
        assert_eq!(classify(&i, Some(3)), Classification::Subchapter);
    }

    #[test]
    fn text_is_always_paragraph_inside_draft() {
        let i = item("Text", "The Storm");
        assert_eq!(classify(&i, Some(2)), Classification::Paragraph);
    }

    #[test]
    fn outside_draft_known_folders_route() {
        let i = item("Folder", "Characters");
        assert_eq!(
            classify(&i, None),
            Classification::SystemBook("characters")
        );
        let i = item("Folder", "Research");
        assert_eq!(classify(&i, None), Classification::SystemBook("notes"));
    }

    #[test]
    fn outside_draft_unknown_folder_keeps_children() {
        let i = item("Folder", "Reference Materials");
        assert_eq!(classify(&i, None), Classification::SkipKeepChildren);
    }
}
