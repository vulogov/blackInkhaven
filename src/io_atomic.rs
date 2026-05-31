//! 1.2.15+ Phase S.4 — atomic file write helpers.
//!
//! Standalone module for "write to disk such that a
//! crash mid-write leaves either the old contents or
//! the new, never a half-written file".  Implemented
//! as the well-known temp + fsync + rename + parent-
//! dir fsync idiom.
//!
//! Used by the panic-hook rescue flush in
//! [`crate::crash`] (where the same atomicity is what
//! makes rescue buffers reliable) and by every
//! paragraph save / sidecar save in the editor.
//!
//! Before 1.2.15 those callers used `std::fs::write`
//! which truncates the target THEN writes — a power
//! loss or `kill -9` between the truncate and the
//! write would leave the user with an empty file.
//! The atomic flow writes to a side-by-side temp
//! and only swaps it in once the bytes are durably
//! on disk.
//!
//! POSIX details:
//!
//! 1. `OpenOptions::write|create|truncate` opens the
//!    temp file.
//! 2. `write_all` lands the bytes in the kernel page
//!    cache.
//! 3. `sync_all` flushes the file (data + metadata)
//!    to the device.
//! 4. `rename` does the atomic swap.
//! 5. On Unix, `open(parent) + sync_all` durably
//!    commits the directory entry pointing at the
//!    new inode — without this the swap can roll
//!    back on power loss.  Windows: skipped (you
//!    can't open a directory as a file).
//!
//! Failure at any step bubbles to the caller as a
//! `std::io::Error`.  Partial state cleanup is
//! best-effort: the temp file may be left behind if
//! a step before rename fails.  Doctor scan picks
//! these up as `*.tmp` orphans.

use std::io::Write;
use std::path::Path;

/// Atomic write.  See module docs for the durability
/// guarantee.
pub fn write(target: &Path, body: &[u8]) -> std::io::Result<()> {
    let parent = target.parent().unwrap_or(Path::new("."));
    let tmp_name = match target.file_name() {
        Some(name) => {
            let mut s = name.to_os_string();
            s.push(".tmp");
            s
        }
        None => return Err(std::io::Error::other("io_atomic: target has no file_name")),
    };
    let tmp = parent.join(tmp_name);

    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)?;
    f.write_all(body)?;
    f.sync_all()?;
    drop(f);

    std::fs::rename(&tmp, target)?;

    #[cfg(unix)]
    {
        if let Ok(d) = std::fs::File::open(parent) {
            let _ = d.sync_all();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_replaces_existing_atomically() {
        let dir = std::env::temp_dir().join(format!(
            "io-atomic-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("doc.txt");

        // First write — creates the file.
        write(&target, b"first").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "first");

        // Second write — overwrites atomically.
        write(&target, b"second").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "second");

        // No tmp orphan.
        let tmp = dir.join("doc.txt.tmp");
        assert!(!tmp.exists(), "tmp file should have been renamed away");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_to_missing_parent_returns_error_no_panic() {
        let target = std::env::temp_dir()
            .join(format!("io-atomic-missing-{}", std::process::id()))
            .join("subdir-that-does-not-exist")
            .join("doc.txt");
        let err = write(&target, b"hi").unwrap_err();
        // io::ErrorKind::NotFound or PermissionDenied
        // are the expected shapes; we don't pin a
        // specific kind because POSIX vs Windows
        // differ.  What matters: no panic, error
        // returned.
        assert!(matches!(
            err.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
        ), "got {err:?}");
    }
}
