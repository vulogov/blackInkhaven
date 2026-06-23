//! The Fast track — deterministic, instant (no LLM) Socratic observation. Five
//! categories ship today (`modal_claims`, `hedged_uncertainty`,
//! `structural_patterns`, `unattributed_dialogue`, `sentence_length_anomalies`);
//! `pronoun_ambiguity` + `tense_voice_shifts` await the UD parser. Each detector
//! produces a **question**, never a correction (the non-prescriptive spine).
//!
//! Multilingual (EN/RU/ES/FR/DE): the paragraph's language is detected, its marker
//! table drives the vocabulary categories, and the question renders in that
//! language (with an English `question_en` fallback). An uncertain detection
//! degrades to English — the same discipline as WORLD-4's fact-checker.

use crate::world::fact_check_lang::{contains_word, detect_with_confidence};

use super::intent::{ConsultationResult, FindingContext, IntentLedger};
use super::lang::{self, Lang, LangMarkers, Msg};
use super::text;
use super::types::{Category, Persona, Severity, SocraticFinding};

/// A sentence past this many words draws a (gentle) length question.
const LONG_SENTENCE_WORDS: usize = 45;
/// This many consecutive sentences sharing an opening word / an exact length
/// reads as a structural pattern worth noticing.
const STRUCTURAL_RUN: usize = 3;
const SAME_LENGTH_RUN: usize = 4;
/// This many spoken segments with no attribution verb reads as a run of
/// unattributed dialogue.
const DIALOGUE_RUN: usize = 4;

/// Run the Fast track over one paragraph for the active persona, consulting the
/// intent ledger. Returns the emitted findings (suppressed ones are dropped per
/// the RFC). Pure + deterministic.
pub fn check_paragraph(
    text: &str,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
) -> Vec<SocraticFinding> {
    // Read the paragraph in its language; an uncertain detection degrades to
    // English (markers + question text) rather than guessing.
    let (detected, confident) = detect_with_confidence(text);
    let lang = if confident { detected } else { Lang::En };
    let m = lang::markers(lang);
    let lower = text.to_lowercase();
    let sentences = text::sentences(text);

    let mut out = Vec::new();
    detect_modal_claims(&lower, lang, &m, persona, ledger, ctx, &mut out);
    detect_hedged_uncertainty(&lower, lang, &m, persona, ledger, ctx, &mut out);
    detect_structural_patterns(&sentences, lang, persona, ledger, ctx, &mut out);
    detect_unattributed_dialogue(text, &lower, lang, &m, persona, ledger, ctx, &mut out);
    detect_sentence_length(&sentences, lang, persona, ledger, ctx, &mut out);
    // The two parser-adjacent categories use conservative English-only heuristics
    // (no UD parser bundled); they stay quiet (Notice severity) to limit noise.
    if lang == Lang::En {
        detect_tense_shift(&sentences, persona, ledger, ctx, &mut out);
        detect_pronoun_ambiguity(&sentences, persona, ledger, ctx, &mut out);
    }
    out
}

/// Past-tense auxiliaries and present-tense auxiliaries — reliable function-word
/// signals of a sentence's tense (more robust than `-ed` detection).
const PAST_AUX: &[&str] = &["was", "were", "had", "did"];
const PRESENT_AUX: &[&str] = &["is", "are", "am"];

/// `tense_voice_shifts` — a paragraph that is clearly past-tense but slips into
/// the present (or the reverse). Conservative: needs a ≥4-sentence paragraph with
/// a strong past majority and a clear present outlier (dialogue ignored).
fn detect_tense_shift(
    sentences: &[&str],
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    if sentences.len() < 4 {
        return;
    }
    let (mut past, mut present) = (0usize, 0usize);
    for s in sentences {
        if s.contains('"') || s.contains('\u{201c}') {
            continue; // dialogue: present tense there is normal
        }
        let low = s.to_lowercase();
        let is_past = PAST_AUX.iter().any(|w| contains_word(&low, w));
        let is_present = !is_past && PRESENT_AUX.iter().any(|w| contains_word(&low, w));
        if is_past {
            past += 1;
        } else if is_present {
            present += 1;
        }
    }
    // A clear past majority with a present outlier (simple-past verbs without an
    // auxiliary aren't counted, so the bar is ≥2 aux-past + present dominant).
    if past >= 2 && present >= 1 && past >= present * 2 {
        push(out, persona, ledger, ctx, Lang::En, Category::TenseVoiceShifts, Severity::Notice, Msg::TenseShift);
    }
}

/// Third-person pronouns whose antecedent may be ambiguous.
const PRONOUNS: &[&str] = &["he", "she", "they", "him", "her", "them", "his", "their"];

/// `pronoun_ambiguity` — a pronoun whose antecedent isn't unambiguous because the
/// sentence just before names two or more distinct people. Conservative + noisy,
/// so Notice severity (hidden by default). Emits at most one finding.
fn detect_pronoun_ambiguity(
    sentences: &[&str],
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    for i in 1..sentences.len() {
        let low = sentences[i].to_lowercase();
        let has_pronoun = PRONOUNS.iter().any(|p| contains_word(&low, p));
        if has_pronoun && capitalized_names(sentences[i - 1]).len() >= 2 {
            push(out, persona, ledger, ctx, Lang::En, Category::PronounAmbiguity, Severity::Notice, Msg::PronounAmbiguity);
            return;
        }
    }
}

/// Distinct capitalized words in a sentence that look like proper names: skip the
/// first word (sentence-initial capitals aren't a signal) and a small stoplist.
fn capitalized_names(sentence: &str) -> std::collections::BTreeSet<String> {
    const STOP: &[&str] = &[
        "i", "the", "a", "an", "and", "but", "or", "if", "when", "then", "so", "yet", "he", "she",
        "they", "it", "we", "you", "his", "her", "their", "mr", "mrs", "ms", "lord", "lady", "sir",
    ];
    let mut names = std::collections::BTreeSet::new();
    for w in sentence.split_whitespace() {
        // Sentence-initial function words are filtered by the stoplist rather than
        // by position, so a name that opens the sentence still counts.
        let trimmed = w.trim_matches(|c: char| !c.is_alphabetic());
        let mut chars = trimmed.chars();
        if let Some(first) = chars.next() {
            if first.is_uppercase() && trimmed.chars().count() > 1 {
                let lower = trimmed.to_lowercase();
                if !STOP.contains(&lower.as_str()) {
                    names.insert(lower);
                }
            }
        }
    }
    names
}

/// `modal_claims` — a passage that treats an outcome as inevitable. Defused by a
/// nearby conditional or hedge.
#[allow(clippy::too_many_arguments)]
fn detect_modal_claims(
    lower: &str,
    lang: Lang,
    m: &LangMarkers,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    if m.modal_defuse.iter().any(|w| contains_word(lower, w)) {
        return; // conditional / hedged context — not actually asserting necessity
    }
    if let Some(marker) = m.modal_strong.iter().find(|w| contains_word(lower, w)) {
        push(out, persona, ledger, ctx, lang, Category::ModalClaims, Severity::Inquiry, Msg::ModalStrong(marker));
    } else if let Some(marker) = m.modal_moderate.iter().find(|w| contains_word(lower, w)) {
        push(out, persona, ledger, ctx, lang, Category::ModalClaims, Severity::Notice, Msg::ModalModerate(marker));
    }
}

/// `hedged_uncertainty` — authorial hedging worth being conscious of.
#[allow(clippy::too_many_arguments)]
fn detect_hedged_uncertainty(
    lower: &str,
    lang: Lang,
    m: &LangMarkers,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    if let Some(marker) = m.hedge.iter().find(|w| contains_word(lower, w)) {
        push(out, persona, ledger, ctx, lang, Category::HedgedUncertainty, Severity::Notice, Msg::Hedge(marker));
    }
}

/// `structural_patterns` — a run of sentences sharing an opening word (anaphora)
/// or an exact length (a monotone cadence). Language-agnostic shape; localized
/// question. Emits at most one finding.
fn detect_structural_patterns(
    sentences: &[&str],
    lang: Lang,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    let firsts: Vec<Option<String>> = sentences.iter().map(|s| text::first_word(s)).collect();
    if let Some(word) =
        longest_equal_run(&firsts).filter(|(_, n)| *n >= STRUCTURAL_RUN).and_then(|(w, _)| w)
    {
        push(out, persona, ledger, ctx, lang, Category::StructuralPatterns, Severity::Notice, Msg::Anaphora(&word));
        return;
    }
    let lens: Vec<Option<usize>> = sentences.iter().map(|s| Some(text::word_count(s))).collect();
    if longest_equal_run(&lens).is_some_and(|(_, n)| n >= SAME_LENGTH_RUN) {
        push(out, persona, ledger, ctx, lang, Category::StructuralPatterns, Severity::Notice, Msg::Monotone);
    }
}

/// `unattributed_dialogue` — a run of spoken segments with no attribution verb
/// anywhere in the passage.
#[allow(clippy::too_many_arguments)]
fn detect_unattributed_dialogue(
    text: &str,
    lower: &str,
    lang: Lang,
    m: &LangMarkers,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    let segments = text::dialogue_segment_count(text);
    if segments < DIALOGUE_RUN {
        return;
    }
    if m.attribution.iter().any(|v| contains_word(lower, v)) {
        return; // a speaker is tagged somewhere
    }
    push(out, persona, ledger, ctx, lang, Category::UnattributedDialogue, Severity::Inquiry, Msg::UnattributedDialogue(segments));
}

/// `sentence_length_anomalies` — a single very long sentence.
fn detect_sentence_length(
    sentences: &[&str],
    lang: Lang,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    if let Some(n) = sentences.iter().map(|s| text::word_count(s)).max() {
        if n > LONG_SENTENCE_WORDS {
            push(out, persona, ledger, ctx, lang, Category::SentenceLengthAnomalies, Severity::Notice, Msg::LongSentence(n));
        }
    }
}

/// Build a finding (question in `lang`, English fallback), apply the persona's
/// mute, then consult the ledger. Emits only when the persona doesn't mute the
/// category and no declared intent suppresses it.
#[allow(clippy::too_many_arguments)]
fn push(
    out: &mut Vec<SocraticFinding>,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    lang: Lang,
    category: Category,
    severity: Severity,
    msg: Msg,
) {
    if persona.mutes(category) {
        return;
    }
    if !matches!(ledger.consult(category, ctx), ConsultationResult::Emit) {
        return; // suppressed by declared intent
    }
    out.push(SocraticFinding {
        category,
        severity,
        persona_id: persona.id.clone(),
        question: lang::render(&msg, lang),
        question_en: lang::render(&msg, Lang::En),
        suppressed_by: None,
    });
}

/// The value and length of the longest run of equal consecutive items (ignoring
/// `None`s, which break a run). Returns `(value, run_length)`.
fn longest_equal_run<T: Clone + PartialEq>(items: &[Option<T>]) -> Option<(Option<T>, usize)> {
    let mut best: Option<(Option<T>, usize)> = None;
    let mut i = 0;
    while i < items.len() {
        let Some(v) = &items[i] else {
            i += 1;
            continue;
        };
        let mut j = i + 1;
        while j < items.len() && items[j].as_ref() == Some(v) {
            j += 1;
        }
        let run = j - i;
        if best.as_ref().is_none_or(|(_, n)| run > *n) {
            best = Some((Some(v.clone()), run));
        }
        i = j;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inner_socrates::intent::{IntentEntry, IntentKind, IntentScope, ScopeLevel};

    fn socrates() -> Persona {
        Persona::default_inner_socrates()
    }

    fn check(text: &str) -> Vec<SocraticFinding> {
        check_paragraph(text, &socrates(), &IntentLedger::default(), &FindingContext::default())
    }

    #[test]
    fn flags_asserted_necessity_as_a_question() {
        let f = check("The regent had to declare war; the council left him nothing else.");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].category, Category::ModalClaims);
        assert_eq!(f[0].severity, Severity::Inquiry);
        assert!(f[0].question.ends_with('?'), "{}", f[0].question);
        assert!(f[0].question.contains("had to"));
    }

    #[test]
    fn conditional_context_defuses_the_modal() {
        let f = check("If the council pressed him, the regent must act.");
        assert!(f.iter().all(|x| x.category != Category::ModalClaims), "got {f:?}");
    }

    #[test]
    fn flags_hedging() {
        let f = check("She seemed to know the road, somehow.");
        assert!(f.iter().any(|x| x.category == Category::HedgedUncertainty));
        assert!(f.iter().all(|x| x.question.ends_with('?')));
    }

    #[test]
    fn russian_modal_is_detected_and_localized() {
        // "The messenger had to ride three days" — должен был (must/had to).
        let f = check("Гонец должен был скакать три долгих дня без отдыха через горы и реки.");
        let modal: Vec<_> = f.iter().filter(|x| x.category == Category::ModalClaims).collect();
        assert_eq!(modal.len(), 1, "got {f:?}");
        // The question renders in Russian (Cyrillic); the English fallback uses
        // the English template (it quotes the Russian marker word, so it is not
        // pure ASCII, but the framing text is English).
        assert!(modal[0].question.chars().any(|c| ('а'..='я').contains(&c)));
        assert!(modal[0].question_en.contains("inevitable"));
        assert!(modal[0].question.ends_with('?'));
    }

    #[test]
    fn spanish_hedge_is_detected() {
        let f = check("El mensajero parecía conocer el camino, de algún modo, entre los montes.");
        assert!(f.iter().any(|x| x.category == Category::HedgedUncertainty), "got {f:?}");
    }

    #[test]
    fn german_modal_is_detected() {
        let f = check("Der Bote musste durch das weite Land und über die hohen Berge reiten.");
        assert!(f.iter().any(|x| x.category == Category::ModalClaims), "got {f:?}");
    }

    #[test]
    fn flags_anaphora_opening_word_run() {
        let f = check("He ran. He fell. He rose.");
        let sp: Vec<_> = f.iter().filter(|x| x.category == Category::StructuralPatterns).collect();
        assert_eq!(sp.len(), 1);
        assert!(sp[0].question.ends_with('?'));
    }

    #[test]
    fn flags_unattributed_dialogue_run() {
        let f = check("\u{201c}Where?\u{201d} \u{201c}There.\u{201d} \u{201c}Why?\u{201d} \u{201c}Because of the war.\u{201d}");
        let d: Vec<_> = f.iter().filter(|x| x.category == Category::UnattributedDialogue).collect();
        assert_eq!(d.len(), 1, "got {f:?}");
        assert_eq!(d[0].severity, Severity::Inquiry);
    }

    #[test]
    fn attribution_verb_silences_dialogue_finding() {
        let f = check("\u{201c}Where?\u{201d} she asked. \u{201c}There.\u{201d} \u{201c}Why?\u{201d} \u{201c}The war.\u{201d}");
        assert!(f.iter().all(|x| x.category != Category::UnattributedDialogue), "got {f:?}");
    }

    #[test]
    fn flags_a_very_long_sentence() {
        let long = "The regent walked through the hall and into the garden and past the fountain \
                    and around the wall and down the steps and along the path and over the bridge \
                    and through the gate and into the field and toward the distant and waiting army \
                    that had gathered there.";
        let f = check(long);
        assert!(f.iter().any(|x| x.category == Category::SentenceLengthAnomalies), "got {f:?}");
    }

    #[test]
    fn flags_a_tense_shift() {
        // Three clearly past sentences, one present outlier (not dialogue).
        let f = check(
            "The regent rode north. The roads were empty. The cold had settled deep. He is afraid now.",
        );
        assert!(f.iter().any(|x| x.category == Category::TenseVoiceShifts), "got {f:?}");
        assert!(f.iter().filter(|x| x.category == Category::TenseVoiceShifts).all(|x| x.question.ends_with('?')));
    }

    #[test]
    fn consistent_tense_raises_no_shift() {
        let f = check("The regent rode north. The roads were empty. The cold had settled. He was afraid.");
        assert!(f.iter().all(|x| x.category != Category::TenseVoiceShifts), "got {f:?}");
    }

    #[test]
    fn flags_ambiguous_pronoun() {
        // Two named people, then a bare pronoun.
        let f = check("Mara met Corin at the gate. She did not trust the silence.");
        assert!(f.iter().any(|x| x.category == Category::PronounAmbiguity), "got {f:?}");
    }

    #[test]
    fn single_referent_raises_no_pronoun_finding() {
        let f = check("Mara crossed the yard. She did not trust the silence.");
        assert!(f.iter().all(|x| x.category != Category::PronounAmbiguity), "got {f:?}");
    }

    #[test]
    fn persona_can_mute_a_category() {
        let mut p = socrates();
        p.emphasis.insert(Category::ModalClaims, 0.0);
        let f = check_paragraph(
            "The regent had to declare war.",
            &p,
            &IntentLedger::default(),
            &FindingContext::default(),
        );
        assert!(f.is_empty(), "muted category produces nothing; got {f:?}");
    }

    #[test]
    fn declared_intent_suppresses_the_finding() {
        let ledger = IntentLedger {
            entries: vec![IntentEntry {
                id: "e1".into(),
                kind: IntentKind::StylisticChoice,
                description: "The regent's fatalism is a deliberate motif".into(),
                scope: IntentScope::Chapter("ch07".into()),
                coverage: vec![Category::ModalClaims],
                scope_level: ScopeLevel::Project,
            }],
        };
        let ctx = FindingContext { chapter_id: Some("ch07".into()), ..Default::default() };
        let f = check_paragraph("The regent had to declare war.", &socrates(), &ledger, &ctx);
        assert!(f.is_empty(), "declared intent suppresses; got {f:?}");

        let elsewhere = FindingContext { chapter_id: Some("ch01".into()), ..Default::default() };
        let f2 = check_paragraph("The regent had to declare war.", &socrates(), &ledger, &elsewhere);
        assert_eq!(f2.len(), 1);
    }
}
