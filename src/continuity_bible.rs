//! 1.2.19+ C.3 — the continuity bible.
//!
//! Turns the Characters book from a manual scaffold into
//! a living continuity checker.  An AI pass
//! (`inkhaven continuity extract`) walks the manuscript
//! and records per-character *established facts* —
//! appearance, relationships, possessions, origin — as
//! `(character, attribute, value, chapter)` rows in a
//! project sidecar.  The `continuity-drift` doctor scan
//! then flags attributes that *change across chapters*
//! ("ch.3: eyes green; ch.17: eyes brown").
//!
//! ## Multilingual
//!
//! Two tiers, as the 1.2.19 plan lays out:
//!
//!   * **Extraction (Tier 1)** — the AI pass uses a
//!     per-language prompt (`continuity-extract`), so
//!     facts come out in the manuscript's language.
//!   * **Drift comparison (Tier 2)** — values are
//!     normalised through the project's Snowball stemmer
//!     (+ a `ё`→`е` fold) before comparison, so inflected
//!     restatements of the same fact don't false-flag
//!     (ru `зелёные глаза` / `зелёными глазами` collapse).
//!
//! Embedding-based synonym-aware comparison ("green" vs
//! "emerald" → consistent) is a C.3.b refinement; C.3
//! ships the stemmer-normalised comparison, which catches
//! the genuine "green vs brown" drift while tolerating
//! inflection.
//!
//! ## This module
//!
//! Pure: the fact model, the sidecar JSON round-trip, and
//! the `detect_drift` classifier.  The AI extraction +
//! the doctor-scan wiring live in the CLI.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::parse_stemmer_language;
use rust_stemmers::Stemmer;

/// One established fact about a character, as of a
/// particular chapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterFact {
    /// Character name (as it appears in the Characters
    /// book / prose).
    pub character: String,
    /// Canonical attribute key (`eye_color`, `hometown`,
    /// `occupation`, …) — the AI is prompted to use a
    /// stable snake_case key.
    pub attribute: String,
    /// The value as established in `chapter`.
    pub value: String,
    /// Chapter title the fact was drawn from.
    pub chapter: String,
}

/// The whole bible — every extracted fact.  Serialised to
/// `<project>/.inkhaven/continuity.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContinuityBible {
    /// Inkhaven version that produced the extraction.
    #[serde(default)]
    pub version: String,
    /// Language the facts were extracted in (drives the
    /// drift comparison's stemmer).
    #[serde(default)]
    pub language: String,
    pub facts: Vec<CharacterFact>,
    /// 1.3.12 — manuscript fingerprint at extraction time; consumers flag the
    /// bible stale when it no longer matches. 0 in older sidecars.
    #[serde(default)]
    pub manuscript_fingerprint: u64,
}

impl ContinuityBible {
    /// Sidecar path for a project.
    pub fn sidecar_path(project_root: &Path) -> PathBuf {
        project_root.join(".inkhaven").join("continuity.json")
    }

    /// Load the bible from the project sidecar.  Returns
    /// an empty bible (not an error) when the sidecar
    /// doesn't exist yet.
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

    /// Persist the bible atomically to the sidecar.
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

    /// Distinct character names, sorted.
    pub fn characters(&self) -> Vec<String> {
        let mut names: Vec<String> =
            self.facts.iter().map(|f| f.character.clone()).collect();
        names.sort();
        names.dedup();
        names
    }
}

/// A flagged continuity drift: one character's attribute
/// taking different values across chapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    pub character: String,
    pub attribute: String,
    /// The conflicting `(chapter, value)` rows, in the
    /// order first seen.
    pub conflicts: Vec<(String, String)>,
}

/// Normalise a value for comparison: lowercase, fold
/// `ё`→`е`, stem each word with the language's Snowball
/// algorithm (when available), join.  This is what makes
/// inflected restatements compare equal.
fn normalise(value: &str, stemmer: &Option<Stemmer>) -> String {
    value
        .split_whitespace()
        .map(|w| {
            let trimmed = w.trim_matches(|c: char| !c.is_alphanumeric());
            crate::text::normalize_stem(trimmed, stemmer)
        })
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Detect drift: group facts by `(character, attribute)`
/// and flag any group whose normalised values disagree
/// across chapters.
///
/// `language` selects the comparison stemmer.  Pure.
pub fn detect_drift(bible: &ContinuityBible, language: &str) -> Vec<Drift> {
    let stemmer = parse_stemmer_language(language).map(Stemmer::create);

    // (character_lc, attribute_lc) → ordered (chapter,
    // value, normalised) rows.
    let mut groups: BTreeMap<(String, String), Vec<(String, String, String)>> =
        BTreeMap::new();
    for f in &bible.facts {
        let key = (
            f.character.trim().to_lowercase(),
            f.attribute.trim().to_lowercase(),
        );
        let norm = normalise(&f.value, &stemmer);
        groups.entry(key).or_default().push((
            f.chapter.clone(),
            f.value.clone(),
            norm,
        ));
    }

    let mut drifts = Vec::new();
    for ((_char_lc, _attr_lc), rows) in groups {
        // Distinct normalised values?
        let distinct: std::collections::HashSet<&String> =
            rows.iter().map(|(_, _, n)| n).collect();
        if distinct.len() < 2 {
            continue;
        }
        // Emit the conflict, keeping one (chapter, value)
        // per distinct normalised value (first seen).
        let mut seen: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut conflicts: Vec<(String, String)> = Vec::new();
        // Recover the original-case character + attribute
        // from the first row's fact.
        for (chapter, value, norm) in &rows {
            if seen.insert(norm.clone()) {
                conflicts.push((chapter.clone(), value.clone()));
            }
        }
        // Find the original-case names from the bible.
        let (character, attribute) = bible
            .facts
            .iter()
            .find(|f| {
                f.character.trim().to_lowercase() == _char_lc
                    && f.attribute.trim().to_lowercase() == _attr_lc
            })
            .map(|f| (f.character.clone(), f.attribute.clone()))
            .unwrap_or((_char_lc.clone(), _attr_lc.clone()));
        drifts.push(Drift {
            character,
            attribute,
            conflicts,
        });
    }

    // Deterministic order.
    drifts.sort_by(|a, b| {
        a.character
            .cmp(&b.character)
            .then_with(|| a.attribute.cmp(&b.attribute))
    });
    drifts
}

/// Parse the AI extraction response — one fact per line in
/// the pipe-delimited form `character | attribute | value`.
/// Tolerant: blank lines + malformed lines are skipped;
/// surrounding markdown / preamble is ignored.  Pure, so
/// the (non-deterministic) live call is the only untested
/// boundary.
pub fn parse_extraction(raw: &str, chapter: &str) -> Vec<CharacterFact> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim().trim_start_matches(['-', '*', '•']).trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '|').map(str::trim).collect();
        if parts.len() != 3 {
            continue;
        }
        let (character, attribute, value) = (parts[0], parts[1], parts[2]);
        if character.is_empty() || attribute.is_empty() || value.is_empty() {
            continue;
        }
        // Skip a header row the model might echo.
        if character.eq_ignore_ascii_case("character")
            && attribute.eq_ignore_ascii_case("attribute")
        {
            continue;
        }
        out.push(CharacterFact {
            character: character.to_string(),
            attribute: attribute.to_string().to_lowercase().replace(' ', "_"),
            value: value.to_string(),
            chapter: chapter.to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(c: &str, a: &str, v: &str, ch: &str) -> CharacterFact {
        CharacterFact {
            character: c.into(),
            attribute: a.into(),
            value: v.into(),
            chapter: ch.into(),
        }
    }

    // ── drift detection ───────────────────────────────

    #[test]
    fn flags_changed_attribute_across_chapters() {
        let bible = ContinuityBible {
            facts: vec![
                fact("Helena", "eye_color", "green", "Ch1"),
                fact("Helena", "eye_color", "brown", "Ch9"),
            ],
            ..Default::default()
        };
        let d = detect_drift(&bible, "english");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].character, "Helena");
        assert_eq!(d[0].attribute, "eye_color");
        assert_eq!(d[0].conflicts.len(), 2);
    }

    #[test]
    fn consistent_attribute_not_flagged() {
        let bible = ContinuityBible {
            facts: vec![
                fact("Helena", "eye_color", "green", "Ch1"),
                fact("Helena", "eye_color", "green", "Ch9"),
            ],
            ..Default::default()
        };
        assert!(detect_drift(&bible, "english").is_empty());
    }

    #[test]
    fn inflected_value_not_flagged_via_stemmer() {
        // "green eyes" vs "green eye" — same stem; the
        // plural/singular inflection must not flag.
        let bible = ContinuityBible {
            facts: vec![
                fact("Helena", "eyes", "green eyes", "Ch1"),
                fact("Helena", "eyes", "green eye", "Ch9"),
            ],
            ..Default::default()
        };
        assert!(
            detect_drift(&bible, "english").is_empty(),
            "stemmer should collapse eyes/eye",
        );
    }

    #[test]
    fn russian_inflected_value_not_flagged() {
        // "зелёные" vs "зелёными" — same stem after the
        // ё-fold; must not flag.
        let bible = ContinuityBible {
            facts: vec![
                fact("Елена", "глаза", "зелёные", "Гл1"),
                fact("Елена", "глаза", "зелёными", "Гл9"),
            ],
            ..Default::default()
        };
        assert!(
            detect_drift(&bible, "russian").is_empty(),
            "Russian stemmer + ё-fold should collapse the inflections",
        );
    }

    #[test]
    fn russian_genuine_change_flagged() {
        // зелёные (green) vs карие (brown) → real drift.
        let bible = ContinuityBible {
            facts: vec![
                fact("Елена", "глаза", "зелёные", "Гл1"),
                fact("Елена", "глаза", "карие", "Гл9"),
            ],
            ..Default::default()
        };
        let d = detect_drift(&bible, "russian");
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn different_attributes_isolated() {
        let bible = ContinuityBible {
            facts: vec![
                fact("Helena", "eye_color", "green", "Ch1"),
                fact("Helena", "eye_color", "brown", "Ch9"),
                fact("Helena", "hometown", "Harbor", "Ch1"),
                fact("Helena", "hometown", "Harbor", "Ch9"),
            ],
            ..Default::default()
        };
        let d = detect_drift(&bible, "english");
        // Only eye_color drifts; hometown is consistent.
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].attribute, "eye_color");
    }

    #[test]
    fn different_characters_isolated() {
        let bible = ContinuityBible {
            facts: vec![
                fact("Helena", "eye_color", "green", "Ch1"),
                fact("Marcus", "eye_color", "brown", "Ch1"),
            ],
            ..Default::default()
        };
        // Different characters — not a contradiction.
        assert!(detect_drift(&bible, "english").is_empty());
    }

    #[test]
    fn case_insensitive_grouping() {
        let bible = ContinuityBible {
            facts: vec![
                fact("helena", "Eye_Color", "green", "Ch1"),
                fact("Helena", "eye_color", "brown", "Ch9"),
            ],
            ..Default::default()
        };
        // Same character + attribute despite case → drift.
        assert_eq!(detect_drift(&bible, "english").len(), 1);
    }

    // ── sidecar round-trip ────────────────────────────

    #[test]
    fn sidecar_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let bible = ContinuityBible {
            version: "1.2.19".into(),
            language: "english".into(),
            facts: vec![fact("Helena", "eye_color", "green", "Ch1")],
            manuscript_fingerprint: 0,
        };
        bible.save(tmp.path()).unwrap();
        let loaded = ContinuityBible::load(tmp.path()).unwrap();
        assert_eq!(loaded.facts, bible.facts);
        assert_eq!(loaded.language, "english");
    }

    #[test]
    fn load_missing_sidecar_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let b = ContinuityBible::load(tmp.path()).unwrap();
        assert!(b.facts.is_empty());
    }

    #[test]
    fn characters_sorted_and_deduped() {
        let bible = ContinuityBible {
            facts: vec![
                fact("Marcus", "a", "x", "Ch1"),
                fact("Helena", "a", "x", "Ch1"),
                fact("Helena", "b", "y", "Ch2"),
            ],
            ..Default::default()
        };
        assert_eq!(bible.characters(), vec!["Helena", "Marcus"]);
    }

    // ── extraction parsing ────────────────────────────

    #[test]
    fn parses_pipe_delimited_facts() {
        let raw = "Helena | eye color | green\n\
                   Helena | hometown | the Harbor\n\
                   Marcus | occupation | ledger-keeper";
        let f = parse_extraction(raw, "Chapter 1");
        assert_eq!(f.len(), 3);
        assert_eq!(f[0].character, "Helena");
        // "eye color" → snake_case key.
        assert_eq!(f[0].attribute, "eye_color");
        assert_eq!(f[0].value, "green");
        assert_eq!(f[0].chapter, "Chapter 1");
    }

    #[test]
    fn extraction_skips_malformed_and_preamble() {
        let raw = "Here are the facts I found:\n\
                   \n\
                   - Helena | eye color | green\n\
                   this line has no pipes\n\
                   character | attribute | value\n\
                   Marcus | mood |\n\
                   * Marcus | weapon | a lacquered box";
        let f = parse_extraction(raw, "Ch1");
        // Only the two well-formed rows (header + empty-value skipped).
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].character, "Helena");
        assert_eq!(f[1].character, "Marcus");
        assert_eq!(f[1].attribute, "weapon");
    }

    #[test]
    fn extraction_strips_list_markers() {
        let raw = "• Helena | hair | dark";
        let f = parse_extraction(raw, "Ch1");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].character, "Helena");
        assert_eq!(f[0].value, "dark");
    }
}
