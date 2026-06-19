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
