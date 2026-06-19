//! Phonology engine (LANG-1) — deterministic phonotactic validation (P1.1),
//! IPA sonority + sonority-aware syllabification (P1.2). Allophony / stress /
//! tone evaluation join here in later P1 increments.

pub mod allophony_eval;
pub mod ipa;
pub mod rewrite;
pub mod romanize;
pub mod stress_eval;
pub mod syllable;
pub mod tone_eval;
pub mod validator;
