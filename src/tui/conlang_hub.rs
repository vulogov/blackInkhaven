//! ConLang hub (LANG-1 P2.7b) — the `Ctrl+B X` overview modal.
//!
//! Builds a read-only, scrollable summary of every language under the
//! `Language` system book: phonology shape, lexicon size, prosody, writing,
//! and linked speakers. Surfaces the CLI suite's data in the editor; the
//! deep operations stay on the CLI (`language audit` / `generate-lexicon` /
//! `query` / `scan-manuscript`) + `Ctrl+B Q` translation.

use std::path::Path;

use crate::conlang::links::ConlangLinks;
use crate::conlang::types::PhonemeKind;
use crate::conlang::{Phonology, TemplateRole};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{NodeKind, Store, SYSTEM_TAG_LANGUAGES};

use super::modal::ConlangHubRow;

pub(super) fn build_rows(store: &Store, hierarchy: &Hierarchy, project_root: &Path) -> Vec<ConlangHubRow> {
    let mut rows: Vec<ConlangHubRow> = Vec::new();
    let header = |t: String| ConlangHubRow { text: t, header: true };
    let stat = |t: String| ConlangHubRow { text: t, header: false };

    let Some(lang_root) = hierarchy
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES))
    else {
        return rows;
    };
    let links = ConlangLinks::load(project_root).unwrap_or_default();
    let langs: Vec<&Node> = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book)
        .collect();

    if langs.is_empty() {
        rows.push(header("ConLang suite".into()));
        rows.push(stat("  No languages yet — `inkhaven language init <name>`".into()));
        return rows;
    }

    for lang in &langs {
        rows.push(header(format!("Language: {}", lang.title)));
        match load_phonology(store, hierarchy, lang) {
            Some(p) => {
                let (c, v) = p.phonemes.iter().fold((0, 0), |(c, v), ph| match ph.kind {
                    PhonemeKind::Consonant => (c + 1, v),
                    PhonemeKind::Vowel => (c, v + 1),
                });
                rows.push(stat(format!("  Phonemes      : {} ({c} C, {v} V)", p.phonemes.len())));
                rows.push(stat(format!(
                    "  Templates     : {} root · {} constraint(s)",
                    p.templates_for(TemplateRole::Root).len(),
                    p.constraints.len()
                )));
                rows.push(stat(format!("  Allophony     : {} rule(s)", p.allophony.len())));
                let stress = p
                    .stress
                    .as_ref()
                    .map(|s| format!("{:?}", s.primary).to_lowercase())
                    .unwrap_or_else(|| "—".into());
                let tone = if p.tone.is_some() { "yes" } else { "—" };
                rows.push(stat(format!("  Prosody       : stress {stress} · tone {tone}")));
                let rom = if p.romanizations.is_empty() {
                    "per-phoneme".to_string()
                } else {
                    format!("{} scheme(s)", p.romanizations.len())
                };
                rows.push(stat(format!("  Romanization  : {rom}")));
            }
            None => rows.push(stat("  Phonology     : not defined yet".into())),
        }
        rows.push(stat(format!(
            "  Lexicon       : {} entr(y/ies)",
            count_dictionary(store, hierarchy, lang)
        )));
        let (places, chars) = links.speakers_of(&lang.title);
        rows.push(stat(format!(
            "  Speakers      : {} place(s) · {} character(s)",
            places.len(),
            chars.len()
        )));
        rows.push(stat(String::new()));
    }
    rows.push(stat(
        "Ctrl+B Q translate · CLI: language audit · generate-lexicon · query · scan-manuscript".into(),
    ));
    rows
}

/// LANG-1 P2.7c — detect a `:<ident>:` insertion trigger ending at char
/// column `col` on `line`. Returns `(ident, start_col)`; the span to replace
/// is `[start_col, col)`. `None` unless the cursor sits immediately after a
/// `:<ident>:` whose `<ident>` is a non-empty run of `[A-Za-z0-9_-]`.
pub(super) fn detect_trigger(line: &str, col: usize) -> Option<(String, usize)> {
    let chars: Vec<char> = line.chars().collect();
    if col == 0 || col > chars.len() || chars[col - 1] != ':' {
        return None;
    }
    let close = col - 1;
    let mut j = close;
    while j > 0 {
        j -= 1;
        let ch = chars[j];
        if ch == ':' {
            let ident: String = chars[j + 1..close].iter().collect();
            return (!ident.is_empty()).then_some((ident, j));
        }
        if !(ch.is_alphanumeric() || ch == '-' || ch == '_') {
            return None;
        }
    }
    None
}

/// LANG-1 P2.7c — resolve a `:lang:` identifier to a Language sub-book by
/// title (case-insensitive) or by its `Meta/overview` `iso_code`.
pub(super) fn resolve_language(store: &Store, hierarchy: &Hierarchy, ident: &str) -> Option<Node> {
    let lang_root = hierarchy
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES))?;
    let langs: Vec<&Node> = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book)
        .collect();
    // Title match first.
    if let Some(l) = langs.iter().find(|l| l.title.eq_ignore_ascii_case(ident)) {
        return Some((*l).clone());
    }
    // Then ISO code from each language's Meta/overview.
    for l in &langs {
        if let Some(iso) = meta_iso(store, hierarchy, l) {
            if iso.eq_ignore_ascii_case(ident) {
                return Some((*l).clone());
            }
        }
    }
    None
}

fn meta_iso(store: &Store, hierarchy: &Hierarchy, lang_book: &Node) -> Option<String> {
    let meta = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Meta"))?;
    let overview = hierarchy
        .children_of(Some(meta.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Paragraph && n.title.eq_ignore_ascii_case("overview"))?;
    let bytes = store.get_content(overview.id).ok()??;
    let m = crate::language_entry::parse_meta_overview(&String::from_utf8_lossy(&bytes)).ok()??;
    let iso = m.iso_code.trim();
    (!iso.is_empty()).then(|| iso.to_string())
}

/// Load a language's dictionary as `(headword, gloss)` pairs for the
/// `:lang:` insertion picker.
pub(super) fn load_entries(store: &Store, hierarchy: &Hierarchy, lang_book: &Node) -> Vec<(String, String)> {
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary"))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for id in hierarchy.collect_subtree(chapter.id) {
        let Some(node) = hierarchy.get(id) else { continue };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        if let Ok(Some(bytes)) = store.get_content(node.id) {
            if let Ok(Some(e)) = crate::language_entry::parse(&String::from_utf8_lossy(&bytes)) {
                let word = e.word.trim().to_string();
                if !word.is_empty() {
                    out.push((word, e.translation.trim().to_string()));
                }
            }
        }
    }
    out
}

fn load_phonology(store: &Store, hierarchy: &Hierarchy, lang_book: &Node) -> Option<Phonology> {
    let chapter = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Phonology"))?;
    for para in hierarchy.children_of(Some(chapter.id)) {
        if para.kind != NodeKind::Paragraph {
            continue;
        }
        if let Ok(Some(bytes)) = store.get_content(para.id) {
            let body = String::from_utf8_lossy(&bytes);
            if let Ok(Some(p)) = Phonology::from_hjson(&body) {
                if !p.phonemes.is_empty() {
                    return Some(p);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::detect_trigger;

    #[test]
    fn detects_a_completed_lang_trigger() {
        // "said :qya:" — cursor (col 10) just after the closing ':'.
        assert_eq!(detect_trigger("said :qya:", 10), Some(("qya".to_string(), 5)));
        // at the very start of a line.
        assert_eq!(detect_trigger(":sjn:", 5), Some(("sjn".to_string(), 0)));
    }

    #[test]
    fn ignores_non_triggers() {
        assert_eq!(detect_trigger("note: foo", 5), None); // no opening ':'
        assert_eq!(detect_trigger("::", 2), None); // empty ident
        assert_eq!(detect_trigger("xy", 2), None); // doesn't end in ':'
        assert_eq!(detect_trigger("http://", 5), None); // "http:" — no opening ':'
        assert_eq!(detect_trigger(":a b:", 5), None); // space breaks the ident run
    }

    #[test]
    fn span_excludes_text_before_the_trigger() {
        // The replace span [start, col) must cover exactly ":qya:".
        let (ident, start) = detect_trigger("the orc said :qya:", 18).unwrap();
        assert_eq!(ident, "qya");
        assert_eq!(&"the orc said :qya:"[start..18], ":qya:");
    }
}

fn count_dictionary(store: &Store, hierarchy: &Hierarchy, lang_book: &Node) -> usize {
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary"))
    else {
        return 0;
    };
    hierarchy
        .collect_subtree(chapter.id)
        .into_iter()
        .filter_map(|id| hierarchy.get(id))
        .filter(|n| n.kind == NodeKind::Paragraph)
        .filter(|n| {
            store
                .get_content(n.id)
                .ok()
                .flatten()
                .and_then(|b| crate::language_entry::parse(&String::from_utf8_lossy(&b)).ok().flatten())
                .is_some()
        })
        .count()
}
