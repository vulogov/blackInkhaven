//! 1.3.0 PDF-1 P1 — imposition: rearranging a linear PDF into print-ready
//! signatures for hand-binding / small print-shop workflows (RFC §8.2).
//!
//! The math is pure + unit-tested first (`layout`, `creep`); the sheet
//! emission (Form XObjects), marks, config, and surfaces build on top.

pub mod creep;
pub mod layout;

/// The four binding styles PDF-1 targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingStyle {
    /// One signature, all sheets nested + folded once (booklet / chapbook).
    SaddleStitch,
    /// Multiple folded signatures gathered + glued (the novel case).
    PerfectBound,
    /// One continuous strip, accordion-folded; pages run in source order.
    Concertina,
    /// Japanese stab binding — each leaf its own sheet, stab-bound spine.
    Stab,
}

impl BindingStyle {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "saddle_stitch" | "saddle-stitch" | "saddle" => Some(Self::SaddleStitch),
            "perfect_bound" | "perfect-bound" | "perfect" => Some(Self::PerfectBound),
            "concertina" | "accordion" => Some(Self::Concertina),
            "stab" | "japanese_stab" | "japanese-stab" => Some(Self::Stab),
            _ => None,
        }
    }
    /// Folded styles fold sheets down the middle (2 pages per side);
    /// stab / concertina are 1 page per side.
    pub fn columns_per_side(self) -> usize {
        match self {
            Self::Stab | Self::Concertina => 1,
            _ => 2,
        }
    }
    pub fn is_folded(self) -> bool {
        matches!(self, Self::SaddleStitch | Self::PerfectBound)
    }
}

/// Where padding blanks go when the page count isn't a signature multiple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlankPolicy {
    Prepend,
    Append,
    Balance,
    /// Refuse to pad — the caller errors if blanks would be needed.
    Error,
}

impl BlankPolicy {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "prepend" => Some(Self::Prepend),
            "append" => Some(Self::Append),
            "balance" => Some(Self::Balance),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Creep (shingling) compensation strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreepStrategy {
    None,
    Shingle,
    Pushout,
}

impl CreepStrategy {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" | "off" => Some(Self::None),
            "shingle" => Some(Self::Shingle),
            "pushout" | "push_out" | "push-out" => Some(Self::Pushout),
            _ => None,
        }
    }
}
