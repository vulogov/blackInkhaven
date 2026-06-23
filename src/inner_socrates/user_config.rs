//! The user-level configuration layer (RFC §3.12 / §8.27) — `~/.config/inkhaven/`,
//! XDG-respecting, the home for cross-project personas and genres. Project config
//! wins over user, which wins over bundled defaults. The directory is not created
//! eagerly; callers that write to it ensure it exists.

use std::path::PathBuf;

/// The user-level config root: `$XDG_CONFIG_HOME/inkhaven` if set, else
/// `~/.config/inkhaven`. `None` only if neither `$XDG_CONFIG_HOME` nor `$HOME` is
/// set (a degenerate environment).
pub fn root() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(xdg).join("inkhaven"));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config").join("inkhaven"))
}

/// `~/.config/inkhaven/personas/` — user-level personas, available in any project.
pub fn personas_dir() -> Option<PathBuf> {
    root().map(|r| r.join("personas"))
}

/// `~/.config/inkhaven/genres/` — user-level genre overrides.
pub fn genres_dir() -> Option<PathBuf> {
    root().map(|r| r.join("genres"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respects_xdg_then_home() {
        // We don't mutate process env in a shared test; just assert the shape is
        // well-formed and the subdirectories hang off the root.
        if let Some(r) = root() {
            assert_eq!(personas_dir().unwrap(), r.join("personas"));
            assert_eq!(genres_dir().unwrap(), r.join("genres"));
            assert!(r.ends_with("inkhaven"));
        }
    }
}
