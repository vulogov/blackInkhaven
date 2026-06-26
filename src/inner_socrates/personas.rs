//! Reader Personas (RFC §3.6 / §8.17) — the distinct careful-reader perspectives
//! the author switches between. Five bundled personas ship; users add their own as
//! HJSON files. Loading is two-level (project > user > bundled), and the active
//! persona is per-project state. The persona's category **emphasis weights** scale
//! salience (`0.0` mutes a category) and feed the Slow prompt's voice section.
//!
//! The bundled personas are embedded here (zero external assets); user/project
//! personas parse from HJSON via [`parse_persona`].

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::types::{Category, Persona};
use super::{Result, SocratesError};

/// The on-disk persona format (HJSON). `emphasis` keys are category ids.
#[derive(Debug, Deserialize)]
struct PersonaFile {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    voice_summary: String,
    #[serde(default)]
    voice_notes: String,
    #[serde(default)]
    emphasis: HashMap<String, f32>,
}

/// Parse a persona from an HJSON string. Unknown emphasis keys are ignored.
pub fn parse_persona(body: &str) -> Result<Persona> {
    let f: PersonaFile =
        serde_hjson::from_str(body).map_err(|e| SocratesError::Parse(e.to_string()))?;
    if f.id.trim().is_empty() {
        return Err(SocratesError::Parse("persona has no id".into()));
    }
    let emphasis = f
        .emphasis
        .into_iter()
        .filter_map(|(k, v)| Category::from_id(&k).map(|c| (c, v)))
        .collect();
    Ok(Persona {
        id: f.id,
        name: f.name,
        description: f.description,
        voice_summary: f.voice_summary,
        voice_notes: f.voice_notes,
        emphasis,
    })
}

/// The five bundled personas, each calibrated (per the RFC character sheets) to
/// produce a meaningfully different reading.
pub fn bundled() -> Vec<Persona> {
    use Category::*;
    let p = |id: &str, name: &str, summary: &str, notes: &str, emph: &[(Category, f32)]| Persona {
        id: id.into(),
        name: name.into(),
        description: summary.into(),
        voice_summary: summary.into(),
        voice_notes: notes.into(),
        emphasis: emph.iter().copied().collect(),
    };
    vec![
        p(
            "inner-socrates",
            "Inner Socrates",
            "Every question opens what the prose has closed.",
            "You are a classical interrogator. Brief, direct, never reassuring, never prescriptive. \
             You ask what the prose presupposes, what alternatives it closed off, what it leaves out. \
             You respect the author enough to question rather than approve.",
            &[(AssumptionSurfacing, 1.2), (FramingInterrogation, 1.1), (SignificanceProbing, 1.1)],
        ),
        p(
            "careful-editor",
            "The Careful Editor",
            "Notice what the prose is doing — to itself and to the reader's attention.",
            "You read with precision and a gentle, questioning attention to clarity and momentum. \
             You notice when the prose is doing work invisibly. You acknowledge craft while asking \
             about it — a little warmer than Socrates, never less exact.",
            &[(TensionDetection, 1.2), (ImplicitComparison, 1.1), (ModalClaims, 1.3)],
        ),
        p(
            "skeptical-reader",
            "The Skeptical Reader",
            "What's not being said is often louder than what is.",
            "You are probing and slightly adversarial, but fair. You are attentive to omissions and \
             unexamined framings; you refuse the prose's framing at face value and press on what it \
             leaves out — without arguing, without prescribing.",
            &[(AssumptionSurfacing, 1.3), (FramingInterrogation, 1.3), (DramatizationGap, 1.2)],
        ),
        p(
            "first-time-reader",
            "The First-Time Reader",
            "Pretend you've read nothing of this book before this scene.",
            "You are curious and occasionally confused. You are attentive to what the prose assumes \
             about prior knowledge, treating each scene as if encountered for the first time, without \
             context the author has but a new reader does not.",
            &[(AssumptionSurfacing, 1.1), (FramingInterrogation, 0.9), (ImplicitComparison, 0.7)],
        ),
        p(
            "slow-reader",
            "The Slow Reader",
            "The rhythm of prose is doing something. What?",
            "You read at the level of sentence and paragraph, attentive to rhythm, texture, and \
             recurrence — repeated words, patterns of cadence, scenes that echo earlier scenes. You \
             notice how the prose moves rather than only what it claims.",
            &[(StructuralPatterns, 1.3), (SentenceLengthAnomalies, 1.2), (TemporalDensity, 1.3), (ImplicitComparison, 1.2)],
        ),
        // ── Nonfiction personas (1.4.6 AUDIENCE-1) ──
        // All four mute DramatizationGap + TemporalDensity (narrative/scene-time,
        // meaningless in expository prose) and UnattributedDialogue (no dialogue
        // runs). Absent categories default to 1.0, so the mutes are explicit 0.0.
        p(
            "skeptical-practitioner",
            "The Skeptical Practitioner",
            "Every procedure is a reproduction attempt; what did you leave out?",
            "You are a practising engineer who has been burned by incomplete documentation. \
             You read technical prose as a reproduction attempt: every procedure must be \
             complete, every claim must be testable, every assumption must be surfaced. \
             You ask about what is omitted — the error case not mentioned, the prerequisite \
             silently assumed, the configuration value not explained. You never correct; \
             you ask the author what they decided to leave out and why.",
            &[
                (AssumptionSurfacing, 1.4),
                (FramingInterrogation, 1.3),
                (SignificanceProbing, 1.2),
                (ImplicitComparison, 1.1),
                (ModalClaims, 1.2),
                (DramatizationGap, 0.0),
                (TemporalDensity, 0.0),
                (UnattributedDialogue, 0.0),
            ],
        ),
        p(
            "domain-newcomer",
            "The Domain Newcomer",
            "Every undefined term is a door that won't open for me.",
            "You are a careful, motivated reader encountering this subject for the first \
             time. You have no prior knowledge except what the prose has built. Every \
             undefined term is a door that won't open for you. Every concept introduced by \
             example without definition is a guess you must make. You ask about what the \
             author assumed you already know — not to criticise, but to find the place where \
             you, the newcomer, would stop following.",
            &[
                (AssumptionSurfacing, 1.5),
                (SignificanceProbing, 1.1),
                (FramingInterrogation, 1.0),
                (StructuralPatterns, 1.1),
                (ModalClaims, 0.9),
                (TensionDetection, 0.4),
                (DramatizationGap, 0.0),
                (TemporalDensity, 0.0),
                (UnattributedDialogue, 0.0),
            ],
        ),
        p(
            "expert-reviewer",
            "The Expert Reviewer",
            "Does the evidence support the claim, and is the scope stated?",
            "You are a peer reviewer for an academic or technical publication. You are a \
             domain expert who reads for logical rigour: the evidence must support the \
             claim, the scope must be stated, the limitations must be acknowledged. You \
             ask about assertions made without support, comparisons made without stated \
             criteria, causal language applied to correlational evidence, and conclusions \
             that exceed what the evidence establishes. You never suggest how to fix — \
             you ask the question the author must answer before publication.",
            &[
                (AssumptionSurfacing, 1.3),
                (FramingInterrogation, 1.5),
                (SignificanceProbing, 1.3),
                (ImplicitComparison, 1.4),
                (ModalClaims, 1.4),
                (HedgedUncertainty, 1.2),
                (TensionDetection, 0.2),
                (DramatizationGap, 0.0),
                (TemporalDensity, 0.0),
                (UnattributedDialogue, 0.0),
            ],
        ),
        p(
            "end-user",
            "The End User",
            "What do I do next, and how will I know when I'm done?",
            "You are a user following this documentation to complete a task. You are not \
             reading to learn the theory — you need to know what to do, in what order, \
             and what success looks like. You ask where the next step is, what to do when \
             the stated outcome does not happen, whether this step's output is the next \
             step's input, and whether you will know when you are done. You never critique \
             the writing — you ask whether you could follow it.",
            &[
                (SignificanceProbing, 1.5),
                (AssumptionSurfacing, 1.3),
                (StructuralPatterns, 1.3),
                (FramingInterrogation, 1.1),
                (ImplicitComparison, 0.5),
                (TensionDetection, 0.0),
                (DramatizationGap, 0.0),
                (TemporalDensity, 0.0),
                (UnattributedDialogue, 0.0),
            ],
        ),
    ]
}

/// All available personas for a project: bundled, overlaid by user-level
/// (`~/.config/inkhaven/personas/`), overlaid by project-level
/// (`<project>/books/intent/01-personas/`). Later levels win by `id`.
pub fn load_all(project: &Path) -> Vec<Persona> {
    let mut by_id: indexmap_lite::OrderMap = indexmap_lite::OrderMap::new();
    for p in bundled() {
        by_id.insert(p.id.clone(), p);
    }
    for dir in [super::user_config::personas_dir(), Some(project_personas_dir(project))] {
        let Some(dir) = dir else { continue };
        for p in load_dir(&dir) {
            by_id.insert(p.id.clone(), p);
        }
    }
    by_id.into_values()
}

/// Project-level persona directory.
pub fn project_personas_dir(project: &Path) -> std::path::PathBuf {
    project.join("books").join("intent").join("01-personas")
}

/// Parse every `*.hjson` persona file in a directory (skips unreadable / invalid).
fn load_dir(dir: &Path) -> Vec<Persona> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("hjson") {
            continue;
        }
        if let Ok(body) = std::fs::read_to_string(&path) {
            if let Ok(p) = parse_persona(&body) {
                out.push(p);
            }
        }
    }
    out
}

/// Look up one persona by id from the loaded set (falls back to the default).
pub fn by_id(project: &Path, id: &str) -> Persona {
    load_all(project).into_iter().find(|p| p.id == id).unwrap_or_else(Persona::default_inner_socrates)
}

/// The active persona for a project: the one the store records as active; else
/// the project config's `inner_socrates_default_persona` (AUDIENCE-1, e.g. a
/// technical book defaulting to `skeptical-practitioner`); else the bundled
/// `inner-socrates`. An explicit `set_active_persona` always wins over config.
pub fn active(project: &Path) -> Persona {
    let id = super::storage::InnerSocratesStore::open_for_project(project)
        .ok()
        .and_then(|s| s.active_persona_id().ok().flatten());
    match id {
        Some(id) => by_id(project, &id),
        None => match config_default_persona(project) {
            Some(id) => by_id(project, &id),
            None => Persona::default_inner_socrates(),
        },
    }
}

/// The `inner_socrates_default_persona` config value for a project, trimmed and
/// non-empty, if set. Best-effort — a missing / unreadable config yields `None`.
/// Public so the TUI overview can flag when the active persona came from config.
pub fn config_default_persona(project: &Path) -> Option<String> {
    crate::config::Config::load_layered(
        &crate::project::ProjectLayout::new(project).config_path(),
    )
    .ok()
    .and_then(|c| c.inner_socrates_default_persona)
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

/// A tiny insertion-ordered map (avoids a new dependency on `indexmap`).
mod indexmap_lite {
    use super::Persona;

    pub struct OrderMap {
        order: Vec<String>,
        map: std::collections::HashMap<String, Persona>,
    }
    impl OrderMap {
        pub fn new() -> Self {
            Self { order: Vec::new(), map: std::collections::HashMap::new() }
        }
        pub fn insert(&mut self, k: String, v: Persona) {
            if !self.map.contains_key(&k) {
                self.order.push(k.clone());
            }
            self.map.insert(k, v);
        }
        pub fn into_values(mut self) -> Vec<Persona> {
            self.order.iter().filter_map(|k| self.map.remove(k)).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_distinct_bundled_personas() {
        let ps = bundled();
        // 5 fiction (1.3.x) + 4 nonfiction (1.4.6 AUDIENCE-1).
        assert_eq!(ps.len(), 9);
        let ids: std::collections::BTreeSet<_> = ps.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), 9);
        assert!(ids.contains("inner-socrates"));
        // Personas weight different categories (they read differently).
        let socr = ps.iter().find(|p| p.id == "inner-socrates").unwrap();
        let slow = ps.iter().find(|p| p.id == "slow-reader").unwrap();
        assert!(socr.emphasis_for(Category::AssumptionSurfacing) > 1.0);
        assert!(slow.emphasis_for(Category::StructuralPatterns) > 1.0);
        assert!(socr.emphasis_for(Category::StructuralPatterns) < slow.emphasis_for(Category::StructuralPatterns));
    }

    #[test]
    fn nonfiction_personas_mute_narrative_categories() {
        let ps = bundled();
        // The four nonfiction personas exist…
        for id in ["skeptical-practitioner", "domain-newcomer", "expert-reviewer", "end-user"] {
            let p = ps.iter().find(|p| p.id == id)
                .unwrap_or_else(|| panic!("missing nonfiction persona `{id}`"));
            // …and each mutes the fiction-only categories.
            assert!(p.mutes(Category::DramatizationGap), "{id} should mute DramatizationGap");
            assert!(p.mutes(Category::TemporalDensity), "{id} should mute TemporalDensity");
            assert!(p.mutes(Category::UnattributedDialogue), "{id} should mute UnattributedDialogue");
        }
    }

    #[test]
    fn nonfiction_persona_emphasis_matches_character_sheets() {
        let ps = bundled();
        let get = |id: &str| ps.iter().find(|p| p.id == id).unwrap().clone();
        // skeptical-practitioner leans hardest on surfacing what's omitted.
        let prac = get("skeptical-practitioner");
        assert_eq!(prac.emphasis_for(Category::AssumptionSurfacing), 1.4);
        assert_eq!(prac.emphasis_for(Category::FramingInterrogation), 1.3);
        // domain-newcomer: assumptions about the reader are paramount.
        let newc = get("domain-newcomer");
        assert_eq!(newc.emphasis_for(Category::AssumptionSurfacing), 1.5);
        assert!(newc.emphasis_for(Category::TensionDetection) < 1.0); // attenuated, not muted
        // expert-reviewer: scope precision dominates.
        let rev = get("expert-reviewer");
        assert_eq!(rev.emphasis_for(Category::FramingInterrogation), 1.5);
        assert_eq!(rev.emphasis_for(Category::ImplicitComparison), 1.4);
        // end-user: "what does this step accomplish?" leads.
        let usr = get("end-user");
        assert_eq!(usr.emphasis_for(Category::SignificanceProbing), 1.5);
        assert!(usr.mutes(Category::TensionDetection)); // task-followers don't read for tension
    }

    #[test]
    fn parses_a_persona_file() {
        let body = r#"{
            id: "my-grandmother"
            name: "My Skeptical Grandmother"
            voice_summary: "She has read everything and believes none of it."
            emphasis: {
                framing_interrogation: 1.5
                hedged_uncertainty: 0.0
                not_a_real_category: 2.0
            }
        }"#;
        let p = parse_persona(body).unwrap();
        assert_eq!(p.id, "my-grandmother");
        assert_eq!(p.emphasis_for(Category::FramingInterrogation), 1.5);
        assert!(p.mutes(Category::HedgedUncertainty));
        // Unknown emphasis keys are dropped.
        assert_eq!(p.emphasis.len(), 2);
    }

    #[test]
    fn rejects_persona_without_id() {
        assert!(parse_persona(r#"{ name: "Nameless" }"#).is_err());
    }

    // ── AUDIENCE-1: config-default persona resolution ──

    #[test]
    fn active_falls_back_to_config_default_then_inner_socrates() {
        // No config, no DB row → bundled inner-socrates.
        let bare = tempfile::tempdir().unwrap();
        assert_eq!(active(bare.path()).id, "inner-socrates");

        // Config default set, no explicit DB row → the config default persona.
        let cfg_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            cfg_dir.path().join("inkhaven.hjson"),
            "{\n  inner_socrates_default_persona: skeptical-practitioner\n}\n",
        )
        .unwrap();
        assert_eq!(active(cfg_dir.path()).id, "skeptical-practitioner");
        assert_eq!(
            config_default_persona(cfg_dir.path()).as_deref(),
            Some("skeptical-practitioner")
        );
    }

    #[test]
    fn explicit_active_persona_beats_config_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("inkhaven.hjson"),
            "{\n  inner_socrates_default_persona: skeptical-practitioner\n}\n",
        )
        .unwrap();
        // An explicit `persona set` writes the DB singleton…
        super::super::storage::InnerSocratesStore::open_for_project(dir.path())
            .unwrap()
            .set_active_persona("expert-reviewer")
            .unwrap();
        // …and wins over the config default.
        assert_eq!(active(dir.path()).id, "expert-reviewer");
    }
}
