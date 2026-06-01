//! Filesystem read / write words for Bund (1.2.4+).
//!
//! Policy:
//! * `ink.fs.read` → `fs_read` category, **default-allowed**.
//!   Reading is non-destructive; lets bundled prompts /
//!   templates pull project-external content.
//! * `ink.fs.write` → `fs_write` category, **default-denied**.
//!   Users opt in via `scripting.enabled_categories:
//!   ["fs_write"]`.
//!
//! 1.2.15+ Phase S.6 (H2) — both words confine
//! paths to the project root by default via
//! [`crate::path_safety::resolve_within`].  A path
//! resolving outside the project (absolute, `..`-
//! escape, etc.) is rejected with a security
//! error.  Power users can disable confinement
//! with `scripting.fs_unsandboxed: true`.
//!
//! UTF-8 is assumed on read; non-UTF-8 bytes
//! surface as a clean error rather than a panic.
//! Writes overwrite existing files (within the
//! sandbox).

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{pull, push, require_depth, value_to_string};

pub fn register(vm: &mut VM) -> Result<()> {
    vm.register_inline("ink.fs.read".to_string(), ink_fs_read)
        .map_err(|e| anyhow!("register ink.fs.read: {e}"))?;
    vm.register_inline("ink.fs.write".to_string(), ink_fs_write)
        .map_err(|e| anyhow!("register ink.fs.write: {e}"))?;
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

// ── ink.fs.read ─────────────────────────────────────────────────────
// Stack: ( path -- string )
// Reads the file at `path` as UTF-8 text. Returns the file's
// contents on success; errors on missing file / read failure /
// non-UTF-8 bytes.

fn ink_fs_read(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_fs_read(vm).map_err(to_bund_err)
}

fn do_ink_fs_read(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.fs.read";
    require_depth(vm, 1, tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let resolved = resolve_fs_path(tag, &path)?;
    let bytes = std::fs::read(&resolved)
        .map_err(|e| anyhow!("{tag} `{path}`: {e}"))?;
    let s = String::from_utf8(bytes)
        .map_err(|e| anyhow!("{tag} `{path}`: not UTF-8: {e}"))?;
    push(vm, Value::from_string(s));
    Ok(vm)
}

// ── ink.fs.write ────────────────────────────────────────────────────
// Stack: ( path content -- )
// Writes `content` to `path`, creating the file if needed and
// overwriting any existing contents. Errors on directory-write
// (path is a dir) / permission failure / disk-full.

fn ink_fs_write(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_fs_write(vm).map_err(to_bund_err)
}

fn do_ink_fs_write(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.fs.write";
    require_depth(vm, 2, tag)?;
    let content = value_to_string(pull(vm, tag)?, "content", tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let resolved = resolve_fs_path(tag, &path)?;
    // 1.2.15+ Phase S.4 — atomic write so a script
    // crash mid-write can't truncate the user's
    // file.  Combined with the S.6 sandbox above:
    // confined target + atomic flow.
    crate::io_atomic::write(&resolved, content.as_bytes())
        .map_err(|e| anyhow!("{tag} `{path}`: {e}"))?;
    Ok(vm)
}

/// 1.2.15+ Phase S.6 (H2) — resolve a Bund-supplied
/// path against the project root.  Returns the
/// absolute path on success or a security error on
/// rejection.
///
/// Behaviour:
///   * No active store registered → reject (Bund
///     scripts run from CLI flows without
///     `--project` can't use `ink.fs.*` against
///     the project tree because they don't HAVE
///     one).  Power users can fall back to
///     `scripting.fs_unsandboxed: true` if they
///     genuinely need that.
///   * Active store + sandbox enabled → confine
///     via `path_safety::resolve_within`.
///   * Active store + `fs_unsandboxed: true` →
///     pass through as-is (legacy behaviour).
fn resolve_fs_path(tag: &str, raw: &str) -> Result<std::path::PathBuf> {
    let unsandboxed = crate::scripting::active_policy()
        .map(|p| p.fs_unsandboxed)
        .unwrap_or(false);
    if unsandboxed {
        return Ok(std::path::PathBuf::from(raw));
    }
    let store = crate::scripting::active_store().ok_or_else(|| {
        anyhow!(
            "{tag} `{raw}`: rejected — no project store registered; \
             paths can't be confined.  Enable `scripting.fs_unsandboxed: true` \
             to opt out (trusted projects only)."
        )
    })?;
    let root = store.project_root();
    crate::path_safety::resolve_within_str(root, raw).map_err(|e| {
        anyhow!(
            "{tag} `{raw}`: rejected by sandbox: {e}.  \
             Enable `scripting.fs_unsandboxed: true` to opt out (trusted projects only)."
        )
    })
}
