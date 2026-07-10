//! INDEX-1 — the back-of-book index builder. Pure: given a set of index terms and the
//! manuscript's content units, it finds where each term appears (whole-word,
//! case-insensitive), deduplicates locations to the chapter, and returns an
//! alphabetised index with `see`-references. No I/O — testable without a project.

/// One content unit the index searches — a paragraph or subchapter, with its chapter
/// and a stable anchor (`file#slug`) for the web index.
#[derive(Debug, Clone)]
pub struct IndexUnit {
    pub chapter: String,
    pub anchor: String,
    pub text: String,
}

/// One place a term appears (deduplicated to the chapter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexLocation {
    pub chapter: String,
    pub anchor: String,
}

/// One index entry — a term with its locations, or a `see`-reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub term: String,
    pub locations: Vec<IndexLocation>,
    /// `Some(canonical)` → "term. See canonical." (a synonym cross-reference).
    pub see: Option<String>,
}

/// Build the index. `terms` are located in `units`; `see_refs` are
/// `(synonym, canonical)` cross-references. Terms found nowhere are dropped;
/// entries are sorted case-insensitively by term.
pub fn build(terms: &[String], see_refs: &[(String, String)], units: &[IndexUnit]) -> Vec<IndexEntry> {
    // Pre-lowercase each unit's text once.
    let lc: Vec<(String, &IndexUnit)> = units.iter().map(|u| (u.text.to_lowercase(), u)).collect();

    let mut entries: Vec<IndexEntry> = Vec::new();
    for term in terms {
        let needle = term.trim().to_lowercase();
        if needle.is_empty() {
            continue;
        }
        let mut locations: Vec<IndexLocation> = Vec::new();
        for (text_lc, u) in &lc {
            if contains_word(text_lc, &needle)
                && !locations.iter().any(|l| l.chapter == u.chapter)
            {
                locations.push(IndexLocation { chapter: u.chapter.clone(), anchor: u.anchor.clone() });
            }
        }
        if !locations.is_empty() {
            entries.push(IndexEntry { term: term.trim().to_string(), locations, see: None });
        }
    }

    for (synonym, canonical) in see_refs {
        let s = synonym.trim();
        if s.is_empty() || s.eq_ignore_ascii_case(canonical.trim()) {
            continue;
        }
        // A see-ref is only useful if the canonical term made it into the index.
        if entries.iter().any(|e| e.term.eq_ignore_ascii_case(canonical.trim()) && e.see.is_none()) {
            entries.push(IndexEntry {
                term: s.to_string(),
                locations: Vec::new(),
                see: Some(canonical.trim().to_string()),
            });
        }
    }

    entries.sort_by(|a, b| a.term.to_lowercase().cmp(&b.term.to_lowercase()));
    entries.dedup_by(|a, b| a.term.eq_ignore_ascii_case(&b.term) && a.see == b.see);
    entries
}

/// Whether `needle` (lowercase) appears in `hay` (lowercase) as a whole word — bounded
/// by non-alphanumeric characters (so "art" does not match "start").
fn contains_word(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(rel) = hay[from..].find(needle) {
        let i = from + rel;
        let before_ok = hay[..i].chars().next_back().map_or(true, |c| !c.is_alphanumeric());
        let after = &hay[i + needle.len()..];
        let after_ok = after.chars().next().map_or(true, |c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        from = i + needle.len();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(chapter: &str, anchor: &str, text: &str) -> IndexUnit {
        IndexUnit { chapter: chapter.into(), anchor: anchor.into(), text: text.into() }
    }

    #[test]
    fn locates_terms_dedupes_by_chapter_and_alphabetises() {
        let units = vec![
            unit("The Numbers", "ch1.html#a", "The long peace held. Deaths fell."),
            unit("The Numbers", "ch1.html#b", "War is the exception, peace the rule."),
            unit("Origins", "ch2.html#a", "The origins of war run deep."),
        ];
        let idx = build(&["peace".into(), "war".into(), "unicorn".into()], &[], &units);
        // "unicorn" appears nowhere → dropped. Sorted: peace, war.
        assert_eq!(idx.len(), 2);
        assert_eq!(idx[0].term, "peace");
        // peace appears twice in one chapter → deduped to a single location.
        assert_eq!(idx[0].locations.len(), 1);
        assert_eq!(idx[0].locations[0].chapter, "The Numbers");
        assert_eq!(idx[1].term, "war");
        assert_eq!(idx[1].locations.len(), 2);
    }

    #[test]
    fn whole_word_only() {
        let units = vec![unit("C", "c#a", "The artist made a start; art matters.")];
        // "art" matches the standalone word, not "artist"/"start".
        let idx = build(&["art".into()], &[], &units);
        assert_eq!(idx.len(), 1);
        assert_eq!(idx[0].locations.len(), 1);
        // "cat" does not match anything.
        assert!(build(&["cat".into()], &[], &units).is_empty());
    }

    #[test]
    fn see_reference_points_at_a_present_term() {
        let units = vec![unit("C", "c#a", "Homicide fell across the peace.")];
        let idx = build(
            &["peace".into()],
            &[("calm".into(), "peace".into()), ("dragon".into(), "wyrm".into())],
            &units,
        );
        // calm → see peace (peace is present). dragon → wyrm (absent) → dropped.
        let calm = idx.iter().find(|e| e.term == "calm").expect("calm see-ref");
        assert_eq!(calm.see.as_deref(), Some("peace"));
        assert!(idx.iter().all(|e| e.term != "dragon"));
    }
}
