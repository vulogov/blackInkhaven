//! 1.2.18+ R.3 — reading-time computation for the
//! status-bar chip (and the R.4 reader-pace preview).
//!
//! Converts word counts to read-aloud / silent-reading
//! durations at a configurable words-per-minute, and
//! computes a per-book breakdown: total length + the
//! time remaining from the open paragraph to the book's
//! end.
//!
//! Pure + cheap — `compute` walks one book's paragraph
//! subtree (O(n) via the I.1.5 children index) and sums
//! the cached `Node::word_count` per paragraph, so it's
//! safe to call on the status-bar render path.

use uuid::Uuid;

use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

/// Per-book reading-time breakdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BookReadingTime {
    /// Whole-book read time in seconds.
    pub total_secs: u64,
    /// Read time from the open paragraph (inclusive) to
    /// the book's end, in seconds.
    pub remaining_secs: u64,
    /// Total words in the book (for diagnostics / the
    /// preview).
    pub total_words: u64,
}

/// Convert a word count to seconds at `wpm`.  `wpm == 0`
/// is treated as "no estimate" → 0.
pub(crate) fn reading_secs(words: u64, wpm: u32) -> u64 {
    if wpm == 0 {
        return 0;
    }
    words.saturating_mul(60) / wpm as u64
}

/// Compute the reading-time breakdown for the book
/// containing `open_paragraph_id`.  Returns `None` when
/// the id isn't in the hierarchy or isn't inside a book.
pub(crate) fn compute(
    h: &Hierarchy,
    open_paragraph_id: Uuid,
    wpm: u32,
) -> Option<BookReadingTime> {
    let open = h.get(open_paragraph_id)?;
    let book = root_book_of(h, open)?;

    // Paragraph word counts in display (pre-order) order.
    let para_words: Vec<(Uuid, u64)> = h
        .collect_subtree(book.id)
        .into_iter()
        .filter_map(|id| h.get(id))
        .filter(|n| n.kind == NodeKind::Paragraph)
        .map(|n| (n.id, n.word_count))
        .collect();

    let total_words: u64 = para_words.iter().map(|(_, w)| w).sum();
    // Remaining = from the open paragraph to the end.
    // When the open node isn't a paragraph in this book
    // (e.g. cursor on a branch), fall back to the full
    // total so the chip still shows something sensible.
    let remaining_words: u64 = match para_words
        .iter()
        .position(|(id, _)| *id == open_paragraph_id)
    {
        Some(i) => para_words[i..].iter().map(|(_, w)| w).sum(),
        None => total_words,
    };

    Some(BookReadingTime {
        total_secs: reading_secs(total_words, wpm),
        remaining_secs: reading_secs(remaining_words, wpm),
        total_words,
    })
}

/// Walk up to the root book of `node`.  `ancestors`
/// returns root→…→parent, so the first entry is the root
/// book; when `node` has no ancestors it may itself be a
/// book (returned), otherwise `None`.
fn root_book_of<'a>(h: &'a Hierarchy, node: &'a Node) -> Option<&'a Node> {
    match h.ancestors(node).first() {
        Some(root) if root.kind == NodeKind::Book => Some(root),
        _ if node.kind == NodeKind::Book => Some(node),
        _ => None,
    }
}

/// Format a duration as a compact `1h23m` / `12m` /
/// `45s` string for the status-bar chip.
pub(crate) fn fmt_compact(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h{m:02}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── reading_secs ──────────────────────────────────

    #[test]
    fn reading_secs_at_200wpm() {
        // 200 words at 200 wpm = 60s.
        assert_eq!(reading_secs(200, 200), 60);
        // 600 words at 200 wpm = 180s.
        assert_eq!(reading_secs(600, 200), 180);
    }

    #[test]
    fn reading_secs_zero_wpm_is_zero() {
        assert_eq!(reading_secs(1000, 0), 0);
    }

    #[test]
    fn reading_secs_zero_words_is_zero() {
        assert_eq!(reading_secs(0, 200), 0);
    }

    // ── fmt_compact ───────────────────────────────────

    #[test]
    fn fmt_compact_seconds() {
        assert_eq!(fmt_compact(45), "45s");
    }

    #[test]
    fn fmt_compact_minutes() {
        assert_eq!(fmt_compact(125), "2m");
    }

    #[test]
    fn fmt_compact_hours() {
        assert_eq!(fmt_compact(3 * 3600 + 25 * 60), "3h25m");
    }

    #[test]
    fn fmt_compact_zero() {
        assert_eq!(fmt_compact(0), "0s");
    }

    // ── compute (in-memory hierarchy) ─────────────────

    fn node(
        id: Uuid,
        kind: &str,
        slug: &str,
        path: &[&str],
        parent: Option<Uuid>,
        order: u32,
        words: u64,
    ) -> Node {
        let raw = serde_json::json!({
            "id": id,
            "kind": kind,
            "title": slug,
            "slug": slug,
            "path": path,
            "parent_id": parent,
            "order": order,
            "file": null,
            "word_count": words,
            "modified_at": "2026-01-01T00:00:00Z",
        });
        serde_json::from_value(raw).expect("test node deserialises")
    }

    /// book → ch → [p1 200w, p2 400w, p3 600w].
    fn sample() -> (Hierarchy, Vec<Uuid>) {
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::now_v7()).collect();
        let (book, ch, p1, p2, p3) =
            (ids[0], ids[1], ids[2], ids[3], ids[4]);
        let nodes = vec![
            node(book, "book", "b", &[], None, 1, 0),
            node(ch, "chapter", "c", &["b"], Some(book), 1, 0),
            node(p1, "paragraph", "p1", &["b", "c"], Some(ch), 1, 200),
            node(p2, "paragraph", "p2", &["b", "c"], Some(ch), 2, 400),
            node(p3, "paragraph", "p3", &["b", "c"], Some(ch), 3, 600),
        ];
        let h = build(nodes);
        (h, ids)
    }

    fn build(nodes: Vec<Node>) -> Hierarchy {
        Hierarchy::from_nodes_for_test(nodes)
    }

    #[test]
    fn compute_total_and_remaining() {
        let (h, ids) = sample();
        // Total = 200+400+600 = 1200 words.  At 200 wpm
        // = 360s.
        let bt = compute(&h, ids[2], 200).unwrap(); // open p1
        assert_eq!(bt.total_words, 1200);
        assert_eq!(bt.total_secs, 360);
        // Remaining from p1 (inclusive) = all = 360s.
        assert_eq!(bt.remaining_secs, 360);
    }

    #[test]
    fn compute_remaining_from_middle() {
        let (h, ids) = sample();
        // Open p2: remaining = 400+600 = 1000 words =
        // 300s.
        let bt = compute(&h, ids[3], 200).unwrap();
        assert_eq!(bt.remaining_secs, 300);
        assert_eq!(bt.total_secs, 360);
    }

    #[test]
    fn compute_remaining_from_last() {
        let (h, ids) = sample();
        // Open p3: remaining = 600 words = 180s.
        let bt = compute(&h, ids[4], 200).unwrap();
        assert_eq!(bt.remaining_secs, 180);
    }

    #[test]
    fn compute_unknown_id_is_none() {
        let (h, _ids) = sample();
        assert!(compute(&h, Uuid::now_v7(), 200).is_none());
    }
}
