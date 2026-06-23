//! The Fast track — deterministic, instant (no LLM) Socratic observation. P0
//! ships two English categories (`modal_claims`, `hedged_uncertainty`); the
//! remaining five and the other four languages land in P1 / P5. Each detector
//! produces a **question**, never a correction (the non-prescriptive spine).
//!
//! Reuses WORLD-4's language machinery: `detect` gates the English detectors (a
//! non-English paragraph is skipped until its patterns ship), and `contains_word`
//! gives Unicode-aware whole-word matching.

use crate::world::fact_check_lang::{contains_word, detect, Lang};

use super::intent::{ConsultationResult, FindingContext, IntentLedger};
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

/// English dialogue attribution verbs — their presence means the speaker is
/// tagged somewhere in the passage.
const ATTRIBUTION_VERBS: &[&str] = &[
    "said", "asked", "replied", "whispered", "shouted", "muttered", "answered", "cried",
    "called", "added", "continued", "murmured", "growled", "snapped", "demanded", "breathed",
];

/// Markers of asserted inevitability (strong) — a claim treated as having no
/// alternative.
const MODAL_STRONG: &[&str] =
    &["must", "had to", "couldn't help but", "inevitably", "no choice", "no other choice"];
/// Markers that assert a proposition as simply given (moderate).
const MODAL_MODERATE: &[&str] = &["certainly", "obviously", "naturally", "surely", "of course"];
/// Nearby words that defuse a modal claim — a conditional or a co-occurring hedge
/// means the inevitability isn't actually being asserted.
const MODAL_DEFUSE: &[&str] = &["if", "unless", "perhaps", "maybe", "might", "could have"];

/// Markers of authorial hedging.
const HEDGE_MARKERS: &[&str] =
    &["perhaps", "might have", "seemed to", "as if", "somehow", "apparently", "may have"];

/// Run the Fast track over one paragraph for the active persona, consulting the
/// intent ledger. Returns the emitted findings (suppressed ones are dropped per
/// the RFC; they are logged elsewhere once storage lands). Pure + deterministic.
pub fn check_paragraph(
    text: &str,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
) -> Vec<SocraticFinding> {
    // Language gate — only the English detectors exist today; a non-English
    // paragraph is skipped (graceful degradation, mirroring WORLD-4).
    if detect(text) != Lang::En {
        return Vec::new();
    }
    let lower = text.to_lowercase();
    let sentences = text::sentences(text);
    let mut out = Vec::new();
    detect_modal_claims(&lower, persona, ledger, ctx, &mut out);
    detect_hedged_uncertainty(&lower, persona, ledger, ctx, &mut out);
    detect_structural_patterns(&sentences, persona, ledger, ctx, &mut out);
    detect_unattributed_dialogue(text, &lower, &sentences, persona, ledger, ctx, &mut out);
    detect_sentence_length(&sentences, persona, ledger, ctx, &mut out);
    out
}

/// `structural_patterns` — a run of sentences sharing an opening word (anaphora)
/// or an exact length (a monotone cadence). Emits at most one finding.
fn detect_structural_patterns(
    sentences: &[&str],
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    // Same opening word, STRUCTURAL_RUN in a row.
    let firsts: Vec<Option<String>> = sentences.iter().map(|s| text::first_word(s)).collect();
    if let Some(word) = longest_equal_run(&firsts).filter(|(_, n)| *n >= STRUCTURAL_RUN).and_then(|(w, _)| w) {
        push(
            out,
            persona,
            ledger,
            ctx,
            Category::StructuralPatterns,
            Severity::Notice,
            format!(
                "Several sentences here open with \u{201c}{word}\u{201d}. Is the repetition a \
                 deliberate cadence?"
            ),
        );
        return;
    }
    // Same exact length, SAME_LENGTH_RUN in a row.
    let lens: Vec<Option<usize>> =
        sentences.iter().map(|s| Some(text::word_count(s))).collect();
    if longest_equal_run(&lens).is_some_and(|(_, n)| n >= SAME_LENGTH_RUN) {
        push(
            out,
            persona,
            ledger,
            ctx,
            Category::StructuralPatterns,
            Severity::Notice,
            "A run of sentences here are near-identical in length. Is the even rhythm intended?"
                .to_string(),
        );
    }
}

/// `unattributed_dialogue` — a run of spoken segments with no attribution verb
/// anywhere in the passage.
#[allow(clippy::too_many_arguments)]
fn detect_unattributed_dialogue(
    text: &str,
    lower: &str,
    _sentences: &[&str],
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    let segments = text::dialogue_segment_count(text);
    if segments < DIALOGUE_RUN {
        return;
    }
    if ATTRIBUTION_VERBS.iter().any(|v| contains_word(lower, v)) {
        return; // a speaker is tagged somewhere
    }
    push(
        out,
        persona,
        ledger,
        ctx,
        Category::UnattributedDialogue,
        Severity::Inquiry,
        format!(
            "{segments} lines of dialogue pass here without a speaker tag. Can the reader still \
             tell who is speaking?"
        ),
    );
}

/// `sentence_length_anomalies` — a single very long sentence.
fn detect_sentence_length(
    sentences: &[&str],
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    if let Some(n) = sentences.iter().map(|s| text::word_count(s)).max() {
        if n > LONG_SENTENCE_WORDS {
            push(
                out,
                persona,
                ledger,
                ctx,
                Category::SentenceLengthAnomalies,
                Severity::Notice,
                format!(
                    "One sentence here runs to {n} words. Is its length carrying the reader, or \
                     losing them?"
                ),
            );
        }
    }
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

/// `modal_claims` — a passage that treats an outcome as inevitable. Defused by a
/// nearby conditional or hedge.
fn detect_modal_claims(
    lower: &str,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    if MODAL_DEFUSE.iter().any(|w| contains_word(lower, w)) {
        return; // conditional / hedged context — not actually asserting necessity
    }
    if let Some(marker) = MODAL_STRONG.iter().find(|m| contains_word(lower, m)) {
        push(
            out,
            persona,
            ledger,
            ctx,
            Category::ModalClaims,
            Severity::Inquiry,
            format!(
                "This passage treats an outcome as inevitable (\u{201c}{marker}\u{201d}). \
                 What alternatives did you decide to leave out?"
            ),
        );
    } else if let Some(marker) = MODAL_MODERATE.iter().find(|m| contains_word(lower, m)) {
        push(
            out,
            persona,
            ledger,
            ctx,
            Category::ModalClaims,
            Severity::Notice,
            format!(
                "The prose asserts this as given (\u{201c}{marker}\u{201d}). \
                 Is that certainty the narrator\u{2019}s, or a character\u{2019}s?"
            ),
        );
    }
}

/// `hedged_uncertainty` — authorial hedging worth being conscious of.
fn detect_hedged_uncertainty(
    lower: &str,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    out: &mut Vec<SocraticFinding>,
) {
    if let Some(marker) = HEDGE_MARKERS.iter().find(|m| contains_word(lower, m)) {
        push(
            out,
            persona,
            ledger,
            ctx,
            Category::HedgedUncertainty,
            Severity::Notice,
            format!(
                "The prose hedges here (\u{201c}{marker}\u{201d}). \
                 Is the uncertainty the character\u{2019}s, or the telling\u{2019}s?"
            ),
        );
    }
}

/// Build a finding, apply the persona's mute, then consult the ledger. Emits only
/// when the persona doesn't mute the category and no declared intent suppresses it.
fn push(
    out: &mut Vec<SocraticFinding>,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    category: Category,
    severity: Severity,
    question: String,
) {
    if persona.mutes(category) {
        return;
    }
    match ledger.consult(category, ctx) {
        ConsultationResult::Emit => out.push(SocraticFinding {
            category,
            severity,
            persona_id: persona.id.clone(),
            question_en: question.clone(),
            question,
            suppressed_by: None,
        }),
        ConsultationResult::Suppress { .. } => {
            // Suppressed by declared intent — dropped from the emit list (the
            // snapshot log keeps suppressions once storage lands, P2).
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inner_socrates::intent::{IntentEntry, IntentKind, IntentScope, ScopeLevel};

    fn socrates() -> Persona {
        Persona::default_inner_socrates()
    }

    #[test]
    fn flags_asserted_necessity_as_a_question() {
        let f = check_paragraph(
            "The regent had to declare war; the council left him nothing else.",
            &socrates(),
            &IntentLedger::default(),
            &FindingContext::default(),
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].category, Category::ModalClaims);
        assert_eq!(f[0].severity, Severity::Inquiry);
        // Non-prescriptive: it is a question.
        assert!(f[0].question.ends_with('?'), "{}", f[0].question);
        assert!(f[0].question.contains("had to"));
    }

    #[test]
    fn conditional_context_defuses_the_modal() {
        // "if" nearby → the necessity isn't actually asserted.
        let f = check_paragraph(
            "If the council pressed him, the regent must act.",
            &socrates(),
            &IntentLedger::default(),
            &FindingContext::default(),
        );
        assert!(f.iter().all(|x| x.category != Category::ModalClaims), "got {f:?}");
    }

    #[test]
    fn flags_hedging() {
        let f = check_paragraph(
            "She seemed to know the road, somehow.",
            &socrates(),
            &IntentLedger::default(),
            &FindingContext::default(),
        );
        assert!(f.iter().any(|x| x.category == Category::HedgedUncertainty));
        assert!(f.iter().all(|x| x.question.ends_with('?')));
    }

    #[test]
    fn non_english_is_skipped() {
        let f = check_paragraph(
            "Гонец должен был скакать три дня без отдыха через горы и реки.",
            &socrates(),
            &IntentLedger::default(),
            &FindingContext::default(),
        );
        assert!(f.is_empty(), "non-English skipped until P5; got {f:?}");
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
    fn flags_anaphora_opening_word_run() {
        let f = check_paragraph(
            "He ran. He fell. He rose.",
            &socrates(),
            &IntentLedger::default(),
            &FindingContext::default(),
        );
        let sp: Vec<_> = f.iter().filter(|x| x.category == Category::StructuralPatterns).collect();
        assert_eq!(sp.len(), 1);
        assert!(sp[0].question.ends_with('?'));
        assert!(sp[0].question.to_lowercase().contains("he"));
    }

    #[test]
    fn flags_monotone_same_length_run() {
        let f = check_paragraph(
            "He ran fast. She fell hard. They rose again. We left town.",
            &socrates(),
            &IntentLedger::default(),
            &FindingContext::default(),
        );
        assert!(f.iter().any(|x| x.category == Category::StructuralPatterns));
    }

    #[test]
    fn flags_unattributed_dialogue_run() {
        let f = check_paragraph(
            "\u{201c}Where?\u{201d} \u{201c}There.\u{201d} \u{201c}Why?\u{201d} \u{201c}Because of the war.\u{201d}",
            &socrates(),
            &IntentLedger::default(),
            &FindingContext::default(),
        );
        let d: Vec<_> = f.iter().filter(|x| x.category == Category::UnattributedDialogue).collect();
        assert_eq!(d.len(), 1, "got {f:?}");
        assert_eq!(d[0].severity, Severity::Inquiry);
        assert!(d[0].question.ends_with('?'));
    }

    #[test]
    fn attribution_verb_silences_dialogue_finding() {
        let f = check_paragraph(
            "\u{201c}Where?\u{201d} she asked. \u{201c}There.\u{201d} \u{201c}Why?\u{201d} \u{201c}The war.\u{201d}",
            &socrates(),
            &IntentLedger::default(),
            &FindingContext::default(),
        );
        assert!(f.iter().all(|x| x.category != Category::UnattributedDialogue), "got {f:?}");
    }

    #[test]
    fn flags_a_very_long_sentence() {
        // A 50-word sentence with plenty of English function words.
        let long = "The regent walked through the hall and into the garden and past the fountain \
                    and around the wall and down the steps and along the path and over the bridge \
                    and through the gate and into the field and toward the distant and waiting army \
                    that had gathered there.";
        let f = check_paragraph(long, &socrates(), &IntentLedger::default(), &FindingContext::default());
        assert!(f.iter().any(|x| x.category == Category::SentenceLengthAnomalies), "got {f:?}");
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

        // Same prose in a different chapter is not covered → still flagged.
        let elsewhere = FindingContext { chapter_id: Some("ch01".into()), ..Default::default() };
        let f2 = check_paragraph("The regent had to declare war.", &socrates(), &ledger, &elsewhere);
        assert_eq!(f2.len(), 1);
    }
}
