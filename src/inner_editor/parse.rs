//! IE-P2 — parse the LLM's structured response into `EditorFinding`s.
//!
//! The contract (set by `prompt::system_prompt`): a JSON array of
//! `{category, severity, observation, observation_en, evidence, conditional}`.
//! Tolerant of code fences / surrounding prose; keeps only the categories the
//! tuning declared active; drops malformed items rather than failing.

use super::types::{EditorCategory, EditorFinding, EditorSeverity};

/// Parse the raw LLM response, keeping only findings whose category is in
/// `active` (the tuning's whitelist).
pub fn parse_findings(raw: &str, active: &[EditorCategory]) -> Vec<EditorFinding> {
    let Some(json) = extract_json_array(raw) else {
        return Vec::new();
    };
    let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|v| {
            let category = EditorCategory::from_id(v.get("category").and_then(|c| c.as_str())?)?;
            if !active.contains(&category) {
                return None;
            }
            let observation = v.get("observation").and_then(|o| o.as_str())?.trim().to_string();
            if observation.is_empty() {
                return None;
            }
            // English fallback: the model's `observation_en` if present, else the
            // observation itself (the AI bridge tolerates either).
            let observation_en = v
                .get("observation_en")
                .and_then(|o| o.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| observation.clone());
            let severity =
                EditorSeverity::from_id(v.get("severity").and_then(|s| s.as_str()).unwrap_or(""));
            let evidence = v
                .get("evidence")
                .and_then(|e| e.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            // Default conditional = true (the safer, non-prescriptive framing) when
            // the model omits it.
            let conditional = v.get("conditional").and_then(|c| c.as_bool()).unwrap_or(true);
            Some(EditorFinding {
                category,
                severity,
                observation,
                observation_en,
                evidence,
                conditional,
                suppressed_by: None,
            })
        })
        .collect()
}

/// Extract the first balanced top-level JSON array from `raw` — tolerating
/// ```json fences, leading prose, and trailing chatter. String-aware so a `]`
/// inside an observation doesn't end the array early.
fn extract_json_array(raw: &str) -> Option<String> {
    let start = raw.find('[')?;
    let bytes = raw.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'[' => depth += 1,
            b']' => {
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

    const ALL: [EditorCategory; 8] = EditorCategory::ALL;

    #[test]
    fn parses_a_clean_array() {
        let raw = r#"[
            {"category":"craft_praise","severity":"praise","observation":"the cadence lands",
             "observation_en":"the cadence lands","evidence":"second sentence","conditional":false},
            {"category":"style_observation","severity":"note","observation":"register shifts",
             "observation_en":"register shifts","conditional":true}
        ]"#;
        let fs = parse_findings(raw, &ALL);
        assert_eq!(fs.len(), 2);
        assert_eq!(fs[0].severity, EditorSeverity::Praise);
        assert_eq!(fs[0].evidence.as_deref(), Some("second sentence"));
        assert!(!fs[0].conditional);
        assert_eq!(fs[1].category, EditorCategory::StyleObservation);
    }

    #[test]
    fn tolerates_fences_and_prose_and_inner_brackets() {
        let raw = "Here are my notes:\n```json\n[{\"category\":\"tautology\",\"severity\":\"concern\",\
                   \"observation\":\"the idea repeats [twice] here\",\"observation_en\":\"x\"}]\n```\nDone.";
        let fs = parse_findings(raw, &ALL);
        assert_eq!(fs.len(), 1);
        assert!(fs[0].observation.contains("[twice]"));
        // conditional defaults to true when omitted.
        assert!(fs[0].conditional);
    }

    #[test]
    fn filters_inactive_categories_and_bad_items() {
        let raw = r#"[
            {"category":"belief_stance","severity":"note","observation":"a"},
            {"category":"not_a_category","severity":"note","observation":"b"},
            {"category":"tautology","severity":"note","observation":""}
        ]"#;
        // belief_stance NOT in the active set → dropped; bad category dropped;
        // empty observation dropped.
        let active = [EditorCategory::Tautology, EditorCategory::StyleObservation];
        assert!(parse_findings(raw, &active).is_empty());
    }

    #[test]
    fn empty_or_garbage_yields_nothing() {
        assert!(parse_findings("[]", &ALL).is_empty());
        assert!(parse_findings("no json here", &ALL).is_empty());
        assert!(parse_findings("[ broken", &ALL).is_empty());
    }
}
