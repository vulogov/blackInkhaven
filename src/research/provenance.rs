//! RESRCH-2.1 (T-P1) — fact provenance. A `.inkhaven/fact-sources.json` sidecar
//! mapping each inserted Facts node to where it came from: the model, a manual
//! entry, or a promoted note (web / document origins are reserved for the
//! external-retrieval cuts, R2-B/C). The keystone of the "trust" release — once
//! facts can come from outside, "where did this come from?" must already exist.
//!
//! Plain serde JSON over the established `.inkhaven/` sidecar pattern (like
//! `research/thread.rs`); no DuckDB. Timestamps are RFC3339 strings.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::project::ProjectLayout;

/// One fact's source record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SourceRecord {
    /// `model` | `manual` | `promoted` (open string; R2-B/C add `web` / `document`).
    pub origin: String,
    /// Origin-specific detail: the notes path for `promoted`; a URL / file later.
    #[serde(default)]
    pub detail: String,
    /// The research query the fact was extracted from (empty for manual).
    #[serde(default)]
    pub query: String,
    /// The thread the fact was inserted from.
    #[serde(default)]
    pub thread: String,
    pub created_at: String,
}

impl SourceRecord {
    pub(super) fn new(origin: &str, detail: &str, query: &str, thread: &str, now: String) -> SourceRecord {
        SourceRecord {
            origin: origin.to_string(),
            detail: detail.to_string(),
            query: query.to_string(),
            thread: thread.to_string(),
            created_at: now,
        }
    }

    /// A one-line human summary for the overlay / `/sources` report.
    pub(super) fn summary(&self) -> String {
        match self.origin.as_str() {
            "web" if !self.detail.is_empty() => format!("web: {}", self.detail),
            "document" if !self.detail.is_empty() => format!("document: {}", self.detail),
            "model" if !self.query.is_empty() => format!("model · from query: {}", self.query),
            "promoted" if !self.detail.is_empty() => format!("promoted from note: {}", self.detail),
            "manual" => "manual entry".to_string(),
            other => other.to_string(),
        }
    }
}

/// The provenance sidecar: fact-node-id → source record.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct Provenance {
    #[serde(default)]
    pub facts: BTreeMap<String, SourceRecord>,
}

impl Provenance {
    fn path(layout: &ProjectLayout) -> std::path::PathBuf {
        layout.root.join(".inkhaven").join("fact-sources.json")
    }

    /// Load the sidecar, or an empty map when absent / unreadable.
    pub(super) fn load(layout: &ProjectLayout) -> Provenance {
        let p = Provenance::path(layout);
        match std::fs::read_to_string(p) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Provenance::default(),
        }
    }

    fn save(&self, layout: &ProjectLayout) -> Result<()> {
        let dir = layout.root.join(".inkhaven");
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let json = serde_json::to_string_pretty(self).context("serialise provenance")?;
        crate::io_atomic::write(&Provenance::path(layout), json.as_bytes())
            .context("write fact-sources.json")?;
        Ok(())
    }

    /// Record (or overwrite) a fact's source and persist.
    pub(super) fn record(layout: &ProjectLayout, node_id: &str, rec: SourceRecord) {
        let mut prov = Provenance::load(layout);
        prov.facts.insert(node_id.to_string(), rec);
        let _ = prov.save(layout);
    }

    /// The source record for a node, if any.
    pub(super) fn for_node(&self, node_id: &str) -> Option<&SourceRecord> {
        self.facts.get(node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_layout(tag: &str) -> ProjectLayout {
        let dir = std::env::temp_dir().join(format!("resrch-prov-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        ProjectLayout::new(dir)
    }

    #[test]
    fn record_and_load_roundtrip() {
        let layout = tmp_layout("roundtrip");
        Provenance::record(
            &layout,
            "node-1",
            SourceRecord::new("model", "", "why is the sky green?", "rome", "2026-07-01T10:00:00Z".into()),
        );
        let prov = Provenance::load(&layout);
        let rec = prov.for_node("node-1").unwrap();
        assert_eq!(rec.origin, "model");
        assert_eq!(rec.query, "why is the sky green?");
        assert!(rec.summary().contains("from query"));
        assert!(prov.for_node("missing").is_none());
    }

    #[test]
    fn summaries_by_origin() {
        let m = SourceRecord::new("manual", "", "", "t", "now".into());
        assert_eq!(m.summary(), "manual entry");
        let p = SourceRecord::new("promoted", "notes/rome/idea", "", "t", "now".into());
        assert!(p.summary().contains("notes/rome/idea"));
        let d = SourceRecord::new("document", "rome-aqueducts", "", "t", "now".into());
        assert!(d.summary().contains("document: rome-aqueducts"));
    }
}
