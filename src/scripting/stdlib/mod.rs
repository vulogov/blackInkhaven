//! Inkhaven-specific Bund stdlib. Layered on top of bundcore's
//! vanilla stdlib (arithmetic, strings, conditionals) by
//! `init_adam()`.
//!
//! All `ink.*` words registered here are **read-only** in Phase 1
//! — they look up nodes, paragraphs, search hits, and snapshots
//! through the active project `Store`. Write-side words land in
//! later phases (P4 hooks, P5 script nodes) under the protection
//! of the policy sandbox (P3).

pub mod helpers;
mod ink;

use anyhow::Result;
use rust_multistackvm::multistackvm::VM;

/// Register every inkhaven-specific word on the supplied VM. Called
/// once from `init_adam()` after `Bund::new()` has loaded bundcore's
/// own stdlib.
pub fn register_ink_stdlib(vm: &mut VM) -> Result<()> {
    ink::register(vm)
}
