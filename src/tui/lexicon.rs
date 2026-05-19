//! Place/Character name lexicon for editor highlighting.
//!
//! After every save (and on project open) we walk the Places and Characters
//! system books, collect every paragraph title nested under them, and compile
//! the two lists into anchored regexes. The editor renderer then overlays
//! cyan (Places) and yellow (Characters) on each match.
//!
//! Match semantics:
//! * **Case-insensitive** — proper nouns vary in capitalisation in prose.
//! * **Whole-word** — wrapped with `(?i)\b…\b` so "Tom" doesn't light up
//!   "atom".
//! * **Phrase** — multi-word titles ("King's Landing") match as a phrase.
//! * **Longest-first** — names are sorted by descending length before
//!   alternation so "Robb Stark" wins over "Robb" in a regex that's leftmost-
//!   first.
//!
//! Regex metacharacters in titles are escaped via `regex::escape`.

use regex::Regex;
use uuid::Uuid;

use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

/// Highlight category for a lexicon hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexCategory {
    Place,
    Character,
}

/// One match on a row of editor text, in character (not byte) coordinates.
#[derive(Debug, Clone, Copy)]
pub struct LexHit {
    pub col_start: usize,
    pub col_end: usize,
    pub category: LexCategory,
}

#[derive(Default, Debug)]
pub struct Lexicon {
    places: Option<Regex>,
    characters: Option<Regex>,
}

impl Lexicon {
    /// Walk the hierarchy collecting paragraph titles under the given book
    /// IDs and compile two regexes. Either ID being `None` (the book is
    /// missing) or having no paragraph descendants yields a regex of `None`
    /// for that category — the renderer treats that as "no hits".
    pub fn build(
        hierarchy: &Hierarchy,
        places_book: Option<Uuid>,
        characters_book: Option<Uuid>,
    ) -> Self {
        let places = places_book
            .and_then(|id| compile_for_subtree(hierarchy, id));
        let characters = characters_book
            .and_then(|id| compile_for_subtree(hierarchy, id));
        Self {
            places,
            characters,
        }
    }

    /// All hits on a single editor row. Returned in arbitrary order; callers
    /// that need stable left-to-right ordering should sort by `col_start`.
    pub fn row_hits(&self, line: &str) -> Vec<LexHit> {
        let mut out = Vec::new();
        if let Some(re) = &self.places {
            push_matches(&mut out, re, line, LexCategory::Place);
        }
        if let Some(re) = &self.characters {
            push_matches(&mut out, re, line, LexCategory::Character);
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.places.is_none() && self.characters.is_none()
    }
}

fn push_matches(out: &mut Vec<LexHit>, re: &Regex, line: &str, category: LexCategory) {
    for m in re.find_iter(line) {
        let col_start = line[..m.start()].chars().count();
        let col_end = line[..m.end()].chars().count();
        if col_end > col_start {
            out.push(LexHit {
                col_start,
                col_end,
                category,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_pair(places: &[&str], characters: &[&str]) -> Lexicon {
        let places_re = if places.is_empty() {
            None
        } else {
            let mut names: Vec<&str> = places.to_vec();
            names.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()));
            let pat = names
                .iter()
                .map(|n| regex::escape(n))
                .collect::<Vec<_>>()
                .join("|");
            Regex::new(&format!(r"(?i)\b(?:{pat})\b")).ok()
        };
        let chars_re = if characters.is_empty() {
            None
        } else {
            let mut names: Vec<&str> = characters.to_vec();
            names.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()));
            let pat = names
                .iter()
                .map(|n| regex::escape(n))
                .collect::<Vec<_>>()
                .join("|");
            Regex::new(&format!(r"(?i)\b(?:{pat})\b")).ok()
        };
        Lexicon {
            places: places_re,
            characters: chars_re,
        }
    }

    #[test]
    fn finds_place_hits() {
        let lex = build_pair(&["Rohan", "Gondor"], &[]);
        let hits = lex.row_hits("They rode from Rohan to Gondor.");
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.category == LexCategory::Place));
    }

    #[test]
    fn whole_word_only() {
        let lex = build_pair(&[], &["Tom"]);
        // "Tomorrow" must NOT match.
        let hits = lex.row_hits("Tomorrow is another day.");
        assert!(hits.is_empty());
        let hits = lex.row_hits("Then Tom arrived.");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].category, LexCategory::Character);
    }

    #[test]
    fn case_insensitive() {
        let lex = build_pair(&[], &["Frodo"]);
        let hits = lex.row_hits("frodo and FRODO and Frodo");
        assert_eq!(hits.len(), 3);
    }

    #[test]
    fn multi_word_phrase() {
        let lex = build_pair(&["King's Landing"], &[]);
        let hits = lex.row_hits("She returned to King's Landing alone.");
        assert_eq!(hits.len(), 1);
        let line = "She returned to King's Landing alone.";
        let matched: String = line
            .chars()
            .skip(hits[0].col_start)
            .take(hits[0].col_end - hits[0].col_start)
            .collect();
        assert_eq!(matched, "King's Landing");
    }

    #[test]
    fn unicode_offsets_are_char_indices() {
        let lex = build_pair(&[], &["Москва"]);
        let line = "Дорога в Москва";
        let hits = lex.row_hits(line);
        assert_eq!(hits.len(), 1);
        // "Дорога в " is 9 chars (including the trailing space).
        assert_eq!(hits[0].col_start, 9);
    }

    #[test]
    fn empty_lexicon_returns_no_hits() {
        let lex = Lexicon::default();
        assert!(lex.is_empty());
        assert!(lex.row_hits("anything at all").is_empty());
    }
}

fn compile_for_subtree(hierarchy: &Hierarchy, root: Uuid) -> Option<Regex> {
    let mut names: Vec<String> = hierarchy
        .collect_subtree(root)
        .into_iter()
        // Skip the book itself; only its descendants supply names.
        .filter(|id| *id != root)
        .filter_map(|id| hierarchy.get(id))
        .filter(|n| n.kind == NodeKind::Paragraph)
        .map(|n| n.title.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    // De-dup so a name listed twice doesn't blow up the alternation.
    names.sort();
    names.dedup();
    if names.is_empty() {
        return None;
    }
    // Longest-first so the leftmost-first regex engine prefers full phrases.
    names.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()));

    let alternation = names
        .iter()
        .map(|n| regex::escape(n))
        .collect::<Vec<_>>()
        .join("|");
    // (?i) case-insensitive, \b word boundaries.
    let pattern = format!(r"(?i)\b(?:{alternation})\b");
    Regex::new(&pattern).ok()
}
