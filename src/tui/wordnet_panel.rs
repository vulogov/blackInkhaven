//! State for the in-editor WordNet panel (1.8.5).
//!
//! When the cursor is on a word and the WordNet chord fires, the app builds a
//! [`WordnetPanel`] from the word's [`Suggestion`](crate::wordnet::Suggestion)
//! list and shows it as an overlay: an arrow-navigable list of candidate
//! replacements (synonyms, then hypernyms/hyponyms/antonyms). Selecting one
//! replaces the word in the editor. This module is the pure state + navigation;
//! rendering and key wiring live in the app.

use crate::wordnet::Suggestion;

/// The overlay's live state.
#[derive(Debug, Clone, PartialEq)]
pub struct WordnetPanel {
    /// The word being looked up (for the title and to avoid self-replacement).
    pub word: String,
    /// The language code searched (`en`, `ru`, …).
    pub lang: String,
    /// Candidate replacements, already ordered and de-duplicated.
    pub suggestions: Vec<Suggestion>,
    /// The highlighted row.
    pub selected: usize,
}

impl WordnetPanel {
    pub fn new(word: impl Into<String>, lang: impl Into<String>, suggestions: Vec<Suggestion>) -> Self {
        Self { word: word.into(), lang: lang.into(), suggestions, selected: 0 }
    }

    /// Move the highlight down, saturating at the last row.
    pub fn down(&mut self) {
        if self.selected + 1 < self.suggestions.len() {
            self.selected += 1;
        }
    }

    /// Move the highlight up, saturating at the first row.
    pub fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// The currently highlighted replacement word, if any.
    pub fn selected_word(&self) -> Option<&str> {
        self.suggestions.get(self.selected).map(|s| s.word.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sug(kind: &'static str, word: &str) -> Suggestion {
        Suggestion { kind, word: word.into() }
    }

    #[test]
    fn navigation_saturates_at_both_ends() {
        let mut p = WordnetPanel::new(
            "big",
            "en",
            vec![sug("synonym", "large"), sug("antonym", "small")],
        );
        assert_eq!(p.selected, 0);
        p.up(); // already at top
        assert_eq!(p.selected, 0);
        p.down();
        assert_eq!(p.selected_word(), Some("small"));
        p.down(); // already at bottom
        assert_eq!(p.selected, 1);
        p.up();
        assert_eq!(p.selected_word(), Some("large"));
    }

    #[test]
    fn empty_panel_has_no_selection() {
        let p = WordnetPanel::new("xyzzy", "en", vec![]);
        assert_eq!(p.selected_word(), None);
    }
}
