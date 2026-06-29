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
/// fraction through the book (`0.0..=1.0`) + the beat's place on the
/// framework's dramatic-intensity curve (`expected_tension`, 0 = calm,
/// 1 = peak). Both `target_position` and `expected_tension` are authored
/// canon — the shape of the rise and fall — not author data, so nothing
/// about them is stored per-project or migrated.
#[derive(Debug, Clone, Copy)]
pub struct BeatSpec {
    pub name: &'static str,
    pub act: u8,
    pub target_position: f32,
    pub expected_tension: f32,
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
    let threads = if b.threads.is_empty() {
        "[]".to_string()
    } else {
        format!(
            "[{}]",
            b.threads
                .iter()
                .map(|t| format!("\"{}\"", esc(t)))
                .collect::<Vec<_>>()
                .join(", ")
        )
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
  threads:         {threads}\n  \
  // planned | drafted | done\n  \
  status:          \"{status}\"\n  \
  // Author's notes for this structural beat.\n  \
  notes:           \"{notes}\"\n\
}}\n",
        fw = esc(&b.framework),
        beat = esc(&b.beat),
        act = b.act,
        pos = b.target_position,
        mapped = mapped,
        threads = threads,
        status = esc(&b.status),
        notes = esc(&b.notes),
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
chapter summaries — never invent plot — do these: (1) map each framework beat to the single \
best-fitting chapter, or say it has no clear home; (2) diagnose the structure plainly: missing or \
weak beats, where the middle sags, and pacing problems; (3) if scene cards are listed, flag any \
scene that states a goal but doesn't turn (no disaster) and suggest the turn it's missing. Be \
specific and concise. No preamble."
}

/// Compose the analyze user prompt from a framework + the book digest's
/// rendered context (`BookDigest::as_context`) + any scene cards.
pub fn analyze_user_prompt(framework: Framework, digest_context: &str, scenes: &[Scene]) -> String {
    let mut beats = String::new();
    for b in framework.beats() {
        beats.push_str(&format!(
            "- {} (act {}, ~{:.0}%)\n",
            b.name,
            b.act,
            b.target_position * 100.0
        ));
    }
    let scene_block = if scenes.is_empty() {
        String::new()
    } else {
        let mut s = String::from("\nSCENE CARDS (goal → conflict → disaster):\n");
        for sc in scenes {
            s.push_str(&format!(
                "- [{}] {}: goal={} | conflict={} | disaster={}\n",
                if sc.chapter.is_empty() { "?" } else { &sc.chapter },
                sc.title,
                if sc.goal.trim().is_empty() { "—" } else { sc.goal.trim() },
                if sc.conflict.trim().is_empty() { "—" } else { sc.conflict.trim() },
                if sc.disaster.trim().is_empty() { "—" } else { sc.disaster.trim() },
            ));
        }
        s
    };
    format!(
        "STORY-STRUCTURE FRAMEWORK: {label}\nBeats (with target position through the book):\n{beats}{scene_block}\n\
BOOK:\n{digest_context}\n\nMap the beats to chapters, then diagnose the structure.",
        label = framework.label(),
    )
}

// ── plan-first scaffolding (PLANNING-2 P2) ──────────────────────────

pub const SCAFFOLD_SLUG: &str = "plan-scaffold";

pub fn scaffold_system_prompt() -> &'static str {
    "You are a story architect. Given a premise and a beat sheet, write a concrete 1–2 sentence \
intention for EACH beat — what actually happens at that beat in this story. These are planning \
notes, not prose. Output exactly one line per beat in the form `<Beat Name>: <intention>`, using \
the beat names verbatim, in order, with no numbering and no preamble."
}

pub fn scaffold_user_prompt(framework: Framework, premise: &str) -> String {
    let mut names = String::new();
    for b in framework.beats() {
        names.push_str(&format!("- {}\n", b.name));
    }
    format!(
        "PREMISE / LOGLINE: {premise}\n\nSTORY-STRUCTURE FRAMEWORK: {label}\n\nWrite a `<Beat Name>: \
<intention>` line for each beat below, in order.\n\nBEATS:\n{names}",
        label = framework.label(),
    )
}

/// Parse a scaffold response into `(beat name, intention)` pairs, matching
/// each line's leading label (case-insensitive, tolerant of a `1.`/`-`
/// prefix) to a beat.  Unmatched beats are simply absent — never
/// mis-assigned.
pub fn parse_scaffold(raw: &str, beats: &[Beat]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let Some((name, intention)) = line.split_once(':') else {
            continue;
        };
        let name = name
            .trim()
            .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-' || c == ')')
            .trim();
        let intention = intention.trim();
        if intention.is_empty() {
            continue;
        }
        if let Some(b) = beats.iter().find(|b| b.beat.eq_ignore_ascii_case(name)) {
            if !out.iter().any(|(n, _): &(String, String)| n == &b.beat) {
                out.push((b.beat.clone(), intention.to_string()));
            }
        }
    }
    out
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
    /// Thread (arc) slugs this beat advances.
    pub threads: Vec<String>,
    /// Referenced thread slugs that don't exist in the Threads book.
    pub unknown_threads: Vec<String>,
    /// The beat's intention note (filled by `plan scaffold`, or by hand).
    pub notes: String,
}

/// Word-share of one act: the framework's expected fraction vs. the
/// draft's actual fraction (None when an act-boundary beat is unmapped).
#[derive(Debug, Clone, Serialize)]
pub struct ActPacing {
    pub act: u8,
    pub expected: f32,
    pub actual: Option<f32>,
}

/// A mapping target: a chapter's slug (the `mapped_chapter` value) + where
/// it sits in the book.
#[derive(Debug, Clone, Serialize)]
pub struct ChapterRef {
    pub slug: String,
    pub position: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanReport {
    pub beats: Vec<BeatStatus>,
    /// Unmapped beat names.
    pub gaps: Vec<String>,
    pub acts: Vec<ActPacing>,
    pub warnings: Vec<String>,
    /// The book's chapters (slug + position) — the values to put in a
    /// beat's `mapped_chapter`.
    pub chapters: Vec<ChapterRef>,
    /// Thread slugs in the Threads book — the values to put in a beat's
    /// `threads`.
    pub available_threads: Vec<String>,
    /// Expected-vs-actual narrative intensity (P0 1.3.4). `None` until the
    /// caller attaches it (the CLI builds the open-obligation spans).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tension: Option<TensionCurve>,
    /// Scene-card craft status (P3 1.3.4) — empty until the caller loads the
    /// Planning book's scene cards.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<SceneStatus>,
}

/// Diagnose a structure: coverage (gaps), per-beat position drift, and
/// per-act word-share pacing.  Pure — `chapters` carry the word-derived
/// positions, so this is fully testable with synthetic inputs.
pub fn analyze(
    beats: &[Beat],
    chapters: &[ChapterPos],
    drift_threshold: f32,
    known_threads: &std::collections::BTreeSet<String>,
) -> PlanReport {
    use std::collections::{BTreeSet, HashMap};
    let pos: HashMap<&str, f32> =
        chapters.iter().map(|c| (c.slug.as_str(), c.start)).collect();

    let mut statuses = Vec::with_capacity(beats.len());
    let mut gaps = Vec::new();
    let mut mapped_without_thread = 0usize;
    for b in beats {
        let actual = b
            .mapped_chapter
            .as_deref()
            .and_then(|c| pos.get(c).copied());
        if b.mapped_chapter.is_none() {
            gaps.push(b.beat.clone());
        }
        let unknown_threads: Vec<String> = b
            .threads
            .iter()
            .filter(|t| !known_threads.is_empty() && !known_threads.contains(*t))
            .cloned()
            .collect();
        if b.mapped_chapter.is_some() && b.threads.is_empty() {
            mapped_without_thread += 1;
        }
        statuses.push(BeatStatus {
            beat: b.beat.clone(),
            act: b.act,
            target_position: b.target_position,
            mapped_chapter: b.mapped_chapter.clone(),
            actual_position: actual,
            drift: actual.map(|a| a - b.target_position),
            threads: b.threads.clone(),
            unknown_threads,
            notes: b.notes.clone(),
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
    for s in &statuses {
        for t in &s.unknown_threads {
            warnings.push(format!("thread: `{}` references unknown thread `{t}`", s.beat));
        }
    }
    // Only nudge once the author is actually using thread-links — don't
    // nag projects that haven't adopted them.
    if mapped_without_thread > 0 && beats.iter().any(|b| !b.threads.is_empty()) {
        warnings.push(format!(
            "threads: {mapped_without_thread} mapped beat(s) advance no tracked thread — link them in each beat's `threads`"
        ));
    }

    let chapter_refs = chapters
        .iter()
        .map(|c| ChapterRef { slug: c.slug.clone(), position: c.start })
        .collect();
    PlanReport {
        beats: statuses,
        gaps,
        acts,
        warnings,
        chapters: chapter_refs,
        available_threads: known_threads.iter().cloned().collect(),
        tension: None,
        scenes: Vec::new(),
    }
}

// ── scene cards (P3 1.3.4) ──────────────────────────────────────────

fn default_kind() -> String {
    "scene".to_string()
}

/// A planning card finer than a beat — one of two kinds (Swain):
/// a **scene** is proactive (goal → conflict → disaster); a **sequel** is
/// reactive (reaction → dilemma → decision). Both store as an HJSON
/// paragraph under the Planning book's `Scenes` chapter; `kind` discriminates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    /// `scene` (proactive) | `sequel` (reactive).
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Chapter slug this card belongs to.
    #[serde(default)]
    pub chapter: String,
    pub title: String,
    // proactive (scene) triple
    #[serde(default)]
    pub goal: String,
    #[serde(default)]
    pub conflict: String,
    #[serde(default)]
    pub disaster: String,
    // reactive (sequel) triple
    #[serde(default)]
    pub reaction: String,
    #[serde(default)]
    pub dilemma: String,
    #[serde(default)]
    pub decision: String,
    #[serde(default = "default_status")]
    pub status: String,
    /// CHAR-1 — character names from the Characters book whose arc this scene
    /// advances. Optional; backward-compatible.
    #[serde(default)]
    pub characters: Vec<String>,
    /// CHAR-1 — free-text note on the arc work this scene does. Display-only;
    /// not parsed or checked.
    #[serde(default)]
    pub arc_function: Option<String>,
}

impl Scene {
    /// A proactive scene card (goal/conflict/disaster).
    pub fn new_scene(chapter: &str, title: &str, goal: &str, conflict: &str, disaster: &str) -> Self {
        Self {
            kind: "scene".into(),
            chapter: chapter.into(),
            title: title.into(),
            goal: goal.into(),
            conflict: conflict.into(),
            disaster: disaster.into(),
            reaction: String::new(),
            dilemma: String::new(),
            decision: String::new(),
            status: default_status(),
            characters: Vec::new(),
            arc_function: None,
        }
    }
    /// A reactive sequel card (reaction/dilemma/decision).
    pub fn new_sequel(chapter: &str, title: &str, reaction: &str, dilemma: &str, decision: &str) -> Self {
        Self {
            kind: "sequel".into(),
            chapter: chapter.into(),
            title: title.into(),
            goal: String::new(),
            conflict: String::new(),
            disaster: String::new(),
            reaction: reaction.into(),
            dilemma: dilemma.into(),
            decision: decision.into(),
            status: default_status(),
            characters: Vec::new(),
            arc_function: None,
        }
    }
    pub fn is_sequel(&self) -> bool {
        self.kind.eq_ignore_ascii_case("sequel")
    }
    /// The three labelled slots for this card's kind: scene →
    /// goal/conflict/disaster, sequel → reaction/dilemma/decision.
    pub fn slots(&self) -> [(&'static str, &str); 3] {
        if self.is_sequel() {
            [
                ("reaction", self.reaction.as_str()),
                ("dilemma", self.dilemma.as_str()),
                ("decision", self.decision.as_str()),
            ]
        } else {
            [
                ("goal", self.goal.as_str()),
                ("conflict", self.conflict.as_str()),
                ("disaster", self.disaster.as_str()),
            ]
        }
    }
}

/// Render a card as its pure-HJSON paragraph body (content_type `hjson`),
/// the triple chosen by `kind`. Round-trips through [`parse_scene`].
pub fn scene_body(s: &Scene) -> String {
    let kind = if s.is_sequel() { "sequel" } else { "scene" };
    let triple = if s.is_sequel() {
        format!(
            "  // The POV character's emotional response to the prior disaster.\n  \
  reaction:  \"{reaction}\"\n  \
  // The bad-options bind it forces.\n  \
  dilemma:   \"{dilemma}\"\n  \
  // The choice that launches the next goal. A sequel that reaches a\n  \
  // dilemma but never decides stalls the story.\n  \
  decision:  \"{decision}\"\n",
            reaction = esc(&s.reaction),
            dilemma = esc(&s.dilemma),
            decision = esc(&s.decision),
        )
    } else {
        format!(
            "  // What the POV character wants in this scene.\n  \
  goal:      \"{goal}\"\n  \
  // What stands in the way.\n  \
  conflict:  \"{conflict}\"\n  \
  // The turn — how the scene ends worse / changed. A scene with no\n  \
  // disaster doesn't turn.\n  \
  disaster:  \"{disaster}\"\n",
            goal = esc(&s.goal),
            conflict = esc(&s.conflict),
            disaster = esc(&s.disaster),
        )
    };
    format!(
        "// planning {kind} card\n\
{{\n  \
  kind:      \"{kind}\"\n  \
  chapter:   \"{chapter}\"\n  \
  title:     \"{title}\"\n\
{triple}  \
  // planned | drafted | done\n  \
  status:    \"{status}\"\n\
}}\n",
        chapter = esc(&s.chapter),
        title = esc(&s.title),
        status = esc(&s.status),
    )
}

/// Parse a Planning-book card paragraph back into a [`Scene`].
pub fn parse_scene(body: &str) -> Option<Scene> {
    serde_hjson::from_str(body).ok()
}

/// One card's craft status: its kind, which of the three slots are filled,
/// and the weak flag (a scene with no disaster / a sequel with no decision).
#[derive(Debug, Clone, Serialize)]
pub struct SceneStatus {
    pub title: String,
    pub chapter: String,
    pub kind: String,
    /// The three slots present, in `slots()` order.
    pub filled: [bool; 3],
    pub weak: bool,
}

/// Deterministic weak-card diagnosis. A **scene** is weak when it states a
/// goal but never turns (no disaster); a **sequel** is weak when it reaches
/// a dilemma but never decides (no decision). Once the author uses sequels,
/// two scenes in a row (no sequel between) flags the first's unprocessed
/// disaster. Pure — returns per-card status + warnings.
pub fn analyze_scenes(scenes: &[Scene]) -> (Vec<SceneStatus>, Vec<String>) {
    let mut statuses = Vec::with_capacity(scenes.len());
    let mut warnings = Vec::new();
    for s in scenes {
        let [(_, a), (_, b), (_, c)] = s.slots();
        let filled = [!a.trim().is_empty(), !b.trim().is_empty(), !c.trim().is_empty()];
        let weak = if s.is_sequel() {
            filled[1] && !filled[2] // dilemma but no decision
        } else {
            filled[0] && !filled[2] // goal but no disaster
        };
        if weak {
            warnings.push(if s.is_sequel() {
                format!("sequel: `{}` reaches a dilemma but never decides (no decision)", s.title)
            } else {
                format!("scene: `{}` states a goal but never turns (no disaster)", s.title)
            });
        }
        statuses.push(SceneStatus {
            title: s.title.clone(),
            chapter: s.chapter.clone(),
            kind: if s.is_sequel() { "sequel".into() } else { "scene".into() },
            filled,
            weak,
        });
    }
    // Alternation: only nag once sequels are in use, so scene-only projects
    // aren't flagged. Two scenes back-to-back → the first's disaster goes
    // unprocessed (a skipped sequel).
    if scenes.iter().any(|s| s.is_sequel()) {
        for w in scenes.windows(2) {
            if !w[0].is_sequel() && !w[1].is_sequel() {
                warnings.push(format!(
                    "rhythm: `{}`'s disaster goes unprocessed — no sequel before `{}`",
                    w[0].title, w[1].title
                ));
            }
        }
    }
    (statuses, warnings)
}

// ── scene scaffold (P0 1.3.5, AI card from the prose) ───────────────

/// The prompt-override slug (Prompts book / `prompts.hjson`) for the
/// scene-card scaffolder.
pub const SCENE_SCAFFOLD_SLUG: &str = "plan-scene-scaffold";

pub fn scene_scaffold_system_prompt() -> &'static str {
    "You are a story editor. Read ONE chapter and identify its dominant scene as goal / conflict / \
disaster (Swain's scene model): what the POV character actively wants in this chapter, what stands \
in the way, and the disaster the scene turns on — how it ends worse or changed. Use ONLY the text; \
never invent. Output EXACTLY three lines, one sentence each, concrete and specific:\n\
goal: <…>\nconflict: <…>\ndisaster: <…>\nNo preamble, no extra lines."
}

/// Compose the scene-scaffold user prompt from the chapter title + prose.
pub fn scene_scaffold_user_prompt(chapter_title: &str, prose: &str) -> String {
    format!("CHAPTER: {chapter_title}\n\n{prose}")
}

/// Parse the scaffolder's reply into `(goal, conflict, disaster)`. Tolerant
/// of list markers, bold, and case; missing fields come back empty. Pure.
pub fn parse_scene_scaffold(raw: &str) -> (String, String, String) {
    let field = |key: &str| -> String {
        for line in raw.lines() {
            let l = line.trim().trim_start_matches(['-', '*', '•', '#', ' ']).trim();
            let l = l.trim_start_matches("**").trim();
            if let Some((k, v)) = l.split_once(':') {
                if k.trim().trim_matches('*').eq_ignore_ascii_case(key) {
                    return v.trim().trim_matches('*').trim().to_string();
                }
            }
        }
        String::new()
    };
    (field("goal"), field("conflict"), field("disaster"))
}

// ── tension second opinion (P3 1.3.5, AI-rated intensity) ───────────

/// The prompt-override slug for the per-chapter intensity rater.
pub const TENSION_RATE_SLUG: &str = "plan-tension-rate";

pub fn tension_rate_system_prompt() -> &'static str {
    "You are a story editor rating dramatic intensity. Read ONE chapter and rate how much narrative \
tension it carries on a 0–100 scale: 0 = calm setup or denouement, 50 = steady rising action, \
100 = peak crisis / climax. Judge the felt pressure on the reader, not the word count. Reply with \
ONLY the integer — nothing else."
}

pub fn tension_rate_user_prompt(chapter_title: &str, prose: &str) -> String {
    format!("CHAPTER: {chapter_title}\n\n{prose}")
}

/// Parse the rater's reply into an intensity 0..1 — the first integer in
/// 0..=100, clamped. None if there's no number. Pure.
pub fn parse_intensity(raw: &str) -> Option<f32> {
    let mut digits = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            if digits.len() == 3 {
                break;
            }
        } else if !digits.is_empty() {
            break;
        }
    }
    digits
        .parse::<u32>()
        .ok()
        .map(|n| (n.min(100) as f32) / 100.0)
}

// ── tension curve (P0 1.3.4, deterministic) ─────────────────────────

/// A narrative obligation carrying `weight` tension while it is *open*
/// across the book-position span `[start, end)` (positions 0..1). Built by
/// the CLI from the tension ledger (one open question = weight 1.0) and
/// from open Threads (weight = the thread's 0–10 `tension` / 10). Pure data
/// so [`tension_curve`] stays testable without any I/O.
#[derive(Debug, Clone, Copy)]
pub struct OpenSpan {
    pub start: f32,
    pub end: f32,
    pub weight: f32,
}

/// One beat on the tension curve: the framework's `expected` intensity vs
/// the manuscript's `actual` (normalized open-obligation load at the beat's
/// mapped position).
#[derive(Debug, Clone, Serialize)]
pub struct TensionPoint {
    pub beat: String,
    /// The beat's mapped chapter start (None if unmapped).
    pub position: Option<f32>,
    pub expected: f32,
    /// Normalized 0..1 open-load at `position` (None if unmapped or no data).
    pub actual: Option<f32>,
    /// `expected - actual` (positive = flat against the framework's shape).
    pub gap: Option<f32>,
    /// AI-rated intensity at the beat's chapter (0..1), the 1.3.5 second
    /// opinion. None until `plan tension rate` runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai: Option<f32>,
}

/// Expected vs actual (vs AI-rated) narrative intensity across the book.
#[derive(Debug, Clone, Serialize)]
pub struct TensionCurve {
    pub points: Vec<TensionPoint>,
    /// `(position, normalized actual)` sampled at each chapter start plus
    /// the book end — the overlay's actual line.
    pub series: Vec<(f32, f32)>,
    /// `(position, ai-rated intensity)` per chapter — the second-opinion
    /// line; empty until `plan tension rate` runs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ai_series: Vec<(f32, f32)>,
    /// False when there were zero open obligations (no ledger, no linked
    /// threads): only `expected` is meaningful, render a hint not a curve.
    pub has_actual: bool,
    /// True once AI ratings are present.
    pub has_ai: bool,
    /// Beats flagged flat (high expected, low actual beyond the threshold).
    pub warnings: Vec<String>,
}

/// Summed weight of the obligations open at `position`.
fn open_load(spans: &[OpenSpan], position: f32) -> f32 {
    spans
        .iter()
        .filter(|s| s.start <= position && position < s.end)
        .map(|s| s.weight)
        .sum()
}

/// A stored beat's expected intensity, looked up from its framework table
/// (canonical — not author data, so nothing to store or migrate). Unknown
/// framework / beat falls back to a neutral 0.5.
fn expected_tension_for(framework: &str, beat: &str) -> f32 {
    Framework::parse(framework)
        .and_then(|fw| {
            fw.beats()
                .iter()
                .find(|b| b.name == beat)
                .map(|b| b.expected_tension)
        })
        .unwrap_or(0.5)
}

/// Build the tension curve. Pure: `chapters` supply the sample positions,
/// `spans` the open obligations. A beat is flagged *flat* when its expected
/// intensity is high (≥ 0.5) and its actual falls more than `flat_threshold`
/// below it. Actual is normalized to the book's own peak load, so the
/// **shape** is comparable to expected even for a lightly-tagged book.
pub fn tension_curve(
    beats: &[Beat],
    chapters: &[ChapterPos],
    spans: &[OpenSpan],
    ai_ratings: &std::collections::BTreeMap<String, f32>,
    flat_threshold: f32,
) -> TensionCurve {
    use std::collections::HashMap;
    let pos: HashMap<&str, f32> = chapters.iter().map(|c| (c.slug.as_str(), c.start)).collect();

    // Sample at every chapter start, plus the book end.
    let mut sample_pos: Vec<f32> = chapters.iter().map(|c| c.start).collect();
    sample_pos.push(1.0);
    let raw: Vec<f32> = sample_pos.iter().map(|&p| open_load(spans, p)).collect();
    let max_load = raw.iter().copied().fold(0.0f32, f32::max);
    let has_actual = max_load > 0.0;
    let norm = |load: f32| if max_load > 0.0 { (load / max_load).clamp(0.0, 1.0) } else { 0.0 };

    let series: Vec<(f32, f32)> = sample_pos
        .iter()
        .zip(&raw)
        .map(|(&p, &l)| (p, norm(l)))
        .collect();

    // AI second opinion: one point per chapter that has a rating.
    let has_ai = !ai_ratings.is_empty();
    let ai_series: Vec<(f32, f32)> = chapters
        .iter()
        .filter_map(|c| ai_ratings.get(&c.slug).map(|&v| (c.start, v.clamp(0.0, 1.0))))
        .collect();

    let mut points = Vec::with_capacity(beats.len());
    let mut warnings = Vec::new();
    for b in beats {
        let expected = expected_tension_for(&b.framework, &b.beat);
        let position = b.mapped_chapter.as_deref().and_then(|c| pos.get(c).copied());
        let actual = if has_actual {
            position.map(|p| norm(open_load(spans, p)))
        } else {
            None
        };
        let ai = b
            .mapped_chapter
            .as_deref()
            .and_then(|c| ai_ratings.get(c).map(|&v| v.clamp(0.0, 1.0)));
        let gap = actual.map(|a| expected - a);
        if let Some(g) = gap {
            if expected >= 0.5 && g > flat_threshold {
                warnings.push(format!(
                    "tension: `{}` is flat — actual {:.0}% vs expected {:.0}%",
                    b.beat,
                    actual.unwrap_or(0.0) * 100.0,
                    expected * 100.0
                ));
            }
        }
        points.push(TensionPoint {
            beat: b.beat.clone(),
            position,
            expected,
            actual,
            gap,
            ai,
        });
    }
    TensionCurve {
        points,
        series,
        ai_series,
        has_actual,
        has_ai,
        warnings,
    }
}

/// Render sorted `(position, value)` control points (values 0..1) as a
/// `width`-cell block-ramp sparkline (`▁`..`█`), linear-interpolating
/// between points and clamping past the ends. The structure outline's
/// tension overlay draws the expected + actual curves with this. Pure.
pub fn intensity_sparkline(points: &[(f32, f32)], width: usize) -> String {
    const RAMP: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let sample = |p: f32| -> f32 {
        if points.is_empty() {
            return 0.0;
        }
        if p <= points[0].0 {
            return points[0].1;
        }
        let last = points[points.len() - 1];
        if p >= last.0 {
            return last.1;
        }
        for w in points.windows(2) {
            let ((x0, y0), (x1, y1)) = (w[0], w[1]);
            if p >= x0 && p <= x1 {
                let f = if (x1 - x0).abs() < 1e-6 { 0.0 } else { (p - x0) / (x1 - x0) };
                return y0 + (y1 - y0) * f;
            }
        }
        last.1
    };
    (0..width)
        .map(|i| {
            let v = sample((i as f32 + 0.5) / width.max(1) as f32).clamp(0.0, 1.0);
            RAMP[((v * 7.0).round() as usize).min(7)]
        })
        .collect()
}

// ── built-in framework tables (positions monotonic non-decreasing) ──

const THREE_ACT: &[BeatSpec] = &[
    BeatSpec { name: "Opening", act: 1, target_position: 0.00, expected_tension: 0.10 },
    BeatSpec { name: "Inciting Incident", act: 1, target_position: 0.10, expected_tension: 0.35 },
    // Plot Point One launches act 2 (the act-1 turning point); Plot Point
    // Two launches act 3 — so the act boundaries land at 25% / 75% and the
    // expected word-share is the canonical 25 / 50 / 25.
    BeatSpec { name: "Plot Point One", act: 2, target_position: 0.25, expected_tension: 0.45 },
    BeatSpec { name: "First Pinch Point", act: 2, target_position: 0.375, expected_tension: 0.55 },
    BeatSpec { name: "Midpoint", act: 2, target_position: 0.50, expected_tension: 0.65 },
    BeatSpec { name: "Second Pinch Point", act: 2, target_position: 0.625, expected_tension: 0.75 },
    BeatSpec { name: "Plot Point Two", act: 3, target_position: 0.75, expected_tension: 0.85 },
    BeatSpec { name: "Climax", act: 3, target_position: 0.90, expected_tension: 1.00 },
    BeatSpec { name: "Resolution", act: 3, target_position: 1.00, expected_tension: 0.15 },
];

const SAVE_THE_CAT: &[BeatSpec] = &[
    BeatSpec { name: "Opening Image", act: 1, target_position: 0.00, expected_tension: 0.10 },
    BeatSpec { name: "Theme Stated", act: 1, target_position: 0.05, expected_tension: 0.15 },
    BeatSpec { name: "Set-Up", act: 1, target_position: 0.08, expected_tension: 0.20 },
    BeatSpec { name: "Catalyst", act: 1, target_position: 0.10, expected_tension: 0.35 },
    BeatSpec { name: "Debate", act: 1, target_position: 0.15, expected_tension: 0.30 },
    BeatSpec { name: "Break into Two", act: 2, target_position: 0.20, expected_tension: 0.40 },
    BeatSpec { name: "B Story", act: 2, target_position: 0.22, expected_tension: 0.35 },
    BeatSpec { name: "Fun and Games", act: 2, target_position: 0.30, expected_tension: 0.45 },
    BeatSpec { name: "Midpoint", act: 2, target_position: 0.50, expected_tension: 0.65 },
    BeatSpec { name: "Bad Guys Close In", act: 2, target_position: 0.62, expected_tension: 0.75 },
    BeatSpec { name: "All Is Lost", act: 2, target_position: 0.75, expected_tension: 0.90 },
    BeatSpec { name: "Dark Night of the Soul", act: 2, target_position: 0.77, expected_tension: 0.80 },
    BeatSpec { name: "Break into Three", act: 3, target_position: 0.80, expected_tension: 0.70 },
    BeatSpec { name: "Finale", act: 3, target_position: 0.90, expected_tension: 1.00 },
    BeatSpec { name: "Final Image", act: 3, target_position: 1.00, expected_tension: 0.15 },
];

const STORY_CIRCLE: &[BeatSpec] = &[
    BeatSpec { name: "You (comfort zone)", act: 1, target_position: 0.00, expected_tension: 0.10 },
    BeatSpec { name: "Need", act: 1, target_position: 0.125, expected_tension: 0.30 },
    BeatSpec { name: "Go (cross the threshold)", act: 2, target_position: 0.25, expected_tension: 0.45 },
    BeatSpec { name: "Search (adapt)", act: 2, target_position: 0.375, expected_tension: 0.55 },
    BeatSpec { name: "Find (get what they wanted)", act: 2, target_position: 0.50, expected_tension: 0.65 },
    BeatSpec { name: "Take (pay the price)", act: 2, target_position: 0.625, expected_tension: 0.85 },
    BeatSpec { name: "Return", act: 3, target_position: 0.75, expected_tension: 1.00 },
    BeatSpec { name: "Change", act: 3, target_position: 0.875, expected_tension: 0.25 },
];

const HERO_JOURNEY: &[BeatSpec] = &[
    BeatSpec { name: "Ordinary World", act: 1, target_position: 0.00, expected_tension: 0.10 },
    BeatSpec { name: "Call to Adventure", act: 1, target_position: 0.08, expected_tension: 0.35 },
    BeatSpec { name: "Refusal of the Call", act: 1, target_position: 0.12, expected_tension: 0.30 },
    BeatSpec { name: "Meeting the Mentor", act: 1, target_position: 0.17, expected_tension: 0.28 },
    BeatSpec { name: "Crossing the Threshold", act: 2, target_position: 0.25, expected_tension: 0.45 },
    BeatSpec { name: "Tests, Allies, Enemies", act: 2, target_position: 0.35, expected_tension: 0.50 },
    BeatSpec { name: "Approach to the Inmost Cave", act: 2, target_position: 0.45, expected_tension: 0.60 },
    BeatSpec { name: "The Ordeal", act: 2, target_position: 0.50, expected_tension: 0.80 },
    BeatSpec { name: "Reward", act: 2, target_position: 0.60, expected_tension: 0.45 },
    BeatSpec { name: "The Road Back", act: 3, target_position: 0.75, expected_tension: 0.65 },
    BeatSpec { name: "Resurrection", act: 3, target_position: 0.90, expected_tension: 1.00 },
    BeatSpec { name: "Return with the Elixir", act: 3, target_position: 1.00, expected_tension: 0.15 },
];

const SEVEN_POINT: &[BeatSpec] = &[
    BeatSpec { name: "Hook", act: 1, target_position: 0.00, expected_tension: 0.15 },
    BeatSpec { name: "Plot Turn One", act: 2, target_position: 0.25, expected_tension: 0.40 },
    BeatSpec { name: "Pinch Point One", act: 2, target_position: 0.375, expected_tension: 0.55 },
    BeatSpec { name: "Midpoint", act: 2, target_position: 0.50, expected_tension: 0.65 },
    BeatSpec { name: "Pinch Point Two", act: 2, target_position: 0.625, expected_tension: 0.80 },
    BeatSpec { name: "Plot Turn Two", act: 3, target_position: 0.75, expected_tension: 0.90 },
    // The seven-point Resolution *is* the climax-and-wrap, not a separate
    // denouement — so it carries peak tension, unlike three-act's Resolution.
    BeatSpec { name: "Resolution", act: 3, target_position: 1.00, expected_tension: 1.00 },
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
                assert!(
                    (0.0..=1.0).contains(&b.expected_tension),
                    "{}/{} expected_tension in range",
                    fw.slug(),
                    b.name
                );
                prev_pos = b.target_position;
                prev_act = b.act;
            }
            assert!(beats[0].target_position < 1e-6, "{} opens at 0", fw.slug());
            // The opening is calm and the dramatic peak sits in the back half
            // — the canonical rise-and-fall shape every framework encodes.
            let peak = beats
                .iter()
                .max_by(|a, b| a.expected_tension.partial_cmp(&b.expected_tension).unwrap())
                .unwrap();
            assert!((peak.expected_tension - 1.0).abs() < 1e-6, "{} peaks at 1.0", fw.slug());
            assert!(peak.target_position >= 0.6, "{} peak is in the back half", fw.slug());
            assert!(beats[0].expected_tension < 0.3, "{} opens calm", fw.slug());
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
        let r = analyze(&beats, &chapters, 0.10, &Default::default());
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
        let r = analyze(&Framework::ThreeAct.seed_beats(), &[], 0.10, &Default::default());
        let exp: Vec<f32> = r.acts.iter().map(|a| a.expected).collect();
        assert_eq!(exp.len(), 3);
        assert!((exp[0] - 0.25).abs() < 1e-6, "act1 25%");
        assert!((exp[1] - 0.50).abs() < 1e-6, "act2 50%");
        assert!((exp[2] - 0.25).abs() < 1e-6, "act3 25%");
        // Every framework: proportions sum to 1 and act 1 is a sane setup.
        for fw in Framework::ALL {
            let r = analyze(&fw.seed_beats(), &[], 0.10, &Default::default());
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
    fn analyze_surfaces_and_validates_thread_links() {
        let mut a = beat("A", 1, 0.0, Some("c1"));
        a.threads = vec!["the-inheritance".into()];
        let mut b = beat("B", 2, 0.5, Some("c2"));
        b.threads = vec!["ghost-thread".into()]; // not in Threads
        let c = beat("C", 3, 0.9, Some("c3")); // mapped, no thread
        let chapters = vec![
            ChapterPos { slug: "c1".into(), start: 0.0 },
            ChapterPos { slug: "c2".into(), start: 0.5 },
            ChapterPos { slug: "c3".into(), start: 0.9 },
        ];
        let known: std::collections::BTreeSet<String> =
            ["the-inheritance".to_string()].into_iter().collect();
        let r = analyze(&[a, b, c], &chapters, 0.10, &known);
        // surfaced on the status
        assert_eq!(r.beats[0].threads, vec!["the-inheritance"]);
        // unknown thread flagged
        assert_eq!(r.beats[1].unknown_threads, vec!["ghost-thread"]);
        assert!(r.warnings.iter().any(|w| w.contains("unknown thread `ghost-thread`")));
        // C is mapped with no thread, and the author IS using threads → nudge
        assert!(r.warnings.iter().any(|w| w.contains("advance no tracked thread")));
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
        let r = analyze(&beats, &chapters, 0.10, &Default::default());
        assert!(r.gaps.is_empty());
        assert!(r.warnings.is_empty(), "unexpected warnings: {:?}", r.warnings);
        assert!((r.acts.iter().find(|p| p.act == 2).unwrap().actual.unwrap() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn scaffold_prompt_and_parse() {
        let p = scaffold_user_prompt(Framework::ThreeAct, "A lighthouse keeper hides a body");
        assert!(p.contains("A lighthouse keeper hides a body"));
        assert!(p.contains("Three-Act"));
        assert!(p.contains("- Midpoint"));

        let beats = Framework::ThreeAct.seed_beats();
        let raw = "Opening: A quiet town wakes.\n\
                   1. Midpoint: The truth lands hard.\n\
                   Climax: They finally face it.\n\
                   Nonsense: ignore me\n\
                   Resolution:   ";
        let out = parse_scaffold(raw, &beats);
        assert!(out.iter().any(|(n, i)| n == "Opening" && i == "A quiet town wakes."));
        // the "1. " prefix is stripped before matching
        assert!(out.iter().any(|(n, i)| n == "Midpoint" && i == "The truth lands hard."));
        assert!(out.iter().any(|(n, _)| n == "Climax"));
        // a non-beat label is ignored; an empty intention is skipped
        assert!(!out.iter().any(|(n, _)| n == "Nonsense"));
        assert!(!out.iter().any(|(n, _)| n == "Resolution"));
    }

    #[test]
    fn analyze_prompt_carries_framework_and_context() {
        let p = analyze_user_prompt(Framework::SaveTheCat, "TITLE: X\nCHAPTER SUMMARIES:\n1. Foo", &[]);
        assert!(p.contains("Save the Cat"));
        assert!(p.contains("Midpoint (act 2, ~50%)"));
        assert!(p.contains("CHAPTER SUMMARIES:"));
        assert!(!p.contains("SCENE CARDS"), "no scene block when none supplied");
        assert!(!analyze_system_prompt().is_empty());
    }

    #[test]
    fn analyze_prompt_includes_scene_cards() {
        let scenes = vec![Scene::new_scene(
            "the-wharf",
            "Confrontation",
            "get the manifest",
            "he stonewalls",
            "",
        )];
        let p = analyze_user_prompt(Framework::ThreeAct, "TITLE: X", &scenes);
        assert!(p.contains("SCENE CARDS"));
        assert!(p.contains("Confrontation"));
        assert!(p.contains("get the manifest"));
        assert!(p.contains("disaster=—"), "empty disaster rendered as —");
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
        // a mapped beat round-trips its chapter slug, threads, and notes
        // (these were once hard-coded in beat_body — pin them).
        let mut mapped = mid.clone();
        mapped.mapped_chapter = Some("the-wharf".into());
        mapped.threads = vec!["the-inheritance".into(), "the-secret".into()];
        mapped.notes = "The truth lands; she can't unsee it.".into();
        let back = parse_beat(&beat_body(&mapped)).unwrap();
        assert_eq!(back.mapped_chapter.as_deref(), Some("the-wharf"));
        assert_eq!(back.threads, vec!["the-inheritance", "the-secret"]);
        assert_eq!(back.notes, "The truth lands; she can't unsee it.");
    }

    // ── tension curve (P0 1.3.4) ────────────────────────────────────

    #[test]
    fn open_load_counts_covering_spans() {
        let spans = vec![
            OpenSpan { start: 0.0, end: 0.5, weight: 1.0 },
            OpenSpan { start: 0.2, end: 0.8, weight: 1.0 },
            OpenSpan { start: 0.6, end: 1.0, weight: 2.0 },
        ];
        assert_eq!(open_load(&spans, 0.1), 1.0); // first only
        assert_eq!(open_load(&spans, 0.3), 2.0); // first + second
        assert_eq!(open_load(&spans, 0.7), 3.0); // second + third (weight 2)
        assert_eq!(open_load(&spans, 0.9), 2.0); // third only
        assert_eq!(open_load(&spans, 0.5), 1.0); // half-open: first closed at its end
    }

    #[test]
    fn expected_tension_resolves_from_framework_table() {
        assert!((expected_tension_for("three_act", "Climax") - 1.0).abs() < 1e-6);
        assert!(expected_tension_for("three_act", "Opening") < 0.3);
        // unknown framework / beat → neutral 0.5
        assert!((expected_tension_for("nope", "x") - 0.5).abs() < 1e-6);
        assert!((expected_tension_for("three_act", "Nonexistent") - 0.5).abs() < 1e-6);
    }

    fn tbeat(name: &str, target: f32, mapped: &str) -> Beat {
        Beat {
            framework: "three_act".into(),
            beat: name.into(),
            act: 2,
            target_position: target,
            mapped_chapter: Some(mapped.into()),
            threads: vec![],
            status: "planned".into(),
            notes: String::new(),
        }
    }

    #[test]
    fn tension_curve_flags_a_flat_high_beat() {
        let beats = vec![tbeat("Midpoint", 0.5, "mid"), tbeat("Climax", 0.9, "end")];
        let chapters = vec![
            ChapterPos { slug: "mid".into(), start: 0.5 },
            ChapterPos { slug: "end".into(), start: 0.9 },
        ];
        // heavy obligations only around the climax; the midpoint is empty
        let spans = vec![
            OpenSpan { start: 0.85, end: 1.0, weight: 1.0 },
            OpenSpan { start: 0.85, end: 1.0, weight: 1.0 },
            OpenSpan { start: 0.85, end: 1.0, weight: 1.0 },
        ];
        let curve = tension_curve(&beats, &chapters, &spans, &std::collections::BTreeMap::new(), 0.25);
        assert!(curve.has_actual);
        assert!(!curve.has_ai, "no AI ratings supplied");
        let mid = curve.points.iter().find(|p| p.beat == "Midpoint").unwrap();
        let climax = curve.points.iter().find(|p| p.beat == "Climax").unwrap();
        assert_eq!(mid.actual, Some(0.0), "no obligations open at the midpoint");
        assert_eq!(climax.actual, Some(1.0), "climax carries the normalized peak");
        // midpoint expected 0.65, actual 0.0 → gap 0.65 > 0.25 → flat
        assert!(curve.warnings.iter().any(|w| w.contains("Midpoint") && w.contains("flat")));
        assert!(!curve.warnings.iter().any(|w| w.contains("Climax")), "climax isn't flat");
    }

    #[test]
    fn intensity_sparkline_tracks_control_points() {
        // rising 0→1 → cells climb left to right, ending full.
        let rise = intensity_sparkline(&[(0.0, 0.0), (1.0, 1.0)], 8);
        let r: Vec<char> = rise.chars().collect();
        assert_eq!(r.len(), 8);
        assert!(r[0] < r[7], "rises left→right");
        assert_eq!(r[7], '█');
        // flat-zero → all the lowest cell.
        let flat = intensity_sparkline(&[(0.0, 0.0), (1.0, 0.0)], 5);
        assert!(flat.chars().all(|c| c == '▁'), "flat low: {flat}");
        // a peak in the middle reads as a peak.
        let peak = intensity_sparkline(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], 9);
        let p: Vec<char> = peak.chars().collect();
        assert!(p[4] > p[0] && p[4] > p[8], "peaks in the middle: {peak}");
        // empty control points don't panic.
        assert_eq!(intensity_sparkline(&[], 4).chars().count(), 4);
    }

    #[test]
    fn scene_body_round_trips() {
        let mut s = Scene::new_scene(
            "the-wharf",
            "Mara confronts the harbourmaster",
            "get the manifest",
            "he stonewalls",
            "he names her father as the debtor",
        );
        s.status = "drafted".into();
        let back = parse_scene(&scene_body(&s)).expect("parses");
        assert_eq!(back.kind, "scene");
        assert_eq!(back.chapter, "the-wharf");
        assert_eq!(back.goal, "get the manifest");
        assert_eq!(back.disaster, "he names her father as the debtor");
        assert_eq!(back.status, "drafted");
    }

    #[test]
    fn sequel_body_round_trips_and_weak_check_flips() {
        let seq = Scene::new_sequel(
            "the-wharf",
            "Mara reels",
            "she's gutted",
            "pay the debt or expose her father",
            "", // dilemma but no decision → stalls
        );
        let back = parse_scene(&scene_body(&seq)).expect("parses");
        assert!(back.is_sequel());
        assert_eq!(back.reaction, "she's gutted");
        assert!(back.goal.is_empty(), "sequel body carries no goal");
        let (st, warn) = analyze_scenes(&[seq]);
        assert!(st[0].weak, "dilemma + no decision → weak");
        assert!(warn.iter().any(|w| w.contains("never decides")));
    }

    #[test]
    fn alternation_flags_back_to_back_scenes_once_sequels_exist() {
        // scene, scene → the first's disaster is unprocessed, BUT only once a
        // sequel is in use anywhere.
        let scene_only = vec![
            Scene::new_scene("c1", "A", "g", "c", "d"),
            Scene::new_scene("c2", "B", "g", "c", "d"),
        ];
        let (_, w0) = analyze_scenes(&scene_only);
        assert!(!w0.iter().any(|w| w.contains("rhythm")), "no nag without sequels");
        let mixed = vec![
            Scene::new_scene("c1", "A", "g", "c", "d"),
            Scene::new_scene("c2", "B", "g", "c", "d"),
            Scene::new_sequel("c2", "B-after", "r", "dl", "de"),
        ];
        let (_, w1) = analyze_scenes(&mixed);
        assert!(w1.iter().any(|w| w.contains("rhythm") && w.contains("`A`")));
    }

    #[test]
    fn parse_scene_scaffold_extracts_the_triple() {
        let raw = "goal: reach the harbourmaster before dusk\n\
                   conflict: the ledger is missing a page\n\
                   disaster: the page names her father";
        let (g, c, d) = parse_scene_scaffold(raw);
        assert_eq!(g, "reach the harbourmaster before dusk");
        assert_eq!(c, "the ledger is missing a page");
        assert_eq!(d, "the page names her father");
        // tolerant of list markers / bold / preamble / case
        let messy = "Here is the scene:\n- **Goal:** find the will\n* Conflict: the room is locked\nDISASTER: it's already gone";
        let (g2, c2, d2) = parse_scene_scaffold(messy);
        assert_eq!(g2, "find the will");
        assert_eq!(c2, "the room is locked");
        assert_eq!(d2, "it's already gone");
        // a missing field comes back empty
        let (_, _, d3) = parse_scene_scaffold("goal: x\nconflict: y");
        assert!(d3.is_empty());
    }

    #[test]
    fn analyze_scenes_flags_a_scene_that_doesnt_turn() {
        let scenes = vec![
            Scene::new_scene("c1", "Turns", "find the letter", "the room is locked", "it's gone"),
            // goal but no disaster → no turn
            Scene::new_scene("c2", "Flat", "win the argument", "", ""),
        ];
        let (st, warn) = analyze_scenes(&scenes);
        assert_eq!(st.len(), 2);
        assert!(!st[0].weak, "a scene with a disaster turns");
        assert!(st[1].weak, "goal + no disaster → flat");
        assert_eq!(warn.len(), 1);
        assert!(warn[0].contains("Flat") && warn[0].contains("never turns"));
    }

    #[test]
    fn tension_curve_no_data_is_expected_only() {
        let beats = vec![tbeat("Midpoint", 0.5, "mid")];
        let chapters = vec![ChapterPos { slug: "mid".into(), start: 0.5 }];
        let curve = tension_curve(&beats, &chapters, &[], &std::collections::BTreeMap::new(), 0.25);
        assert!(!curve.has_actual, "no spans → no actual curve");
        assert_eq!(curve.points[0].actual, None);
        assert!(curve.points[0].expected > 0.6, "expected still resolves from the table");
        assert!(curve.warnings.is_empty(), "no flat flags without data");
    }

    #[test]
    fn parse_intensity_reads_the_first_integer() {
        assert_eq!(parse_intensity("72"), Some(0.72));
        assert_eq!(parse_intensity("Intensity: 90/100"), Some(0.90));
        assert_eq!(parse_intensity("0"), Some(0.0));
        assert_eq!(parse_intensity("250"), Some(1.0), "clamped to 100");
        assert_eq!(parse_intensity("no number here"), None);
    }

    #[test]
    fn tension_curve_carries_the_ai_second_opinion() {
        let beats = vec![tbeat("Midpoint", 0.5, "mid")];
        let chapters = vec![
            ChapterPos { slug: "mid".into(), start: 0.5 },
            ChapterPos { slug: "end".into(), start: 0.9 },
        ];
        let mut ai = std::collections::BTreeMap::new();
        ai.insert("mid".to_string(), 0.3f32);
        ai.insert("end".to_string(), 0.95f32);
        let curve = tension_curve(&beats, &chapters, &[], &ai, 0.25);
        assert!(curve.has_ai);
        assert_eq!(curve.ai_series.len(), 2, "one point per rated chapter");
        let mid = &curve.points[0];
        assert_eq!(mid.ai, Some(0.3), "the beat picks up its chapter's AI rating");
    }
}
