//! 1.3.33+ (road to 1.4.0) — the command palette's data layer.
//!
//! A pure projection of the live keybinding registry ([`KeyBindings`]) into a flat,
//! fuzzy-filterable list of commands. The `Ctrl+P` modal (P1) renders [`collect`]'s
//! output and executes a selection via `App::run_action`. Keeping this layer pure
//! (no `App`, no rendering) makes it unit- and property-testable.

use std::collections::HashSet;

use super::keybind::{Action, BindingEntry, KeyBindings};

/// One selectable command in the palette.
#[derive(Debug, Clone)]
pub struct PaletteEntry {
    pub action: Action,
    /// Short UI name (`action.label()`), e.g. "save".
    pub label: String,
    /// Longer help text (`action.description()`).
    pub description: String,
    /// The rendered chord, prefixed by its layer (e.g. `"Ctrl+B W"`, `"F2"`).
    pub chord: String,
}

fn push_table(
    out: &mut Vec<PaletteEntry>,
    seen: &mut HashSet<Action>,
    prefix: Option<&str>,
    table: &[BindingEntry],
) {
    for e in table {
        // Skip unbound placeholders and label-less internal actions.
        if matches!(e.action, Action::None) {
            continue;
        }
        let label = e.action.label();
        if label.is_empty() {
            continue;
        }
        // One row per action: a command bound to two chords shows once (first wins,
        // matching the status-bar hint's de-dupe).
        if !seen.insert(e.action.clone()) {
            continue;
        }
        let chord = match prefix {
            Some(p) => format!("{p} {}", e.chord.to_display_string()),
            None => e.chord.to_display_string(),
        };
        out.push(PaletteEntry {
            action: e.action.clone(),
            label,
            description: e.action.description(),
            chord,
        });
    }
}

/// Project the binding tables into a sorted, de-duped palette. Sub-chord tables are
/// prefixed with their layer chord (`Ctrl+B` / `Ctrl+V` / `Ctrl+Z`); the top-level
/// table is rendered bare.
pub fn collect(b: &KeyBindings) -> Vec<PaletteEntry> {
    let meta = b.meta_prefix.to_display_string();
    let view = b.view_prefix.as_ref().map(|c| c.to_display_string());
    let bund = b.bund_prefix.as_ref().map(|c| c.to_display_string());

    let mut out = Vec::new();
    let mut seen: HashSet<Action> = HashSet::new();
    push_table(&mut out, &mut seen, None, &b.top_level);
    push_table(&mut out, &mut seen, Some(&meta), &b.meta_sub);
    if let Some(v) = &view {
        push_table(&mut out, &mut seen, Some(v), &b.view_sub);
    }
    if let Some(z) = &bund {
        push_table(&mut out, &mut seen, Some(z), &b.bund_sub);
    }
    out.sort_by(|a, b| a.label.cmp(&b.label).then(a.chord.cmp(&b.chord)));
    out
}

/// Score a single entry against a lower-cased query. 0 = no match.
fn score(e: &PaletteEntry, q: &str) -> i32 {
    let label = e.label.to_lowercase();
    if label.starts_with(q) {
        4
    } else if label.contains(q) {
        3
    } else if e.chord.to_lowercase().contains(q) {
        2
    } else if e.description.to_lowercase().contains(q) {
        1
    } else {
        0
    }
}

/// Indices of entries matching `query`, best first (stable on ties via original
/// order). An empty query returns every index in palette order.
pub fn fuzzy_filter(entries: &[PaletteEntry], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (0..entries.len()).collect();
    }
    let mut scored: Vec<(i32, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let s = score(e, &q);
            (s > 0).then_some((s, i))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> KeyBindings {
        KeyBindings::defaults()
    }

    #[test]
    fn collect_is_nonempty_and_well_formed() {
        let entries = collect(&defaults());
        assert!(entries.len() > 20, "the default registry yields many commands");
        for e in &entries {
            assert!(!e.label.is_empty(), "every entry has a label");
            assert!(!e.chord.is_empty(), "every entry has a rendered chord");
            assert!(!matches!(e.action, Action::None));
        }
    }

    #[test]
    fn collect_dedupes_by_action() {
        let entries = collect(&defaults());
        let mut seen = HashSet::new();
        for e in &entries {
            assert!(seen.insert(e.action.clone()), "no action appears twice");
        }
    }

    #[test]
    fn sub_chords_carry_their_prefix() {
        let b = defaults();
        let meta = b.meta_prefix.to_display_string();
        let entries = collect(&b);
        // At least one meta-layer command renders with the Ctrl+B prefix.
        assert!(
            entries.iter().any(|e| e.chord.starts_with(&meta)),
            "meta sub-chords are prefixed with {meta}"
        );
    }

    #[test]
    fn empty_query_returns_all_in_order() {
        let entries = collect(&defaults());
        let idx = fuzzy_filter(&entries, "   ");
        assert_eq!(idx, (0..entries.len()).collect::<Vec<_>>());
    }

    #[test]
    fn query_filters_and_ranks() {
        let entries = collect(&defaults());
        // "save" should match the editor save command and rank a label-prefix hit
        // above a mere description hit.
        let idx = fuzzy_filter(&entries, "save");
        assert!(!idx.is_empty(), "`save` matches something");
        let top = &entries[idx[0]];
        assert!(
            top.label.to_lowercase().contains("save")
                || top.label.to_lowercase().starts_with("save"),
            "top hit for `save` is a label match, got {:?}",
            top.label
        );
        // Every returned index is valid.
        assert!(idx.iter().all(|&i| i < entries.len()));
    }

    #[test]
    fn nonsense_query_matches_nothing() {
        let entries = collect(&defaults());
        assert!(fuzzy_filter(&entries, "zzqxnonsense").is_empty());
    }

    // 1.3.33+ — registry proptests (road to 1.4.0 stability rider).
    mod prop {
        use super::{collect, fuzzy_filter};
        use crate::tui::keybind::KeyBindings;
        use proptest::prelude::*;

        proptest! {
            /// `fuzzy_filter` must never panic on any query and must only ever return
            /// in-range, unique indices (the modal indexes `entries[idx]` directly).
            #[test]
            fn fuzzy_filter_returns_valid_unique_indices(q in ".{0,32}") {
                let entries = collect(&KeyBindings::defaults());
                let idx = fuzzy_filter(&entries, &q);
                let mut seen = std::collections::HashSet::new();
                for &i in &idx {
                    prop_assert!(i < entries.len(), "index in range");
                    prop_assert!(seen.insert(i), "no duplicate index");
                }
            }

            /// A blank query (any amount of whitespace) returns every entry.
            #[test]
            fn whitespace_query_returns_all(q in "[ \\t]{0,8}") {
                let entries = collect(&KeyBindings::defaults());
                prop_assert_eq!(fuzzy_filter(&entries, &q).len(), entries.len());
            }
        }
    }
}
