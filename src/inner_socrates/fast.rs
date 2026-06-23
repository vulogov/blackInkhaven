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
use super::types::{Category, Persona, Severity, SocraticFinding};

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
    let mut out = Vec::new();
    detect_modal_claims(&lower, persona, ledger, ctx, &mut out);
    detect_hedged_uncertainty(&lower, persona, ledger, ctx, &mut out);
    out
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
