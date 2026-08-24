//! Bund sandbox policy.
//!
//! Inkhaven is single-user desktop software, so the threat model is
//! *accidental* damage from a script the user didn't fully understand
//! (a save-hook from a tutorial pasted unaltered, an AI prompt
//! template that turned out to be more aggressive than expected) —
//! not malicious privilege escalation between users. Even so, the
//! safety net is real: ship with destructive categories denied by
//! default and let writers opt in explicitly via HJSON.
//!
//! ## Mechanism
//!
//! Modelled on bdslib's `vm/policy.rs:430-450`:
//!
//! 1. After every word has been registered against the VM (bundcore
//!    stdlib + inkhaven's `ink.*` layer), walk the word→category
//!    table.
//! 2. For each word whose category is denied (or whose name is
//!    explicitly denied / not explicitly allowed), call
//!    `vm.register_inline()` again with the **same name** but our
//!    `denied_stub` as the handler. `register_inline` is upsert —
//!    the original handler is dropped.
//! 3. When the script later calls a denied word, `denied_stub` runs
//!    and returns a clean error.
//!
//! ## Resolution order for a given word
//!
//! 1. In `enabled_words` → allow (overrides everything).
//! 2. In `disabled_words` → deny.
//! 3. Category in `disabled_categories` → deny.
//! 4. Otherwise → allow.
//!
//! ## Naming the offender
//!
//! `VMInlineFn` is a bare function pointer, so a single stub can't
//! capture per-word context. We log every denial at `apply_policy`
//! time (`policy: denying <word>`) and emit a generic
//! `script denied by inkhaven policy` error from the stub. Users
//! who hit a denial read `.inkhaven.log` for the specific word.

use anyhow::{anyhow, Result};
use easy_error::{bail, Error as BundError};
use rust_multistackvm::multistackvm::VM;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Stable category names. Strings instead of an enum so adding a
/// new category is a one-line entry in the table without a
/// migration / serde-rename dance.
///
/// `STORE_WRITE`, `FS_READ`, `FS_WRITE`, `NET`, `SHELL`,
/// `CODE_EVAL` are placeholders for words inkhaven will register
/// in later phases — they're listed here so `inkhaven.hjson`
/// authors can name the category in `disabled_categories` even
/// before the corresponding words exist.
#[allow(dead_code)]
pub mod category {
    pub const STORE_READ: &str = "store_read";
    pub const STORE_WRITE: &str = "store_write";
    pub const FS_READ: &str = "fs_read";
    pub const FS_WRITE: &str = "fs_write";
    pub const NET: &str = "net";
    pub const SHELL: &str = "shell";
    pub const CODE_EVAL: &str = "code_eval";
    /// Runtime keymap mutation via `ink.key.*`. Default-denied
    /// because a script can otherwise hijack the user's chord
    /// muscle memory or lock them out (well — Ctrl+Q is hard-
    /// blocked, but everything else is fair game).
    pub const KEYMAP: &str = "keymap";
    /// Read-only access to the live editor buffer (cursor query,
    /// buffer text, find). Default-allowed — non-destructive.
    pub const EDITOR_READ: &str = "editor_read";
    /// Mutate the live editor buffer — insert, scroll, delete,
    /// goto. Default-denied. The user opts in once and the rest
    /// of their hooks / scripts gain editor reach.
    pub const EDITOR_WRITE: &str = "editor_write";
    /// AI state mutation — clear chat history, set system
    /// prompt, post a user prompt. Default-denied.
    pub const AI_WRITE: &str = "ai_write";
    /// Read AI chat history. Default-allowed.
    pub const AI_READ: &str = "ai_read";
    /// Runtime theme mutation (`ink.theme.set`). Default-denied —
    /// a script can otherwise recolour the interface invisibly.
    pub const THEME_WRITE: &str = "theme_write";
    /// 1.2.9+ — audio output.  Currently scoped to TTS
    /// playback (`ink.tts.speak`).  Default-allowed; the
    /// feature is independently gated by
    /// `editor.tts.enabled` in HJSON, so a script can't
    /// produce audio unless the user already opted in.
    pub const AUDIO: &str = "audio";
}

/// Categories denied out of the box. A user has to actively flip
/// these on in `inkhaven.hjson` to use destructive operations.
/// Currently inkhaven registers zero words in these categories —
/// the deny is forward-looking, ready for P4/P5 additions.
pub const DEFAULT_DENIED_CATEGORIES: &[&str] = &[
    category::STORE_WRITE,
    category::EDITOR_WRITE,
    category::AI_WRITE,
    category::THEME_WRITE,
    category::FS_WRITE,
    category::NET,
    category::SHELL,
    category::CODE_EVAL,
    category::KEYMAP,
];

/// Word → category table. Every word inkhaven registers should
/// appear here; uncategorised words are silently allowed but lose
/// the protection of `disabled_categories`.
///
/// Phase 1 only registered six read-only `ink.*` words, all in
/// `store_read`. Phase 4 (hooks) and Phase 5 (script nodes) will
/// add the destructive variants under `store_write`; phase 6+
/// might surface filesystem and network words.
pub const WORD_CATEGORIES: &[(&str, &str)] = &[
    // ── store_read (default-allowed) ──────────────────────────
    // 1.3.24 PANE-1 — Output channel. print/log and reads stay always-available
    // (the ergonomic output path, RFC §9 "none"); structured emit and the state
    // mutations require fs_write.
    ("ink.io.print", category::STORE_READ),
    ("ink.io.log", category::STORE_READ),
    ("ink.io.message.list", category::STORE_READ),
    ("ink.io.message.count", category::STORE_READ),
    ("ink.io.notify", category::FS_WRITE),
    ("ink.io.message.dismiss", category::FS_WRITE),
    ("ink.io.message.pin", category::FS_WRITE),
    ("ink.io.message.unpin", category::FS_WRITE),
    ("ink.node.list", category::STORE_READ),
    ("ink.node.get", category::STORE_READ),
    ("ink.node.children", category::STORE_READ),
    ("ink.paragraph.text", category::STORE_READ),
    ("ink.search.text", category::STORE_READ),
    ("ink.snapshot.list", category::STORE_READ),
    ("ink.path.to_uuid", category::STORE_READ),
    ("ink.paragraph.target", category::STORE_READ),
    // 1.2.6+ tags — reads.
    ("ink.tag.list", category::STORE_READ),
    ("ink.tag.list_for", category::STORE_READ),
    ("ink.tag.search", category::STORE_READ),
    // 1.2.6+ events — reads.
    ("ink.event.list", category::STORE_READ),
    ("ink.event.list_orphans", category::STORE_READ),
    // Timeline-critique reads — all pull events/calendar/config and push a
    // report; none mutate the store.
    ("ink.event.critique.run", category::STORE_READ),
    ("ink.event.critique.orphan_check", category::STORE_READ),
    ("ink.event.critique.fuzzy_overlap_check", category::STORE_READ),
    ("ink.event.critique.config", category::STORE_READ),
    ("ink.event.critique.custom", category::STORE_READ),
    // Inner Socrates inspectors — read persona/findings/ledger/usage; the
    // Fast track runs in-memory over supplied text and persists nothing.
    ("ink.inner_socrates.check.fast", category::STORE_READ),
    ("ink.inner_socrates.findings.list", category::STORE_READ),
    ("ink.inner_socrates.ledger.list", category::STORE_READ),
    ("ink.inner_socrates.persona.active", category::STORE_READ),
    ("ink.inner_socrates.personas.list", category::STORE_READ),
    ("ink.inner_socrates.usage.today", category::STORE_READ),
    // World fact-check timeline queries — read-only lookups over the calendar.
    ("ink.world.report", category::STORE_READ),
    ("ink.world.undescribed", category::STORE_READ),
    ("ink.world.check", category::STORE_READ),
    ("ink.world.fact_check.timeline.effective_date", category::STORE_READ),
    ("ink.world.fact_check.timeline.events_for_character", category::STORE_READ),
    ("ink.world.fact_check.timeline.events_for_place", category::STORE_READ),
    ("ink.world.fact_check.timeline.events_near", category::STORE_READ),
    ("ink.world.fact_check.timeline.season_for", category::STORE_READ),
    // 1.2.16+ Phase I.4.a — threads (read) +
    // review (read).
    ("ink.thread.list", category::STORE_READ),
    ("ink.review.list", category::STORE_READ),
    // 1.4.5 SOURCES-1 — bibliography reads (all read-only).
    ("ink.sources.list", category::STORE_READ),
    ("ink.sources.get", category::STORE_READ),
    ("ink.sources.check", category::STORE_READ),
    ("ink.sources.bibtex", category::STORE_READ),
    // 1.4.8 TERMS-1 — glossary reads (store_read); declare_intent writes.
    ("ink.terms.list", category::STORE_READ),
    ("ink.terms.get", category::STORE_READ),
    ("ink.terms.check", category::STORE_READ),
    // 1.4.9 REUSE-1 — snippet reads (all read-only).
    ("ink.snippets.list", category::STORE_READ),
    ("ink.snippets.get", category::STORE_READ),
    ("ink.snippets.check", category::STORE_READ),
    // NARR-1 — narrative-voice profiling. All read/compute; `refresh` writes only
    // the derived `.inkhaven/prose.duckdb` cache (not the manuscript), so it stays
    // store_read (default-allowed) rather than store_write.
    ("ink.prose.profile", category::STORE_READ),
    ("ink.prose.drift", category::STORE_READ),
    ("ink.prose.violations", category::STORE_READ),
    ("ink.prose.refresh", category::STORE_READ),
    // DIALOG-1 — dialogue read words; `refresh` writes only the derived
    // dialogue.duckdb cache (not the manuscript), so it stays store_read too.
    ("ink.dialogue.stats", category::STORE_READ),
    ("ink.dialogue.fingerprint", category::STORE_READ),
    ("ink.dialogue.violations", category::STORE_READ),
    ("ink.dialogue.spans", category::STORE_READ),
    ("ink.dialogue.refresh", category::STORE_READ),
    // SEMNET — graph reads over edges.db; rebuild/promote/dismiss mutate the
    // graph layer (annotation over your nodes; the manuscript is untouched, but
    // they change persisted edges, so store_write).
    ("ink.graph.stats", category::STORE_READ),
    ("ink.graph.neighbors", category::STORE_READ),
    ("ink.graph.contradicting", category::STORE_READ),
    ("ink.graph.loci", category::STORE_READ),
    ("ink.graph.paths", category::STORE_READ),
    ("ink.graph.pending", category::STORE_READ),
    ("ink.graph.rebuild", category::STORE_WRITE),
    ("ink.graph.promote", category::STORE_WRITE),
    ("ink.graph.dismiss", category::STORE_WRITE),
    // CHORUS — voice-at-scale reads; they refresh the derived prose/dialogue
    // caches (not the manuscript), so they stay store_read, like `prose.refresh`.
    ("ink.chorus.voices", category::STORE_READ),
    ("ink.chorus.distinct", category::STORE_READ),
    ("ink.chorus.drift", category::STORE_READ),
    ("ink.chorus.headhops", category::STORE_READ),
    ("ink.chorus.tense", category::STORE_READ),
    ("ink.chorus.register", category::STORE_READ),
    // INNER-STYLIST — synthesised findings (read); suppress/unsuppress write the
    // derived inner_stylist.db (not the manuscript), but they change persisted
    // author decisions, so store_write.
    ("ink.stylist.findings", category::STORE_READ),
    ("ink.stylist.suppressions", category::STORE_READ),
    ("ink.stylist.suppress", category::STORE_WRITE),
    ("ink.stylist.unsuppress", category::STORE_WRITE),
    // SENTINEL — the deterministic continuity ledger; both are read-only sweeps
    // (the LLM coherence pass is not exposed to Bund).
    ("ink.continuity.findings", category::STORE_READ),
    ("ink.continuity.check", category::STORE_READ),
    // LECTOR — the deterministic read-through; read-only (the LLM synthetic
    // first-read is not exposed to Bund).
    ("ink.readthrough.report", category::STORE_READ),
    ("ink.readthrough.curve", category::STORE_READ),
    ("ink.readthrough.check", category::STORE_READ),
    // REDLINE — the unified revision worklist; read-only (the AI editorial letter
    // and every prose rewrite are not exposed to Bund). `collect` opens its own
    // read handle, so both stay store_read.
    ("ink.revise.findings", category::STORE_READ),
    ("ink.revise.check", category::STORE_READ),
    // CHRONICLE — the draft-history intelligence; read-only. Marking (which writes
    // a milestone) is deliberately NOT exposed to Bund.
    ("ink.chronicle.marks", category::STORE_READ),
    ("ink.chronicle.trend", category::STORE_READ),
    ("ink.chronicle.check", category::STORE_READ),
    // KEN — the epistemic check (who knows what, when); read-only, deterministic.
    // The opt-in --deep LLM implied_irony pass is not exposed (it costs).
    ("ink.knowledge.grants", category::STORE_READ),
    ("ink.knowledge.findings", category::STORE_READ),
    ("ink.knowledge.check", category::STORE_READ),
    ("ink.bonds.ties", category::STORE_READ),
    ("ink.bonds.findings", category::STORE_READ),
    ("ink.bonds.check", category::STORE_READ),
    // CHAR-1 — character-arc reads; `plan`/`refresh` write only the derived
    // char.duckdb cache (not the manuscript), so they stay store_read too.
    ("ink.char.arc", category::STORE_READ),
    ("ink.char.stalls", category::STORE_READ),
    ("ink.char.checks", category::STORE_READ),
    ("ink.char.plan", category::STORE_READ),
    ("ink.char.refresh", category::STORE_READ),
    // INNER-THEOLOGIAN-1 — `signals` recomputes only the derived inner_theologian.db
    // cache (store_read); `suppress` mutates the suppression flag (store_write, below).
    // The `inner_theologian.*` spelling matches the other two readers'
    // (`ink.inner_socrates.*` / `ink.inner_editor.*`); the shorter `theologian.*`
    // names are kept for back-compat. Both are real, policy-gated words.
    ("ink.theologian.signals", category::STORE_READ),
    ("ink.inner_theologian.signals", category::STORE_READ),
    // MYTH-1 — declared-inventory + deterministic reads (symbols/motifs/
    // archetypes/density/findings) recompute only derived caches → store_read;
    // `suppress` mutates the findings table → store_write (below).
    ("ink.myth.symbols", category::STORE_READ),
    ("ink.myth.motifs", category::STORE_READ),
    ("ink.myth.archetypes", category::STORE_READ),
    ("ink.myth.density", category::STORE_READ),
    ("ink.myth.findings", category::STORE_READ),
    // WORLD-6 — utopia coherence reads (model/findings/violations) are
    // store_read; `suppress` mutates the findings table → store_write (below).
    ("ink.utopia.model", category::STORE_READ),
    ("ink.utopia.findings", category::STORE_READ),
    ("ink.utopia.violations", category::STORE_READ),
    // OUTLINE-1 — reading the outline is store_read; the paragraph copy/move
    // mutators are store_write (below).
    ("ink.outline.print", category::STORE_READ),
    // 3.0.4 Phase-1 — read-only wrappers over existing features. All
    // deterministic reads (store / config / filesystem-metadata); nothing here
    // calls the LLM (the AI passes of these families stay CLI-only, matching the
    // LECTOR / REDLINE / KEN precedent) or mutates anything.
    // Inner Rigor — the deterministic, zero-AI reasoning reader.
    ("ink.rigor.scan", category::STORE_READ),
    ("ink.rigor.check", category::STORE_READ),
    ("ink.rigor.paragraph", category::STORE_READ),
    // Planning — the canonical framework beat tables (pure static data).
    ("ink.planning.frameworks", category::STORE_READ),
    ("ink.planning.beats", category::STORE_READ),
    // Cost — the AI ledger tally + configured caps (read-only reporting).
    ("ink.cost.usage", category::STORE_READ),
    ("ink.cost.caps", category::STORE_READ),
    // Goals — the writing streak (writing-day history, no live total).
    ("ink.goals.streak", category::STORE_READ),
    // WordNet — installed-sources listing (filesystem `exists()` checks only;
    // sense lookups/fetch/import land later).
    ("ink.wordnet.list", category::STORE_READ),
    // Doctor — the cheap standalone health checks over the live store. `scan`
    // runs the full project scan over the active store (read-only); `autofix`
    // applies repairs, so it lives under store_write (below).
    ("ink.doctor.integrity", category::STORE_READ),
    ("ink.doctor.vectors", category::STORE_READ),
    ("ink.doctor.scan", category::STORE_READ),
    // Backup — the last-backup timestamp (reads the sidecar; making a backup is
    // fs_write, deferred).
    ("ink.backup.last", category::STORE_READ),
    // 3.0.4 Phase-2 — load-bearing read-only wrappers. All deterministic reads
    // (active store / project sidecars / installed indexes); no LLM, network, or
    // writes. The AI passes of these families (research contradict/converge,
    // planning analyze, world critique) stay CLI-only.
    // WordNet — sense lookups load an installed `.wn` index (FS read, no network).
    ("ink.wordnet.lookup", category::STORE_READ),
    ("ink.wordnet.suggest", category::STORE_READ),
    // Companions — the examined-authorship cockpit (inner sidecar DBs + World
    // health via the already-open store; never reopens the project).
    ("ink.companions.findings", category::STORE_READ),
    ("ink.companions.promotions", category::STORE_READ),
    ("ink.companions.world", category::STORE_READ),
    ("ink.companions.summary", category::STORE_READ),
    // Research — the evidence base: Facts inventory, provenance, source chunks,
    // the persisted SCHOLAR report. Ingest (net) / LLM scans / fact writes stay
    // CLI-only.
    ("ink.research.facts", category::STORE_READ),
    ("ink.research.undisputed", category::STORE_READ),
    ("ink.research.provenance", category::STORE_READ),
    ("ink.research.sources", category::STORE_READ),
    ("ink.research.report", category::STORE_READ),
    // Index Locorum / Verborum — the scholarly indexes (harvest = disk reads of
    // the same files assembly compiles; no write, no LLM).
    ("ink.locorum.build", category::STORE_READ),
    ("ink.locorum.malformed", category::STORE_READ),
    ("ink.locorum.render", category::STORE_READ),
    ("ink.verborum.build", category::STORE_READ),
    ("ink.verborum.render", category::STORE_READ),
    // Backup — enumerating the backup zips is a project-directory read (fs_read,
    // default-allowed; the dir is derived from the project root, no user path).
    ("ink.backup.list", category::FS_READ),
    // Cost — today's spend (opens the companion sidecar DBs fresh, safe).
    ("ink.cost.today", category::STORE_READ),
    // Goals — the full progress snapshot (per-book word-count walk + streak).
    ("ink.goals.snapshot", category::STORE_READ),
    // Planning — the deterministic structural report + its gap projection (the AI
    // critique, `plan analyze`, is not exposed).
    ("ink.planning.check", category::STORE_READ),
    ("ink.planning.gaps", category::STORE_READ),

    // ── store_write (default-denied) ──────────────────────────
    // 1.2.3+: Bund scripts can mutate the project tree, status
    // tags, paragraph bodies, and DB state. Default-denied; opt
    // in by listing "store_write" in scripting.enabled_categories.
    ("ink.tree.add", category::STORE_WRITE),
    ("ink.tree.delete", category::STORE_WRITE),
    ("ink.tree.rename", category::STORE_WRITE),
    ("ink.tree.move_up", category::STORE_WRITE),
    ("ink.tree.move_down", category::STORE_WRITE),
    ("ink.tree.morph", category::STORE_WRITE),
    // OUTLINE-1 — cross-parent paragraph copy/move.
    ("ink.outline.paragraph_copy", category::STORE_WRITE),
    ("ink.outline.paragraph_move", category::STORE_WRITE),
    // WORLD-6 — suppressing a coherence finding mutates the store.
    ("ink.utopia.suppress", category::STORE_WRITE),
    // INNER-THEOLOGIAN-1 — suppressing a signal mutates the suppression flag.
    ("ink.theologian.suppress", category::STORE_WRITE),
    ("ink.inner_theologian.suppress", category::STORE_WRITE),
    // MYTH-1 — suppressing a myth finding mutates the findings table.
    ("ink.myth.suppress", category::STORE_WRITE),
    ("ink.paragraph.set_status", category::STORE_WRITE),
    ("ink.paragraph.set_target", category::STORE_WRITE),
    ("ink.paragraph.save", category::STORE_WRITE),
    // 1.2.6+ tag mutations.
    ("ink.tag.add", category::STORE_WRITE),
    ("ink.tag.remove", category::STORE_WRITE),
    // 1.2.6+ event mutations.
    ("ink.event.add", category::STORE_WRITE),
    ("ink.event.set_end", category::STORE_WRITE),
    ("ink.event.set_precision", category::STORE_WRITE),
    ("ink.event.set_track", category::STORE_WRITE),
    ("ink.event.link_paragraph", category::STORE_WRITE),
    // 1.2.16+ Phase I.4.a — review mutations.
    // Inherit the existing store_write category
    // gate; projects that already enable
    // store_write for tree mutation automatically
    // grant review-write too.
    ("ink.review.add_comment", category::STORE_WRITE),
    ("ink.review.resolve", category::STORE_WRITE),
    ("ink.db.sync", category::STORE_WRITE),
    ("ink.db.checkpoint", category::STORE_WRITE),
    ("ink.db.reindex", category::STORE_WRITE),
    // 1.4.8 TERMS-1 — declaring a deliberate terminology variant writes an
    // intent-ledger row.
    ("ink.terms.declare_intent", category::STORE_WRITE),
    // 3.0.4 Phase-3 — the importers write Book/Chapter/Paragraph nodes into the
    // active store (store_write, default-denied). They also read an external
    // bundle path, confined by the fs sandbox (`resolve_fs_path`) — so importing
    // an out-of-project bundle additionally needs `fs_unsandboxed`.
    ("ink.import.scrivener", category::STORE_WRITE),
    ("ink.import.epub", category::STORE_WRITE),
    // 3.0.4 — doctor autofix applies repairs (delete orphan rows, rematerialize
    // bdslib-only files, quarantine corrupt sidecars) over the active store.
    ("ink.doctor.autofix", category::STORE_WRITE),

    // ── keymap (default-denied) ───────────────────────────────
    ("ink.key.bind", category::KEYMAP),
    ("ink.key.bind_lambda", category::KEYMAP),
    ("ink.key.unbind", category::KEYMAP),
    ("ink.key.list", category::KEYMAP),

    // ── editor_read (default-allowed) ─────────────────────────
    ("ink.editor.cursor", category::EDITOR_READ),
    ("ink.editor.text", category::EDITOR_READ),
    ("ink.editor.find", category::EDITOR_READ),

    // ── editor_write (default-denied) ─────────────────────────
    ("ink.editor.goto", category::EDITOR_WRITE),
    ("ink.editor.set_cursor", category::EDITOR_WRITE),
    // 1.2.6+ — `ink.story.render` writes a PNG file, so it lives
    // under `fs_write` (default-denied). The user opts in with
    // `enabled_categories: ["fs_write"]` in their HJSON.
    ("ink.story.render", category::FS_WRITE),
    ("ink.editor.insert", category::EDITOR_WRITE),
    ("ink.editor.scroll", category::EDITOR_WRITE),
    ("ink.editor.delete_line", category::EDITOR_WRITE),
    ("ink.editor.delete_to_bol", category::EDITOR_WRITE),
    ("ink.editor.delete_to_eol", category::EDITOR_WRITE),

    // ── ai_read (default-allowed) ─────────────────────────────
    ("ink.ai.history", category::AI_READ),

    // ── ai_write (default-denied) ─────────────────────────────
    ("ink.ai.clear_history", category::AI_WRITE),
    ("ink.ai.send", category::AI_WRITE),
    ("ink.ai.set_system_prompt", category::AI_WRITE),

    // ── editor_write (Phase C addition) ───────────────────────
    ("ink.editor.replace", category::EDITOR_WRITE),
    // 1.2.4+: replace_all has the same category — both rewrite
    // the open buffer.
    ("ink.editor.replace_all", category::EDITOR_WRITE),
    // 1.2.4+: search.load opens an existing paragraph in the
    // editor — no project mutation, behaves like a read.
    ("ink.search.load", category::EDITOR_READ),
    // 1.2.4+: AI poll is a read of in-flight inference state;
    // send_blocking spawns one, so it shares ai_write with the
    // existing send.
    ("ink.ai.poll", category::AI_READ),
    ("ink.ai.send_blocking", category::AI_WRITE),

    // ── theme_write (default-denied) ──────────────────────────
    ("ink.theme.set", category::THEME_WRITE),

    // ── store_write (Typst pipeline mutates artefacts dir) ────
    ("ink.typst.assemble", category::STORE_WRITE),
    ("ink.typst.build", category::STORE_WRITE),
    ("ink.typst.take", category::STORE_WRITE),

    // ── editor_read (Bund output pane is non-destructive UI) ──
    // Pane open/close/clear/line only mutate transient modal
    // state, recoverable with Esc, never touch the project.
    ("ink.pane.show", category::EDITOR_READ),
    ("ink.pane.close", category::EDITOR_READ),
    ("ink.pane.clear", category::EDITOR_READ),
    ("ink.pane.line", category::EDITOR_READ),

    // ── editor_read (Bund input modal — UI prompt, hook-driven) ──
    // ink.input only opens a modal; the typed string flows back
    // through `hooks::fire(name, …)` which honours its own
    // policy gate when the hook itself calls write words.
    ("ink.input", category::EDITOR_READ),

    // ── audio (1.2.9+, default-allowed) ───────────────────────
    // TTS playback.  Feature is independently gated by
    // `editor.tts.enabled` in HJSON, so allowing this
    // category by default is safe — a script can't
    // produce audio unless the user already opted in.
    ("ink.tts.speak", category::AUDIO),

    // ── fs_read / fs_write (default-denied) ─────────────────
    // 1.2.4+: filesystem IO from Bund. Default-denied — opt in
    // via `enabled_categories: ["fs_read"]` etc. Paths are
    // passed verbatim, no sandboxing — the user opts in, the
    // user gets the responsibility.
    ("ink.fs.read", category::FS_READ),
    ("ink.fs.write", category::FS_WRITE),

    // 3.0.4 Phase-3 — creating a backup writes a zip to the project's backup dir
    // (fs_write, default-denied; the dir is derived from the project root, no
    // user path). The Scrivener dry-run preview reads the external bundle and
    // writes nothing (fs_read; the path is sandbox-confined like `ink.fs.read`).
    ("ink.backup.make", category::FS_WRITE),
    ("ink.import.scrivener_preview", category::FS_READ),

    // 1.3.0 PDF-1 — only the disk-crossing `ink.pdf.*` words are
    // categorised.  `load` reads a file (fs_read); `save` writes one
    // (fs_write, default-denied — a script can't write PDFs without the
    // capability).  The in-memory ops (pages / extract / delete / rotate
    // / reorder / merge / metadata) touch neither store nor disk, so they
    // stay uncategorised (allowed; they only persist via `save`).
    ("ink.pdf.load", category::FS_READ),
    ("ink.pdf.save", category::FS_WRITE),

    // 1.3.1 SUBMISSION-1 — every `ink.export.*` word writes an artefact to
    // a (sandboxed) path, so all are fs_write (default-denied).
    ("ink.export.docx", category::FS_WRITE),
    ("ink.export.manuscript", category::FS_WRITE),
    ("ink.export.markdown", category::FS_WRITE),
    ("ink.export.tex", category::FS_WRITE),
    ("ink.export.epub", category::FS_WRITE),

    // 1.3.21 — ConLang Suite from Bund.  The inspectors only read the
    // language's book blocks + run the (pure) engine → store_read
    // (default-allowed).  init / define / add_word create book nodes, so they
    // inherit the store_write deny-by-default gate, exactly like ink.tree.*.
    // POEM PO-P9 — the poetry engines (all read-only, default-allowed).
    ("ink.poem.syllable_count", category::STORE_READ),
    ("ink.poem.scan_line", category::STORE_READ),
    ("ink.poem.rhyme", category::STORE_READ),
    ("ink.poem.status", category::STORE_READ),
    // PO-P16 — inner_poet.db: reading findings is a read; suppressing writes.
    ("ink.poem.findings", category::STORE_READ),
    ("ink.poem.suppress", category::STORE_WRITE),
    ("ink.lang.list", category::STORE_READ),
    ("ink.lang.generate_word", category::STORE_READ),
    ("ink.lang.syllabify", category::STORE_READ),
    ("ink.lang.ipa", category::STORE_READ),
    ("ink.lang.stress", category::STORE_READ),
    ("ink.lang.tone", category::STORE_READ),
    ("ink.lang.transliterate", category::STORE_READ),
    ("ink.lang.gloss", category::STORE_READ),
    ("ink.lang.paradigm", category::STORE_READ),
    ("ink.lang.derive", category::STORE_READ),
    ("ink.lang.agree", category::STORE_READ),
    ("ink.lang.sentence", category::STORE_READ),
    ("ink.lang.translate", category::STORE_READ),
    ("ink.lang.reverse", category::STORE_READ),
    ("ink.lang.cross", category::STORE_READ),
    ("ink.lang.memory", category::STORE_READ),
    ("ink.lang.corpus", category::STORE_READ),
    ("ink.lang.eval", category::STORE_READ),
    ("ink.lang.relative", category::STORE_READ),
    ("ink.lang.complement", category::STORE_READ),
    ("ink.lang.coordinate", category::STORE_READ),
    ("ink.lang.stats", category::STORE_READ),
    ("ink.lang.audit", category::STORE_READ),
    ("ink.lang.query", category::STORE_READ),
    ("ink.lang.gaps", category::STORE_READ),
    ("ink.lang.sound_change", category::STORE_READ),
    ("ink.lang.cognates", category::STORE_READ),
    ("ink.lang.family_tree", category::STORE_READ),
    ("ink.lang.names", category::STORE_READ),
    ("ink.lang.prose", category::STORE_READ),
    ("ink.lang.poem", category::STORE_READ),
    ("ink.lang.varieties", category::STORE_READ),
    ("ink.lang.lect", category::STORE_READ),
    ("ink.lang.borrow", category::STORE_READ),
    ("ink.lang.areal", category::STORE_READ),
    ("ink.lang.idiolect", category::STORE_READ),
    ("ink.lang.ecology", category::STORE_READ),
    // ink.lang.dict is a pure data constructor — uncategorised (allowed).
    ("ink.lang.init", category::STORE_WRITE),
    ("ink.lang.define", category::STORE_WRITE),
    ("ink.lang.add_word", category::STORE_WRITE),
    ("ink.lang.remember", category::STORE_WRITE),
    ("ink.lang.remove_word", category::STORE_WRITE),
    ("ink.lang.derive_add", category::STORE_WRITE),
    ("ink.lang.grammar_set", category::STORE_WRITE),
    ("ink.lang.idiom_add", category::STORE_WRITE),
    ("ink.lang.metaphor_add", category::STORE_WRITE),
    // The AI-backed words call the LLM → ai_write (default-denied). They stay
    // advisory (return data, never write the book), so a script that commits
    // their output goes through the separately-gated store_write words.
    ("ink.lang.compose", category::AI_WRITE),
    ("ink.lang.reconstruct", category::AI_WRITE),
    ("ink.lang.realism_check", category::AI_WRITE),
    ("ink.lang.generate_lexicon", category::AI_WRITE),
    // File / document output → fs_write (default-denied). `glyph_lint` only
    // reads an SVG (fs_read). `glyph_draft` writes a file AND calls the LLM, so
    // it is gated fs_write here and additionally checks ai_write at run time.
    ("ink.lang.glyph_lint", category::FS_READ),
    ("ink.lang.export", category::FS_WRITE),
    ("ink.lang.dictionary", category::FS_WRITE),
    ("ink.lang.grammar_book", category::FS_WRITE),
    ("ink.lang.font_build", category::FS_WRITE),
    ("ink.lang.glyph_draft", category::FS_WRITE),
    // 1.4.1 BOOK_RAG-1 — Book-scope retrieval. All read-only: the store-backed
    // words retrieve/compose/inspect; the pure helpers don't touch the store
    // but stay under store_read so the whole surface disables in one category.
    // Nothing mutates and nothing calls the LLM (retrieval is local).
    ("ink.book_rag.retrieve", category::STORE_READ),
    ("ink.book_rag.context", category::STORE_READ),
    ("ink.book_rag.scope", category::STORE_READ),
    ("ink.book_rag.config", category::STORE_READ),
    ("ink.book_rag.system_prompt", category::STORE_READ),
    ("ink.book_rag.estimate_tokens", category::STORE_READ),
    ("ink.book_rag.cited_ids", category::STORE_READ),
    ("ink.book_rag.validate_citations", category::STORE_READ),
    // 1.4.3 INNER_EDITOR-1 — read-only inspectors (store_read), the ledger
    // mutator (store_write), and the LLM engage pass (ai_write).
    ("ink.inner_editor.findings.list", category::STORE_READ),
    ("ink.inner_editor.usage.today", category::STORE_READ),
    ("ink.inner_editor.config", category::STORE_READ),
    ("ink.inner_editor.categories", category::STORE_READ),
    ("ink.inner_editor.suggestions", category::STORE_READ),
    ("ink.inner_editor.system_prompt", category::STORE_READ),
    ("ink.inner_editor.intent.declare", category::STORE_WRITE),
    ("ink.inner_editor.engage", category::AI_WRITE),
];

/// Words that are **deliberately** left out of [`WORD_CATEGORIES`] because they
/// touch none of the protected resources (store / filesystem / network / AI /
/// editor). They are pure, in-memory value transforms: a script can observe or
/// reshape data it already holds, but cannot read or persist anything through
/// them. Any effect reaches the outside world only via a *categorised* word
/// (e.g. `ink.pdf.load`/`ink.pdf.save` gate the whole PDF pipeline; the
/// intermediate ops mutate an in-memory handle only).
///
/// This list exists so `every_registered_word_is_classified` can tell a
/// genuinely-pure word apart from one whose category was simply forgotten — the
/// latter would be silently allowed, escaping `disabled_categories`. It has no
/// runtime role (a pure word is allowed precisely *by its absence* from
/// [`WORD_CATEGORIES`]); it is the intentional-purity ledger the coverage test
/// checks against.
#[allow(dead_code)] // consumed by the classification tests; a documentation artifact otherwise
pub const PURE_UNCATEGORISED: &[&str] = &[
    // 3.0.6 — `ink.words` introspects the registered word table itself (VM state
    // only); it touches no store / filesystem / network, so it is intentionally
    // pure (always available, even under a locked-down policy).
    "ink.words",
    // Pure data constructor — builds a Bund dict from stack values.
    "ink.lang.dict",
    // PDF pipeline: `load` (fs_read) and `save` (fs_write) are the only FS gates;
    // every op below transforms an in-memory `PdfDoc` handle and never touches disk.
    "ink.pdf.pages",
    "ink.pdf.extract",
    "ink.pdf.delete",
    "ink.pdf.rotate",
    "ink.pdf.reorder",
    "ink.pdf.merge",
    "ink.pdf.impose",
    "ink.pdf.cover",
    "ink.pdf.barcode",
    "ink.pdf.preflight",
    "ink.pdf.grayscale",
    "ink.pdf.optimize",
    "ink.pdf.watermark",
    "ink.pdf.sample",
    "ink.pdf.title",
    "ink.pdf.set_title",
    "ink.pdf.set_author",
    "ink.pdf.strip_metadata",
];

/// Policy loaded from `inkhaven.hjson`'s `scripting` stanza. All
/// three lists default to empty — combined with
/// `DEFAULT_DENIED_CATEGORIES` they give the conservative
/// "destructive categories off, safe categories on" default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// Categories the user has actively denied. Layered on top of
    /// `DEFAULT_DENIED_CATEGORIES` — the effective deny-set is the
    /// union.
    #[serde(default)]
    pub disabled_categories: Vec<String>,

    /// Specific words to deny regardless of their category.
    #[serde(default)]
    pub disabled_words: Vec<String>,

    /// Specific words to allow even when their category is denied.
    /// Used to grant a single tool from an otherwise-denied family
    /// (e.g. enable `file.read` without enabling all of `fs_read`).
    #[serde(default)]
    pub enabled_words: Vec<String>,

    /// Categories the user has actively enabled, overriding the
    /// built-in defaults. Use this to opt in to a single
    /// destructive family (e.g. `"keymap"`) without disabling
    /// the entire default-deny baseline.
    #[serde(default)]
    pub enabled_categories: Vec<String>,

    /// When `true`, disable the built-in default deny list and use
    /// only `disabled_categories` / `disabled_words` verbatim. Power
    /// users only — off by default.
    #[serde(default)]
    pub no_default_deny: bool,

    /// 1.2.15+ Phase S.6 (H2) — when `true`,
    /// `ink.fs.read` and `ink.fs.write` operate on
    /// unrestricted filesystem paths.  Default
    /// false: paths are confined to the project
    /// root via `crate::path_safety::resolve_within`.
    ///
    /// Confinement applies even when the user has
    /// enabled the `fs_read` / `fs_write`
    /// categories — the category gate decides "is
    /// the script ALLOWED to touch the filesystem",
    /// the sandbox decides "what surface area
    /// counts as filesystem for that script".
    /// Setting this `true` collapses the surface
    /// to "anywhere this UID can reach", which is
    /// the pre-1.2.15 behaviour.
    ///
    /// Recommended only for trusted projects where
    /// scripts genuinely need to reach a shared
    /// location outside the project tree.
    #[serde(default)]
    pub fs_unsandboxed: bool,

    /// 1.2.15+ Phase S.6 (H1) — gate for the
    /// auto-load of Script-book paragraphs at
    /// project open.  Three values:
    ///
    ///   * `"ask"` (default) — scripts are run only
    ///     when `<project>/.inkhaven/trust` exists
    ///     and contains the marker line `trust`
    ///     (case-insensitive).  Without that file
    ///     the user gets a status-bar notice that
    ///     scripts are pending opt-in.  Eliminates
    ///     the "open a malicious project, scripts
    ///     auto-execute" risk.
    ///   * `"trust"` — run scripts unconditionally.
    ///     Use only on projects where the user
    ///     authored or audited the scripts.  The
    ///     `.inkhaven/trust` file becomes
    ///     unnecessary.
    ///   * `"deny"` — never run scripts regardless
    ///     of the trust file.  Useful for opening
    ///     a project for read-only review.
    ///
    /// Note: a malicious project's HJSON could set
    /// this to `"trust"` itself.  The intended
    /// audience for this knob is the project
    /// author publishing their own work.  Users
    /// opening a project they did not write should
    /// always start from `"ask"` defaults and
    /// review the scripts before creating the
    /// trust file.
    #[serde(default = "default_trust_decision")]
    pub trust_decision: String,

    /// Bund script run once after Adam is constructed, after stdlib
    /// registration, after policy application. The natural home for
    /// defining hook lambdas (`hook.on_save`, `hook.on_rename`, …)
    /// and any custom user words. Empty = no bootstrap.
    #[serde(default)]
    pub bootstrap: String,
}

fn default_trust_decision() -> String {
    "ask".to_string()
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            disabled_categories: Vec::new(),
            disabled_words: Vec::new(),
            enabled_words: Vec::new(),
            enabled_categories: Vec::new(),
            no_default_deny: false,
            fs_unsandboxed: false,
            trust_decision: default_trust_decision(),
            bootstrap: String::new(),
        }
    }
}

impl Policy {
    /// Whether `category` is denied under this policy. Used by words that need a
    /// *second* capability beyond their table category (e.g. an AI word that
    /// also writes a file checks both `ai_write` and `fs_write`).
    pub fn denies(&self, category: &str) -> bool {
        self.effective_denied_categories().contains(category)
    }

    /// True when the policy is the trivial "allow everything"
    /// state — used by `init_adam` to skip the apply pass.
    pub fn is_open(&self) -> bool {
        self.disabled_categories.is_empty()
            && self.disabled_words.is_empty()
            && self.no_default_deny
    }

    /// Resolve effective denied categories: defaults +
    /// `disabled_categories`, with anything in `enabled_categories`
    /// subtracted so a user can opt in to a single default-denied
    /// family (e.g. `keymap`) without disabling the rest of the
    /// baseline.
    fn effective_denied_categories(&self) -> HashSet<&str> {
        let mut s: HashSet<&str> = HashSet::new();
        if !self.no_default_deny {
            for c in DEFAULT_DENIED_CATEGORIES {
                s.insert(*c);
            }
        }
        for c in &self.disabled_categories {
            s.insert(c.as_str());
        }
        for c in &self.enabled_categories {
            s.remove(c.as_str());
        }
        s
    }
}

/// Apply `policy` to `vm` — re-register every denied word with
/// `denied_stub`. Safe to call after the rest of the stdlib has
/// been registered; word resolution at script run time uses the
/// most recently registered handler.
pub fn apply_policy(vm: &mut VM, policy: &Policy) -> Result<()> {
    let denied_categories = policy.effective_denied_categories();
    let enabled: HashSet<&str> = policy.enabled_words.iter().map(String::as_str).collect();
    let denied_words: HashSet<&str> =
        policy.disabled_words.iter().map(String::as_str).collect();

    for (word, cat) in WORD_CATEGORIES {
        if enabled.contains(*word) {
            continue; // explicit allowlist wins
        }
        let cat_denied = denied_categories.contains(*cat);
        let word_denied = denied_words.contains(*word);
        if cat_denied || word_denied {
            tracing::warn!(
                target: "inkhaven::scripting::policy",
                "denying {} (category {})",
                word,
                cat
            );
            vm.register_inline(word.to_string(), denied_stub)
                .map_err(|e| anyhow!("policy: re-register {word} as denied: {e}"))?;
        }
    }
    Ok(())
}

/// The handler every denied word is re-registered with. Returns a
/// generic error — the specific word name is in the log line emitted
/// at apply-policy time (stderr in CLI mode, `.inkhaven.log` in TUI).
fn denied_stub(_vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    bail!(
        "script denied by inkhaven policy — earlier log lines name the offending word"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards the policy footgun: a word registered but absent from BOTH
    /// [`WORD_CATEGORIES`] and [`PURE_UNCATEGORISED`] is silently *allowed* —
    /// it escapes `disabled_categories` entirely. Every `ink.*` verb the stdlib
    /// registers must therefore be consciously classified as either gated (a
    /// category) or pure (the allowlist). Adding a verb without doing so fails
    /// here, not in production.
    #[test]
    fn every_registered_word_is_classified() {
        use rust_multistackvm::multistackvm::VM;
        let mut vm = VM::new();
        crate::scripting::stdlib::register_ink_stdlib(&mut vm).unwrap();

        let gated: std::collections::HashSet<&str> =
            WORD_CATEGORIES.iter().map(|(w, _)| *w).collect();
        let pure: std::collections::HashSet<&str> =
            PURE_UNCATEGORISED.iter().copied().collect();

        // `register_inline` keys the handler as `<name>_inline`; aliases live in
        // `name_mapping` and inherit the canonical word's gate, so we only police
        // the canonical `ink.*` names here.
        let mut unclassified: Vec<String> = vm
            .inline_fun
            .keys()
            .filter_map(|k| k.strip_suffix("_inline"))
            .filter(|name| name.starts_with("ink."))
            .filter(|name| !gated.contains(*name) && !pure.contains(*name))
            .map(String::from)
            .collect();
        unclassified.sort();

        assert!(
            unclassified.is_empty(),
            "{} ink.* verb(s) are neither in WORD_CATEGORIES nor PURE_UNCATEGORISED \
             (they would be silently allowed, bypassing disabled_categories). \
             Classify each — add a category or, if it touches no protected resource, \
             add it to PURE_UNCATEGORISED:\n{}",
            unclassified.len(),
            unclassified.join("\n"),
        );
    }

    /// 3.0.7 — the Bund word reference must not rot: every registered `ink.*`
    /// word must appear in `Documentation/Bund/WORD_REFERENCE.md` with a matching
    /// category, and the doc must list no word that isn't registered. The doc is
    /// embedded at compile time, so this fails the build the moment the reference
    /// falls out of sync with the policy table (the `WORD_CATEGORIES` + pure sets
    /// are the authoritative surface, per `every_registered_word_is_classified`).
    #[test]
    fn word_reference_doc_matches_the_policy_table() {
        const DOC: &str =
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Documentation/Bund/WORD_REFERENCE.md"));

        // Authoritative: word -> expected category string.
        let mut expected: std::collections::HashMap<&str, &str> =
            WORD_CATEGORIES.iter().copied().collect();
        for w in PURE_UNCATEGORISED {
            expected.insert(w, "pure");
        }

        // Parse the doc's table rows: `| \`ink.x\` | category | sig | desc |`.
        let mut doc: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for line in DOC.lines() {
            let line = line.trim_start();
            if !line.starts_with("| `ink.") {
                continue;
            }
            let cols: Vec<&str> = line.split('|').collect();
            if cols.len() < 3 {
                continue;
            }
            let word = cols[1].trim().trim_matches('`').trim();
            let cat = cols[2].trim();
            if word.starts_with("ink.") {
                doc.insert(word.to_string(), cat.to_string());
            }
        }

        let mut missing: Vec<&str> = Vec::new();
        let mut wrong: Vec<String> = Vec::new();
        for (w, c) in &expected {
            match doc.get(*w) {
                None => missing.push(w),
                Some(dc) if dc != c => wrong.push(format!("{w}: doc={dc} policy={c}")),
                _ => {}
            }
        }
        let mut extra: Vec<&str> =
            doc.keys().map(String::as_str).filter(|w| !expected.contains_key(w)).collect();
        missing.sort();
        wrong.sort();
        extra.sort();

        assert!(
            missing.is_empty() && wrong.is_empty() && extra.is_empty(),
            "Documentation/Bund/WORD_REFERENCE.md is out of sync with the policy table.\n\
             MISSING (add a reference row): {missing:?}\n\
             WRONG CATEGORY: {wrong:?}\n\
             EXTRA (typo / removed word): {extra:?}",
        );
    }

    /// The pure-allowlist must not rot: every entry must still be a registered
    /// word, and must not also carry a category (which would be contradictory).
    #[test]
    fn pure_allowlist_has_no_stale_or_conflicting_entries() {
        use rust_multistackvm::multistackvm::VM;
        let mut vm = VM::new();
        crate::scripting::stdlib::register_ink_stdlib(&mut vm).unwrap();
        let gated: std::collections::HashSet<&str> =
            WORD_CATEGORIES.iter().map(|(w, _)| *w).collect();
        for word in PURE_UNCATEGORISED {
            assert!(
                vm.inline_fun.contains_key(&format!("{word}_inline")),
                "{word} is on PURE_UNCATEGORISED but is not a registered word (stale entry)",
            );
            assert!(
                !gated.contains(word),
                "{word} is on PURE_UNCATEGORISED and also in WORD_CATEGORIES — pick one",
            );
        }
    }

    #[test]
    fn default_policy_is_conservative() {
        let p = Policy::default();
        let denied = p.effective_denied_categories();
        assert!(denied.contains(category::FS_WRITE));
        assert!(denied.contains(category::NET));
        assert!(denied.contains(category::SHELL));
        assert!(denied.contains(category::CODE_EVAL));
        // Read-only categories stay open by default.
        assert!(!denied.contains(category::STORE_READ));
        assert!(!denied.contains(category::FS_READ));
    }

    #[test]
    fn no_default_deny_clears_baseline() {
        let p = Policy {
            no_default_deny: true,
            ..Policy::default()
        };
        assert!(p.effective_denied_categories().is_empty());
    }

    #[test]
    fn enabled_words_override_category_deny() {
        // User wants store_read denied wholesale, but explicitly
        // re-enables ink.node.list.
        let p = Policy {
            disabled_categories: vec![category::STORE_READ.into()],
            enabled_words: vec!["ink.node.list".into()],
            ..Policy::default()
        };
        let denied_cats = p.effective_denied_categories();
        let enabled: HashSet<&str> = p.enabled_words.iter().map(String::as_str).collect();
        // Walk every store_read word: the enabled one stays allowed.
        for (word, cat) in WORD_CATEGORIES {
            if *cat == category::STORE_READ {
                let cat_denied = denied_cats.contains(*cat);
                let effectively_denied = cat_denied && !enabled.contains(*word);
                if *word == "ink.node.list" {
                    assert!(!effectively_denied, "ink.node.list should be allowed");
                } else {
                    assert!(effectively_denied, "{word} should be denied");
                }
            }
        }
    }

    // 1.2.16+ Phase I.4.a — policy entries for the
    // new ink.review.* + ink.thread.list words.
    // The tests catch silent drift: if someone
    // refactors the WORD_CATEGORIES table and
    // forgets to keep these entries, the deny
    // contract for review writes silently lifts.

    #[test]
    fn review_list_classified_as_store_read() {
        let cat = WORD_CATEGORIES
            .iter()
            .find(|(w, _)| *w == "ink.review.list")
            .map(|(_, c)| *c);
        assert_eq!(cat, Some(category::STORE_READ));
    }

    #[test]
    fn review_writes_classified_as_store_write() {
        for word in ["ink.review.add_comment", "ink.review.resolve"] {
            let cat = WORD_CATEGORIES
                .iter()
                .find(|(w, _)| *w == word)
                .map(|(_, c)| *c);
            assert_eq!(
                cat,
                Some(category::STORE_WRITE),
                "{word} should inherit the store_write gate"
            );
        }
    }

    #[test]
    fn thread_list_classified_as_store_read() {
        let cat = WORD_CATEGORIES
            .iter()
            .find(|(w, _)| *w == "ink.thread.list")
            .map(|(_, c)| *c);
        assert_eq!(cat, Some(category::STORE_READ));
    }

    // POEM PO-P9 — the poetry engines are pure reads (default-allowed); pin them
    // so a refactor can't accidentally gate them behind a category.
    #[test]
    fn poem_words_are_store_read() {
        let cat = |w: &str| WORD_CATEGORIES.iter().find(|(n, _)| *n == w).map(|(_, c)| *c);
        for w in [
            "ink.poem.syllable_count",
            "ink.poem.scan_line",
            "ink.poem.rhyme",
            "ink.poem.status",
            "ink.poem.findings",
        ] {
            assert_eq!(cat(w), Some(category::STORE_READ), "{w} must be store_read");
        }
        // PO-P16 — suppressing a finding writes the store; it must be gated.
        assert_eq!(
            cat("ink.poem.suppress"),
            Some(category::STORE_WRITE),
            "ink.poem.suppress must be store_write (default-denied)"
        );
    }

    // 1.3.0 PDF-1 — pin the disk-crossing pdf words so a refactor can't
    // silently un-gate `ink.pdf.save` (file write).
    #[test]
    fn pdf_disk_words_classified() {
        let cat = |w: &str| WORD_CATEGORIES.iter().find(|(n, _)| *n == w).map(|(_, c)| *c);
        assert_eq!(cat("ink.pdf.load"), Some(category::FS_READ));
        assert_eq!(
            cat("ink.pdf.save"),
            Some(category::FS_WRITE),
            "ink.pdf.save must inherit the fs_write deny-by-default gate"
        );
    }

    // 1.3.1 SUBMISSION-1 — every ink.export.* word writes a file and must
    // stay fs_write (default-denied).
    #[test]
    fn export_disk_words_classified() {
        let cat = |w: &str| WORD_CATEGORIES.iter().find(|(n, _)| *n == w).map(|(_, c)| *c);
        for w in [
            "ink.export.docx",
            "ink.export.manuscript",
            "ink.export.markdown",
            "ink.export.tex",
            "ink.export.epub",
        ] {
            assert_eq!(cat(w), Some(category::FS_WRITE), "{w} must be fs_write");
        }
    }

    // 1.3.21 — pin the ink.lang.* categories so a refactor can't silently
    // un-gate the language mutators (which create book nodes) or wrongly gate
    // the read-only inspectors.
    #[test]
    fn lang_words_classified() {
        let cat = |w: &str| WORD_CATEGORIES.iter().find(|(n, _)| *n == w).map(|(_, c)| *c);
        for w in [
            "ink.lang.list",
            "ink.lang.generate_word",
            "ink.lang.syllabify",
            "ink.lang.ipa",
            "ink.lang.gloss",
            "ink.lang.sentence",
        ] {
            assert_eq!(cat(w), Some(category::STORE_READ), "{w} must be store_read");
        }
        let mutators = [
            "ink.lang.init",
            "ink.lang.define",
            "ink.lang.add_word",
            "ink.lang.remove_word",
            "ink.lang.derive_add",
            "ink.lang.grammar_set",
            "ink.lang.idiom_add",
            "ink.lang.metaphor_add",
        ];
        for w in mutators {
            assert_eq!(
                cat(w),
                Some(category::STORE_WRITE),
                "{w} must inherit the store_write deny-by-default gate"
            );
        }
        // The AI-backed words are ai_write (default-denied).
        for w in [
            "ink.lang.compose",
            "ink.lang.reconstruct",
            "ink.lang.realism_check",
            "ink.lang.generate_lexicon",
        ] {
            assert_eq!(cat(w), Some(category::AI_WRITE), "{w} must be ai_write");
        }
        // And the mutators + AI words are denied under the default policy.
        let p = Policy::default();
        let denied = p.effective_denied_categories();
        for w in mutators {
            assert!(denied.contains(cat(w).unwrap()), "{w} denied by default");
        }
        assert!(denied.contains(category::AI_WRITE), "ai_write denied by default");
        // File-output words: fs_write (glyph_lint only reads → fs_read).
        assert_eq!(cat("ink.lang.glyph_lint"), Some(category::FS_READ));
        for w in [
            "ink.lang.dictionary",
            "ink.lang.grammar_book",
            "ink.lang.font_build",
            "ink.lang.glyph_draft",
        ] {
            assert_eq!(cat(w), Some(category::FS_WRITE), "{w} must be fs_write");
            assert!(denied.contains(cat(w).unwrap()), "{w} denied by default");
        }
    }

    // 1.4.1 BOOK_RAG-1 — the whole `ink.book_rag.*` surface is read-only and
    // available by default; pin it so a refactor can't silently re-gate it
    // (it would break Book-scope scripting) or wrongly mark it destructive.
    #[test]
    fn book_rag_words_classified() {
        let cat = |w: &str| WORD_CATEGORIES.iter().find(|(n, _)| *n == w).map(|(_, c)| *c);
        let words = [
            "ink.book_rag.retrieve",
            "ink.book_rag.context",
            "ink.book_rag.scope",
            "ink.book_rag.config",
            "ink.book_rag.system_prompt",
            "ink.book_rag.estimate_tokens",
            "ink.book_rag.cited_ids",
            "ink.book_rag.validate_citations",
        ];
        let policy = Policy::default();
        let denied = policy.effective_denied_categories();
        for w in words {
            assert_eq!(cat(w), Some(category::STORE_READ), "{w} must be store_read");
            assert!(!denied.contains(cat(w).unwrap()), "{w} must be allowed by default");
        }
    }

    // 1.4.3 INNER_EDITOR-1 — pin the ink.inner_editor.* gates: inspectors stay
    // store_read (allowed), the ledger mutator store_write + engage ai_write
    // (both denied by default).
    #[test]
    fn inner_editor_words_classified() {
        let cat = |w: &str| WORD_CATEGORIES.iter().find(|(n, _)| *n == w).map(|(_, c)| *c);
        let policy = Policy::default();
        let denied = policy.effective_denied_categories();
        for w in [
            "ink.inner_editor.findings.list",
            "ink.inner_editor.usage.today",
            "ink.inner_editor.config",
            "ink.inner_editor.categories",
            "ink.inner_editor.suggestions",
            "ink.inner_editor.system_prompt",
        ] {
            assert_eq!(cat(w), Some(category::STORE_READ), "{w} must be store_read");
            assert!(!denied.contains(cat(w).unwrap()), "{w} allowed by default");
        }
        assert_eq!(cat("ink.inner_editor.intent.declare"), Some(category::STORE_WRITE));
        assert!(denied.contains(category::STORE_WRITE));
        assert_eq!(cat("ink.inner_editor.engage"), Some(category::AI_WRITE));
        assert!(denied.contains(category::AI_WRITE));
    }

    // 3.0.4 Phase-1 — the read-only feature wrappers are all store_read and must
    // stay allowed by default (they only read); pin them so a refactor can't
    // silently re-gate them or wrongly mark one destructive.
    #[test]
    fn phase1_feature_wrappers_are_store_read() {
        let cat = |w: &str| WORD_CATEGORIES.iter().find(|(n, _)| *n == w).map(|(_, c)| *c);
        let policy = Policy::default();
        let denied = policy.effective_denied_categories();
        for w in [
            "ink.rigor.scan",
            "ink.rigor.check",
            "ink.rigor.paragraph",
            "ink.planning.frameworks",
            "ink.planning.beats",
            "ink.cost.usage",
            "ink.cost.caps",
            "ink.goals.streak",
            "ink.wordnet.list",
            "ink.doctor.integrity",
            "ink.doctor.vectors",
            "ink.doctor.scan",
            "ink.backup.last",
        ] {
            assert_eq!(cat(w), Some(category::STORE_READ), "{w} must be store_read");
            assert!(!denied.contains(cat(w).unwrap()), "{w} must be allowed by default");
        }
    }

    // 3.0.4 Phase-2 — the load-bearing read wrappers. All default-allowed reads
    // (store_read, plus backup.list = fs_read); pin them so a refactor can't
    // silently re-gate them or wrongly mark one destructive.
    #[test]
    fn phase2_feature_wrappers_are_reads() {
        let cat = |w: &str| WORD_CATEGORIES.iter().find(|(n, _)| *n == w).map(|(_, c)| *c);
        let policy = Policy::default();
        let denied = policy.effective_denied_categories();
        for w in [
            "ink.wordnet.lookup",
            "ink.wordnet.suggest",
            "ink.companions.findings",
            "ink.companions.promotions",
            "ink.companions.world",
            "ink.companions.summary",
            "ink.research.facts",
            "ink.research.undisputed",
            "ink.research.provenance",
            "ink.research.sources",
            "ink.research.report",
            "ink.locorum.build",
            "ink.locorum.malformed",
            "ink.locorum.render",
            "ink.verborum.build",
            "ink.verborum.render",
            "ink.cost.today",
            "ink.goals.snapshot",
            "ink.planning.check",
            "ink.planning.gaps",
        ] {
            assert_eq!(cat(w), Some(category::STORE_READ), "{w} must be store_read");
            assert!(!denied.contains(cat(w).unwrap()), "{w} must be allowed by default");
        }
        // backup.list reads the project's backup directory → fs_read (also
        // default-allowed).
        assert_eq!(cat("ink.backup.list"), Some(category::FS_READ));
        assert!(!denied.contains(category::FS_READ), "fs_read allowed by default");
    }

    // 3.0.4 Phase-3 — the opt-in write wrappers. The importers (store_write) and
    // backup.make (fs_write) MUST be denied by default; only the Scrivener
    // dry-run preview (fs_read) is allowed. Pin the whole chain so a refactor
    // can't silently let an auto-loaded untrusted script import or backup.
    #[test]
    fn phase3_write_wrappers_default_denied() {
        let cat = |w: &str| WORD_CATEGORIES.iter().find(|(n, _)| *n == w).map(|(_, c)| *c);
        let policy = Policy::default();
        let denied = policy.effective_denied_categories();
        assert_eq!(cat("ink.import.scrivener"), Some(category::STORE_WRITE));
        assert_eq!(cat("ink.import.epub"), Some(category::STORE_WRITE));
        assert_eq!(cat("ink.doctor.autofix"), Some(category::STORE_WRITE));
        assert_eq!(cat("ink.backup.make"), Some(category::FS_WRITE));
        for w in
            ["ink.import.scrivener", "ink.import.epub", "ink.doctor.autofix", "ink.backup.make"]
        {
            assert!(denied.contains(cat(w).unwrap()), "{w} must be denied by default");
        }
        // The dry-run preview only reads — allowed by default.
        assert_eq!(cat("ink.import.scrivener_preview"), Some(category::FS_READ));
        assert!(!denied.contains(category::FS_READ), "preview allowed by default");
    }

    #[test]
    fn review_writes_default_denied() {
        // Default Policy denies STORE_WRITE; that
        // category gates the review-write words.
        // Pin the chain so a future refactor of
        // DEFAULT_DENIED_CATEGORIES doesn't
        // accidentally let scripts add comments
        // on auto-loaded untrusted projects.
        let p = Policy::default();
        let denied = p.effective_denied_categories();
        assert!(denied.contains(category::STORE_WRITE));
        for word in ["ink.review.add_comment", "ink.review.resolve"] {
            let cat = WORD_CATEGORIES
                .iter()
                .find(|(w, _)| *w == word)
                .map(|(_, c)| *c)
                .unwrap();
            assert!(
                denied.contains(cat),
                "{word} should be denied by default"
            );
        }
    }
}
