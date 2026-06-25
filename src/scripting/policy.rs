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
    // 1.2.16+ Phase I.4.a — threads (read) +
    // review (read).
    ("ink.thread.list", category::STORE_READ),
    ("ink.review.list", category::STORE_READ),

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
