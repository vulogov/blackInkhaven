//! Build the multilingual entity-name lexicon from the current
//! project hierarchy + config, plus the `LexiconKind` enum that
//! routes the editor's meta chords (`P` / `C` / `G` / `Y`) to
//! the right system book for lexicon-RAG inference. Extracted
//! from `tui::app` in the 1.2.7 refactor.

use std::collections::HashMap;

use uuid::Uuid;

use crate::config::Config;
use crate::language_entry::{self, DictionaryEntry};
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

/// 1.2.13+ Phase B.2 — side-product of `build_lexicon`.
/// Maps a lowercased surface form (lemma or any inflection
/// value) to the `DictionaryEntry` it came from, so the
/// editor footer can render the
/// `[word · POS · translation]` chip when the cursor lands
/// on a Language hit.
#[derive(Debug, Default, Clone)]
pub(crate) struct LanguageEntryIndex {
    forms: HashMap<String, DictionaryEntry>,
}

impl LanguageEntryIndex {
    pub(crate) fn is_empty(&self) -> bool {
        self.forms.is_empty()
    }

    /// Case-insensitive lookup.  The buffer-text under a
    /// Language hit may not exactly match the lemma — the
    /// user could have written it in a different case or
    /// hit an inflected form — so we lowercase both sides.
    pub(crate) fn lookup(&self, form: &str) -> Option<&DictionaryEntry> {
        self.forms.get(&form.to_lowercase())
    }
}

/// Locate the Places and Characters system books in the loaded hierarchy
/// and compile a fresh `Lexicon` from their nested paragraph titles. Called
/// at startup and after every successful save. Stemmer languages come from
/// `editor.stemming.languages` in the project config. Unknown language
/// names are skipped silently (with a tracing warning) so a typo doesn't
/// break the editor.
///
/// 1.2.13+ — also returns a `LanguageEntryIndex` covering every word in
/// every Language sub-book's Dictionary chapter.  The index is keyed by
/// the lowercased surface form (lemma + every inflection-paradigm form)
/// and lets the editor footer render a `[word · POS · translation]`
/// chip when the cursor lands on a Language hit.  Empty when the project
/// has no Language books or every entry's body is pre-Phase-B (no
/// fenced HJSON block).
pub(super) fn build_lexicon(
    hierarchy: &Hierarchy,
    cfg: &Config,
    store: &Store,
) -> (super::lexicon::Lexicon, LanguageEntryIndex) {
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
    let mut dictionary_roots: Vec<Uuid> = Vec::new();
    if let Some(lang_root) = languages {
        for lang_book in hierarchy.children_of(Some(lang_root)) {
            for chapter in hierarchy.children_of(Some(lang_book.id)) {
                if chapter.title.eq_ignore_ascii_case("Dictionary") {
                    books.push((chapter.id, LexCategory::Language));
                    dictionary_roots.push(chapter.id);
                }
            }
        }
    }
    let mut lexicon = super::lexicon::Lexicon::build(hierarchy, &books, algos);

    // Phase B.2 — paradigm expansion + entry index.
    // Walk every Dictionary subtree's paragraphs, read
    // each body via the store, parse the fenced HJSON
    // block, and:
    //   * register every surface form (lemma + inflection
    //     values) in the LanguageEntryIndex,
    //   * feed every form OTHER THAN the lemma into the
    //     lexicon as an extra Language-category name
    //     (the lemma is already in the lexicon by virtue
    //     of being the paragraph title).
    let mut index = LanguageEntryIndex::default();
    let mut extras: Vec<(String, super::lexicon::LexCategory)> = Vec::new();
    for root in &dictionary_roots {
        for id in hierarchy.collect_subtree(*root) {
            if id == *root {
                continue;
            }
            let Some(node) = hierarchy.get(id) else {
                continue;
            };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let body = match store.get_content(id) {
                Ok(Some(bytes)) => bytes,
                _ => continue,
            };
            let body_str = match std::str::from_utf8(&body) {
                Ok(s) => s,
                Err(_) => continue, // binary content shouldn't happen for dictionary entries
            };
            let entry = match language_entry::parse(body_str) {
                Ok(Some(e)) => e,
                Ok(None) => continue, // pre-Phase-B entry — title still in lexicon, just no index data
                Err(err) => {
                    tracing::warn!(
                        "language entry `{}` HJSON parse failed: {}",
                        node.title,
                        err
                    );
                    continue;
                }
            };
            let title_lc = node.title.trim().to_lowercase();
            // Insert under the title (the canonical lemma)
            // first so an entry whose `word` field
            // disagrees with its paragraph title still
            // looks up by the title the lexicon hit
            // matched.
            if !title_lc.is_empty() {
                index.forms.insert(title_lc, entry.clone());
            }
            for form in entry.surface_forms() {
                let key = form.to_lowercase();
                if key.is_empty() {
                    continue;
                }
                // Don't double-add the lemma — the
                // paragraph title already covers it.
                if key != node.title.trim().to_lowercase() {
                    extras.push((form.to_string(), super::lexicon::LexCategory::Language));
                }
                index.forms.entry(key).or_insert_with(|| entry.clone());
            }
        }
    }
    lexicon.add_extra_forms(extras);

    (lexicon, index)
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
