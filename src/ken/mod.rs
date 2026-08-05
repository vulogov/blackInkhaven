//! KEN-1 (2.6) — epistemic continuity: who knows what, when.
//!
//! KEN is SENTINEL's "referenced-before-introduced" invariant extended to
//! knowledge. SENTINEL flags an entity *named before it exists*; KEN flags a
//! character *acting on a fact before they could know it* — same forward-walk +
//! mention-detection shape, a new axis. It reasons only over what it can ground
//! (event presence, author `secret:`/`know:` tags, named mentions in attributed
//! dialogue / POV narration); it stays silent where it can't, and it never edits
//! prose.
//!
//! KEN-P0 laid the pure substrate (reading-order position, the knowledge model,
//! the finding shape); KEN-P1..P6 wired it into grants, use-detection, the check,
//! the worklist bridge, the dashboard, and the opt-in LLM `implied_irony` pass.

pub mod check;
pub mod deep;
pub mod grants;
pub mod walk;

use uuid::Uuid;

/// A position in reading order — the forward-walk key. Ordered lexicographically
/// (chapter first, then scene within the chapter), so `Ord` gives "earlier in the
/// book" for free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct ScenePos {
    pub chapter_ord: u32,
    pub scene_index: u32,
}

/// A knowable thing — an event subject, a named entity, or an author-declared
/// secret. `secret` topics raise the severity of an ungranted reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeItem {
    pub topic: String,
    pub secret: bool,
}

/// How a character came to (possibly) know a topic — the provenance of a grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantSource {
    /// Present at the event that establishes it (from `TlEvent.characters`).
    Presence,
    /// The author declared it (a `know:` / `secret:` tag).
    Declared,
}

/// The earliest point a character could know a topic. `anchor` is the paragraph
/// the grant was derived from (the `know:` tag's paragraph, or the event's linked
/// paragraph) — the jump target for a `dropped_reveal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub character: String,
    pub topic: String,
    pub at: ScenePos,
    pub source: GrantSource,
    pub anchor: Option<Uuid>,
}

/// How a topic surfaced as *used* by a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseVia {
    /// Named in the character's own attributed dialogue (DIALOG-1 speaker).
    Dialogue,
    /// Referenced in narration while this character holds the scene's POV.
    Pov,
}

/// A place where a character references / acts on a topic — the thing checked
/// against their earliest grant. `anchor` is the paragraph the use occurs in (the
/// jump target for a finding).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    pub character: String,
    pub topic: String,
    pub at: ScenePos,
    pub via: UseVia,
    pub anchor: Uuid,
}

/// Epistemic-finding severity. Higher `rank` = more severe (matches the sibling
/// readers' convention).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A nudge / soft observation.
    Info,
    /// A dangling knowledge thread (e.g. a reveal that never lands).
    Notice,
    /// A hard epistemic violation — a character knows what they can't yet.
    Break,
}

impl Severity {
    pub fn rank(self) -> u8 {
        match self {
            Severity::Info => 0,
            Severity::Notice => 1,
            Severity::Break => 2,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Notice => "notice",
            Severity::Break => "break",
        }
    }
}

/// One epistemic-continuity finding (mirrors `ContinuityFinding` / `ReaderFinding`).
/// `kind` is one of `premature_knowledge` | `leaked_secret` | `dropped_reveal` |
/// `implied_irony`; `anchor` is the paragraph to jump to when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeFinding {
    pub kind: &'static str,
    pub severity: Severity,
    pub chapter: u32,
    pub anchor: Option<Uuid>,
    pub character: String,
    pub topic: String,
    pub message: String,
}

/// The earliest grant of `topic` to `character`, if any — the point at which they
/// could first know it. `None` means they were never granted it (any use is
/// premature). Pure.
pub fn earliest_grant<'a>(grants: &'a [Grant], character: &str, topic: &str) -> Option<&'a Grant> {
    grants
        .iter()
        .filter(|g| g.character == character && g.topic == topic)
        .min_by_key(|g| g.at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_pos_orders_by_chapter_then_scene() {
        let a = ScenePos { chapter_ord: 1, scene_index: 2 };
        let b = ScenePos { chapter_ord: 2, scene_index: 0 };
        let c = ScenePos { chapter_ord: 1, scene_index: 3 };
        assert!(a < b, "earlier chapter is earlier");
        assert!(a < c, "same chapter, earlier scene is earlier");
        assert!(c < b, "chapter dominates scene");
        assert_eq!(a.min(c), a);
    }

    #[test]
    fn severity_rank_orders_break_highest() {
        assert!(Severity::Break.rank() > Severity::Notice.rank());
        assert!(Severity::Notice.rank() > Severity::Info.rank());
        assert_eq!(Severity::Break.label(), "break");
    }

    #[test]
    fn earliest_grant_picks_the_first_matching_and_ignores_others() {
        let grants = vec![
            Grant {
                character: "Mara".into(),
                topic: "the betrayal".into(),
                at: ScenePos { chapter_ord: 7, scene_index: 0 },
                source: GrantSource::Declared,
                anchor: None,
            },
            Grant {
                character: "Mara".into(),
                topic: "the betrayal".into(),
                at: ScenePos { chapter_ord: 4, scene_index: 1 },
                source: GrantSource::Presence, // earlier — should win
                anchor: None,
            },
            Grant {
                character: "Bob".into(),
                topic: "the betrayal".into(),
                at: ScenePos { chapter_ord: 1, scene_index: 0 },
                source: GrantSource::Presence, // different character
                anchor: None,
            },
            Grant {
                character: "Mara".into(),
                topic: "the map".into(),
                at: ScenePos { chapter_ord: 1, scene_index: 0 },
                source: GrantSource::Declared, // different topic
                anchor: None,
            },
        ];
        let g = earliest_grant(&grants, "Mara", "the betrayal").expect("Mara knows the betrayal");
        assert_eq!(g.at, ScenePos { chapter_ord: 4, scene_index: 1 });
        assert_eq!(g.source, GrantSource::Presence);
        assert!(earliest_grant(&grants, "Mara", "the heir").is_none(), "never granted → None");
        assert!(earliest_grant(&grants, "Sella", "the betrayal").is_none(), "other character → None");
    }
}
