//! The Inner Poet (POEM-3) — the fifth Inner-family reader, reached by
//! `Ctrl+B J → P`. It reads a poem and reports what it is doing prosodically:
//! never writes, never prescribes. This module is the deterministic **fast
//! track** (metre + rhyme scan → Output pane); the LLM slow track lands later.

pub mod fast;
