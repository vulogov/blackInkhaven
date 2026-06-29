//! CHAR-1 (C-P5) — arc checks. **Stall detection** is deterministic (no LLM):
//! the longest run of `changed = false` after the baseline chapter. The four
//! **completeness checks** (start / midpoint / end alignment, arc earned) are
//! LLM, fired only for characters with a declared arc, with arc-type-specific
//! framing for the earned check.

use anyhow::Result;

use crate::config::Config;

use super::llm::{char_llm_call, extract_json_object};
use super::store::CharStore;
use super::{
    ArcCheckType, ArcDeclaration, ArcVerdict, CharacterArcCheck, CharacterState,
};

/// Detect the longest stall — a run of `changed = false` chapters after the
/// first (baseline) appearance. Returns a `StallLocation` check when the run
/// reaches `threshold`. Deterministic. Chapters where the character does not
/// appear are simply absent from `states`, so they never join a run.
pub(crate) fn detect_stall(
    name: &str,
    states: &[CharacterState],
    threshold: u32,
) -> Option<CharacterArcCheck> {
    if states.len() < 2 {
        return None;
    }
    let mut best: Option<(u32, u32, usize, usize)> = None; // (start_ch, end_ch, len, run_start_idx)
    let mut run_start: Option<usize> = None;
    for i in 1..states.len() {
        if !states[i].changed {
            run_start.get_or_insert(i);
            let start_idx = run_start.unwrap();
            let len = i - start_idx + 1;
            if best.is_none_or(|(_, _, bl, _)| len > bl) {
                best = Some((states[start_idx].chapter_ord, states[i].chapter_ord, len, start_idx));
            }
        } else {
            run_start = None;
        }
    }
    let (start_ch, end_ch, len, run_start_idx) = best?;
    if (len as u32) < threshold {
        return None;
    }
    // Average agency across the stall run.
    let scores: Vec<f32> = states[run_start_idx..run_start_idx + len]
        .iter()
        .filter_map(|s| s.agency_score)
        .collect();
    let avg = if scores.is_empty() {
        String::from("n/a")
    } else {
        format!("{:.2}", scores.iter().sum::<f32>() / scores.len() as f32)
    };
    Some(CharacterArcCheck {
        character_name: name.to_string(),
        check_type: ArcCheckType::StallLocation,
        verdict: ArcVerdict::Stalled,
        description: format!(
            "Character {name} shows no observable state change in chapters {start_ch}-{end_ch} \
             ({len} chapters). Agency score in this range averaged {avg}."
        ),
        chapter_ord: Some(start_ch),
    })
}

const CHECK_SYSTEM: &str = "You are evaluating whether a fiction manuscript delivers a character \
arc the author declared. Judge ONLY observable evidence — behaviour, speech, decisions. Ignore \
motivations not shown in the text and do not use training-data knowledge of the character or story. \
Return ONLY JSON: {\"verdict\":\"aligned|gap|earned\",\"description\":\"2-3 sentences\"}";

/// Build an alignment-check user prompt (start / midpoint / end).
pub(super) fn build_alignment_prompt(declared_state: &str, summaries: &[String]) -> String {
    let chain = summaries
        .iter()
        .enumerate()
        .map(|(i, s)| format!("[{}] {s}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "The author declared this state:\n{declared_state}\n\nThe character's state summaries here:\n{chain}\n\n\
         Does the prose establish the declared state? Look for observable evidence. \
         Verdict \"aligned\" or \"gap\"."
    )
}

/// Build the arc-earned user prompt with arc-type-specific framing + the state
/// chain + any cross-system enrichment.
pub(super) fn build_earned_prompt(
    decl: &ArcDeclaration,
    states: &[CharacterState],
) -> String {
    let chain = states
        .iter()
        .map(|s| {
            let mark = if s.changed { "✦" } else { "·" };
            format!("ch.{} {mark} {}", s.chapter_ord, s.state_summary)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let enrich = enrichment_context(states);
    format!(
        "Declared arc type: {}\nDeclared final state:\n{}\n\nState chain across the chapters they appear in:\n{chain}\n{enrich}\n\
         For a {} arc to feel earned, {}. Was the movement toward the final state prepared across the arc, \
         or does it arrive without prior grounding? Verdict \"earned\" or \"gap\".",
        decl.arc_type.as_code(),
        decl.desired_state_end,
        decl.arc_type.as_code(),
        decl.arc_type.earned_framing(),
    )
}

/// A short supporting-signals block from the cached enrichment (§10.3). Empty
/// when no signals are present.
fn enrichment_context(states: &[CharacterState]) -> String {
    let total_utt: u32 = states.iter().filter_map(|s| s.utterance_count).sum();
    let silent: Vec<u32> = states
        .iter()
        .filter(|s| s.utterance_count == Some(0))
        .map(|s| s.chapter_ord)
        .collect();
    let hedge = states.iter().find_map(|s| s.chapter_hedge_density);
    let inter_first = states.first().and_then(|s| s.chapter_interiority_ratio);
    let inter_last = states.last().and_then(|s| s.chapter_interiority_ratio);
    let mut lines = Vec::new();
    if total_utt > 0 {
        let mut l = format!("- The character spoke {total_utt} time(s) across these chapters");
        if let Some(h) = hedge {
            l.push_str(&format!("; book hedge density {h:.3}"));
        }
        if !silent.is_empty() {
            l.push_str(&format!("; silent in chapters {silent:?}"));
        }
        lines.push(l);
    }
    if let (Some(a), Some(b)) = (inter_first, inter_last) {
        lines.push(format!(
            "- Interiority ratio moved from {a:.2} to {b:.2} across the arc"
        ));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("\nAdditional signals:\n{}", lines.join("\n"))
    }
}

/// Parse an LLM check verdict. Tolerant; unparseable → `Aligned` (no false
/// problem).
pub(super) fn parse_check(raw: &str) -> (ArcVerdict, String) {
    let json = extract_json_object(raw);
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
    let verdict = v
        .get("verdict")
        .and_then(|x| x.as_str())
        .map(ArcVerdict::from_code)
        .unwrap_or(ArcVerdict::Aligned);
    let description = v
        .get("description")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    (verdict, description)
}

/// Run all arc checks for one character with a declaration: stall (deterministic)
/// + start/midpoint/end/earned (LLM). Skips characters with fewer than
/// `min_chapters` appearances (but still records a stall). Clears prior checks.
pub(crate) fn run_arc_checks(
    store: &CharStore,
    cfg: &Config,
    book_slug: &str,
    decl: &ArcDeclaration,
    states: &[CharacterState],
    stall_threshold: u32,
    min_chapters: usize,
) -> Result<Vec<CharacterArcCheck>> {
    let name = &decl.character_name;
    let now = chrono::Utc::now().to_rfc3339();
    store.clear_checks(book_slug, name)?;
    let mut out = Vec::new();

    if let Some(stall) = detect_stall(name, states, stall_threshold) {
        store.upsert_check(book_slug, &stall, &now)?;
        out.push(stall);
    }
    if states.len() < min_chapters {
        return Ok(out);
    }

    let summaries = |slice: &[CharacterState]| slice.iter().map(|s| s.state_summary.clone()).collect::<Vec<_>>();
    let first3: Vec<CharacterState> = states.iter().take(3).cloned().collect();
    let last3: Vec<CharacterState> = states.iter().rev().take(3).rev().cloned().collect();

    // Start alignment.
    let (v, d) = parse_check(&char_llm_call(
        cfg,
        CHECK_SYSTEM,
        &build_alignment_prompt(&decl.desired_state_start, &summaries(&first3)),
    )?);
    out.push(record(store, book_slug, name, ArcCheckType::StartAlignment, v, d, &now)?);

    // Midpoint (only if declared).
    if let Some(mid) = &decl.desired_midpoint_state {
        let m = states.len() / 2;
        let around: Vec<CharacterState> =
            states[m.saturating_sub(1)..(m + 2).min(states.len())].to_vec();
        let (v, d) = parse_check(&char_llm_call(
            cfg,
            CHECK_SYSTEM,
            &build_alignment_prompt(mid, &summaries(&around)),
        )?);
        out.push(record(store, book_slug, name, ArcCheckType::MidpointAlignment, v, d, &now)?);
    }

    // End alignment.
    let (v, d) = parse_check(&char_llm_call(
        cfg,
        CHECK_SYSTEM,
        &build_alignment_prompt(&decl.desired_state_end, &summaries(&last3)),
    )?);
    out.push(record(store, book_slug, name, ArcCheckType::EndAlignment, v, d, &now)?);

    // Arc earned.
    let (v, d) = parse_check(&char_llm_call(cfg, CHECK_SYSTEM, &build_earned_prompt(decl, states))?);
    out.push(record(store, book_slug, name, ArcCheckType::ArcEarned, v, d, &now)?);

    Ok(out)
}

fn record(
    store: &CharStore,
    book_slug: &str,
    name: &str,
    check_type: ArcCheckType,
    verdict: ArcVerdict,
    description: String,
    now: &str,
) -> Result<CharacterArcCheck> {
    let c = CharacterArcCheck {
        character_name: name.to_string(),
        check_type,
        verdict,
        description,
        chapter_ord: None,
    };
    store.upsert_check(book_slug, &c, now)?;
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(ch: u32, changed: bool, agency: Option<f32>) -> CharacterState {
        CharacterState {
            character_name: "Mara".into(),
            chapter_ord: ch,
            state_summary: format!("s{ch}"),
            changed,
            change_description: None,
            agency_score: agency,
            active_count: 0,
            passive_count: 0,
            utterance_count: Some(0),
            chapter_hedge_density: None,
            chapter_interiority_ratio: None,
        }
    }

    #[test]
    fn stall_at_and_under_threshold() {
        // ch1 baseline, ch2-5 unchanged → run of 4.
        let states = vec![
            st(1, false, Some(0.71)),
            st(2, false, Some(0.68)),
            st(3, false, Some(0.62)),
            st(4, false, Some(0.58)),
            st(5, false, Some(0.51)),
        ];
        let s = detect_stall("Mara", &states, 4).unwrap();
        assert_eq!(s.verdict, ArcVerdict::Stalled);
        assert_eq!(s.chapter_ord, Some(2)); // run starts after baseline
        assert!(s.description.contains("chapters 2-5"));
        assert!(s.description.contains("averaged 0.6")); // avg of 0.68..0.51
        // Threshold 5 → not flagged (run is 4).
        assert!(detect_stall("Mara", &states, 5).is_none());
    }

    #[test]
    fn change_breaks_the_run() {
        let states = vec![
            st(1, false, None),
            st(2, false, None),
            st(3, true, None), // breaks
            st(4, false, None),
            st(5, false, None),
        ];
        // Longest run is 2 (ch.1-2 or ch.4-5) → under threshold 4.
        assert!(detect_stall("Mara", &states, 4).is_none());
    }

    #[test]
    fn parse_check_tolerant() {
        assert_eq!(
            parse_check("{\"verdict\":\"gap\",\"description\":\"no build\"}"),
            (ArcVerdict::Gap, "no build".to_string())
        );
        assert_eq!(parse_check("garbage").0, ArcVerdict::Aligned); // safe default
        assert_eq!(parse_check("ok {\"verdict\":\"earned\",\"description\":\"\"}").0, ArcVerdict::Earned);
    }

    #[test]
    fn earned_prompt_injects_arc_framing_and_chain() {
        let decl = ArcDeclaration {
            character_name: "Mara".into(),
            arc_type: super::super::ArcType::Corruption,
            desired_state_start: "near truth".into(),
            desired_midpoint_state: None,
            desired_state_end: "fully compromised".into(),
        };
        let states = vec![st(1, false, None), st(2, true, None)];
        let p = build_earned_prompt(&decl, &states);
        assert!(p.contains("corruption"));
        assert!(p.contains("compromise")); // arc-type framing
        assert!(p.contains("fully compromised"));
        assert!(p.contains("ch.2 ✦"));
    }
}
