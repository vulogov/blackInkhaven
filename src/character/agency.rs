//! CHAR-1 (C-P3) — the deterministic agency score. Per character per chapter:
//! the ratio of *active* presence (the character acts) to total active+passive
//! presence in their relevant sentences. No LLM, no dependency parser — a
//! windowed name↔action-verb heuristic, gated by NARR-1's passive detection.
//!
//! `agency = active / (active + passive)`, or `None` when both are 0 (mentioned
//! only in neutral context — insufficient signal, not zero agency).

use crate::prose::{CompiledLexicon, ProseLanguage, detect_passive};

use super::{ActionVerbs, is_action_verb};

/// Token-distance windows (RFC §15 defaults).
#[derive(Debug, Clone, Copy)]
pub(super) struct AgencyWindows {
    pub before: usize,
    pub after: usize,
}

impl Default for AgencyWindows {
    fn default() -> Self {
        AgencyWindows { before: 5, after: 8 }
    }
}

/// Compute `(agency_score, active_count, passive_count)` for `character` over
/// `text` (the character's relevant paragraphs). `others` is the rest of the
/// Characters roster (lowercased), used to break the active window when another
/// named character sits between the name and the verb.
pub(super) fn compute_agency(
    text: &str,
    character: &str,
    others: &[String],
    lang: &ProseLanguage,
    lx: &CompiledLexicon,
    av: &ActionVerbs,
    win: AgencyWindows,
) -> (Option<f32>, u32, u32) {
    let name_parts: Vec<String> = character.to_lowercase().split_whitespace().map(str::to_string).collect();
    let other_parts: Vec<Vec<String>> = others
        .iter()
        .map(|o| o.to_lowercase().split_whitespace().map(str::to_string).collect())
        .filter(|p: &Vec<String>| !p.is_empty())
        .collect();
    if name_parts.is_empty() {
        return (None, 0, 0);
    }

    let (mut active, mut passive) = (0u32, 0u32);
    for sentence in split_sentences(text) {
        let toks = tokenize(&sentence);
        let name_pos = match_positions(&toks, &name_parts);
        if name_pos.is_empty() {
            continue;
        }
        let other_pos: Vec<usize> =
            other_parts.iter().flat_map(|p| match_positions(&toks, p)).collect();
        let verb_pos: Vec<usize> =
            toks.iter().enumerate().filter(|(_, t)| is_action_verb(t, av)).map(|(i, _)| i).collect();
        let is_passive = detect_passive(&sentence, lang, lx);

        let mut sent_active = false;
        let mut sent_passive = false;
        for &v in &verb_pos {
            // Active: a name in [v-before, v) with no other name between it and v.
            let active_hit = name_pos.iter().any(|&n| {
                n < v && v - n <= win.before && !other_pos.iter().any(|&o| o > n && o < v)
            });
            // Passive: a name in (v, v+after].
            let passive_hit = name_pos.iter().any(|&n| n > v && n - v <= win.after);
            if active_hit {
                sent_active = true;
            }
            if passive_hit {
                sent_passive = true;
            }
        }
        // Passive subject of a passive construction: the name sits near the
        // sentence start and the sentence is passive.
        if is_passive && name_pos.iter().any(|&n| n <= 5) {
            sent_passive = true;
        }
        // A passive sentence is never counted as active (NARR-1 gate).
        if sent_active && !is_passive {
            active += 1;
        }
        if sent_passive {
            passive += 1;
        }
    }

    let total = active + passive;
    let score = (total > 0).then(|| active as f32 / total as f32);
    (score, active, passive)
}

/// Whitespace tokenizer → lowercased, surrounding punctuation trimmed.
fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.to_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Start indices where `parts` (a 1+-token name) matches consecutively. A
/// single-token name also matches a token sharing its first 5 chars (declension
/// stem, per RFC §7.1).
fn match_positions(toks: &[String], parts: &[String]) -> Vec<usize> {
    let mut out = Vec::new();
    if parts.is_empty() || toks.len() < parts.len() {
        return out;
    }
    if parts.len() == 1 {
        let name = &parts[0];
        let stem: String = name.chars().take(5).collect();
        let use_stem = name.chars().count() >= 5;
        for (i, t) in toks.iter().enumerate() {
            if t == name || (use_stem && t.chars().take(5).collect::<String>() == stem) {
                out.push(i);
            }
        }
        return out;
    }
    for start in 0..=toks.len() - parts.len() {
        if parts.iter().enumerate().all(|(j, p)| &toks[start + j] == p) {
            out.push(start);
        }
    }
    out
}

/// Naive sentence split on terminal punctuation — adequate for the windowed
/// heuristic (it doesn't need NARR-1's abbreviation-aware splitter).
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '.' | '!' | '?' | '\n') {
            if !cur.trim().is_empty() {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex() -> CompiledLexicon {
        CompiledLexicon::for_language(&ProseLanguage::En)
    }

    #[test]
    fn active_when_name_precedes_action_verb() {
        let av = super::super::verbs_for(&ProseLanguage::En);
        let (score, active, passive) = compute_agency(
            "Mara struck the table. Mara opened the door.",
            "Mara",
            &[],
            &ProseLanguage::En,
            &lex(),
            av,
            AgencyWindows::default(),
        );
        assert_eq!(active, 2);
        assert_eq!(passive, 0);
        assert_eq!(score, Some(1.0));
    }

    #[test]
    fn passive_when_name_follows_verb() {
        let av = super::super::verbs_for(&ProseLanguage::En);
        // "struck Mara" → Mara is the patient (after the verb).
        let (score, active, passive) = compute_agency(
            "The guard struck Mara hard.",
            "Mara",
            &[],
            &ProseLanguage::En,
            &lex(),
            av,
            AgencyWindows::default(),
        );
        assert_eq!(active, 0);
        assert_eq!(passive, 1);
        assert_eq!(score, Some(0.0));
    }

    #[test]
    fn intervening_other_name_blocks_active() {
        let av = super::super::verbs_for(&ProseLanguage::En);
        // "Mara watched as Aldric struck" — Aldric sits between Mara and struck.
        let (_s, active, _p) = compute_agency(
            "Mara saw Aldric struck the wall.",
            "Mara",
            &["Aldric".into()],
            &ProseLanguage::En,
            &lex(),
            av,
            AgencyWindows::default(),
        );
        assert_eq!(active, 0); // Aldric intervenes
    }

    #[test]
    fn null_score_when_no_signal() {
        let av = super::super::verbs_for(&ProseLanguage::En);
        let (score, active, passive) = compute_agency(
            "Mara was in the room. The light was dim.",
            "Mara",
            &[],
            &ProseLanguage::En,
            &lex(),
            av,
            AgencyWindows::default(),
        );
        // No action verb adjacent; "was in" is not transitive action.
        assert_eq!((active, passive), (0, 0));
        assert_eq!(score, None);
    }

    #[test]
    fn passive_construction_counts_subject_as_passive() {
        let av = super::super::verbs_for(&ProseLanguage::En);
        // Passive voice with Mara as the subject near the start.
        let (_s, active, passive) = compute_agency(
            "Mara was taken by the guards.",
            "Mara",
            &[],
            &ProseLanguage::En,
            &lex(),
            av,
            AgencyWindows::default(),
        );
        assert_eq!(active, 0);
        assert!(passive >= 1);
    }
}
