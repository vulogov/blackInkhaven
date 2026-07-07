//! WORLD-12 — the AI world-critique pass. Given the author's declared world
//! (`world.hjson`) plus a compact summary of what it compiles to, an LLM looks
//! for consistency problems, physical implausibilities, and missed chances for
//! realism, and returns concrete recommendations. Advisory only — it never edits
//! `world.hjson`; `realworld critique` prints the findings and, on request,
//! writes each as a Notes-book recommendation.
//!
//! This module is pure: the prompt text, the finding type, and a tolerant parser.
//! The CLI (`src/cli/realworld.rs`) builds the summary, runs the cost-capped call
//! (mirroring the fact-check slow track), and commits Notes entries.

use serde::Deserialize;

/// One critique finding: a facet of the world, what's off, and what to do.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CritiqueItem {
    /// The world facet — astronomy / geology / climate / hydrology / demographics
    /// / culture / history / magic (free string; the model is guided, not bound).
    #[serde(default)]
    pub aspect: String,
    /// The inconsistency or implausibility observed.
    #[serde(default)]
    pub issue: String,
    /// A concrete, actionable recommendation to fix or deepen it.
    #[serde(default)]
    pub recommendation: String,
    /// `high` / `medium` / `low` (free string; normalized on display).
    #[serde(default)]
    pub severity: String,
}

impl CritiqueItem {
    /// Drop items with nothing useful in them (the model occasionally emits a
    /// blank object when the world is sound).
    pub fn is_usable(&self) -> bool {
        !self.issue.trim().is_empty() || !self.recommendation.trim().is_empty()
    }

    /// Severity rank for sorting (high first); unknown → medium.
    pub fn severity_rank(&self) -> u8 {
        match self.severity.trim().to_lowercase().as_str() {
            "high" => 0,
            "low" => 2,
            _ => 1,
        }
    }
}

/// The system prompt. Constrains the model to a JSON array and to advisory,
/// non-prose-editing behaviour, and asks it to answer in the project language.
pub fn critique_system(lang: &str) -> String {
    format!(
        "You are a rigorous worldbuilding reviewer. You are given a fantasy/SF world's \
         declared definition (HJSON) and a summary of what it deterministically compiles to \
         (its astronomy, geology, climate, hydrology, demographics, peoples, and magic). \
         Your job is to find CONSISTENCY problems (a declared value that contradicts its \
         compiled consequence), PHYSICAL IMPLAUSIBILITIES (an axial tilt, star class, sea \
         level, or population that would not produce the described world), and MISSED \
         OPPORTUNITIES to make the world more realistic. \
         Respond with ONLY a JSON array (no prose, no code fences) of objects with keys: \
         \"aspect\" (the world facet), \"issue\" (what is off, citing the declared value), \
         \"recommendation\" (a concrete, actionable fix or deepening), and \"severity\" \
         (one of high, medium, low). Be specific and grounded in the numbers given; do not \
         invent facts not implied by the input. If the world is sound, return []. \
         Write the \"issue\" and \"recommendation\" text in {lang}."
    )
}

/// Assemble the user prompt from the raw declaration and the compiled summary.
pub fn build_critique_prompt(hjson: &str, compiled_summary: &str) -> String {
    // Bound the declaration so a huge hand-edited file can't blow the context.
    // Cut on a char boundary — a byte slice would panic on a world.hjson whose
    // byte MAX_HJSON lands mid-character (non-ASCII names are common).
    const MAX_HJSON: usize = 8_000;
    let decl = if hjson.len() > MAX_HJSON {
        let mut cut = MAX_HJSON;
        while cut > 0 && !hjson.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}\n… (truncated)", &hjson[..cut])
    } else {
        hjson.to_string()
    };
    format!(
        "=== DECLARED world.hjson ===\n{decl}\n\n=== COMPILED consequences ===\n{compiled_summary}\n\n\
         Return the JSON array of findings now."
    )
}

/// Parse the model's reply into findings. Tolerant: pulls the first JSON array
/// out of any surrounding prose or ```json fences, drops empty items, and sorts
/// by severity (high first). Returns empty on anything unparseable.
pub fn parse_critique(raw: &str) -> Vec<CritiqueItem> {
    let Some(json) = extract_json_array(raw) else {
        return Vec::new();
    };
    let mut items: Vec<CritiqueItem> = serde_json::from_str(&json).unwrap_or_default();
    items.retain(|i| i.is_usable());
    items.sort_by_key(|i| i.severity_rank());
    items
}

/// Extract the first top-level `[ … ]` array substring, ignoring code fences and
/// prose. Bracket-balanced (respecting strings/escapes) so nested objects survive.
fn extract_json_array(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let start = raw.find('[')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for i in start..bytes.len() {
        let c = bytes[i] as char;
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(raw[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_fenced_json_array_and_sorts_by_severity() {
        let raw = r#"Here are the findings:
```json
[
  {"aspect":"climate","issue":"minor","recommendation":"tweak","severity":"low"},
  {"aspect":"astronomy","issue":"tilt 89° would give extreme seasons","recommendation":"lower it","severity":"high"}
]
```
Hope this helps."#;
        let items = parse_critique(raw);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].severity, "high"); // sorted high-first
        assert_eq!(items[0].aspect, "astronomy");
    }

    #[test]
    fn drops_empty_items_and_tolerates_junk() {
        let raw = r#"[{"aspect":"x","issue":"","recommendation":""},{"aspect":"geology","issue":"real","recommendation":"do"}]"#;
        let items = parse_critique(raw);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].aspect, "geology");
        // Unparseable → empty, never panics.
        assert!(parse_critique("no json here").is_empty());
        assert!(parse_critique("").is_empty());
    }

    #[test]
    fn a_large_multibyte_hjson_truncates_without_panicking() {
        // H1: a >8000-byte file of 3-byte chars would panic on a byte slice at a
        // non-boundary. Build one that straddles the cut and confirm no panic.
        let big = "名".repeat(5000); // 3 bytes each → 15000 bytes
        let prompt = build_critique_prompt(&big, "summary");
        assert!(prompt.contains("(truncated)"));
        assert!(prompt.contains("COMPILED consequences"));
    }

    #[test]
    fn nested_objects_and_strings_with_brackets_survive() {
        let raw = r#"[{"aspect":"magic","issue":"a rule mentions [brackets]","recommendation":"fine","severity":"medium"}]"#;
        let items = parse_critique(raw);
        assert_eq!(items.len(), 1);
        assert!(items[0].issue.contains("[brackets]"));
    }
}
