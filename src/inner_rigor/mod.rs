//! RIGOR (1.6.20+) — the reasoning-rigor reader. A deterministic, zero-AI,
//! multilingual reader that scans manuscript prose for **argument-rigor** signals —
//! false dichotomy, question-begging (unargued assertion), straw man,
//! overgeneralization, and non-sequitur — via language-keyed cue markers, and
//! surfaces each as an *advisory* Output finding (glyph `⊬`).
//!
//! It is the argument-side complement to the Inner Theologian's deterministic
//! *moral* reader: where 1.6.18–1.6.19 made a work's claims *grounded* and its
//! citations *validated*, this reader asks whether its arguments are *valid*. Like
//! the Theologian it is never a verdict — a cue is a candidate weakness for the
//! author to weigh, in the tradition of the Inner Socrates `philosophical-reader`
//! (the Dialectician), made deterministic. It is stateless: findings emit straight
//! to the Output pane, recomputed each pass.

mod vocab;

use crate::config::{Config, RigorConfig};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

/// One rigor category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigorSignal {
    FalseDichotomy,
    QuestionBegging,
    StrawMan,
    Overgeneralization,
    NonSequitur,
    /// A lexicon term with ≥2 declared senses used repeatedly without pinning one
    /// (needs the scholarly lexicon — see [`WatchedTerm`]).
    Equivocation,
}

impl RigorSignal {
    pub fn as_code(self) -> &'static str {
        match self {
            RigorSignal::FalseDichotomy => "false-dichotomy",
            RigorSignal::QuestionBegging => "question-begging",
            RigorSignal::StrawMan => "straw-man",
            RigorSignal::Overgeneralization => "overgeneralization",
            RigorSignal::NonSequitur => "non-sequitur",
            RigorSignal::Equivocation => "equivocation",
        }
    }

    pub fn from_code(code: &str) -> Option<RigorSignal> {
        Some(match code {
            "false-dichotomy" => RigorSignal::FalseDichotomy,
            "question-begging" => RigorSignal::QuestionBegging,
            "straw-man" => RigorSignal::StrawMan,
            "overgeneralization" => RigorSignal::Overgeneralization,
            "non-sequitur" => RigorSignal::NonSequitur,
            "equivocation" => RigorSignal::Equivocation,
            _ => return None,
        })
    }

    /// A short human label for reports.
    pub fn label(self) -> &'static str {
        match self {
            RigorSignal::FalseDichotomy => "false dichotomy",
            RigorSignal::QuestionBegging => "question-begging",
            RigorSignal::StrawMan => "straw man",
            RigorSignal::Overgeneralization => "overgeneralization",
            RigorSignal::NonSequitur => "non-sequitur",
            RigorSignal::Equivocation => "equivocation",
        }
    }
}

/// A lexicon term the author has flagged as equivocation-prone — projected from a
/// `GlossaryEntry` with ≥2 senses and `watch_equivocation`. The reader counts the
/// term's surface forms in a paragraph; repeated unpinned use is a candidate
/// equivocation.
#[derive(Debug, Clone)]
pub struct WatchedTerm {
    /// The canonical term (as displayed in the advisory).
    pub term: String,
    /// Surface forms to count in prose (canonical + synonyms), lowercased.
    pub forms: Vec<String>,
}

/// Project the equivocation-watched, multi-sense terms of a glossary into
/// [`WatchedTerm`]s. Terms with fewer than two senses (nothing to conflate) are
/// dropped by [`crate::glossary::GlossaryEntry::is_equivocation_watched`].
pub fn watched_terms_from_glossary(entries: &[crate::glossary::GlossaryEntry]) -> Vec<WatchedTerm> {
    entries
        .iter()
        .filter(|e| e.is_valid() && e.is_equivocation_watched())
        .map(|e| WatchedTerm { term: e.term.trim().to_string(), forms: e.surface_forms() })
        .collect()
}

/// One rigor finding, anchored to a paragraph.
#[derive(Debug, Clone)]
pub struct RigorFinding {
    pub signal: RigorSignal,
    pub para_id: String,
    pub chapter_ord: u32,
    /// A localized advisory sentence naming the matched cue.
    pub description: String,
}

/// Which categories are enabled — projected from `RigorConfig`.
#[derive(Debug, Clone, Copy)]
pub struct RigorCats {
    pub false_dichotomy: bool,
    pub question_begging: bool,
    pub straw_man: bool,
    pub overgeneralization: bool,
    pub non_sequitur: bool,
    pub equivocation: bool,
}

impl RigorCats {
    pub fn from_config(cfg: &RigorConfig) -> RigorCats {
        RigorCats {
            false_dichotomy: cfg.false_dichotomy,
            question_begging: cfg.question_begging,
            straw_man: cfg.straw_man,
            overgeneralization: cfg.overgeneralization,
            non_sequitur: cfg.non_sequitur,
            equivocation: cfg.equivocation,
        }
    }
    /// All categories on — for tests and ad-hoc scans.
    #[cfg(test)]
    pub fn all() -> RigorCats {
        RigorCats {
            false_dichotomy: true,
            question_begging: true,
            straw_man: true,
            overgeneralization: true,
            non_sequitur: true,
            equivocation: true,
        }
    }
}

/// Detect rigor signals in one paragraph's plain text (Typst already stripped).
/// At most one finding per marker category per paragraph — the first matched cue —
/// so a dense paragraph does not flood the pane. `watched` are the lexicon's
/// equivocation-prone terms (empty to skip the equivocation check). Returns
/// `(signal, localized description)`.
pub fn detect_paragraph(
    text: &str,
    lang: &crate::prose::ProseLanguage,
    cats: &RigorCats,
    watched: &[WatchedTerm],
) -> Vec<(RigorSignal, String)> {
    let lc = text.to_lowercase();
    let v = vocab::lists_for(lang);
    let t = vocab::text_for(lang);
    let mut out = Vec::new();

    if cats.false_dichotomy {
        if let Some(cue) = vocab::matches_pair(&lc, v.either_or) {
            out.push((RigorSignal::FalseDichotomy, t.false_dichotomy.replace("{cue}", &cue)));
        } else if let Some(cue) = vocab::matches_any(&lc, v.dichotomy) {
            out.push((RigorSignal::FalseDichotomy, t.false_dichotomy.replace("{cue}", cue)));
        }
    }
    if cats.question_begging {
        if let Some(cue) = vocab::matches_any(&lc, v.assertion) {
            out.push((RigorSignal::QuestionBegging, t.question_begging.replace("{cue}", cue)));
        }
    }
    if cats.straw_man {
        if let Some(cue) = vocab::matches_any(&lc, v.straw_man) {
            out.push((RigorSignal::StrawMan, t.straw_man.replace("{cue}", cue)));
        }
    }
    if cats.overgeneralization {
        if let Some(cue) = vocab::matches_any(&lc, v.overgeneralization) {
            out.push((RigorSignal::Overgeneralization, t.overgeneralization.replace("{cue}", cue)));
        }
    }
    if cats.non_sequitur {
        // A conclusion connective with no warrant marker anywhere in the paragraph.
        if let Some(cue) = vocab::matches_any(&lc, v.conclusion) {
            if vocab::matches_any(&lc, v.warrant).is_none() {
                out.push((RigorSignal::NonSequitur, t.non_sequitur.replace("{cue}", cue)));
            }
        }
    }
    if cats.equivocation {
        // A watched, multi-sense term used ≥2 times without pinning a sense — the
        // author flagged it equivocation-prone, so repeated unqualified use is
        // worth a glance. One finding per term.
        for wt in watched {
            let uses: usize = wt.forms.iter().map(|f| vocab::count_word(&lc, f)).sum();
            if uses >= 2 {
                out.push((RigorSignal::Equivocation, t.equivocation.replace("{cue}", &wt.term)));
            }
        }
    }
    out
}

/// Scan a whole book's prose paragraphs for rigor signals. Language and enabled
/// categories come from `cfg.rigor` (falling back to the project language).
/// `watched` are the lexicon's equivocation-prone terms (see
/// [`watched_terms_from_glossary`]). Pure over the store's files — no AI, no
/// persistence.
pub fn scan_book(
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
    watched: &[WatchedTerm],
) -> Vec<RigorFinding> {
    let (lang, _note) = crate::prose::resolve_prose_language(cfg.rigor.language.as_deref(), &cfg.language);
    let cats = RigorCats::from_config(&cfg.rigor);
    let mut out = Vec::new();
    let chapters: Vec<&Node> = h
        .children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .collect();
    for (idx, chapter) in chapters.iter().enumerate() {
        let ord = idx as u32 + 1;
        for id in h.collect_subtree(chapter.id) {
            let Some(p) = h.get(id) else { continue };
            if p.kind != NodeKind::Paragraph || p.content_type.as_deref() == Some("jinja") {
                continue;
            }
            let Some(rel) = p.file.as_ref() else { continue };
            let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else { continue };
            let plain = crate::audiobook::typst_to_plain(&raw);
            for (signal, description) in detect_paragraph(&plain, &lang, &cats, watched) {
                out.push(RigorFinding { signal, para_id: id.to_string(), chapter_ord: ord, description });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseLanguage;

    fn en(text: &str) -> Vec<(RigorSignal, String)> {
        detect_paragraph(text, &ProseLanguage::En, &RigorCats::all(), &[])
    }

    fn watched(term: &str) -> WatchedTerm {
        WatchedTerm { term: term.into(), forms: vec![term.to_lowercase()] }
    }
    fn sigs(text: &str) -> Vec<RigorSignal> {
        en(text).into_iter().map(|(s, _)| s).collect()
    }

    #[test]
    fn detects_each_category() {
        assert!(sigs("It must be either freedom or determinism.").contains(&RigorSignal::FalseDichotomy));
        assert!(sigs("Obviously the soul is immortal.").contains(&RigorSignal::QuestionBegging));
        assert!(sigs("The so-called realist misses the point.").contains(&RigorSignal::StrawMan));
        assert!(sigs("Philosophers never agree on anything.").contains(&RigorSignal::Overgeneralization));
        // Conclusion marker with no warrant → non-sequitur.
        assert!(sigs("The tower is tall. Therefore the architect was wise.").contains(&RigorSignal::NonSequitur));
    }

    #[test]
    fn non_sequitur_suppressed_when_a_warrant_is_present() {
        // "because" is a warrant marker → the conclusion is not flagged.
        let s = sigs("The architect was wise, because the tower still stands; therefore we trust him.");
        assert!(!s.contains(&RigorSignal::NonSequitur));
    }

    #[test]
    fn clean_prose_is_silent() {
        assert!(en("The tower stood for three centuries and drew many pilgrims.").is_empty());
    }

    #[test]
    fn whole_word_only_no_false_positive() {
        // "everyone" absolute would flag, but "everything" is a distinct listed
        // absolute; ensure a bare "or" inside a word doesn't trigger either/or.
        assert!(!sigs("The corridor was narrow.").contains(&RigorSignal::FalseDichotomy));
    }

    #[test]
    fn category_toggle_respected() {
        let cats = RigorCats { question_begging: false, ..RigorCats::all() };
        let s: Vec<_> = detect_paragraph("Obviously true.", &ProseLanguage::En, &cats, &[])
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        assert!(!s.contains(&RigorSignal::QuestionBegging));
    }

    #[test]
    fn russian_markers_detected_and_localized() {
        let out = detect_paragraph(
            "Очевидно, что душа бессмертна.",
            &ProseLanguage::Ru,
            &RigorCats::all(),
            &[],
        );
        let qb = out.iter().find(|(s, _)| *s == RigorSignal::QuestionBegging).expect("question-begging");
        // Localized description (Cyrillic), naming the matched cue.
        assert!(qb.1.contains("самоочевидное") && qb.1.contains("очевидно"));
    }

    #[test]
    fn equivocation_flags_repeated_watched_term() {
        let terms = [watched("reason")];
        // "reason" used twice → candidate equivocation.
        let out = detect_paragraph(
            "Pure reason demands the unconditioned, so reason requires an object beyond sense.",
            &ProseLanguage::En,
            &RigorCats::all(),
            &terms,
        );
        let eq = out.iter().find(|(s, _)| *s == RigorSignal::Equivocation).expect("equivocation");
        assert!(eq.1.contains("reason"));
        // A single use does not flag.
        let single = detect_paragraph("Reason is a faculty.", &ProseLanguage::En, &RigorCats::all(), &terms);
        assert!(!single.iter().any(|(s, _)| *s == RigorSignal::Equivocation));
    }

    #[test]
    fn equivocation_needs_a_watched_multi_sense_term() {
        // No lexicon → no equivocation finding, even on a repeated word.
        let out = en("Reason and reason and reason again.");
        assert!(!out.iter().any(|(s, _)| *s == RigorSignal::Equivocation));
    }

    #[test]
    fn watched_terms_filters_to_multi_sense_watched() {
        use crate::glossary::{GlossaryEntry, Sense};
        let s = |g: &str| Sense { label: String::new(), gloss: g.into() };
        let entries = vec![
            // Watched + 2 senses → included.
            GlossaryEntry {
                term: "reason".into(),
                senses: vec![s("Vernunft"), s("Verstand")],
                watch_equivocation: true,
                ..Default::default()
            },
            // Watched but only 1 sense → dropped (nothing to conflate).
            GlossaryEntry {
                term: "grace".into(),
                senses: vec![s("favor")],
                watch_equivocation: true,
                ..Default::default()
            },
            // 2 senses but not watched → dropped.
            GlossaryEntry {
                term: "being".into(),
                senses: vec![s("existence"), s("essence")],
                watch_equivocation: false,
                ..Default::default()
            },
        ];
        let w = watched_terms_from_glossary(&entries);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].term, "reason");

    }

    #[test]
    fn signal_codes_round_trip() {
        for s in [
            RigorSignal::FalseDichotomy,
            RigorSignal::QuestionBegging,
            RigorSignal::StrawMan,
            RigorSignal::Overgeneralization,
            RigorSignal::NonSequitur,
            RigorSignal::Equivocation,
        ] {
            assert_eq!(RigorSignal::from_code(s.as_code()), Some(s));
        }
        assert_eq!(RigorSignal::from_code("nope"), None);
    }
}
