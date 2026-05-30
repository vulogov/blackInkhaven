//! Build the multilingual entity-name lexicon from the current
//! project hierarchy + config, plus the `LexiconKind` enum that
//! routes the editor's meta chords (`P` / `C` / `G` / `Y`) to
//! the right system book for lexicon-RAG inference. Extracted
//! from `tui::app` in the 1.2.7 refactor.

use uuid::Uuid;

use crate::config::Config;
use crate::store::hierarchy::Hierarchy;

/// Locate the Places and Characters system books in the loaded hierarchy
/// and compile a fresh `Lexicon` from their nested paragraph titles. Called
/// at startup and after every successful save. Stemmer languages come from
/// `editor.stemming.languages` in the project config. Unknown language
/// names are skipped silently (with a tracing warning) so a typo doesn't
/// break the editor.
pub(super) fn build_lexicon(
    hierarchy: &Hierarchy,
    cfg: &Config,
) -> super::lexicon::Lexicon {
    use super::lexicon::LexCategory;
    let mut places: Option<Uuid> = None;
    let mut characters: Option<Uuid> = None;
    let mut notes: Option<Uuid> = None;
    let mut artefacts: Option<Uuid> = None;
    let mut languages: Option<Uuid> = None;
    for node in hierarchy.iter() {
        match node.system_tag.as_deref() {
            Some(crate::store::SYSTEM_TAG_PLACES) => places = Some(node.id),
            Some(crate::store::SYSTEM_TAG_CHARACTERS) => characters = Some(node.id),
            Some(crate::store::SYSTEM_TAG_NOTES) => notes = Some(node.id),
            Some(crate::store::SYSTEM_TAG_ARTEFACTS) => artefacts = Some(node.id),
            Some(crate::store::SYSTEM_TAG_LANGUAGES) => languages = Some(node.id),
            _ => {}
        }
    }
    // Precedence: top-level `language` (when non-empty) wins over the
    // legacy `editor.stemming.languages` list. The former is the one-knob
    // primary setting; the latter stays for power users who want to
    // stem across multiple languages simultaneously.
    let algos: Vec<rust_stemmers::Algorithm> = if !cfg.language.trim().is_empty() {
        match crate::config::parse_stemmer_language(&cfg.language) {
            Some(a) => vec![a],
            None => {
                tracing::warn!(
                    "language `{}` is not a known Snowball algorithm — \
                     stemmer disabled (falling back to exact-phrase matching)",
                    cfg.language
                );
                Vec::new()
            }
        }
    } else {
        cfg.editor
            .stemming
            .languages
            .iter()
            .filter_map(|name| match crate::config::parse_stemmer_language(name) {
                Some(a) => Some(a),
                None => {
                    tracing::warn!(
                        "editor.stemming.languages: unknown language `{name}` — skipped"
                    );
                    None
                }
            })
            .collect()
    };
    // Higher-priority first: Place > Character > Artefact > Note —
    // matches the renderer's overlap precedence so the build-time
    // dedupe and the per-column style picker agree.
    let mut books: Vec<(Uuid, LexCategory)> = Vec::new();
    if let Some(id) = places {
        books.push((id, LexCategory::Place));
    }
    if let Some(id) = characters {
        books.push((id, LexCategory::Character));
    }
    if let Some(id) = artefacts {
        books.push((id, LexCategory::Artefact));
    }
    if let Some(id) = notes {
        books.push((id, LexCategory::Note));
    }
    // 1.2.13+ — Language books.  Each per-language
    // sub-book contributes its `Dictionary` chapter's
    // subtree (skipping `Meta`, `Grammar`,
    // `Phonology`, `Sample texts`) so only the
    // dictionary entries become lexicon hits, not the
    // grammar exposition or sample-text bodies.
    // Dictionary chapter is located by exact title
    // match — the title is fixed by the
    // `inkhaven language init` scaffolder; renaming
    // it would silently lose the overlay (Phase D
    // adds `language doctor` to surface this kind of
    // drift).
    if let Some(lang_root) = languages {
        for lang_book in hierarchy.children_of(Some(lang_root)) {
            for chapter in hierarchy.children_of(Some(lang_book.id)) {
                if chapter.title.eq_ignore_ascii_case("Dictionary") {
                    books.push((chapter.id, LexCategory::Language));
                }
            }
        }
    }
    super::lexicon::Lexicon::build(hierarchy, &books, algos)
}

/// Which system book a lexicon-RAG inference draws context from.
/// Picked by the editor-meta chords (`P` Places, `C` Characters,
/// `G` Notes, `Y` Artefacts).
#[derive(Debug, Clone, Copy)]
pub(super) enum LexiconKind {
    Places,
    Characters,
    Notes,
    Artefacts,
}

impl LexiconKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            LexiconKind::Places => "Place",
            LexiconKind::Characters => "Character",
            LexiconKind::Notes => "Note",
            LexiconKind::Artefacts => "Artefact",
        }
    }
    pub(super) fn system_tag(self) -> &'static str {
        match self {
            LexiconKind::Places => crate::store::SYSTEM_TAG_PLACES,
            LexiconKind::Characters => crate::store::SYSTEM_TAG_CHARACTERS,
            LexiconKind::Notes => crate::store::SYSTEM_TAG_NOTES,
            LexiconKind::Artefacts => crate::store::SYSTEM_TAG_ARTEFACTS,
        }
    }
}
