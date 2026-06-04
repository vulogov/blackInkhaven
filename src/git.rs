//! Best-effort git working-tree status (1.2.20+ Phase G).
//!
//! Inkhaven has no git dependency — a project is plain
//! files the user version-controls however they like.
//! For the quit-time "uncommitted changes" warning we
//! simply shell out to the user's own `git`, parse
//! `status --porcelain`, and treat any failure (not a
//! repo, git not installed, the command erroring) as
//! "nothing to warn about".  Never errors; never blocks
//! the user from quitting.

use std::path::Path;
use std::process::Command;

/// Number of changed entries in the working tree at
/// `root` per `git status --porcelain` (modified, staged,
/// and untracked paths all count).  Returns `None` when
/// `root` isn't a git repo, git is unavailable, or the
/// command fails — callers treat `None` as "don't warn".
pub fn uncommitted_count(root: &Path) -> Option<usize> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("status")
        .arg("--porcelain")
        .output()
        .ok()?;
    if !output.status.success() {
        // Not a repository, or git refused — nothing to say.
        return None;
    }
    let count = output
        .stdout
        .split(|&b| b == b'\n')
        .filter(|line| !line.is_empty())
        .count();
    Some(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_repo_dir_is_none() {
        // A fresh temp dir is not a git repo (and if git is
        // absent the command simply fails) — either way the
        // result is None, so the quit warning stays silent.
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(uncommitted_count(tmp.path()), None);
    }
}
