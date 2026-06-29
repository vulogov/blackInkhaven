//! INNER-THEOLOGIAN-1 (IT-P5) — the slow-track persona and session prompt.
//! Mirrors `inner_socrates::slow::build_slow_prompt`: an English template plus an
//! in-language directive (`write the questions in {language}`), so the model
//! renders in the project language — the codebase's established way to localise
//! an LLM feature (no per-language question banks).

use crate::prose::ProseLanguage;

use super::QuestionCategory;
use super::corpus::questions_for;
use super::lens::suggest_lenses;

/// The persona identity and its hard constraints (RFC §5.1 / §5.3). Belongs to no
/// tradition, advocates none, names its lens, never delivers a verdict.
pub(crate) const THEOLOGIAN_SYSTEM: &str = "You are a reader who approaches a manuscript through the \
lenses of the world's major moral and theological traditions — Catholic, Protestant, Orthodox, \
Gnostic, LDS, Islam, Judaism, Hinduism, Buddhism, Confucianism, and secular moral philosophy — not to \
judge the work by any of them, but to ask what each of them sees, and what those different visions \
reveal about what the work is doing.\n\n\
You belong to no tradition and advocate for none. When you raise a Buddhist question you are not \
recommending Buddhism; when you raise a Gnostic question you are not advocating Gnosticism. Each \
tradition is a lens you pick up and put down. You are always explicit about which tradition raises \
which question, and you always invite the author to say that a lens is irrelevant to their intention \
— that is useful information too.\n\n\
You assume neither that the author is religious nor that the manuscript intends theological content; \
every work has an implicit moral cosmology, and you make it visible whether or not the author placed \
it there consciously. You gloss every tradition-specific term inline so the author needs no prior \
knowledge. You do not adjudicate disputes between traditions, and you do not check works for \
doctrinal correctness.\n\n\
You ask questions and offer observations. You NEVER deliver a verdict, never tell the author their \
work is wrong, sinful, or deficient by any tradition's standard, and never prescribe a change. \
Everything you produce is a question or an invitation to reflection.";

/// The language name for the in-language directive. `Other` → English.
pub(crate) fn language_name(lang: &ProseLanguage) -> &'static str {
    match lang {
        ProseLanguage::En => "English",
        ProseLanguage::Ru => "Russian",
        ProseLanguage::De => "German",
        ProseLanguage::Fr => "French",
        ProseLanguage::Es => "Spanish",
        ProseLanguage::Other(_) => "English",
    }
}

/// Build the slow-track user prompt for a session: the chosen category, the lens
/// hints for the passage, the question templates, the in-language directive, and
/// the passage. `grounding_prefix` (IT-P6) is prepended verbatim when present.
pub(crate) fn build_session_prompt(
    category: QuestionCategory,
    passage: &str,
    grounding_prefix: Option<&str>,
    lang: &ProseLanguage,
    disabled_lenses: &[String],
) -> String {
    let lenses: Vec<_> = suggest_lenses(passage)
        .into_iter()
        .filter(|l| !disabled_lenses.iter().any(|d| d.eq_ignore_ascii_case(l.as_code())))
        .collect();
    let lens_list = if lenses.is_empty() {
        "(author has disabled the suggested lenses — choose any that fit)".to_string()
    } else {
        lenses.iter().map(|l| l.label()).collect::<Vec<_>>().join(", ")
    };
    let qs = questions_for(category)
        .iter()
        .enumerate()
        .map(|(i, q)| format!("{}. {q}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");
    let language = language_name(lang);
    let grounding = grounding_prefix.map(|g| format!("{}\n\n", g.trim())).unwrap_or_default();
    format!(
        "{grounding}Category {} — {}.\n\n\
         The lenses most illuminating for this passage appear to be: {lens_list}. Use the ones that \
         fit; name which tradition raises which question; invite the author to say a lens is \
         irrelevant.\n\n\
         Question templates for this category (adapt them to this passage — do not recite verbatim):\n\
         {qs}\n\n\
         Pose two or three questions. Write them in {language}. Gloss every tradition-specific term \
         inline. Ask only — never judge, never prescribe.\n\n\
         PASSAGE:\n{passage}",
        category.number(),
        category.label(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_states_neutrality_and_no_verdict() {
        assert!(THEOLOGIAN_SYSTEM.contains("belong to no tradition"));
        assert!(THEOLOGIAN_SYSTEM.contains("NEVER deliver a verdict"));
    }

    #[test]
    fn language_names_cover_all() {
        assert_eq!(language_name(&ProseLanguage::Ru), "Russian");
        assert_eq!(language_name(&ProseLanguage::Other("pl".into())), "English");
    }

    #[test]
    fn prompt_injects_category_lenses_language_and_grounding() {
        let p = build_session_prompt(
            QuestionCategory::MoralWeight,
            "He gave his life as a sacrifice for the others.",
            Some("GROUNDING: a stalled redemption arc was declared for Mara."),
            &ProseLanguage::Fr,
            &[],
        );
        assert!(p.contains("Category 1 — Moral weight"));
        assert!(p.contains("Write them in French"));
        assert!(p.contains("GROUNDING:"));
        // The sacrifice marker should surface Orthodox/Catholic/Protestant lenses.
        assert!(p.contains("Orthodox") || p.contains("Catholic"));
        assert!(p.contains("PASSAGE:"));
    }
}
