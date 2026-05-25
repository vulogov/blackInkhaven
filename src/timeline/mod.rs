//! Story timeline (Phase 1 of the 1.2.6+ timeline feature).
//!
//! Timelines in inkhaven are built around three primitives:
//!
//! * `TimelinePoint` — a single `i64` representing ticks
//!   since the calendar's epoch. Negative for pre-epoch
//!   prequels. All time arithmetic is integer arithmetic on
//!   this value; calendar complexity lives in `Calendar`.
//!
//! * `Precision` — how "exact" the user's intent is for a
//!   given point. `Tick` is the base; `Day`, `Month`,
//!   `Season`, `Year` are coarser. The AI critique uses
//!   precision to decide what "overlap" means when comparing
//!   two events with fuzzy dates.
//!
//! * `Calendar` — converts ticks ↔ human-readable strings
//!   per the user's HJSON configuration. Three preset
//!   expansions live in `presets`; `parse` accepts custom
//!   per-project layouts via `config::TimelineConfig`.
//!
//! Phase 1 ships the storage, the calendar, the CLI surface,
//! and a flat Ctrl+V E picker. Swim-lane visualisation +
//! scope navigation land in Phase 2; AI critique in Phase 3.

pub mod calendar;
pub mod critique;
pub mod presets;

pub use calendar::{Calendar, TimelinePoint};
pub use presets::{Precision};
