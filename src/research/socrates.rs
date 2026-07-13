//! SCHOLAR — Inner Socrates over the research corpus.
//!
//! Points Inner Socrates' Dialectician persona at the collected **Facts** rather
//! than manuscript prose: given a topic (or the whole corpus), it asks the
//! Socratic questions a facts corpus must answer to stand — what the facts treat
//! as given, what would refute them, the objection left unaddressed. The entire
//! prompt-assembly + parse chain is reused verbatim from `crate::inner_socrates`;
//! only the target (a facts block) and the genre framing differ.

use std::path::Path;

use crate::inner_socrates::personas;
use crate::inner_socrates::slow;
use crate::inner_socrates::types::{Persona, SocraticFinding};

/// The Dialectician (`philosophical-reader`) — presses unstated premises, terms
/// that shift meaning, and unanswered objections. Fixed regardless of the
/// project's active persona (a facts corpus wants the interrogator).
pub(super) fn dialectician(project: &Path) -> Persona {
    personas::by_id(project, "philosophical-reader")
}

/// Build the `(system, user)` prompts for a Socratic reading of `facts_block`.
/// `genre` frames the reading (academic / philosophy / theology).
pub(super) fn build_prompts(
    persona: &Persona,
    genre: Option<&str>,
    facts_block: &str,
) -> (String, String) {
    let system = slow::slow_system_for(persona.stance, genre);
    let lang = crate::world::fact_check_lang::detect(facts_block);
    let user = slow::build_slow_prompt(persona, facts_block, "None.", &[], lang);
    (system, user)
}

/// Parse the LLM reply (a JSON array) into Socratic questions.
pub(super) fn parse(raw: &str, persona_id: &str) -> Vec<SocraticFinding> {
    slow::parse_slow_findings(raw, persona_id)
}

/// Render the questions into a chat body.
pub(super) fn render(topic: &str, findings: &[SocraticFinding]) -> String {
    if findings.is_empty() {
        return "No Socratic questions rose from the corpus — the facts examined here go, \
                for now, unquestioned. (Try a narrower topic, or gather more.)"
            .to_string();
    }
    let target = if topic.trim().is_empty() {
        "the collected facts".to_string()
    } else {
        format!("“{}”", topic.trim())
    };
    let mut s = format!(
        "The Dialectician's questions on {target} ({}) — the questions your facts must answer to stand:\n",
        findings.len()
    );
    for f in findings {
        s.push_str(&format!(
            "\n  · [{} · {}] {}\n",
            f.category.label(),
            f.severity.label(),
            f.question
        ));
    }
    s
}
