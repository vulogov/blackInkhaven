//! INNER-THEOLOGIAN-1 (IT-P5) — the slow-track persona and session prompt.
//! Mirrors `inner_socrates::slow::build_slow_prompt`: an English template plus an
//! in-language directive (`write the questions in {language}`), so the model
//! renders in the project language — the codebase's established way to localise
//! an LLM feature (no per-language question banks).

use crate::prose::ProseLanguage;

use super::QuestionCategory;
use super::TraditionLens;
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

/// The lenses available for a passage = all eleven minus any the author disabled.
pub(crate) fn available_lenses(disabled: &[String]) -> Vec<TraditionLens> {
    TraditionLens::ALL
        .into_iter()
        .filter(|l| !disabled.iter().any(|d| d.eq_ignore_ascii_case(l.as_code())))
        .collect()
}

/// CALL 1 — lens discovery. Ask the model which of the available lenses genuinely
/// illuminate this passage (and which are tellingly silent), so the analysis call
/// (call 2) focuses on the right ones instead of a fixed default. Surface markers
/// are offered as a soft hint. Returns a prompt expecting a small JSON object.
pub(crate) fn build_discovery_prompt(passage: &str, disabled: &[String]) -> String {
    let avail = available_lenses(disabled);
    let menu = avail
        .iter()
        .map(|l| format!("{} ({})", l.label(), l.as_code()))
        .collect::<Vec<_>>()
        .join(", ");
    let suggested: Vec<_> = suggest_lenses(passage)
        .into_iter()
        .filter(|l| avail.contains(l))
        .collect();
    let hint = if suggested.is_empty() {
        String::new()
    } else {
        format!(
            "Surface markers point especially toward: {}. Weigh them, but you are not bound by them.\n\n",
            suggested.iter().map(|l| l.as_code()).collect::<Vec<_>>().join(", ")
        )
    };
    format!(
        "You are about to read a manuscript passage through the lenses of the world's moral and \
         theological traditions. First, decide which lenses are worth bringing to bear.\n\n\
         Available lenses (code in parentheses): {menu}.\n\n\
         {hint}\
         Consider the passage through ALL of them. Then select the two to four whose questions would \
         be GENUINELY most illuminating for THIS specific passage — vary by passage, do not default \
         to a fixed set. Also note up to two lenses whose SILENCE is telling here (a moral question \
         this work conspicuously never raises).\n\n\
         Return ONLY this JSON, using the codes above:\n\
         {{\"selected\":[\"code\",...],\"silent\":[\"code\",...]}}\n\n\
         PASSAGE:\n{passage}"
    )
}

/// Parse call-1's JSON into `(selected, silent)` lens lists. Tolerant of fences /
/// surrounding prose; unknown codes are dropped.
pub(crate) fn parse_selected_lenses(raw: &str) -> (Vec<TraditionLens>, Vec<TraditionLens>) {
    let json = super::llm::extract_json_object(raw);
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return (Vec::new(), Vec::new()),
    };
    let codes = |key: &str| -> Vec<TraditionLens> {
        v.get(key)
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c.as_str())
                    .filter_map(TraditionLens::from_code)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    (codes("selected"), codes("silent"))
}

/// CALL 2 — analysis. Build the slow-track question prompt using the lenses
/// discovered in call 1 (`selected`), naming any tellingly-`silent` ones. Falls
/// back to "choose for yourself from the available set" if discovery returned
/// nothing. `grounding_prefix` (IT-P6) is prepended verbatim when present.
pub(crate) fn build_session_prompt(
    category: QuestionCategory,
    passage: &str,
    selected: &[TraditionLens],
    silent: &[TraditionLens],
    grounding_prefix: Option<&str>,
    lang: &ProseLanguage,
) -> String {
    let lens_instruction = if selected.is_empty() {
        "Choose the two to four tradition lenses most illuminating for this passage yourself."
            .to_string()
    } else {
        format!(
            "Bring these lenses, chosen as most illuminating for this passage: {}.",
            selected.iter().map(|l| l.label()).collect::<Vec<_>>().join(", ")
        )
    };
    let silent_note = if silent.is_empty() {
        String::new()
    } else {
        format!(
            " Where it is telling, you may also name the silence of: {}.",
            silent.iter().map(|l| l.label()).collect::<Vec<_>>().join(", ")
        )
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
         {lens_instruction}{silent_note} Name which tradition raises which question, and invite the \
         author to say any lens is irrelevant to their intention.\n\n\
         Question templates for this category (adapt them to this passage — do not recite verbatim):\n\
         {qs}\n\n\
         Pose two to four questions. Write them in {language}. Gloss every tradition-specific term \
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
    fn analysis_prompt_uses_selected_lenses_language_grounding() {
        let p = build_session_prompt(
            QuestionCategory::MoralWeight,
            "He gave his life as a sacrifice for the others.",
            &[TraditionLens::Orthodox, TraditionLens::Catholic],
            &[TraditionLens::Confucianism],
            Some("GROUNDING: a stalled redemption arc was declared for Mara."),
            &ProseLanguage::Fr,
        );
        assert!(p.contains("Category 1 — Moral weight"));
        assert!(p.contains("Write them in French"));
        assert!(p.contains("GROUNDING:"));
        assert!(p.contains("Orthodox") && p.contains("Catholic"));
        assert!(p.contains("silence of: Confucianism"));
        assert!(p.contains("PASSAGE:"));
    }

    #[test]
    fn discovery_prompt_lists_all_available_and_requests_json() {
        let p = build_discovery_prompt("He gave his life as a sacrifice.", &["gnostic".into()]);
        assert!(p.contains("\"selected\""));
        assert!(p.contains("Buddhism (buddhism)"));
        // The disabled lens is excluded from the menu.
        assert!(!p.contains("(gnostic)"));
    }

    #[test]
    fn parse_selected_tolerant() {
        let (sel, sil) = parse_selected_lenses(
            "sure: {\"selected\":[\"orthodox\",\"judaism\",\"nope\"],\"silent\":[\"islam\"]}",
        );
        assert_eq!(sel, vec![TraditionLens::Orthodox, TraditionLens::Judaism]);
        assert_eq!(sil, vec![TraditionLens::Islam]);
        assert_eq!(parse_selected_lenses("not json"), (vec![], vec![]));
    }
}
