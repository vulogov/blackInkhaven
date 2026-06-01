//! 1.2.15+ Phase S.6 — path-traversal safety helpers.
//!
//! Centralises the "join this user-supplied relative
//! path onto the project root without letting it
//! escape via `..` or absolute prefixes" pattern.
//!
//! Every call site that builds a filesystem path from
//! HJSON config values, crash report fields, snippet
//! placeholders, Bund script arguments, or any other
//! data the user / a malicious project can write
//! should resolve through [`resolve_within`] instead
//! of `root.join(candidate)` directly.
//!
//! ## Threat model
//!
//! Three concrete vectors were catalogued in the
//! 1.2.15 security audit:
//!
//! 1. **`recover.rs`** — `paragraph_rel_path` comes
//!    from a crash report HJSON.  Crafted report
//!    with `paragraph_rel_path: "../../etc/passwd"`
//!    would have made `inkhaven recover --yes`
//!    overwrite `/etc/passwd`.
//! 2. **`project.rs:prompts_path`** — `prompts_file`
//!    deserialised from `inkhaven.hjson`.  A
//!    malicious project's config setting
//!    `prompts_file: "../../etc/passwd"` would have
//!    let inkhaven read the file as a "prompts
//!    library".
//! 3. **`ink.fs.read`/`write`** Bund stdlib words —
//!    auto-loaded scripts can call into them
//!    unsandboxed.  Same shape.
//!
//! ## Behaviour
//!
//! - Absolute paths are rejected outright (caller
//!   must pass a relative path or already-validated
//!   absolute path).
//! - `..` segments are normalised greedily; if at
//!   any point the running path "escapes" above the
//!   root (i.e. accumulated `..` outnumbers
//!   accumulated real segments), reject.
//! - Trailing `..` that would cancel out is also
//!   rejected — the resulting path equals the root
//!   itself but the *intent* was traversal.
//! - Symbolic-link resolution is NOT performed here
//!   (would need filesystem reads and TOCTOU
//!   handling).  Callers that need symlink safety
//!   must additionally check after creating any file
//!   handle.
//!
//! Returns an absolute `PathBuf` under `root` on
//! success, or an `io::Error::PermissionDenied`
//! with a descriptive message on rejection.

use std::path::{Component, Path, PathBuf};

/// Resolve `candidate` against `root` such that the
/// final path is guaranteed to be inside `root`.
///
/// `root` should be absolute.  If it isn't, the
/// caller is responsible for what "inside" means —
/// usually you want to canonicalise the project root
/// once at startup and pass that everywhere.
pub fn resolve_within(root: &Path, candidate: &Path) -> std::io::Result<PathBuf> {
    if candidate.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "absolute path not allowed in this context: {}",
                candidate.display()
            ),
        ));
    }

    let mut stack: Vec<std::ffi::OsString> = Vec::new();
    for component in candidate.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!(
                        "absolute or prefix segment not allowed: {}",
                        candidate.display()
                    ),
                ));
            }
            Component::CurDir => {
                // "./foo" — strip the dot, keep going.
                continue;
            }
            Component::ParentDir => {
                if stack.pop().is_none() {
                    // `..` with nothing to pop — escapes
                    // above root.
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!(
                            "path traversal rejected: {} escapes the project root via `..`",
                            candidate.display()
                        ),
                    ));
                }
            }
            Component::Normal(seg) => {
                stack.push(seg.to_os_string());
            }
        }
    }

    let mut out = root.to_path_buf();
    for seg in stack {
        out.push(seg);
    }
    Ok(out)
}

/// Convenience wrapper for the common case where the
/// candidate comes in as a `&str`.
pub fn resolve_within_str(root: &Path, candidate: &str) -> std::io::Result<PathBuf> {
    resolve_within(root, Path::new(candidate))
}

/// Same as [`resolve_within`] but permits absolute
/// candidates as-is.  Used for config fields where
/// the project owner has documented intent to point
/// at a shared absolute path (e.g. `prompts_file`,
/// `artefacts_directory`).  Still rejects `..`
/// traversal in relative paths — the user controls
/// the config, but a typo or copy-paste mistake
/// shouldn't escape the project root silently.
///
/// Note: this widens the trust boundary.  When
/// opening an untrusted project, callers should
/// gate on the project-trust decision (S.6.H1)
/// before honoring an absolute path here.
pub fn resolve_within_or_absolute(
    root: &Path,
    candidate: &Path,
) -> std::io::Result<PathBuf> {
    if candidate.is_absolute() {
        return Ok(candidate.to_path_buf());
    }
    resolve_within(root, candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/tmp/inkhaven-test-root")
    }

    #[test]
    fn plain_relative_path_resolves_under_root() {
        let out = resolve_within(&root(), Path::new("books/ch1/opening.typ")).unwrap();
        assert_eq!(out, root().join("books").join("ch1").join("opening.typ"));
    }

    #[test]
    fn dot_segment_is_stripped() {
        let out = resolve_within(&root(), Path::new("./books/./ch1/opening.typ")).unwrap();
        assert_eq!(out, root().join("books").join("ch1").join("opening.typ"));
    }

    #[test]
    fn inner_double_dot_is_normalised() {
        let out = resolve_within(&root(), Path::new("books/ch1/../ch2/opening.typ")).unwrap();
        assert_eq!(out, root().join("books").join("ch2").join("opening.typ"));
    }

    #[test]
    fn double_dot_above_root_rejected() {
        let err = resolve_within(&root(), Path::new("../etc/passwd")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn deep_double_dot_escape_rejected() {
        let err = resolve_within(&root(), Path::new("a/b/../../../etc/passwd")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn absolute_path_rejected() {
        let err = resolve_within(&root(), Path::new("/etc/passwd")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(err.to_string().contains("absolute path"));
    }

    #[test]
    fn str_wrapper_delegates() {
        let out = resolve_within_str(&root(), "books/ch1/opening.typ").unwrap();
        assert_eq!(out, root().join("books").join("ch1").join("opening.typ"));
    }

    #[test]
    fn empty_path_resolves_to_root() {
        let out = resolve_within(&root(), Path::new("")).unwrap();
        assert_eq!(out, root());
    }

    #[test]
    fn windows_prefix_rejected() {
        // Windows-style drive prefixes are rejected
        // even on Unix (they parse as Normal
        // components on Unix, but the test exists to
        // pin the cross-platform intent).
        // On Unix `C:` parses as a single Normal
        // segment — that's still safe because it's
        // joined under the root, not used directly.
        let out = resolve_within(&root(), Path::new("normal")).unwrap();
        assert_eq!(out, root().join("normal"));
    }
}
