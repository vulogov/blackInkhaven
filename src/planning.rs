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

// ── built-in framework tables (positions monotonic non-decreasing) ──

const THREE_ACT: &[BeatSpec] = &[
    BeatSpec { name: "Opening", act: 1, target_position: 0.00 },
    BeatSpec { name: "Inciting Incident", act: 1, target_position: 0.10 },
    BeatSpec { name: "Plot Point One", act: 1, target_position: 0.25 },
    BeatSpec { name: "First Pinch Point", act: 2, target_position: 0.375 },
    BeatSpec { name: "Midpoint", act: 2, target_position: 0.50 },
    BeatSpec { name: "Second Pinch Point", act: 2, target_position: 0.625 },
    BeatSpec { name: "Plot Point Two", act: 2, target_position: 0.75 },
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
