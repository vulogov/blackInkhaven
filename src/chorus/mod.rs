//! CHORUS-1 — voice & style at book scale (the 2.1.0 flagship).
//!
//! NARR-1 (`crate::prose`) profiles the *narrator* book-wide. CHORUS profiles the
//! *cast*: each character's dialogue run through the SAME metric engine, a
//! distinctiveness matrix (CH-P2), per-character drift (CH-P3), and the POV /
//! tense / register discipline pillars — all synthesized by the Inner Stylist
//! reader (CH-P7). See `Documentation/PROPOSALS/CHORUS-1_PLAN.md`.
//!
//! CH-P1 lands character voice fingerprinting ([`voices`]); CH-P2 the
//! distinctiveness matrix ([`distinct`]); CH-P3 per-character drift ([`drift`]);
//! CH-P4 POV & head-hop discipline ([`pov`], [`vocab`]).

pub(crate) mod distinct;
pub(crate) mod drift;
pub(crate) mod pov;
pub(crate) mod scenes;
pub(crate) mod tense;
pub(crate) mod vocab;
pub(crate) mod voices;
