//! 1.3.2 PLANNING-1 P0 — the structure model: story-structure frameworks
//! and their beats.
//!
//! Structure is the Planning Board's axis (acts / beats / turning points)
//! — orthogonal to Timeline (*when*) and Threads (*arc payoff*).  A
//! `Framework` is an ordered table of `{ beat, act, target_position }`;
//! `inkhaven plan init` scaffolds the chosen framework's beats into the
//! `Planning` system book as HJSON-fronted paragraphs (the Threads
//! pattern), parsed back via `serde_hjson`.

use serde::{Deserialize, Serialize};

/// A position in the framework's table: name + act (1/2/3) + the target
/// fraction through the book (`0.0..=1.0`).
#[derive(Debug, Clone, Copy)]
pub struct BeatSpec {
    pub name: &'static str,
    pub act: u8,
    pub target_position: f32,
}

/// A beat as stored in a Planning-book paragraph (pure HJSON body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Beat {
    pub framework: String,
    pub beat: String,
    pub act: u8,
    pub target_position: f32,
    /// Chapter slug this beat maps to (`None` = an unfilled gap).
    #[serde(default)]
    pub mapped_chapter: Option<String>,
    /// Thread (arc) slugs this beat advances.
    #[serde(default)]
    pub threads: Vec<String>,
    /// `planned` | `drafted` | `done`.
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub notes: String,
}

fn default_status() -> String {
    "planned".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framework {
    ThreeAct,
    SaveTheCat,
    StoryCircle,
    HeroJourney,
    SevenPoint,
}

impl Framework {
    /// Used by the P2 framework picker + the tests.
    #[allow(dead_code)]
    pub const ALL: [Self; 5] = [
        Self::ThreeAct,
        Self::SaveTheCat,
        Self::StoryCircle,
        Self::HeroJourney,
        Self::SevenPoint,
    ];

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
            "three_act" | "threeact" | "3act" | "three" => Some(Self::ThreeAct),
            "save_the_cat" | "savethecat" | "stc" | "cat" => Some(Self::SaveTheCat),
            "story_circle" | "storycircle" | "circle" => Some(Self::StoryCircle),
            "hero_journey" | "herojourney" | "heros_journey" | "hero" => Some(Self::HeroJourney),
            "seven_point" | "sevenpoint" | "7point" | "seven" => Some(Self::SevenPoint),
            _ => None,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::ThreeAct => "three_act",
            Self::SaveTheCat => "save_the_cat",
            Self::StoryCircle => "story_circle",
            Self::HeroJourney => "hero_journey",
            Self::SevenPoint => "seven_point",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ThreeAct => "Three-Act",
            Self::SaveTheCat => "Save the Cat",
            Self::StoryCircle => "Story Circle",
            Self::HeroJourney => "Hero's Journey",
            Self::SevenPoint => "Seven-Point",
        }
    }

    pub fn beats(self) -> &'static [BeatSpec] {
        match self {
            Self::ThreeAct => THREE_ACT,
            Self::SaveTheCat => SAVE_THE_CAT,
            Self::StoryCircle => STORY_CIRCLE,
            Self::HeroJourney => HERO_JOURNEY,
            Self::SevenPoint => SEVEN_POINT,
        }
    }

    /// The framework's beats as storable [`Beat`] records (unmapped).
    pub fn seed_beats(self) -> Vec<Beat> {
        self.beats()
            .iter()
            .map(|b| Beat {
                framework: self.slug().to_string(),
                beat: b.name.to_string(),
                act: b.act,
                target_position: b.target_position,
                mapped_chapter: None,
                threads: Vec::new(),
                status: default_status(),
                notes: String::new(),
            })
            .collect()
    }
}

/// Render a beat as the pure-HJSON paragraph body (content_type `hjson`),
/// commented for the author.  Round-trips through [`parse_beat`].
pub fn beat_body(b: &Beat) -> String {
    let mapped = match &b.mapped_chapter {
        Some(c) => format!("\"{}\"", esc(c)),
        None => "null".to_string(),
    };
    format!(
        "// planning beat — framework: {fw}\n\
{{\n  \
  framework:       \"{fw}\"\n  \
  beat:            \"{beat}\"\n  \
  act:             {act}\n  \
  // Target fraction through the book (0.0–1.0).\n  \
  target_position: {pos}\n  \
  // Chapter slug this beat maps to (null = a gap).\n  \
  mapped_chapter:  {mapped}\n  \
  // Thread (arc) slugs this beat advances.\n  \
  threads:         []\n  \
  // planned | drafted | done\n  \
  status:          \"{status}\"\n  \
  // Author's notes for this structural beat.\n  \
  notes:           \"\"\n\
}}\n",
        fw = esc(&b.framework),
        beat = esc(&b.beat),
        act = b.act,
        pos = b.target_position,
        mapped = mapped,
        status = esc(&b.status),
    )
}

/// Parse a Planning-book paragraph body back into a [`Beat`].
// Consumed by P1 (`plan check` reads beats back to compute coverage/pacing)
// + the round-trip test.
#[allow(dead_code)]
pub fn parse_beat(body: &str) -> Option<Beat> {
    serde_hjson::from_str(body).ok()
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ── AI analyze (P3) — prompt composers shared by CLI + TUI ──────────

/// The prompt-override slug (Prompts book / `prompts.hjson`) + the title
/// of the analysis draft.
pub const ANALYZE_SLUG: &str = "plan-analyze";

pub fn analyze_system_prompt() -> &'static str {
    "You are a developmental editor with deep command of story structure. Using ONLY the supplied \
chapter summaries — never invent plot — do two things: (1) map each framework beat to the single \
best-fitting chapter, or say it has no clear home; (2) diagnose the structure plainly: missing or \
weak beats, where the middle sags, and pacing problems. Be specific and concise. No preamble."
}

/// Compose the analyze user prompt from a framework + the book digest's
/// rendered context (`BookDigest::as_context`).
pub fn analyze_user_prompt(framework: Framework, digest_context: &str) -> String {
    let mut beats = String::new();
    for b in framework.beats() {
        beats.push_str(&format!(
            "- {} (act {}, ~{:.0}%)\n",
            b.name,
            b.act,
            b.target_position * 100.0
        ));
    }
    format!(
        "STORY-STRUCTURE FRAMEWORK: {label}\nBeats (with target position through the book):\n{beats}\n\
BOOK:\n{digest_context}\n\nMap the beats to chapters, then diagnose the structure.",
        label = framework.label(),
    )
}

// ── coverage + pacing analysis (P1, deterministic) ──────────────────

/// A chapter's slug + its **start** position as a fraction of the book's
/// total words (`0.0..1.0`).  A beat mapped to a chapter "occurs at" that
/// chapter's start.
#[derive(Debug, Clone)]
pub struct ChapterPos {
    pub slug: String,
    pub start: f32,
}

/// One beat's coverage/drift status.
#[derive(Debug, Clone, Serialize)]
pub struct BeatStatus {
    pub beat: String,
    pub act: u8,
    pub target_position: f32,
    pub mapped_chapter: Option<String>,
    /// Where the mapped chapter actually starts (None if unmapped or the
    /// slug doesn't resolve).
    pub actual_position: Option<f32>,
    /// `actual - target` (None if unmapped).
    pub drift: Option<f32>,
}

/// Word-share of one act: the framework's expected fraction vs. the
/// draft's actual fraction (None when an act-boundary beat is unmapped).
#[derive(Debug, Clone, Serialize)]
pub struct ActPacing {
    pub act: u8,
    pub expected: f32,
    pub actual: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanReport {
    pub beats: Vec<BeatStatus>,
    /// Unmapped beat names.
    pub gaps: Vec<String>,
    pub acts: Vec<ActPacing>,
    pub warnings: Vec<String>,
}

/// Diagnose a structure: coverage (gaps), per-beat position drift, and
/// per-act word-share pacing.  Pure — `chapters` carry the word-derived
/// positions, so this is fully testable with synthetic inputs.
pub fn analyze(beats: &[Beat], chapters: &[ChapterPos], drift_threshold: f32) -> PlanReport {
    use std::collections::{BTreeSet, HashMap};
    let pos: HashMap<&str, f32> =
        chapters.iter().map(|c| (c.slug.as_str(), c.start)).collect();

    let mut statuses = Vec::with_capacity(beats.len());
    let mut gaps = Vec::new();
    for b in beats {
        let actual = b
            .mapped_chapter
            .as_deref()
            .and_then(|c| pos.get(c).copied());
        if b.mapped_chapter.is_none() {
            gaps.push(b.beat.clone());
        }
        statuses.push(BeatStatus {
            beat: b.beat.clone(),
            act: b.act,
            target_position: b.target_position,
            mapped_chapter: b.mapped_chapter.clone(),
            actual_position: actual,
            drift: actual.map(|a| a - b.target_position),
        });
    }

    // Acts present, in order. Each act spans [first beat of act, first beat
    // of the next act) — by target for "expected", by the act-start beat's
    // mapped chapter for "actual".
    let acts_vec: Vec<u8> = beats.iter().map(|b| b.act).collect::<BTreeSet<_>>().into_iter().collect();
    let first_of = |act: u8| beats.iter().find(|b| b.act == act);
    let target_start = |act: u8| -> f32 {
        if acts_vec.first() == Some(&act) {
            0.0
        } else {
            first_of(act).map(|b| b.target_position).unwrap_or(0.0)
        }
    };
    let actual_start = |act: u8| -> Option<f32> {
        if acts_vec.first() == Some(&act) {
            return Some(0.0); // the book starts at the first act
        }
        first_of(act)
            .and_then(|b| b.mapped_chapter.as_deref())
            .and_then(|c| pos.get(c).copied())
    };

    let mut acts = Vec::new();
    for (i, &a) in acts_vec.iter().enumerate() {
        let exp_end = acts_vec.get(i + 1).map(|&n| target_start(n)).unwrap_or(1.0);
        let expected = (exp_end - target_start(a)).max(0.0);
        let act_end = acts_vec.get(i + 1).map(|&n| actual_start(n)).unwrap_or(Some(1.0));
        let actual = match (actual_start(a), act_end) {
            (Some(s), Some(e)) => Some((e - s).max(0.0)),
            _ => None,
        };
        acts.push(ActPacing { act: a, expected, actual });
    }

    let mut warnings = Vec::new();
    for g in &gaps {
        warnings.push(format!("gap: `{g}` is unmapped"));
    }
    for s in &statuses {
        if let (Some(d), Some(a)) = (s.drift, s.actual_position) {
            if d.abs() > drift_threshold {
                warnings.push(format!(
                    "drift: `{}` lands at {:.0}% (target {:.0}%, {:+.0}%)",
                    s.beat,
                    a * 100.0,
                    s.target_position * 100.0,
                    d * 100.0
                ));
            }
        }
    }
    for p in &acts {
        if let Some(a) = p.actual {
            let dev = a - p.expected;
            if dev.abs() > drift_threshold {
                warnings.push(format!(
                    "pacing: Act {} is {:.0}% of words (expected {:.0}%, {})",
                    p.act,
                    a * 100.0,
                    p.expected * 100.0,
                    if dev > 0.0 { "long" } else { "short" }
                ));
            }
        }
    }

    PlanReport { beats: statuses, gaps, acts, warnings }
}

// ── built-in framework tables (positions monotonic non-decreasing) ──

const THREE_ACT: &[BeatSpec] = &[
    BeatSpec { name: "Opening", act: 1, target_position: 0.00 },
    BeatSpec { name: "Inciting Incident", act: 1, target_position: 0.10 },
    // Plot Point One launches act 2 (the act-1 turning point); Plot Point
    // Two launches act 3 — so the act boundaries land at 25% / 75% and the
    // expected word-share is the canonical 25 / 50 / 25.
    BeatSpec { name: "Plot Point One", act: 2, target_position: 0.25 },
    BeatSpec { name: "First Pinch Point", act: 2, target_position: 0.375 },
    BeatSpec { name: "Midpoint", act: 2, target_position: 0.50 },
    BeatSpec { name: "Second Pinch Point", act: 2, target_position: 0.625 },
    BeatSpec { name: "Plot Point Two", act: 3, target_position: 0.75 },
    BeatSpec { name: "Climax", act: 3, target_position: 0.90 },
    BeatSpec { name: "Resolution", act: 3, target_position: 1.00 },
];

const SAVE_THE_CAT: &[BeatSpec] = &[
    BeatSpec { name: "Opening Image", act: 1, target_position: 0.00 },
    BeatSpec { name: "Theme Stated", act: 1, target_position: 0.05 },
    BeatSpec { name: "Set-Up", act: 1, target_position: 0.08 },
    BeatSpec { name: "Catalyst", act: 1, target_position: 0.10 },
    BeatSpec { name: "Debate", act: 1, target_position: 0.15 },
    BeatSpec { name: "Break into Two", act: 2, target_position: 0.20 },
    BeatSpec { name: "B Story", act: 2, target_position: 0.22 },
    BeatSpec { name: "Fun and Games", act: 2, target_position: 0.30 },
    BeatSpec { name: "Midpoint", act: 2, target_position: 0.50 },
    BeatSpec { name: "Bad Guys Close In", act: 2, target_position: 0.62 },
    BeatSpec { name: "All Is Lost", act: 2, target_position: 0.75 },
    BeatSpec { name: "Dark Night of the Soul", act: 2, target_position: 0.77 },
    BeatSpec { name: "Break into Three", act: 3, target_position: 0.80 },
    BeatSpec { name: "Finale", act: 3, target_position: 0.90 },
    BeatSpec { name: "Final Image", act: 3, target_position: 1.00 },
];

const STORY_CIRCLE: &[BeatSpec] = &[
    BeatSpec { name: "You (comfort zone)", act: 1, target_position: 0.00 },
    BeatSpec { name: "Need", act: 1, target_position: 0.125 },
    BeatSpec { name: "Go (cross the threshold)", act: 2, target_position: 0.25 },
    BeatSpec { name: "Search (adapt)", act: 2, target_position: 0.375 },
    BeatSpec { name: "Find (get what they wanted)", act: 2, target_position: 0.50 },
    BeatSpec { name: "Take (pay the price)", act: 2, target_position: 0.625 },
    BeatSpec { name: "Return", act: 3, target_position: 0.75 },
    BeatSpec { name: "Change", act: 3, target_position: 0.875 },
];

const HERO_JOURNEY: &[BeatSpec] = &[
    BeatSpec { name: "Ordinary World", act: 1, target_position: 0.00 },
    BeatSpec { name: "Call to Adventure", act: 1, target_position: 0.08 },
    BeatSpec { name: "Refusal of the Call", act: 1, target_position: 0.12 },
    BeatSpec { name: "Meeting the Mentor", act: 1, target_position: 0.17 },
    BeatSpec { name: "Crossing the Threshold", act: 2, target_position: 0.25 },
    BeatSpec { name: "Tests, Allies, Enemies", act: 2, target_position: 0.35 },
    BeatSpec { name: "Approach to the Inmost Cave", act: 2, target_position: 0.45 },
    BeatSpec { name: "The Ordeal", act: 2, target_position: 0.50 },
    BeatSpec { name: "Reward", act: 2, target_position: 0.60 },
    BeatSpec { name: "The Road Back", act: 3, target_position: 0.75 },
    BeatSpec { name: "Resurrection", act: 3, target_position: 0.90 },
    BeatSpec { name: "Return with the Elixir", act: 3, target_position: 1.00 },
];

const SEVEN_POINT: &[BeatSpec] = &[
    BeatSpec { name: "Hook", act: 1, target_position: 0.00 },
    BeatSpec { name: "Plot Turn One", act: 2, target_position: 0.25 },
    BeatSpec { name: "Pinch Point One", act: 2, target_position: 0.375 },
    BeatSpec { name: "Midpoint", act: 2, target_position: 0.50 },
    BeatSpec { name: "Pinch Point Two", act: 2, target_position: 0.625 },
    BeatSpec { name: "Plot Turn Two", act: 3, target_position: 0.75 },
    BeatSpec { name: "Resolution", act: 3, target_position: 1.00 },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_framework_table_is_well_formed() {
        for fw in Framework::ALL {
            let beats = fw.beats();
            assert!(beats.len() >= 7, "{} has enough beats", fw.slug());
            let names: BTreeSet<_> = beats.iter().map(|b| b.name).collect();
            assert_eq!(names.len(), beats.len(), "{} beat names distinct", fw.slug());
            let mut prev_pos = -1.0f32;
            let mut prev_act = 0u8;
            for b in beats {
                assert!(
                    (0.0..=1.0).contains(&b.target_position),
                    "{}/{} position in range",
                    fw.slug(),
                    b.name
                );
                assert!(
                    b.target_position >= prev_pos,
                    "{}/{} positions monotonic",
                    fw.slug(),
                    b.name
                );
                assert!((1..=3).contains(&b.act), "{}/{} act 1..3", fw.slug(), b.name);
                assert!(b.act >= prev_act, "{}/{} acts non-decreasing", fw.slug(), b.name);
                prev_pos = b.target_position;
                prev_act = b.act;
            }
            assert!(beats[0].target_position < 1e-6, "{} opens at 0", fw.slug());
        }
    }

    #[test]
    fn framework_parse_round_trips_slug() {
        for fw in Framework::ALL {
            assert_eq!(Framework::parse(fw.slug()), Some(fw));
        }
        assert_eq!(Framework::parse("Save The Cat"), Some(Framework::SaveTheCat));
        assert_eq!(Framework::parse("7point"), Some(Framework::SevenPoint));
        assert!(Framework::parse("freytag").is_none());
    }

    fn beat(name: &str, act: u8, target: f32, mapped: Option<&str>) -> Beat {
        Beat {
            framework: "t".into(),
            beat: name.into(),
            act,
            target_position: target,
            mapped_chapter: mapped.map(|s| s.to_string()),
            threads: vec![],
            status: "planned".into(),
            notes: String::new(),
        }
    }

    #[test]
    fn analyze_flags_gaps_drift_and_pacing() {
        let beats = vec![
            beat("A", 1, 0.0, Some("c1")),
            beat("B", 2, 0.5, Some("c2")), // act-2 boundary, lands late
            beat("C", 3, 0.9, None),       // gap — act-3 boundary unmapped
        ];
        let chapters = vec![
            ChapterPos { slug: "c1".into(), start: 0.0 },
            ChapterPos { slug: "c2".into(), start: 0.65 },
        ];
        let r = analyze(&beats, &chapters, 0.10);
        assert_eq!(r.gaps, vec!["C"]);
        let b = r.beats.iter().find(|s| s.beat == "B").unwrap();
        assert!((b.drift.unwrap() - 0.15).abs() < 1e-5, "B drifts +15%");
        assert!(r.beats.iter().find(|s| s.beat == "A").unwrap().drift.unwrap().abs() < 1e-6);
        let act1 = r.acts.iter().find(|p| p.act == 1).unwrap();
        assert!((act1.expected - 0.5).abs() < 1e-6, "act1 expected 0..0.5");
        assert!((act1.actual.unwrap() - 0.65).abs() < 1e-5, "act1 actual 0..0.65");
        // act2's end boundary (C) is unmapped → its actual is unknown.
        assert!(r.acts.iter().find(|p| p.act == 2).unwrap().actual.is_none());
        assert!(r.warnings.iter().any(|w| w.contains("gap: `C`")));
        assert!(r.warnings.iter().any(|w| w.contains("drift: `B`")));
        assert!(r.warnings.iter().any(|w| w.contains("Act 1") && w.contains("long")));
    }

    #[test]
    fn expected_act_proportions_are_canonical() {
        // Three-act resolves to the canonical 25 / 50 / 25 word-share.
        let r = analyze(&Framework::ThreeAct.seed_beats(), &[], 0.10);
        let exp: Vec<f32> = r.acts.iter().map(|a| a.expected).collect();
        assert_eq!(exp.len(), 3);
        assert!((exp[0] - 0.25).abs() < 1e-6, "act1 25%");
        assert!((exp[1] - 0.50).abs() < 1e-6, "act2 50%");
        assert!((exp[2] - 0.25).abs() < 1e-6, "act3 25%");
        // Every framework: proportions sum to 1 and act 1 is a sane setup.
        for fw in Framework::ALL {
            let r = analyze(&fw.seed_beats(), &[], 0.10);
            let sum: f32 = r.acts.iter().map(|a| a.expected).sum();
            assert!((sum - 1.0).abs() < 1e-5, "{} sums to 1", fw.slug());
            assert!(
                (0.15..=0.30).contains(&r.acts[0].expected),
                "{} act1 is a sane setup ({})",
                fw.slug(),
                r.acts[0].expected
            );
        }
    }

    #[test]
    fn analyze_clean_structure_has_no_warnings() {
        // every beat mapped exactly at its act boundary → expected == actual.
        let beats = vec![
            beat("A", 1, 0.0, Some("c1")),
            beat("B", 2, 0.25, Some("c2")),
            beat("C", 3, 0.75, Some("c3")),
        ];
        let chapters = vec![
            ChapterPos { slug: "c1".into(), start: 0.0 },
            ChapterPos { slug: "c2".into(), start: 0.25 },
            ChapterPos { slug: "c3".into(), start: 0.75 },
        ];
        let r = analyze(&beats, &chapters, 0.10);
        assert!(r.gaps.is_empty());
        assert!(r.warnings.is_empty(), "unexpected warnings: {:?}", r.warnings);
        assert!((r.acts.iter().find(|p| p.act == 2).unwrap().actual.unwrap() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn analyze_prompt_carries_framework_and_context() {
        let p = analyze_user_prompt(Framework::SaveTheCat, "TITLE: X\nCHAPTER SUMMARIES:\n1. Foo");
        assert!(p.contains("Save the Cat"));
        assert!(p.contains("Midpoint (act 2, ~50%)"));
        assert!(p.contains("CHAPTER SUMMARIES:"));
        assert!(!analyze_system_prompt().is_empty());
    }

    #[test]
    fn beat_body_round_trips_through_hjson() {
        let beats = Framework::SaveTheCat.seed_beats();
        let mid = beats.iter().find(|b| b.beat == "Midpoint").unwrap();
        let back = parse_beat(&beat_body(mid)).expect("parses");
        assert_eq!(back.framework, "save_the_cat");
        assert_eq!(back.beat, "Midpoint");
        assert_eq!(back.act, 2);
        assert!((back.target_position - 0.50).abs() < 1e-6);
        assert_eq!(back.status, "planned");
        assert!(back.mapped_chapter.is_none());
        // a mapped beat round-trips its chapter slug
        let mut mapped = mid.clone();
        mapped.mapped_chapter = Some("03-the-wharf".into());
        assert_eq!(
            parse_beat(&beat_body(&mapped)).unwrap().mapped_chapter.as_deref(),
            Some("03-the-wharf")
        );
    }
}
