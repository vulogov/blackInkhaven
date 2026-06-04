//! Free-disk-space pre-flight (1.2.20+ Phase G).
//!
//! A thin, best-effort wrapper over `fs2` so the rest of
//! the code can ask "how much room is left where this
//! project lives?" without caring about platform
//! statvfs / `GetDiskFreeSpaceEx` details.
//!
//! Atomic writes (`crate::io_atomic`) already fail
//! cleanly when the disk is full — the original file
//! survives and the error surfaces.  This module is the
//! *proactive* half: warn the user **before** a long
//! export or a session's worth of edits runs the volume
//! dry, while there's still time to free space.

use std::path::Path;

/// Bytes available to the calling user on the filesystem
/// that holds `path`.  Best-effort: returns `None` if the
/// query fails (path missing, unsupported platform, …) so
/// callers treat "unknown" as "don't warn" rather than
/// crashing.
pub fn available_bytes(path: &Path) -> Option<u64> {
    fs2::available_space(path).ok()
}

/// True when the volume holding `path` has less than
/// `threshold_mb` mebibytes available.  `threshold_mb == 0`
/// disables the check (returns `false`).  Unknown space
/// (query failure) is treated as "not low" so a failed
/// probe never nags the user.
pub fn is_low(path: &Path, threshold_mb: u64) -> bool {
    if threshold_mb == 0 {
        return false;
    }
    match available_bytes(path) {
        Some(avail) => avail < threshold_mb.saturating_mul(1024 * 1024),
        None => false,
    }
}

/// Human-readable byte size for status messages: `MB`
/// under a gigabyte, `GB` (one decimal) above.
pub fn human(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else {
        format!("{} MB", bytes / MB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_bytes_for_temp_dir_is_some() {
        // The temp dir always exists on a test host; the
        // query should succeed and report a positive size.
        let n = available_bytes(&std::env::temp_dir());
        assert!(n.is_some());
        assert!(n.unwrap() > 0);
    }

    #[test]
    fn zero_threshold_disables_check() {
        assert!(!is_low(&std::env::temp_dir(), 0));
    }

    #[test]
    fn unknown_path_is_not_low() {
        // A non-existent path → query fails → treated as
        // "not low" (never nag on a failed probe).
        let missing = Path::new("/this/path/should/not/exist/inkhaven");
        assert!(!is_low(missing, 100));
    }

    #[test]
    fn human_formats_mb_and_gb() {
        assert_eq!(human(50 * 1024 * 1024), "50 MB");
        assert_eq!(human(2 * 1024 * 1024 * 1024), "2.0 GB");
    }
}
