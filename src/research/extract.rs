//! RESRCH-1 (R-P10/R-P11) — fact / note extraction. The second LLM call: given
//! the last research response and a clarifying instruction, produce ONE titled
//! entry as JSON `{title, fact}`. The author then edits + confirms it before it
//! is inserted (the confirmation step is non-negotiable — RFC §3).

use serde::Deserialize;

/// Which system book an extraction targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetBook {
    Facts,
    Notes,
}

impl TargetBook {
    pub(super) fn label(self) -> &'static str {
        match self {
            TargetBook::Facts => "Facts",
            TargetBook::Notes => "Notes",
        }
    }

    pub(super) fn system_tag(self) -> &'static str {
        match self {
            TargetBook::Facts => crate::store::SYSTEM_TAG_FACTS,
            TargetBook::Notes => crate::store::SYSTEM_TAG_NOTES,
        }
    }
}

/// One extracted entry (the parsed JSON, or the fallback).
pub(super) struct ExtractedFact {
    pub title: String,
    pub text: String,
}

#[derive(Deserialize)]
struct ExtractDoc {
    #[serde(default)]
    title: String,
    #[serde(default)]
    fact: String,
}

/// The project-language name for the in-language directive (`Other` → English).
pub(super) fn language_name(lang: &crate::prose::ProseLanguage) -> &'static str {
    use crate::prose::ProseLanguage::*;
    match lang {
        En => "English",
        Ru => "Russian",
        De => "German",
        Fr => "French",
        Es => "Spanish",
        Other(_) => "English",
    }
}

/// The extraction system prompt (RFC §10.2 for Facts; §11 variant for Notes).
/// `language` is the project language name — the extraction must stay in it (and
/// in the language of the research response), never silently translate to English.
pub(super) fn system_prompt(book: TargetBook, language: &str, instruction: &str, research: &str) -> String {
    let rules = match book {
        TargetBook::Facts => {
            "Rules:\n\
             - One to three sentences maximum\n\
             - Declarative and self-contained (readable without the research context)\n\
             - Preserve stated uncertainty (\"historians debate...\", \"approximately...\")\n\
             - Do NOT add information not present in the research response\n\
             - Do NOT interpret; extract"
        }
        TargetBook::Notes => {
            "Rules:\n\
             - Capture the author's observation, hypothesis, or connection as expressed in the \
             research response\n\
             - The note may be speculative or tentative; preserve that quality\n\
             - Do NOT add information not present in the research response"
        }
    };
    format!(
        "You are extracting a single {kind} for a writer's reference database.\n\n\
         The author provides a research response and a clarifying instruction.\n\
         Your task: produce ONE {kind} entry.\n\n\
         {rules}\n\
         - LANGUAGE: write BOTH the title and the {kind} text in {language} — the \
         same language as the research response. Do NOT translate to English.\n\n\
         Return JSON only — no preamble, no markdown fences:\n\
         {{\n  \"title\": \"3-7 word title in {language}, no period\",\n  \"fact\": \"The extracted {kind} text in {language}.\"\n}}\n\n\
         Author's clarifying instruction: {instruction}\n\n\
         Research response to extract from:\n{research}",
        kind = if book == TargetBook::Facts { "fact" } else { "note" },
    )
}

/// The default clarifying instruction when `/fact` / `/note` is called bare.
pub(super) fn default_instruction(book: TargetBook) -> &'static str {
    match book {
        TargetBook::Facts => "Extract the single most important fact from the research above.",
        TargetBook::Notes => "Capture the key observation or connection from the research above.",
    }
}

/// Parse the extraction response. On non-JSON output, fall back to the raw text
/// as the body with an empty title, so the author can still edit + insert.
pub(super) fn parse(raw: &str) -> ExtractedFact {
    let slice = extract_json_object(raw);
    if let Ok(doc) = serde_json::from_str::<ExtractDoc>(slice) {
        if !doc.fact.trim().is_empty() || !doc.title.trim().is_empty() {
            return ExtractedFact { title: doc.title.trim().to_string(), text: doc.fact.trim().to_string() };
        }
    }
    ExtractedFact { title: String::new(), text: raw.trim().to_string() }
}

/// First top-level `{…}` object (models wrap JSON in prose / fences).
fn extract_json_object(raw: &str) -> &str {
    match (raw.find('{'), raw.rfind('}')) {
        (Some(a), Some(b)) if b > a => &raw[a..=b],
        _ => raw.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_json() {
        let raw = "Sure:\n```json\n{\"title\":\"Aqua Claudia Capacity\",\"fact\":\"It carried ~190,000 m³/day.\"}\n```";
        let f = parse(raw);
        assert_eq!(f.title, "Aqua Claudia Capacity");
        assert!(f.text.contains("190,000"));
    }

    #[test]
    fn fallback_on_non_json() {
        let f = parse("I could not find a specific figure.");
        assert_eq!(f.title, "");
        assert!(f.text.contains("could not find"));
    }

    #[test]
    fn note_prompt_differs_from_fact() {
        let fact = system_prompt(TargetBook::Facts, "Russian", "x", "y");
        let note = system_prompt(TargetBook::Notes, "Russian", "x", "y");
        assert!(fact.contains("Declarative and self-contained"));
        assert!(note.contains("speculative or tentative"));
        assert!(note.contains("note entry"));
        // The in-language directive is present.
        assert!(fact.contains("in Russian"));
        assert!(note.contains("in Russian"));
    }
}
