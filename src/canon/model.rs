//! CL-P1 — the narrative unit model and the host-node bridge.
//!
//! A canon decision is a smysl unit under an `x.narrative/…` extension schema
//! (retrievable since smysl 1.7 indexes extension-schema units), carrying a
//! `SourceKind::Node` back-reference to the inkhaven paragraph it was derived
//! from: `inkhaven:<node-uuid>#<breadcrumb>`. The epistemic `Status` is inert for
//! fiction — a plot decision is not *measured* — so narrative units sit at
//! `Speculative`; canonicity rides the separate commitment axis (CL-P4).

use smysl::{SchemaId, SourceKind, SourceRef};
use uuid::Uuid;

/// The kinds of load-bearing decision the ledger records, each an
/// `x.narrative/<kind>` extension schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NarrativeKind {
    /// A fact about the world (`x.narrative/world-fact`).
    WorldFact,
    /// A trait or standing fact about a character (`x.narrative/character-trait`).
    CharacterTrait,
    /// A plot commitment or beat (`x.narrative/plot-point`).
    PlotPoint,
    /// Something the reader is meant to learn (`x.narrative/reveal`).
    Reveal,
    /// Groundwork a later reveal or payoff depends on (`x.narrative/setup`).
    Setup,
}

impl NarrativeKind {
    pub const ALL: [NarrativeKind; 5] = [
        NarrativeKind::WorldFact,
        NarrativeKind::CharacterTrait,
        NarrativeKind::PlotPoint,
        NarrativeKind::Reveal,
        NarrativeKind::Setup,
    ];

    /// The extension-schema string (`x.narrative/…`).
    pub fn schema_str(self) -> &'static str {
        match self {
            NarrativeKind::WorldFact => "x.narrative/world-fact",
            NarrativeKind::CharacterTrait => "x.narrative/character-trait",
            NarrativeKind::PlotPoint => "x.narrative/plot-point",
            NarrativeKind::Reveal => "x.narrative/reveal",
            NarrativeKind::Setup => "x.narrative/setup",
        }
    }

    /// The parsed `SchemaId`. The strings are compile-time-valid extension ids
    /// (a test pins this), so parsing does not fail in practice.
    pub fn schema_id(self) -> SchemaId {
        SchemaId::parse(self.schema_str())
            .expect("narrative schema strings are valid extension ids")
    }
}

/// The `SourceRef.reference` linking a canon unit to its inkhaven node:
/// `inkhaven:<uuid>#<breadcrumb>`, or `inkhaven:<uuid>` when no breadcrumb.
pub fn node_reference(node: Uuid, breadcrumb: &str) -> String {
    if breadcrumb.is_empty() {
        format!("inkhaven:{node}")
    } else {
        format!("inkhaven:{node}#{breadcrumb}")
    }
}

/// The reverse-lookup prefix matching every unit derived from `node`. UUIDs are
/// fixed-length and unique, so this never matches another node's units.
pub fn node_prefix(node: Uuid) -> String {
    format!("inkhaven:{node}")
}

/// The `SourceRef` for a node-derived canon unit.
pub fn node_source(node: Uuid, breadcrumb: &str) -> SourceRef {
    SourceRef::new(SourceKind::Node, node_reference(node, breadcrumb))
}
