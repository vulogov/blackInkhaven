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

    /// Recover the kind from an `x.narrative/…` schema string (`None` for any
    /// other schema — the inverse of [`Self::schema_str`]).
    pub fn from_schema_str(s: &str) -> Option<NarrativeKind> {
        NarrativeKind::ALL.into_iter().find(|k| k.schema_str() == s)
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

/// Parse a node source reference (`inkhaven:<uuid>#<breadcrumb>`, or
/// `inkhaven:<uuid>`) back into its node id and optional breadcrumb — the
/// inverse of [`node_reference`].
pub fn parse_node_reference(reference: &str) -> Option<(Uuid, Option<String>)> {
    let rest = reference.strip_prefix("inkhaven:")?;
    let (uuid_str, breadcrumb) = match rest.split_once('#') {
        Some((u, bc)) => (u, (!bc.is_empty()).then(|| bc.to_string())),
        None => (rest, None),
    };
    Some((Uuid::parse_str(uuid_str).ok()?, breadcrumb))
}

/// The `SourceRef` for a node-derived canon unit.
pub fn node_source(node: Uuid, breadcrumb: &str) -> SourceRef {
    SourceRef::new(SourceKind::Node, node_reference(node, breadcrumb))
}
