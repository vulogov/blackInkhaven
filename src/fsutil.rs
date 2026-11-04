//! Small, dependency-free filesystem helpers shared across
//! subsystems (Piper voice install, book assembly, …) so that
//! no caller has to reach into another feature's module for a
//! generic "copy this tree" primitive.

use std::path::Path;

/// Recursively copy `src` into `dst`, creating `dst` as needed.
///
/// Symbolic links are skipped — neither Piper's tarballs nor a
/// project's shared folders need them, and blindly resolving
/// symlinks while copying is a known path-traversal vector
/// (a link inside the tree could pull in files from outside
/// it). Only the *entries* are checked: callers that accept an
/// untrusted `src` should `symlink_metadata` the root
/// themselves. Errors propagate verbatim.
pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            // Skip — see fn-level docs.
            continue;
        }
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
            // std::fs::copy preserves Unix mode bits; no extra
            // chmod needed for executables.
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_nested_tree_byte_for_byte() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(src.join("a/b")).unwrap();
        std::fs::write(src.join("top.txt"), b"top").unwrap();
        std::fs::write(src.join("a/b/deep.bin"), b"\x00\x01\x02").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert_eq!(std::fs::read(dst.join("top.txt")).unwrap(), b"top");
        assert_eq!(std::fs::read(dst.join("a/b/deep.bin")).unwrap(), b"\x00\x01\x02");
    }

    #[test]
    fn creates_dst_and_tolerates_empty_src() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("empty");
        let dst = tmp.path().join("out/nested");
        std::fs::create_dir_all(&src).unwrap();
        copy_dir_recursive(&src, &dst).unwrap();
        assert!(dst.is_dir());
        assert_eq!(std::fs::read_dir(&dst).unwrap().count(), 0);
    }

    #[test]
    fn missing_src_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let err = copy_dir_recursive(&tmp.path().join("nope"), &tmp.path().join("dst"))
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_entries_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside.txt");
        std::fs::write(&outside, b"secret").unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("ok.txt"), b"ok").unwrap();
        std::os::unix::fs::symlink(&outside, src.join("link.txt")).unwrap();
        std::os::unix::fs::symlink(tmp.path(), src.join("sub/escape")).unwrap();

        let dst = tmp.path().join("dst");
        copy_dir_recursive(&src, &dst).unwrap();

        assert_eq!(std::fs::read(dst.join("ok.txt")).unwrap(), b"ok");
        assert!(!dst.join("link.txt").exists(), "file symlink must be skipped");
        assert!(!dst.join("sub/escape").exists(), "dir symlink must be skipped");
        assert!(dst.join("sub").is_dir());
    }
}
