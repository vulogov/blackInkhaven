//! 1.2.19+ C.4 — unresolved-tension detector.
//!
//! Catches the revision-stage problem of a tension /
//! question / goal that's *introduced* but never *paid
//! off* — the gun on the mantel that never fires, the
//! letter whose contents are never revealed.
//!
//! ## Two-step, like the continuity bible (C.3)
//!
//! An AI pass (`inkhaven tension scan`) tags each chapter
//! with the tensions it *introduces* and *resolves* (a
//! short topic per tension), writing a project sidecar.
//! The `unresolved-tension` doctor scan then matches
//! introductions to downstream resolutions and flags the
//! ones with no payoff.  Keeping the AI in a separate
//! explicit pass keeps the doctor scan fast +
//! deterministic + unit-testable.
//!
//! ## Opt-in, on purpose
//!
//! Tension is a judgment call — an "unresolved" thread
//! may be deliberate (a series hook, ambiguity by
//! design), and the AI tagging is itself approximate.
//! So the scan is **excluded from the default
//! `doctor --scan`** and only runs on an explicit
//! `--class unresolved-tension`, with the calibration
//! caveat documented.  This matches the 1.2.19 plan.
//!
//! ## Multilingual
//!
//! Tier 1 for the AI tagging (per-language prompt) +
//! Tier 2 for the matcher: topics are reduced to
//! significant stems through the project's Snowball
//! stemmer (+ `ё`→`е` fold, minus stop-words), so a
//! resolution phrased differently from its introduction
//! still matches.  The matcher is *generous* about
//! counting a tension resolved (any shared significant
//! stem in a later-or-same chapter) — biasing toward
//! fewer false "unresolved" flags.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{built_in_stop_words, parse_stemmer_language};
use rust_stemmers::Stemmer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TensionKind {
    /// A tension / question / goal is raised.
    Introduce,
    /// A previously-raised tension is paid off.
    Resolve,
}

/// One tension tag for a chapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TensionTag {
    pub kind: TensionKind,
    /// Short free-text topic ("the missing letter",
    /// "Helena's secret").
    pub topic: String,
    pub chapter: String,
    /// Reading-order index of the chapter (0-based).  A
    /// resolution counts only when its chapter index is
    /// `>=` the introduction's — you can't pay off a
    /// tension before raising it.
    pub chapter_index: usize,
}

/// The whole ledger, serialised to
/// `<project>/.inkhaven/tensions.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TensionLedger {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub language: String,
    pub tags: Vec<TensionTag>,
}

impl TensionLedger {
    pub fn sidecar_path(project_root: &Path) -> PathBuf {
        project_root.join(".inkhaven").join("tensions.json")
    }

    pub fn load(project_root: &Path) -> std::io::Result<Self> {
        let path = Self::sidecar_path(project_root);
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::default())
            }
            Err(e) => Err(e),
        }
    }

    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = Self::sidecar_path(project_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(self).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        crate::io_atomic::write(&path, &body)
    }
}

/// A flagged unresolved tension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    pub topic: String,
    pub chapter: String,
}

/// Reduce a topic to its significant stems (lowercase,
/// `ё`-fold, stem, drop stop-words + short tokens).
fn significant_stems(
    topic: &str,
    stemmer: &Option<Stemmer>,
    stop: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    topic
        .split_whitespace()
        .filter_map(|w| {
            let lc = w
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
                .replace('ё', "е");
            if lc.chars().count() < 3 {
                return None;
            }
            let stem = match stemmer {
                Some(s) => s.stem(&lc).into_owned(),
                None => lc,
            };
            if stem.is_empty() || stop.contains(&stem) {
                None
            } else {
                Some(stem)
            }
        })
        .collect()
}

/// Detect introduced tensions with no downstream
/// resolution.  `language` selects the matcher's stemmer
/// + stop-words.  Pure.
pub fn detect_unresolved(ledger: &TensionLedger, language: &str) -> Vec<Unresolved> {
    let stemmer = parse_stemmer_language(language).map(Stemmer::create);
    let stop: std::collections::HashSet<String> = built_in_stop_words(language)
        .iter()
        .map(|w| {
            let lc = w.to_lowercase().replace('ё', "е");
            match &stemmer {
                Some(s) => s.stem(&lc).into_owned(),
                None => lc,
            }
        })
        .collect();

    // Pre-stem resolutions with their chapter index.
    let resolves: Vec<(usize, std::collections::HashSet<String>)> = ledger
        .tags
        .iter()
        .filter(|t| t.kind == TensionKind::Resolve)
        .map(|t| (t.chapter_index, significant_stems(&t.topic, &stemmer, &stop)))
        .collect();

    let mut out = Vec::new();
    for intro in ledger.tags.iter().filter(|t| t.kind == TensionKind::Introduce) {
        let intro_stems = significant_stems(&intro.topic, &stemmer, &stop);
        if intro_stems.is_empty() {
            // Nothing to match on — skip rather than
            // flag (avoid noise from vacuous topics).
            continue;
        }
        // Resolved iff some resolution at chapter_index
        // >= this one shares a significant stem.  Generous
        // → conservative flagging.
        let resolved = resolves.iter().any(|(r_idx, r_stems)| {
            *r_idx >= intro.chapter_index && !r_stems.is_disjoint(&intro_stems)
        });
        if !resolved {
            out.push(Unresolved {
                topic: intro.topic.clone(),
                chapter: intro.chapter.clone(),
            });
        }
    }

    out.sort_by(|a, b| a.chapter.cmp(&b.chapter).then(a.topic.cmp(&b.topic)));
    out
}

/// Parse the AI tagging response for one chapter: one tag
/// per line, `introduce | topic` or `resolve | topic`.
/// Tolerant of list markers, preamble, and blank lines.
pub fn parse_tension_lines(
    raw: &str,
    chapter: &str,
    chapter_index: usize,
) -> Vec<TensionTag> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim().trim_start_matches(['-', '*', '•']).trim();
        if line.is_empty() {
            continue;
        }
        let Some((kind_s, topic)) = line.split_once('|') else {
            continue;
        };
        let kind = match kind_s.trim().to_ascii_lowercase().as_str() {
            "introduce" | "introduces" | "introduced" => TensionKind::Introduce,
            "resolve" | "resolves" | "resolved" => TensionKind::Resolve,
            _ => continue,
        };
        let topic = topic.trim();
        if topic.is_empty() {
            continue;
        }
        out.push(TensionTag {
            kind,
            topic: topic.to_string(),
            chapter: chapter.to_string(),
            chapter_index,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(k: TensionKind, topic: &str, ch: &str, idx: usize) -> TensionTag {
        TensionTag {
            kind: k,
            topic: topic.into(),
            chapter: ch.into(),
            chapter_index: idx,
        }
    }

    use TensionKind::{Introduce, Resolve};

    // ── matcher ───────────────────────────────────────

    #[test]
    fn resolved_downstream_not_flagged() {
        let l = TensionLedger {
            tags: vec![
                tag(Introduce, "the missing letter", "Ch1", 0),
                tag(Resolve, "found the letter at last", "Ch5", 4),
            ],
            ..Default::default()
        };
        // "letter" stem shared, resolve is downstream.
        assert!(detect_unresolved(&l, "english").is_empty());
    }

    #[test]
    fn introduced_without_resolution_flagged() {
        let l = TensionLedger {
            tags: vec![tag(Introduce, "the missing letter", "Ch1", 0)],
            ..Default::default()
        };
        let u = detect_unresolved(&l, "english");
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].chapter, "Ch1");
    }

    #[test]
    fn resolution_before_introduction_does_not_count() {
        let l = TensionLedger {
            tags: vec![
                tag(Resolve, "the letter was burned", "Ch1", 0),
                tag(Introduce, "a new missing letter", "Ch5", 4),
            ],
            ..Default::default()
        };
        // The only "letter" resolve is in an EARLIER
        // chapter → the Ch5 introduction is unresolved.
        let u = detect_unresolved(&l, "english");
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].chapter, "Ch5");
    }

    #[test]
    fn unrelated_resolution_does_not_count() {
        let l = TensionLedger {
            tags: vec![
                tag(Introduce, "the missing letter", "Ch1", 0),
                tag(Resolve, "the duel was won", "Ch3", 2),
            ],
            ..Default::default()
        };
        // No shared stem → letter tension still open.
        assert_eq!(detect_unresolved(&l, "english").len(), 1);
    }

    #[test]
    fn same_chapter_resolution_counts() {
        let l = TensionLedger {
            tags: vec![
                tag(Introduce, "the locked door", "Ch4", 3),
                tag(Resolve, "the door finally opened", "Ch4", 3),
            ],
            ..Default::default()
        };
        // Introduced + resolved in the same chapter → not
        // flagged (chapter_index >= holds).
        assert!(detect_unresolved(&l, "english").is_empty());
    }

    #[test]
    fn inflected_topic_matches_via_stemmer() {
        let l = TensionLedger {
            tags: vec![
                tag(Introduce, "the secret betrayal", "Ch1", 0),
                tag(Resolve, "the betrayals were exposed", "Ch9", 8),
            ],
            ..Default::default()
        };
        // betrayal / betrayals → same stem → resolved.
        assert!(detect_unresolved(&l, "english").is_empty());
    }

    #[test]
    fn russian_topic_matches_via_stemmer() {
        let l = TensionLedger {
            tags: vec![
                tag(Introduce, "тайна корабля", "Гл1", 0),
                tag(Resolve, "тайна корабля раскрыта", "Гл9", 8),
            ],
            ..Default::default()
        };
        assert!(
            detect_unresolved(&l, "russian").is_empty(),
            "shared Russian stems should mark it resolved",
        );
    }

    #[test]
    fn multiple_introductions_mixed() {
        let l = TensionLedger {
            tags: vec![
                tag(Introduce, "the missing letter", "Ch1", 0),
                tag(Introduce, "the hidden treasure", "Ch1", 0),
                tag(Resolve, "the letter was found", "Ch5", 4),
            ],
            ..Default::default()
        };
        // letter resolved; treasure unresolved.
        let u = detect_unresolved(&l, "english");
        assert_eq!(u.len(), 1);
        assert!(u[0].topic.contains("treasure"));
    }

    #[test]
    fn vacuous_topic_skipped() {
        // A topic with only stop-words / short tokens has
        // no significant stems → skip, don't flag.
        let l = TensionLedger {
            tags: vec![tag(Introduce, "it is", "Ch1", 0)],
            ..Default::default()
        };
        assert!(detect_unresolved(&l, "english").is_empty());
    }

    // ── sidecar round-trip ────────────────────────────

    #[test]
    fn ledger_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let l = TensionLedger {
            version: "1.2.19".into(),
            language: "english".into(),
            tags: vec![tag(Introduce, "the missing letter", "Ch1", 0)],
        };
        l.save(tmp.path()).unwrap();
        let loaded = TensionLedger::load(tmp.path()).unwrap();
        assert_eq!(loaded.tags, l.tags);
        assert_eq!(loaded.language, "english");
    }

    #[test]
    fn load_missing_ledger_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(TensionLedger::load(tmp.path()).unwrap().tags.is_empty());
    }

    // ── parsing ───────────────────────────────────────

    #[test]
    fn parses_kind_and_topic() {
        let raw = "introduce | the missing letter\n\
                   resolve | Helena's secret\n\
                   - introduces | a strange noise";
        let t = parse_tension_lines(raw, "Ch1", 0);
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].kind, TensionKind::Introduce);
        assert_eq!(t[0].topic, "the missing letter");
        assert_eq!(t[1].kind, TensionKind::Resolve);
        assert_eq!(t[2].kind, TensionKind::Introduce); // "introduces"
        assert_eq!(t[0].chapter_index, 0);
    }

    #[test]
    fn parse_skips_malformed_and_unknown_kind() {
        let raw = "Here are the tensions:\n\
                   \n\
                   maybe | the letter\n\
                   introduce | the duel\n\
                   resolve |\n\
                   no pipe here";
        let t = parse_tension_lines(raw, "Ch1", 0);
        // Only "introduce | the duel" is valid.
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].topic, "the duel");
    }
}
