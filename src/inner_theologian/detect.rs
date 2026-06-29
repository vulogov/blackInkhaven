//! INNER-THEOLOGIAN-1 (IT-P3) — the fast-track ethical-signal detector.
//! Deterministic, no LLM. Three signals over a chapter's ordered paragraphs:
//!
//! - **Moral invisibility** — violence + ≥2 distinct named characters in a
//!   paragraph, with no consequence/acknowledgment in the next N paragraphs.
//! - **Consequence gap** — lethal/severe violence with no consequence in the
//!   next M paragraphs (the broader, un-named / mass-harm case). To avoid
//!   double-flagging, it fires only where moral-invisibility did not.
//! - **Sacred levity** — a sacred/ritual term sharing a paragraph with a comic
//!   or dismissive marker (the most cautious detector; configurable off).
//!
//! All findings are `info`, advisory, and suppressible. The detector is a pure
//! function of (paragraphs, roster, language, windows) — the pipeline (IT-P3
//! wiring) supplies the manuscript walk and the Characters roster.

use crate::prose::ProseLanguage;

use super::vocab::{lists_for, scan_list};
use super::{SignalType, TheologianFinding};

/// Paragraph windows (RFC §14 defaults): how far after a harm event to look for
/// acknowledgment (moral invisibility) / consequence (consequence gap).
#[derive(Debug, Clone, Copy)]
pub(crate) struct DetectWindows {
    pub moral_invisibility: usize,
    pub consequence_gap: usize,
}

impl Default for DetectWindows {
    fn default() -> Self {
        DetectWindows { moral_invisibility: 3, consequence_gap: 5 }
    }
}

/// Detect the three signals across one chapter's ordered paragraphs.
/// `paras` is `(para_id, text)` in reading order; `roster` is the Characters
/// roster (any case — matched case-insensitively).
pub(crate) fn detect_chapter(
    chapter_ord: u32,
    paras: &[(String, String)],
    roster: &[String],
    lang: &ProseLanguage,
    win: DetectWindows,
    sacred_levity_enabled: bool,
) -> Vec<TheologianFinding> {
    let lists = lists_for(lang);
    let lc: Vec<String> = paras.iter().map(|(_, t)| t.to_lowercase()).collect();
    let roster_lc: Vec<String> = roster
        .iter()
        .map(|n| n.trim().to_lowercase())
        .filter(|n| !n.is_empty())
        .collect();

    let mut out = Vec::new();
    for (i, (pid, _)) in paras.iter().enumerate() {
        let para_lc = &lc[i];
        let has_violence = scan_list(para_lc, lists.violence).is_some();

        let mut moral_invis = false;
        if has_violence {
            let names = distinct_names(para_lc, &roster_lc);
            if names.len() >= 2
                && !consequence_in_window(&lc, i, win.moral_invisibility, lists.consequence)
            {
                out.push(finding(
                    SignalType::MoralInvisibility,
                    chapter_ord,
                    pid,
                    format!(
                        "harm between {} and {} with no visible acknowledgment in the following {} paragraphs",
                        names[0].to_uppercase(),
                        names[1].to_uppercase(),
                        win.moral_invisibility
                    ),
                ));
                moral_invis = true;
            }
        }

        // Consequence gap — the broader net; skip if moral-invisibility already
        // fired on this paragraph (it's the more specific reading of the same
        // event).
        if has_violence
            && !moral_invis
            && !consequence_in_window(&lc, i, win.consequence_gap, lists.consequence)
        {
            out.push(finding(
                SignalType::ConsequenceGap,
                chapter_ord,
                pid,
                format!(
                    "lethal or severe violence without depicted consequence in the following {} paragraphs",
                    win.consequence_gap
                ),
            ));
        }

        if sacred_levity_enabled {
            if let Some(term) = scan_list(para_lc, lists.sacred) {
                if scan_list(para_lc, lists.levity).is_some() {
                    out.push(finding(
                        SignalType::SacredLevity,
                        chapter_ord,
                        pid,
                        format!("sacred vocabulary \"{term}\" in a levity-adjacent context"),
                    ));
                }
            }
        }
    }
    out
}

fn finding(signal_type: SignalType, chapter_ord: u32, para_id: &str, description: String) -> TheologianFinding {
    TheologianFinding { signal_type, chapter_ord, para_id: para_id.to_string(), description, suppressed: false }
}

/// Distinct roster names present in a lowercased paragraph, in roster order.
/// Single-token names match on a whole-word boundary; multi-word names match as
/// a substring.
fn distinct_names(para_lc: &str, roster_lc: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in roster_lc {
        let hit = if name.contains(' ') {
            para_lc.contains(name.as_str())
        } else {
            para_lc.split(|c: char| !c.is_alphanumeric()).any(|tok| tok == name)
        };
        if hit && !out.iter().any(|n| n == name) {
            out.push(name.clone());
        }
    }
    out
}

/// Whether any consequence term appears in paragraphs `[i, i+window]` (inclusive
/// of the harm paragraph itself).
fn consequence_in_window(lc: &[String], i: usize, window: usize, list: &[&'static str]) -> bool {
    let end = (i + window).min(lc.len().saturating_sub(1));
    (i..=end).any(|j| scan_list(&lc[j], list).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paras(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn moral_invisibility_fires_for_named_harm_without_ack() {
        let p = paras(&[
            ("p1", "Mara struck Aldric and he fell, killed where he stood."),
            ("p2", "The hall was quiet."),
            ("p3", "Dawn came over the hills."),
        ]);
        let roster = vec!["Mara".into(), "Aldric".into()];
        let f = detect_chapter(5, &p, &roster, &ProseLanguage::En, DetectWindows::default(), true);
        assert!(f.iter().any(|x| x.signal_type == SignalType::MoralInvisibility && x.para_id == "p1"));
        // No double consequence-gap on the same paragraph.
        assert!(!f.iter().any(|x| x.signal_type == SignalType::ConsequenceGap && x.para_id == "p1"));
    }

    #[test]
    fn acknowledgment_in_window_clears_moral_invisibility() {
        let p = paras(&[
            ("p1", "Mara killed Aldric in the dark."),
            ("p2", "She wept, the guilt heavy on her."),
        ]);
        let roster = vec!["Mara".into(), "Aldric".into()];
        let f = detect_chapter(5, &p, &roster, &ProseLanguage::En, DetectWindows::default(), true);
        assert!(f.is_empty(), "consequence within window should clear all violence signals: {f:?}");
    }

    #[test]
    fn consequence_gap_fires_for_unnamed_mass_violence() {
        let p = paras(&[
            ("p1", "The village burned; hundreds perished in the night."),
            ("p2", "The army marched on."),
            ("p3", "They reached the river by noon."),
        ]);
        let roster: Vec<String> = vec![]; // no named victims
        let f = detect_chapter(9, &p, &roster, &ProseLanguage::En, DetectWindows::default(), true);
        assert!(f.iter().any(|x| x.signal_type == SignalType::ConsequenceGap && x.para_id == "p1"));
        assert!(!f.iter().any(|x| x.signal_type == SignalType::MoralInvisibility));
    }

    #[test]
    fn sacred_levity_fires_and_is_gateable() {
        let p = paras(&[("p1", "He chuckled at the holy water and made a joke about grace.")]);
        let roster: Vec<String> = vec![];
        let on = detect_chapter(7, &p, &roster, &ProseLanguage::En, DetectWindows::default(), true);
        assert!(on.iter().any(|x| x.signal_type == SignalType::SacredLevity));
        let off = detect_chapter(7, &p, &roster, &ProseLanguage::En, DetectWindows::default(), false);
        assert!(!off.iter().any(|x| x.signal_type == SignalType::SacredLevity));
    }

    #[test]
    fn serious_sacred_context_does_not_fire_levity() {
        let p = paras(&[("p1", "She knelt in prayer before the altar, her soul heavy.")]);
        let f = detect_chapter(7, &p, &[], &ProseLanguage::En, DetectWindows::default(), true);
        assert!(!f.iter().any(|x| x.signal_type == SignalType::SacredLevity));
    }

    #[test]
    fn window_boundary_consequence_just_outside_still_flags() {
        // window=3: consequence at p5 (i+4) is outside → moral invisibility fires.
        let p = paras(&[
            ("p1", "Mara killed Aldric."),
            ("p2", "x"),
            ("p3", "y"),
            ("p4", "z"),
            ("p5", "Only now did she feel remorse."),
        ]);
        let roster = vec!["Mara".into(), "Aldric".into()];
        let f = detect_chapter(1, &p, &roster, &ProseLanguage::En, DetectWindows::default(), false);
        assert!(f.iter().any(|x| x.signal_type == SignalType::MoralInvisibility && x.para_id == "p1"));
    }
}
