//! PANE-1 — right-side pane architecture (RFC PANE-1).
//!
//! Today this hosts the new **Output** pane (structured notifications); the
//! existing AI conversation pane lives in the TUI app. As PANE-1 lands its later
//! phases, the cycling/focus machinery and pane registry move here too.

// PANE-1 foundation (P0): the message envelope + store API is intentionally
// complete ahead of its in-tree consumers. The TUI pane gestures (pin/snooze),
// the `ink.io.*` Bund stdlib, and the emitter routing (translation, Bund print,
// lexicon proposals, varieties) — scheduled in PANE-1 P1–P3 — consume the rest.
#[allow(dead_code, unused_imports)]
pub mod output;
