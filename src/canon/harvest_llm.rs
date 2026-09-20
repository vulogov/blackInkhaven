//! CL-P7 — opt-in LLM harvest of prose into *proposed* canon decisions.
//!
//! This is the one canon path that consults a model, and it obeys the
//! AI-advisory rule: the model **proposes**, nothing enters the ledger. A
//! harvest writes candidates to a staging sidecar (`.inkhaven/canon-staged.json`);
//! the author reviews (`canon staged`) and **confirms** (`canon accept`), which
//! is the only thing that calls `record_decision`. Proposed decisions enter
//! uncommitted — the author commits what they accept (CL-P4).
//!
//! This module holds the deterministic, testable pieces — the staging file, the
//! multilingual prompt, and the response parser. The model call itself
//! (`collect_blocking`) and the paragraph walk live in the CLI handler, which
//! has the store, config, and AI client.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::project::ProjectLayout;

use super::model::NarrativeKind;

/// A model-proposed canon decision, awaiting the author's confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub node: Uuid,
    pub breadcrumb: String,
    pub kind: NarrativeKind,
    pub gist: String,
}

/// The staging sidecar: proposals harvested but not yet in the ledger.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StagedCanon {
    pub proposals: Vec<Proposal>,
}

impl StagedCanon {
    fn path(layout: &ProjectLayout) -> std::path::PathBuf {
        layout.root.join(".inkhaven").join("canon-staged.json")
    }

    /// Load the sidecar, or an empty set when absent.
    pub fn load(layout: &ProjectLayout) -> Result<StagedCanon> {
        let p = Self::path(layout);
        if !p.is_file() {
            return Ok(StagedCanon::default());
        }
        let bytes = std::fs::read(&p).map_err(|e| anyhow!("read {p:?}: {e}"))?;
        serde_json::from_slice(&bytes).map_err(|e| anyhow!("parse {p:?}: {e}"))
    }

    /// Persist the sidecar atomically (creating `.inkhaven/` if needed).
    pub fn save(&self, layout: &ProjectLayout) -> Result<()> {
        let p = Self::path(layout);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| anyhow!("create {dir:?}: {e}"))?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(|e| anyhow!("serialize staged: {e}"))?;
        crate::io_atomic::write(&p, &json).map_err(|e| anyhow!("write {p:?}: {e}"))
    }

    /// Clear the sidecar (after the proposals are accepted or discarded).
    pub fn clear(layout: &ProjectLayout) -> Result<()> {
        StagedCanon::default().save(layout)
    }
}

/// The project-language name for the in-language extraction directive
/// (`Other`/unknown → English).
pub fn language_name(lang: &crate::prose::ProseLanguage) -> &'static str {
    use crate::prose::ProseLanguage::*;
    match lang {
        En => "English",
        Ru => "Russian",
        De => "German",
        Fr => "French",
        Es => "Spanish",
        Other(_) => "English",
    }
}

/// The extraction system prompt. Multilingual: gists are written in the project
/// `language` and never silently translated. Output is a strict JSON array so the
/// parse is deterministic.
pub fn system_prompt(language: &str) -> String {
    format!(
        "You extract the load-bearing narrative DECISIONS a passage establishes for a \
         story bible — not a summary, only decisions the passage actually commits to.\n\n\
         Output ONLY a JSON array, no prose around it. Each item:\n\
         {{ \"kind\": one of \"world-fact\" | \"character-trait\" | \"plot-point\" | \"reveal\" | \"setup\", \
         \"gist\": a single declarative sentence in {language} }}\n\n\
         Rules:\n\
         - Extract only what the passage establishes; do NOT infer or invent. If it establishes \
         nothing, output [].\n\
         - Write every gist in {language}. Do NOT translate to English.\n\
         - Keep each gist to one sentence, no citation, no commentary."
    )
}

/// Parse the model's JSON array into proposals for `node`/`breadcrumb`. Tolerant
/// of surrounding text (slices to the outer `[ … ]`); skips items with an unknown
/// kind or empty gist. Returns an empty vec on unparseable output rather than
/// erroring — a bad response for one paragraph must not abort a harvest.
pub fn parse_proposals(raw: &str, node: Uuid, breadcrumb: &str) -> Vec<Proposal> {
    #[derive(Deserialize)]
    struct RawItem {
        kind: String,
        gist: String,
    }
    let slice = match (raw.find('['), raw.rfind(']')) {
        (Some(a), Some(b)) if b > a => &raw[a..=b],
        _ => return Vec::new(),
    };
    let items: Vec<RawItem> = match serde_json::from_str(slice) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    items
        .into_iter()
        .filter_map(|it| {
            let kind = NarrativeKind::from_short(&it.kind)?;
            let gist = it.gist.trim();
            if gist.is_empty() {
                return None;
            }
            Some(Proposal {
                node,
                breadcrumb: breadcrumb.to_string(),
                kind,
                gist: gist.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_extracts_known_kinds_skips_junk() {
        let raw = r#"Here is the JSON:
        [
          {"kind": "world-fact", "gist": "The harbour freezes each winter."},
          {"kind": "reveal", "gist": "The keeper is the lost heir."},
          {"kind": "nonsense", "gist": "ignored — unknown kind"},
          {"kind": "setup", "gist": "   "}
        ]
        trailing chatter"#;
        let node = Uuid::from_u128(0x1);
        let got = parse_proposals(raw, node, "ch1/scene1");
        assert_eq!(got.len(), 2, "known kinds with non-empty gists only");
        assert_eq!(got[0].kind, NarrativeKind::WorldFact);
        assert_eq!(got[1].kind, NarrativeKind::Reveal);
        assert!(got.iter().all(|p| p.node == node && p.breadcrumb == "ch1/scene1"));
    }

    #[test]
    fn parse_tolerates_empty_and_garbage() {
        let n = Uuid::from_u128(0x2);
        assert!(parse_proposals("[]", n, "x").is_empty());
        assert!(parse_proposals("the model refused", n, "x").is_empty());
        assert!(parse_proposals("", n, "x").is_empty());
    }

    #[test]
    fn system_prompt_is_multilingual() {
        let p = system_prompt("Russian");
        assert!(p.contains("Russian") && p.contains("Do NOT translate"));
        assert!(p.contains("world-fact") && p.contains("JSON array"));
    }

    #[test]
    fn staging_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let layout = ProjectLayout::new(dir.path());
        assert!(StagedCanon::load(&layout).unwrap().proposals.is_empty());

        let staged = StagedCanon {
            proposals: vec![Proposal {
                node: Uuid::from_u128(0x9),
                breadcrumb: "ch2".into(),
                kind: NarrativeKind::PlotPoint,
                gist: "the escape must wait for the thaw".into(),
            }],
        };
        staged.save(&layout).unwrap();
        let back = StagedCanon::load(&layout).unwrap();
        assert_eq!(back.proposals.len(), 1);
        assert_eq!(back.proposals[0].kind, NarrativeKind::PlotPoint);

        StagedCanon::clear(&layout).unwrap();
        assert!(StagedCanon::load(&layout).unwrap().proposals.is_empty());
    }
}
