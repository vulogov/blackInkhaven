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
}

/// Categories denied out of the box. A user has to actively flip
/// these on in `inkhaven.hjson` to use destructive operations.
/// Currently inkhaven registers zero words in these categories —
/// the deny is forward-looking, ready for P4/P5 additions.
pub const DEFAULT_DENIED_CATEGORIES: &[&str] = &[
    category::STORE_WRITE,
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
    ("ink.node.list", category::STORE_READ),
    ("ink.node.get", category::STORE_READ),
    ("ink.node.children", category::STORE_READ),
    ("ink.paragraph.text", category::STORE_READ),
    ("ink.search.text", category::STORE_READ),
    ("ink.snapshot.list", category::STORE_READ),
    ("ink.path.to_uuid", category::STORE_READ),

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
    ("ink.paragraph.save", category::STORE_WRITE),
    ("ink.db.sync", category::STORE_WRITE),
    ("ink.db.checkpoint", category::STORE_WRITE),
    ("ink.db.reindex", category::STORE_WRITE),

    // ── keymap (default-denied) ───────────────────────────────
    ("ink.key.bind", category::KEYMAP),
    ("ink.key.bind_lambda", category::KEYMAP),
    ("ink.key.unbind", category::KEYMAP),
    ("ink.key.list", category::KEYMAP),
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

    /// Bund script run once after Adam is constructed, after stdlib
    /// registration, after policy application. The natural home for
    /// defining hook lambdas (`hook.on_save`, `hook.on_rename`, …)
    /// and any custom user words. Empty = no bootstrap.
    #[serde(default)]
    pub bootstrap: String,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            disabled_categories: Vec::new(),
            disabled_words: Vec::new(),
            enabled_words: Vec::new(),
            enabled_categories: Vec::new(),
            no_default_deny: false,
            bootstrap: String::new(),
        }
    }
}

impl Policy {
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
}
