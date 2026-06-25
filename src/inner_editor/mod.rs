//! INNER_EDITOR-1 — Inner Editor: literary & stylistic companion.
//!
//! The **second member of the Inner family** (sibling to Inner Socrates). Where
//! Inner Socrates interrogates facts/structure through multiple Reader Personas
//! (Notice/Inquiry/Probe, ◇), Inner Editor *observes* literacy and style through
//! a **single configurable persona** (Praise/Note/Concern, ✎). It reads the
//! prose paragraph-by-paragraph during composition and offers brief, grounded,
//! non-prescriptive observations — richness, tautology, style, vocabulary,
//! belief stance, craft praise, suggestions.
//!
//! LLM-only (no Fast track). Paragraph scope (current ¶ + a small preceding
//! window). Fires on a paragraph-pause idle timer and on `Ctrl+B E E`. Coexists
//! with Inner Socrates on the same trigger; findings are visually distinct in
//! the Output pane.
//!
//! Cost caps **inform, never block** (Inkhaven's permissive principle).
//!
//! This module holds the feature's own pieces; it reuses the shared
//! infrastructure (PANE-1 Output, the multilingual prompt chain, the intent
//! ledger, `ai::usage`) rather than rebuilding it. See
//! `Documentation/PROPOSALS/INNER_EDITOR-1_PLAN.md`.
//!
//! IE-P0 lays the foundation (types + store + config); the engagement engine,
//! surfaces, and conversation mode (IE-P1…P5) consume it. The module-level
//! `dead_code` allow covers items not yet wired; it tightens as phases land.
#![allow(dead_code, unused_imports)]

pub mod engage;
pub mod intent_consult;
pub mod parse;
pub mod prompt;
pub mod storage;
pub mod types;

pub use engage::{engage, gather_context, EngageInput, EngageOutcome, GatheredContext};
pub use prompt::{resolve_tuning, system_prompt, ResolvedTuning, SYSTEM_PROMPT_NAME};
pub use storage::{CooldownRow, InnerEditorStore, StoredEditorFinding};
pub use types::{
    EditorCategory, EditorFinding, EditorSeverity, PraiseFrequency, Tone, Verbosity,
};
