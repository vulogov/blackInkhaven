//! Piper TTS backend — cross-platform neural synthesis.
//!
//! **T.1 stub.**  This module's job in T.1 is to land the
//! type + constructor so the `TtsEngine::Piper` variant
//! compiles and dispatches through the enum like the
//! existing `System(Say)` backend.  The actual synthesis
//! pipeline + binary auto-download + voice catalog lands
//! across T.2–T.5.
//!
//! Constructor currently always returns
//! `Err(PiperUnavailable::NotImplemented)` so the `auto`
//! engine resolver falls through to the 1.2.9 System
//! backend on every host.  Tests assert this contract so a
//! future T.2 implementation can't accidentally regress the
//! T.1 "no behavioural change" guarantee.
//!
//! Cross-references:
//!   * `Documentation/PROPOSALS/1.2.17_PLAN.md` — full
//!     architecture + on-disk layout + phase plan.
//!   * `src/tui/tts.rs` — the `TtsEngine` enum that wraps
//!     this backend alongside the System backend.

use std::path::{Path, PathBuf};

use crate::config::TtsConfig;

/// Why a Piper engine couldn't be constructed.  Each
/// variant maps to a user-facing diagnostic string so the
/// engine resolver can surface a clear reason rather than
/// a generic "Piper unavailable".
#[derive(Debug, Clone)]
pub(crate) enum PiperUnavailable {
    /// T.1 placeholder — Piper backend not yet wired.  The
    /// `auto` engine resolver treats this as "fall back to
    /// System".  Removed in T.2.
    NotImplemented,
    /// The configured `tts.binary_path` doesn't exist or
    /// isn't executable.  Set in T.2.
    #[allow(dead_code)]
    BinaryNotFound(PathBuf),
    /// The voices directory couldn't be resolved within the
    /// project root (path traversal defence).  Set in T.4.
    #[allow(dead_code)]
    VoicesDirInvalid(String),
}

impl PiperUnavailable {
    pub(crate) fn to_user_message(&self) -> String {
        match self {
            Self::NotImplemented => {
                "Piper backend lands in 1.2.17 T.2+.  \
                 T.1 wires the engine abstraction only; \
                 `tts.engine = \"auto\"` falls through to the \
                 1.2.9 System backend on every host."
                    .to_string()
            }
            Self::BinaryNotFound(p) => format!(
                "Piper binary not found at {}.  Set \
                 tts.binary_path or enable \
                 tts.auto_download_binary.",
                p.display(),
            ),
            Self::VoicesDirInvalid(detail) => format!(
                "tts.voices_dir is invalid: {detail}.  The \
                 path must resolve inside the project root \
                 (relative paths are joined to it).",
            ),
        }
    }
}

/// Piper TTS engine handle.  T.1 stub — synthesis methods
/// land in T.5.
///
/// Owns the resolved binary path + voices directory so
/// every `speak` call can dispatch without re-resolution.
/// Send + Sync — the engine sits on `App` which crosses
/// the tokio monitor-task boundary in the 1.2.16 P.4-pre
/// pattern.
#[derive(Debug)]
pub(crate) struct PiperEngine {
    #[allow(dead_code)]
    binary: PathBuf,
    #[allow(dead_code)]
    voices_dir: PathBuf,
    #[allow(dead_code)]
    project_root: PathBuf,
}

impl PiperEngine {
    /// Construct a Piper engine for `project_root` using
    /// `cfg`.  T.1 always returns
    /// `Err(PiperUnavailable::NotImplemented)`; T.2+
    /// will resolve the binary + voices directory.
    pub(crate) fn new(
        _cfg: &TtsConfig,
        _project_root: &Path,
    ) -> Result<Self, PiperUnavailable> {
        Err(PiperUnavailable::NotImplemented)
    }

    /// T.5 will implement.  Returns a clear T.1 error so a
    /// caller that bypasses the engine resolver and
    /// constructs `PiperEngine` directly fails loudly.
    #[allow(dead_code)]
    pub(crate) fn speak(
        &mut self,
        _text: &str,
        _voice: &str,
        _rate_wpm: Option<u16>,
    ) -> Result<(), String> {
        Err(PiperUnavailable::NotImplemented.to_user_message())
    }

    /// T.5 will implement.
    #[allow(dead_code)]
    pub(crate) fn speak_to_file_blocking(
        &mut self,
        _text: &str,
        _voice: &str,
        _rate_wpm: Option<u16>,
        _dest: &Path,
        _timeout: std::time::Duration,
    ) -> Result<u64, String> {
        Err(PiperUnavailable::NotImplemented.to_user_message())
    }

    /// T.5 will implement.  Stub returns false (nothing is
    /// ever playing).
    #[allow(dead_code)]
    pub(crate) fn is_speaking(&mut self) -> bool {
        false
    }

    /// T.5 will implement.  Stub is a no-op.
    #[allow(dead_code)]
    pub(crate) fn stop(&mut self) {}

    /// T.3 will implement — resolves a voice needle
    /// against the catalog (e.g. "irina" → "ru_RU-irina-
    /// medium").  T.1 stub returns the needle unchanged.
    #[allow(dead_code)]
    pub(crate) fn resolve_voice(&self, needle: &str) -> String {
        needle.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TtsConfig;

    #[test]
    fn t1_stub_always_returns_not_implemented() {
        let cfg = TtsConfig::default();
        let tmp = std::env::temp_dir();
        let result = PiperEngine::new(&cfg, &tmp);
        assert!(matches!(
            result,
            Err(PiperUnavailable::NotImplemented)
        ));
    }

    #[test]
    fn user_message_mentions_t2() {
        let msg = PiperUnavailable::NotImplemented.to_user_message();
        assert!(
            msg.contains("T.2"),
            "expected message to reference T.2, got: {msg}",
        );
    }

    #[test]
    fn binary_not_found_message_includes_path() {
        let p = PathBuf::from("/nowhere/piper");
        let msg = PiperUnavailable::BinaryNotFound(p).to_user_message();
        assert!(msg.contains("/nowhere/piper"));
        assert!(msg.contains("tts.binary_path"));
    }

    #[test]
    fn engine_struct_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PiperEngine>();
    }
}
