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

/// The active persona for a project: the one the store records as active, else the
/// default (Inner Socrates).
pub fn active(project: &Path) -> Persona {
    let id = super::storage::InnerSocratesStore::open_for_project(project)
        .ok()
        .and_then(|s| s.active_persona_id().ok().flatten());
    match id {
        Some(id) => by_id(project, &id),
        None => Persona::default_inner_socrates(),
    }
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
    fn five_distinct_bundled_personas() {
        let ps = bundled();
        assert_eq!(ps.len(), 5);
        let ids: std::collections::BTreeSet<_> = ps.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), 5);
        assert!(ids.contains("inner-socrates"));
        // Personas weight different categories (they read differently).
        let socr = ps.iter().find(|p| p.id == "inner-socrates").unwrap();
        let slow = ps.iter().find(|p| p.id == "slow-reader").unwrap();
        assert!(socr.emphasis_for(Category::AssumptionSurfacing) > 1.0);
        assert!(slow.emphasis_for(Category::StructuralPatterns) > 1.0);
        assert!(socr.emphasis_for(Category::StructuralPatterns) < slow.emphasis_for(Category::StructuralPatterns));
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
}
