//! Inkhaven-specific Bund stdlib. Layered on top of bundcore's
//! vanilla stdlib (arithmetic, strings, conditionals) by
//! `init_adam()`.
//!
//! All `ink.*` words registered here are **read-only** in Phase 1
//! — they look up nodes, paragraphs, search hits, and snapshots
//! through the active project `Store`. Write-side words land in
//! later phases (P4 hooks, P5 script nodes) under the protection
//! of the policy sandbox (P3).

mod app;
mod export;
mod fs;
pub mod helpers;
mod ink;
mod inner_socrates;
pub mod io;
mod keymap;
mod world_timeline;
mod event_critique;
mod lang;
mod poetry;
mod book_rag;
mod inner_editor;
mod sources;
mod terms;
mod snippets;
mod prose;
mod dialogue;
mod graph;
mod chorus;
mod stylist;
mod utopia;
#[path = "char.rs"]
mod char_arc;
mod theologian;
mod myth;
pub(crate) mod calc;
mod pdf;
mod review;

use anyhow::Result;
use rust_multistackvm::multistackvm::VM;

/// Register every inkhaven-specific word on the supplied VM. Called
/// once from `init_adam()` after `Bund::new()` has loaded bundcore's
/// own stdlib. Order matters: we register `io` *after* `ink` so the
/// buffered print/println overrides win over bundcore's stdout
/// versions. `keymap` lands last because it's the most powerful and
/// the policy sandbox blocks it by default.
pub fn register_ink_stdlib(vm: &mut VM) -> Result<()> {
    ink::register(vm)?;
    io::register(vm)?;
    inner_socrates::register(vm)?;
    world_timeline::register(vm)?;
    event_critique::register(vm)?;
    keymap::register(vm)?;
    app::register(vm)?;
    fs::register(vm)?;
    review::register(vm)?;
    pdf::register(vm)?;
    export::register(vm)?;
    lang::register(vm)?;
    poetry::register(vm)?;
    book_rag::register(vm)?;
    inner_editor::register(vm)?;
    sources::register(vm)?;
    terms::register(vm)?;
    snippets::register(vm)?;
    prose::register(vm)?;
    dialogue::register(vm)?;
    graph::register(vm)?;
    chorus::register(vm)?;
    stylist::register(vm)?;
    utopia::register(vm)?;
    char_arc::register(vm)?;
    theologian::register(vm)?;
    myth::register(vm)?;
    calc::register(vm)?;
    Ok(())
}
