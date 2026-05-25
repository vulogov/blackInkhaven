//! Line-level diff used by the F6 → V snapshot-diff modal.
//! Builds aligned side-by-side rows over `similar`'s Myers
//! pass and fuses Delete+Insert pairs into single `Changed`
//! rows. Extracted from `tui::app` in the 1.2.7 refactor.

/// One aligned row in the snapshot-diff view. `left_*` holds the
/// snapshot side (or `None` for an addition); `right_*` holds the
/// current-buffer side (or `None` for a deletion).
#[derive(Debug, Clone)]
pub(super) struct SnapshotDiffRow {
    pub left: Option<String>,
    pub right: Option<String>,
    pub kind: SnapshotDiffKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SnapshotDiffKind {
    /// Same line on both sides.
    Equal,
    /// Snapshot had it, buffer dropped it.
    Removed,
    /// Buffer added it.
    Added,
    /// Both sides have a line at this position but they differ.
    Changed,
}

/// Compute aligned line-by-line diff rows for the F6 → V
/// snapshot-diff modal. Uses `similar`'s `TextDiff::from_lines`
/// (Myers algorithm) to get a sequence of `(tag, line)` chunks,
/// then aligns them into side-by-side rows.
///
/// Heuristic for `Changed` rows: when a Delete is immediately
/// followed by an Insert, fuse the pair into one Changed row
/// rather than rendering them as a removal + an addition on
/// different lines. This is what most diff viewers do.
pub(super) fn compute_line_diff(left: &str, right: &str) -> Vec<SnapshotDiffRow> {
    use similar::{ChangeTag, TextDiff};
    let diff = TextDiff::from_lines(left, right);
    let mut out: Vec<SnapshotDiffRow> = Vec::new();
    // Walk the change list; on a Delete, peek to see if the
    // next change is an Insert and fuse them.
    let changes: Vec<_> = diff.iter_all_changes().collect();
    let mut i = 0;
    while i < changes.len() {
        let c = &changes[i];
        let text = c
            .value()
            .strip_suffix('\n')
            .unwrap_or(c.value())
            .to_string();
        match c.tag() {
            ChangeTag::Equal => {
                out.push(SnapshotDiffRow {
                    left: Some(text.clone()),
                    right: Some(text),
                    kind: SnapshotDiffKind::Equal,
                });
                i += 1;
            }
            ChangeTag::Delete => {
                // Look ahead for a paired Insert to fuse.
                if let Some(next) = changes.get(i + 1) {
                    if next.tag() == ChangeTag::Insert {
                        let next_text = next
                            .value()
                            .strip_suffix('\n')
                            .unwrap_or(next.value())
                            .to_string();
                        out.push(SnapshotDiffRow {
                            left: Some(text),
                            right: Some(next_text),
                            kind: SnapshotDiffKind::Changed,
                        });
                        i += 2;
                        continue;
                    }
                }
                out.push(SnapshotDiffRow {
                    left: Some(text),
                    right: None,
                    kind: SnapshotDiffKind::Removed,
                });
                i += 1;
            }
            ChangeTag::Insert => {
                out.push(SnapshotDiffRow {
                    left: None,
                    right: Some(text),
                    kind: SnapshotDiffKind::Added,
                });
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests_diff {
    use super::*;

    #[test]
    fn identical() {
        let r = compute_line_diff("a\nb\nc\n", "a\nb\nc\n");
        assert_eq!(r.len(), 3);
        assert!(r.iter().all(|x| x.kind == SnapshotDiffKind::Equal));
    }

    #[test]
    fn pure_add() {
        let r = compute_line_diff("a\n", "a\nb\n");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].kind, SnapshotDiffKind::Equal);
        assert_eq!(r[1].kind, SnapshotDiffKind::Added);
        assert_eq!(r[1].right.as_deref(), Some("b"));
    }

    #[test]
    fn pure_remove() {
        let r = compute_line_diff("a\nb\n", "a\n");
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].kind, SnapshotDiffKind::Removed);
        assert_eq!(r[1].left.as_deref(), Some("b"));
    }

    #[test]
    fn fused_change() {
        // Single-line rewrite fuses to Changed.
        let r = compute_line_diff("foo\n", "bar\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, SnapshotDiffKind::Changed);
        assert_eq!(r[0].left.as_deref(), Some("foo"));
        assert_eq!(r[0].right.as_deref(), Some("bar"));
    }
}
