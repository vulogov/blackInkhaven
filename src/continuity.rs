//! 1.2.19+ C.2 — continuity lexicon + numeric / temporal
//! / spatial contradiction detection.
//!
//! The Tier-3 multilingual feature: extraction is
//! lexicon-bound, so this is where the genuine
//! per-language work lives.  Digits are language-agnostic
//! (`3 days`), but number-WORDS (`three` / `trois` /
//! `tres`), units (`day` / `jour` / `día`), and
//! directions (`north` / `nord` / `norte`) need a
//! per-language lexicon.
//!
//! ## What ships in C.2
//!
//! Built-in lexicons for **English, French, Spanish**
//! (English + Romance — verifiable by hand).  Russian +
//! German + the `bootstrap-continuity` LLM-seed CLI land
//! in C.2.b (Cyrillic declension + German compounding
//! want native-speaker / LLM verification).  Languages
//! without a bundled lexicon skip the scan with a clear
//! message — the graceful degradation the 1.2.19 plan
//! promised, never silent garbage.
//!
//! ## How contradiction detection stays conservative
//!
//! The whole `numeric-contradiction` doctor class is
//! Info / author-judgment / no-autofix, like the 1.2.16
//! narrative-audit classes — false positives are
//! expected, the author decides.  Two checks, both
//! deliberately narrow:
//!
//!   1. **Spatial direction reversal** — two *directed*
//!      spatial magnitudes (`200 leagues north` …
//!      `200 leagues south`) with opposite cardinal
//!      directions + equal magnitude within a window of
//!      sentences.  High-confidence error signal.
//!   2. **Proximate temporal mismatch** — two durations
//!      with different normalised values in the same (or
//!      adjacent) sentence (`the three-day journey, after
//!      a week of travel`).  Framed as "do these refer to
//!      the same span?"  Narrow window keeps the noise
//!      down.
//!
//! Directions are only extracted as part of a directed
//! magnitude — never bare — which sidesteps the
//! cross-language bare-direction ambiguity (Spanish
//! `este` = "east" AND "this") and the "a character
//! legitimately walked north then south" false positive.

use std::collections::HashMap;

/// Cardinal direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    pub fn opposite(self) -> Direction {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East => Direction::West,
            Direction::West => Direction::East,
        }
    }
}

/// The physical dimension a quantity measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dim {
    /// Value normalised to seconds.
    Temporal,
    /// Value normalised to metres.
    Spatial,
}

/// One extracted quantity, normalised to its dimension's
/// base unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Quantity {
    /// Normalised magnitude (seconds for Temporal, metres
    /// for Spatial).
    pub base_value: f64,
    pub dim: Dim,
    /// Cardinal direction, when the quantity was a
    /// directed spatial magnitude.
    pub direction: Option<Direction>,
    /// The raw text span, for the finding quote.
    pub raw: String,
    /// 0-based index of the sentence it appeared in.
    pub sentence: usize,
}

/// Per-language quantity lexicon.  Keys are lowercased.
#[derive(Debug, Clone, Default)]
pub struct ContinuityLexicon {
    /// Number-words + articles → value (`a`/`one` = 1.0).
    numbers: HashMap<String, f64>,
    /// Scale multipliers (`hundred` = 100, `thousand` =
    /// 1000).
    scales: HashMap<String, f64>,
    /// Temporal unit (singular + plural) → seconds.
    temporal: HashMap<String, f64>,
    /// Spatial unit (singular + plural) → metres.
    spatial: HashMap<String, f64>,
    /// Direction word → cardinal.
    directions: HashMap<String, Direction>,
}

// Base-unit constants.
const MINUTE: f64 = 60.0;
const HOUR: f64 = 3600.0;
const DAY: f64 = 86_400.0;
const WEEK: f64 = 604_800.0;
const MONTH: f64 = 2_592_000.0; // 30 days
const YEAR: f64 = 31_536_000.0; // 365 days

const FOOT: f64 = 0.3048;
const YARD: f64 = 0.9144;
const MILE: f64 = 1609.34;
const KM: f64 = 1000.0;
const LEAGUE: f64 = 5556.0;
const PACE: f64 = 0.762;

fn map(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// Built-in lexicon for a supported language, or `None`
/// when no lexicon ships for it (caller skips the scan
/// with a clear message).
pub fn built_in_lexicon(language: &str) -> Option<ContinuityLexicon> {
    match language.trim().to_ascii_lowercase().as_str() {
        "english" | "en" => Some(english()),
        "french" | "fr" => Some(french()),
        "spanish" | "es" => Some(spanish()),
        _ => None,
    }
}

fn english() -> ContinuityLexicon {
    ContinuityLexicon {
        numbers: map(&[
            ("a", 1.0), ("an", 1.0), ("one", 1.0),
            ("zero", 0.0), ("two", 2.0), ("three", 3.0),
            ("four", 4.0), ("five", 5.0), ("six", 6.0),
            ("seven", 7.0), ("eight", 8.0), ("nine", 9.0),
            ("ten", 10.0), ("eleven", 11.0), ("twelve", 12.0),
            ("thirteen", 13.0), ("fourteen", 14.0),
            ("fifteen", 15.0), ("sixteen", 16.0),
            ("seventeen", 17.0), ("eighteen", 18.0),
            ("nineteen", 19.0), ("twenty", 20.0),
            ("thirty", 30.0), ("forty", 40.0), ("fifty", 50.0),
            ("sixty", 60.0), ("seventy", 70.0), ("eighty", 80.0),
            ("ninety", 90.0),
        ]),
        scales: map(&[("hundred", 100.0), ("thousand", 1000.0)]),
        temporal: map(&[
            ("second", 1.0), ("seconds", 1.0),
            ("minute", MINUTE), ("minutes", MINUTE),
            ("hour", HOUR), ("hours", HOUR),
            ("day", DAY), ("days", DAY),
            ("week", WEEK), ("weeks", WEEK),
            ("month", MONTH), ("months", MONTH),
            ("year", YEAR), ("years", YEAR),
        ]),
        spatial: map(&[
            ("foot", FOOT), ("feet", FOOT),
            ("yard", YARD), ("yards", YARD),
            ("mile", MILE), ("miles", MILE),
            ("kilometer", KM), ("kilometers", KM),
            ("kilometre", KM), ("kilometres", KM),
            ("meter", 1.0), ("meters", 1.0),
            ("metre", 1.0), ("metres", 1.0),
            ("league", LEAGUE), ("leagues", LEAGUE),
            ("pace", PACE), ("paces", PACE),
        ]),
        directions: directions_en(),
    }
}

fn directions_en() -> HashMap<String, Direction> {
    [
        ("north", Direction::North), ("south", Direction::South),
        ("east", Direction::East), ("west", Direction::West),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), *v))
    .collect()
}

fn french() -> ContinuityLexicon {
    ContinuityLexicon {
        numbers: map(&[
            ("un", 1.0), ("une", 1.0),
            ("zéro", 0.0), ("deux", 2.0), ("trois", 3.0),
            ("quatre", 4.0), ("cinq", 5.0), ("six", 6.0),
            ("sept", 7.0), ("huit", 8.0), ("neuf", 9.0),
            ("dix", 10.0), ("onze", 11.0), ("douze", 12.0),
            ("treize", 13.0), ("quatorze", 14.0),
            ("quinze", 15.0), ("seize", 16.0),
            ("vingt", 20.0), ("trente", 30.0),
            ("quarante", 40.0), ("cinquante", 50.0),
            ("soixante", 60.0),
        ]),
        scales: map(&[("cent", 100.0), ("cents", 100.0), ("mille", 1000.0)]),
        temporal: map(&[
            ("seconde", 1.0), ("secondes", 1.0),
            ("minute", MINUTE), ("minutes", MINUTE),
            ("heure", HOUR), ("heures", HOUR),
            ("jour", DAY), ("jours", DAY),
            ("journée", DAY), ("journées", DAY),
            ("semaine", WEEK), ("semaines", WEEK),
            ("mois", MONTH),
            ("an", YEAR), ("ans", YEAR),
            ("année", YEAR), ("années", YEAR),
        ]),
        spatial: map(&[
            ("mètre", 1.0), ("mètres", 1.0),
            ("kilomètre", KM), ("kilomètres", KM),
            ("lieue", 4000.0), ("lieues", 4000.0),
            ("pas", PACE),
        ]),
        directions: [
            ("nord", Direction::North), ("sud", Direction::South),
            ("est", Direction::East), ("ouest", Direction::West),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect(),
    }
}

fn spanish() -> ContinuityLexicon {
    ContinuityLexicon {
        numbers: map(&[
            ("un", 1.0), ("uno", 1.0), ("una", 1.0),
            ("cero", 0.0), ("dos", 2.0), ("tres", 3.0),
            ("cuatro", 4.0), ("cinco", 5.0), ("seis", 6.0),
            ("siete", 7.0), ("ocho", 8.0), ("nueve", 9.0),
            ("diez", 10.0), ("once", 11.0), ("doce", 12.0),
            ("trece", 13.0), ("catorce", 14.0),
            ("quince", 15.0), ("veinte", 20.0),
            ("treinta", 30.0), ("cuarenta", 40.0),
            ("cincuenta", 50.0), ("sesenta", 60.0),
        ]),
        scales: map(&[
            ("cien", 100.0), ("ciento", 100.0),
            ("mil", 1000.0),
        ]),
        temporal: map(&[
            ("segundo", 1.0), ("segundos", 1.0),
            ("minuto", MINUTE), ("minutos", MINUTE),
            ("hora", HOUR), ("horas", HOUR),
            ("día", DAY), ("días", DAY),
            ("semana", WEEK), ("semanas", WEEK),
            ("mes", MONTH), ("meses", MONTH),
            ("año", YEAR), ("años", YEAR),
        ]),
        spatial: map(&[
            ("metro", 1.0), ("metros", 1.0),
            ("kilómetro", KM), ("kilómetros", KM),
            ("milla", MILE), ("millas", MILE),
            ("legua", LEAGUE), ("leguas", LEAGUE),
            ("paso", PACE), ("pasos", PACE),
        ]),
        // Spanish `este` = "east" AND "this" — only ever
        // extracted attached to a spatial magnitude, so
        // the demonstrative never triggers a finding.
        directions: [
            ("norte", Direction::North), ("sur", Direction::South),
            ("este", Direction::East), ("oeste", Direction::West),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect(),
    }
}

/// Split prose into sentences on `.!?` boundaries.
/// Coarse but adequate — the detector only needs sentence
/// *grouping* for the proximity windows, not linguistic
/// precision.
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            let trimmed = cur.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
            cur.clear();
        }
    }
    let tail = cur.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

/// Tokenise a sentence: split on whitespace, trim
/// leading/trailing punctuation, keep internal hyphens +
/// digits.  Lowercased.
fn tokenize(sentence: &str) -> Vec<String> {
    sentence
        .split_whitespace()
        .map(|t| {
            t.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|t| !t.is_empty())
        .collect()
}

/// Parse a number expression starting at `tokens[i]`.
/// Returns `(value, tokens_consumed)`.  Handles: a digit
/// run (`200`, `1,000`), an article/number-word, a
/// hyphenated compound (`twenty-five`), and a
/// `<number> <scale>` pair (`two hundred`, `a thousand`).
fn parse_number(
    tokens: &[String],
    i: usize,
    lex: &ContinuityLexicon,
) -> Option<(f64, usize)> {
    let tok = tokens.get(i)?;

    // Digit run (allow thousands separators + decimals).
    let cleaned: String = tok.chars().filter(|c| *c != ',').collect();
    let base = if cleaned.chars().all(|c| c.is_ascii_digit() || c == '.')
        && cleaned.chars().any(|c| c.is_ascii_digit())
    {
        cleaned.parse::<f64>().ok()?
    } else if let Some((a, b)) = tok.split_once('-') {
        // Hyphenated compound: tens-ones (twenty-five).
        match (lex.numbers.get(a), lex.numbers.get(b)) {
            (Some(t), Some(o)) => t + o,
            _ => *lex.numbers.get(tok)?,
        }
    } else {
        *lex.numbers.get(tok)?
    };

    // Optional trailing scale word: "two hundred".
    if let Some(next) = tokens.get(i + 1) {
        if let Some(scale) = lex.scales.get(next) {
            return Some((base * scale, 2));
        }
    }
    Some((base, 1))
}

/// Extract every quantity from `sentences`.  Pure +
/// multilingual (parametrised entirely by `lex`).
pub fn extract_quantities(
    sentences: &[String],
    lex: &ContinuityLexicon,
) -> Vec<Quantity> {
    let mut out = Vec::new();
    for (s_idx, sentence) in sentences.iter().enumerate() {
        let tokens = tokenize(sentence);
        let mut i = 0;
        while i < tokens.len() {
            let Some((value, consumed)) = parse_number(&tokens, i, lex) else {
                i += 1;
                continue;
            };
            let unit_idx = i + consumed;
            let Some(unit_tok) = tokens.get(unit_idx) else {
                i += consumed;
                continue;
            };
            if let Some(secs) = lex.temporal.get(unit_tok) {
                out.push(Quantity {
                    base_value: value * secs,
                    dim: Dim::Temporal,
                    direction: None,
                    raw: format!("{} {}", tokens[i..unit_idx].join(" "), unit_tok),
                    sentence: s_idx,
                });
                i = unit_idx + 1;
            } else if let Some(metres) = lex.spatial.get(unit_tok) {
                // Optional trailing direction word.
                let direction = tokens
                    .get(unit_idx + 1)
                    .and_then(|d| lex.directions.get(d).copied());
                let raw_end = if direction.is_some() {
                    unit_idx + 2
                } else {
                    unit_idx + 1
                };
                out.push(Quantity {
                    base_value: value * metres,
                    dim: Dim::Spatial,
                    direction,
                    raw: tokens[i..raw_end].join(" "),
                    sentence: s_idx,
                });
                i = raw_end;
            } else {
                i += consumed;
            }
        }
    }
    out
}

/// Tunables for contradiction detection.
#[derive(Debug, Clone)]
pub struct ContradictionConfig {
    /// Window (sentences) for the spatial direction-
    /// reversal check.
    pub spatial_window: usize,
    /// Window (sentences) for the proximate temporal-
    /// mismatch check.  `0` disables temporal checking.
    pub temporal_window: usize,
    /// Relative tolerance for "equal magnitude" in the
    /// spatial check (0.0 = exact).
    pub magnitude_tolerance: f64,
}

impl Default for ContradictionConfig {
    fn default() -> Self {
        Self {
            spatial_window: 3,
            temporal_window: 1,
            magnitude_tolerance: 0.0,
        }
    }
}

/// One flagged contradiction.
#[derive(Debug, Clone, PartialEq)]
pub struct Contradiction {
    pub kind: ContradictionKind,
    pub a_raw: String,
    pub b_raw: String,
    pub a_sentence: usize,
    pub b_sentence: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContradictionKind {
    /// Opposite cardinal directions, equal magnitude,
    /// within the spatial window.
    DirectionReversal,
    /// Two durations of different normalised value within
    /// the temporal window.
    TemporalMismatch,
}

/// Detect contradictions among extracted quantities.
/// Conservative by design — see the module note.
pub fn detect_contradictions(
    quantities: &[Quantity],
    cfg: &ContradictionConfig,
) -> Vec<Contradiction> {
    let mut out = Vec::new();

    // ── Rule 1: spatial direction reversal ────────────
    let directed: Vec<&Quantity> = quantities
        .iter()
        .filter(|q| q.dim == Dim::Spatial && q.direction.is_some())
        .collect();
    for a_i in 0..directed.len() {
        for b_i in (a_i + 1)..directed.len() {
            let a = directed[a_i];
            let b = directed[b_i];
            if b.sentence.saturating_sub(a.sentence) > cfg.spatial_window {
                continue;
            }
            let (Some(da), Some(db)) = (a.direction, b.direction) else {
                continue;
            };
            if db != da.opposite() {
                continue;
            }
            if !magnitudes_equal(a.base_value, b.base_value, cfg.magnitude_tolerance)
            {
                continue;
            }
            out.push(Contradiction {
                kind: ContradictionKind::DirectionReversal,
                a_raw: a.raw.clone(),
                b_raw: b.raw.clone(),
                a_sentence: a.sentence,
                b_sentence: b.sentence,
            });
        }
    }

    // ── Rule 2: proximate temporal mismatch ───────────
    if cfg.temporal_window > 0 {
        let temporal: Vec<&Quantity> =
            quantities.iter().filter(|q| q.dim == Dim::Temporal).collect();
        for a_i in 0..temporal.len() {
            for b_i in (a_i + 1)..temporal.len() {
                let a = temporal[a_i];
                let b = temporal[b_i];
                if b.sentence.saturating_sub(a.sentence) > cfg.temporal_window {
                    continue;
                }
                // Different normalised duration → possible
                // mismatch.  Equal durations (3 days vs 72
                // hours) are consistent — skip.
                if (a.base_value - b.base_value).abs() < f64::EPSILON {
                    continue;
                }
                out.push(Contradiction {
                    kind: ContradictionKind::TemporalMismatch,
                    a_raw: a.raw.clone(),
                    b_raw: b.raw.clone(),
                    a_sentence: a.sentence,
                    b_sentence: b.sentence,
                });
            }
        }
    }

    out
}

fn magnitudes_equal(a: f64, b: f64, tol: f64) -> bool {
    if tol <= 0.0 {
        (a - b).abs() < f64::EPSILON
    } else {
        let denom = a.abs().max(b.abs()).max(f64::EPSILON);
        (a - b).abs() / denom <= tol
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sents(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // ── number parsing ────────────────────────────────

    #[test]
    fn parses_digit_word_compound_and_scale() {
        let lex = english();
        let q = extract_quantities(
            &sents(&[
                "It took 200 days.",
                "It took three days.",
                "It took twenty-five days.",
                "It took two hundred days.",
                "It took a day.",
            ]),
            &lex,
        );
        let vals: Vec<f64> = q.iter().map(|x| x.base_value / DAY).collect();
        assert_eq!(vals, vec![200.0, 3.0, 25.0, 200.0, 1.0]);
    }

    // ── temporal extraction + normalisation ───────────

    #[test]
    fn temporal_normalises_to_seconds() {
        let lex = english();
        let q = extract_quantities(&sents(&["He waited a week."]), &lex);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].dim, Dim::Temporal);
        assert_eq!(q[0].base_value, WEEK);
    }

    // ── spatial + direction extraction ────────────────

    #[test]
    fn spatial_with_direction() {
        let lex = english();
        let q = extract_quantities(&sents(&["They rode 200 leagues north."]), &lex);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].dim, Dim::Spatial);
        assert_eq!(q[0].direction, Some(Direction::North));
        assert_eq!(q[0].base_value, 200.0 * LEAGUE);
    }

    // ── Rule 1: direction reversal ────────────────────

    #[test]
    fn flags_direction_reversal() {
        let lex = english();
        let q = extract_quantities(
            &sents(&[
                "They rode 200 leagues north.",
                "By dusk they had gone 200 leagues south.",
            ]),
            &lex,
        );
        let c = detect_contradictions(&q, &ContradictionConfig::default());
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, ContradictionKind::DirectionReversal);
    }

    #[test]
    fn no_reversal_when_magnitudes_differ() {
        let lex = english();
        let q = extract_quantities(
            &sents(&[
                "They rode 200 leagues north.",
                "Then 50 leagues south.",
            ]),
            &lex,
        );
        // Different magnitudes → legitimate movement, not
        // flagged.
        let c = detect_contradictions(&q, &ContradictionConfig::default());
        assert!(!c.iter().any(|x| x.kind == ContradictionKind::DirectionReversal));
    }

    #[test]
    fn no_reversal_beyond_window() {
        let lex = english();
        let mut v = vec!["They rode 200 leagues north.".to_string()];
        for i in 0..5 {
            v.push(format!("Filler sentence number {i}."));
        }
        v.push("They rode 200 leagues south.".to_string());
        let q = extract_quantities(&v, &lex);
        let c = detect_contradictions(&q, &ContradictionConfig::default());
        assert!(c.is_empty(), "reversal beyond window should not flag");
    }

    // ── Rule 2: temporal mismatch ─────────────────────

    #[test]
    fn flags_proximate_temporal_mismatch() {
        let lex = english();
        let q = extract_quantities(
            &sents(&["The three day journey ended after a week."]),
            &lex,
        );
        let c = detect_contradictions(&q, &ContradictionConfig::default());
        assert!(c.iter().any(|x| x.kind == ContradictionKind::TemporalMismatch));
    }

    #[test]
    fn equal_durations_not_flagged() {
        let lex = english();
        // "twenty-four hours" == "a day" → consistent.
        let q = extract_quantities(
            &sents(&["The twenty-four hours felt like a day."]),
            &lex,
        );
        let c = detect_contradictions(&q, &ContradictionConfig::default());
        assert!(!c.iter().any(|x| x.kind == ContradictionKind::TemporalMismatch));
    }

    #[test]
    fn temporal_window_zero_disables() {
        let lex = english();
        let q = extract_quantities(
            &sents(&["The three day journey ended after a week."]),
            &lex,
        );
        let mut cfg = ContradictionConfig::default();
        cfg.temporal_window = 0;
        let c = detect_contradictions(&q, &cfg);
        assert!(c.is_empty());
    }

    // ── multilingual ──────────────────────────────────

    #[test]
    fn french_temporal_extraction() {
        let lex = french();
        let q = extract_quantities(&sents(&["Il a attendu une semaine."]), &lex);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].base_value, WEEK);
    }

    #[test]
    fn french_direction_reversal() {
        let lex = french();
        let q = extract_quantities(
            &sents(&[
                "Ils ont parcouru 200 lieues nord.",
                "Puis 200 lieues sud.",
            ]),
            &lex,
        );
        let c = detect_contradictions(&q, &ContradictionConfig::default());
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, ContradictionKind::DirectionReversal);
    }

    #[test]
    fn spanish_temporal_extraction() {
        let lex = spanish();
        let q = extract_quantities(&sents(&["Esperó tres días."]), &lex);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].base_value, 3.0 * DAY);
    }

    #[test]
    fn spanish_este_demonstrative_not_a_finding() {
        let lex = spanish();
        // "este" appears as a demonstrative ("this man"),
        // not attached to a magnitude → no spatial
        // quantity, so it can't produce a reversal.
        let q = extract_quantities(
            &sents(&["Este hombre caminó 10 metros norte.", "Luego 10 metros sur."]),
            &lex,
        );
        // Only the two directed magnitudes extracted; the
        // bare "este" demonstrative is ignored.
        assert!(q.iter().all(|x| x.dim == Dim::Spatial));
        let c = detect_contradictions(&q, &ContradictionConfig::default());
        assert_eq!(c.len(), 1, "norte/sur reversal flagged; 'este' ignored");
    }

    // ── lexicon availability ──────────────────────────

    #[test]
    fn unsupported_language_has_no_lexicon() {
        assert!(built_in_lexicon("japanese").is_none());
        assert!(built_in_lexicon("russian").is_none()); // C.2.b
        assert!(built_in_lexicon("german").is_none()); // C.2.b
    }

    #[test]
    fn supported_languages_have_lexicons() {
        for l in ["english", "en", "french", "fr", "spanish", "es"] {
            assert!(built_in_lexicon(l).is_some(), "missing lexicon: {l}");
        }
    }

    // ── direction opposite ────────────────────────────

    #[test]
    fn direction_opposites() {
        assert_eq!(Direction::North.opposite(), Direction::South);
        assert_eq!(Direction::East.opposite(), Direction::West);
        assert_eq!(Direction::South.opposite(), Direction::North);
        assert_eq!(Direction::West.opposite(), Direction::East);
    }

    // ── sentence split ────────────────────────────────

    #[test]
    fn sentence_split_basic() {
        let s = split_sentences("First. Second! Third? Trailing");
        assert_eq!(s.len(), 4);
        assert_eq!(s[3], "Trailing");
    }

    #[test]
    fn no_quantities_in_plain_prose() {
        let lex = english();
        let q = extract_quantities(
            &sents(&["Helena paused at the threshold, listening."]),
            &lex,
        );
        assert!(q.is_empty());
    }
}
