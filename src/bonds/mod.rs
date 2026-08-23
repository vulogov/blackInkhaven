//! BONDS-1 (3.1.0) — relationship continuity: are the bonds between characters
//! earned on the page, or merely asserted?
//!
//! BONDS is KEN's sibling. Where KEN checks *knowledge* (who could know what,
//! when), BONDS checks *bonds* (how two characters relate, and whether the page
//! backs it up). Same declared-then-checked shape: the author *declares* a bond
//! with a `rel:<kind>:<A>:<B>` tag (the `know:`/`secret:` analog), inkhaven
//! *derives* the on-page co-presence for free (the scene cast + `TlEvent`
//! participants), and the **mismatch is the finding**. It reasons only over what
//! it can ground; it stays silent where it can't; it never edits prose.
//!
//! BD-P0 lays the pure substrate: the declared/derived models, the finding shape,
//! and the `rel:` tag grammar. BD-P1..P7 wire it into gather, the deterministic
//! check, the worklist bridge, the dashboard, Bund, and the opt-in `--deep`
//! `implied_cooling` pass. See `Documentation/PROPOSALS/BONDS-1_IMPL.md`.

// Scaffolding for the value core (BD-P0..P3): the model + gather land before the
// check + worklist bridge consume them. Remove this once BD-P3 wires BONDS into
// `collect`, so the warning-free bar guards the whole surface again.
#![allow(dead_code)]

mod gather;

use uuid::Uuid;

// Reading-order key + severity are shared with KEN verbatim — BONDS walks the
// same scenes and ranks findings on the same scale, so it reuses both rather than
// re-declaring them.
pub use crate::ken::{ScenePos, Severity};

/// How a bond between two characters became visible — the provenance of the
/// evidence, mirroring KEN's `GrantSource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BondSource {
    /// Two characters share a scene (derived: scene cast ∪ `TlEvent.characters`).
    CoPresence,
    /// The author declared the bond (a `rel:<kind>:<A>:<B>` tag).
    Declared,
}

/// One author-declared bond state at a point in reading order — a single `rel:`
/// tag occurrence. The pair `(a, b)` is stored **canonically** (sorted) so that
/// `rel:ally:mara:kell` and `rel:ally:kell:mara` are the same bond; `kind` is the
/// declared state *at this point* (a later differently-`kind`ed tag for the same
/// pair is a transition). `anchor` is the tag's paragraph — the jump target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    pub a: String,
    pub b: String,
    pub kind: String,
    pub at: ScenePos,
    pub anchor: Uuid,
}

impl Declared {
    /// Build a declared bond, canonicalizing the pair (normalized + sorted) so a
    /// pair is one bond regardless of the order the author wrote the two names.
    pub fn new(kind: &str, a: &str, b: &str, at: ScenePos, anchor: Uuid) -> Self {
        let (a, b) = pair_key(a, b);
        Declared { a, b, kind: normalize(kind), at, anchor }
    }
}

/// A scene two characters share — the derived evidence a bond is on the page.
/// Pair stored canonically, like [`Declared`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoScene {
    pub a: String,
    pub b: String,
    pub at: ScenePos,
    pub anchor: Uuid,
}

impl CoScene {
    pub fn new(a: &str, b: &str, at: ScenePos, anchor: Uuid) -> Self {
        let (a, b) = pair_key(a, b);
        CoScene { a, b, at, anchor }
    }
}

/// One relationship-continuity finding (mirrors `KnowledgeFinding`). `kind` is one
/// of `unwritten_bond` | `unearned_shift` | `dropped_bond` | `implied_cooling`;
/// `anchor` is the paragraph to jump to; `a`/`b` name the pair (canonical order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BondFinding {
    pub kind: &'static str,
    pub severity: Severity,
    pub chapter: u32,
    pub anchor: Option<Uuid>,
    pub a: String,
    pub b: String,
    pub message: String,
}

/// Normalize a tag token (character name or bond kind): trim + collapse internal
/// whitespace, case preserved — the same rule KEN applies to topics.
fn normalize(s: &str) -> String {
    crate::ken::grants::normalize_topic(s)
}

/// Canonical, order-independent key for a character pair: both names normalized,
/// then sorted so `(a, b)` and `(b, a)` map to the same tuple.
pub fn pair_key(a: &str, b: &str) -> (String, String) {
    let (a, b) = (normalize(a), normalize(b));
    if a <= b { (a, b) } else { (b, a) }
}

/// Parse a `rel:<kind>:<A>:<B>` tag into `(kind, a, b)` (each normalized), or
/// `None` if the tag isn't a well-formed relationship tag. The pair is returned
/// in the author's written order; callers canonicalize via [`Declared::new`] /
/// [`pair_key`].
///
/// Grammar: the `rel:` prefix, then exactly three `:`-separated non-empty fields
/// (`kind`, `A`, `B`). Character names may contain spaces but not `:`.
pub fn parse_rel_tag(tag: &str) -> Option<(String, String, String)> {
    let rest = tag.trim().strip_prefix("rel:")?;
    // `splitn(3, ':')` → [kind, A, "B"]; a 4th `:` would land inside B, so require
    // exactly three fields by checking the last doesn't itself contain `:`.
    let mut it = rest.splitn(3, ':');
    let kind = it.next()?;
    let a = it.next()?;
    let b = it.next()?;
    if b.contains(':') {
        return None; // more than three fields — malformed
    }
    let (kind, a, b) = (normalize(kind), normalize(a), normalize(b));
    if kind.is_empty() || a.is_empty() || b.is_empty() {
        return None;
    }
    Some((kind, a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(c: u32, s: u32) -> ScenePos {
        ScenePos { chapter_ord: c, scene_index: s }
    }

    #[test]
    fn parse_rel_tag_accepts_well_formed_tags() {
        assert_eq!(
            parse_rel_tag("rel:ally:Mara:Kell"),
            Some(("ally".into(), "Mara".into(), "Kell".into()))
        );
        // whitespace is trimmed/collapsed; multi-word names survive.
        assert_eq!(
            parse_rel_tag("rel: rival : Ser  Danel : Mara "),
            Some(("rival".into(), "Ser Danel".into(), "Mara".into()))
        );
    }

    #[test]
    fn parse_rel_tag_rejects_malformed() {
        assert!(parse_rel_tag("know:secret").is_none(), "wrong prefix");
        assert!(parse_rel_tag("rel:ally:Mara").is_none(), "only two fields");
        assert!(parse_rel_tag("rel:ally").is_none(), "one field");
        assert!(parse_rel_tag("rel:ally:Mara:Kell:extra").is_none(), "four fields");
        assert!(parse_rel_tag("rel::Mara:Kell").is_none(), "empty kind");
        assert!(parse_rel_tag("rel:ally::Kell").is_none(), "empty A");
    }

    #[test]
    fn pair_key_is_order_independent_and_normalized() {
        assert_eq!(pair_key("Mara", "Kell"), pair_key("Kell", "Mara"));
        assert_eq!(pair_key(" Mara ", "Kell"), ("Kell".into(), "Mara".into()));
    }

    #[test]
    fn declared_canonicalizes_the_pair() {
        let d1 = Declared::new("ally", "Mara", "Kell", pos(1, 0), Uuid::nil());
        let d2 = Declared::new("ally", "Kell", "Mara", pos(1, 0), Uuid::nil());
        assert_eq!((d1.a.clone(), d1.b.clone()), (d2.a.clone(), d2.b.clone()));
        assert_eq!(d1.a, "Kell");
        assert_eq!(d1.b, "Mara");
    }

    #[test]
    fn scene_pos_reused_from_ken_still_orders_by_reading_position() {
        assert!(pos(1, 3) < pos(2, 0), "chapter dominates scene");
    }
}
