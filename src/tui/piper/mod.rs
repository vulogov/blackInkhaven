//! Piper TTS backend — cross-platform neural synthesis.
//!
//! **T.2.**  Binary resolution + auto-download (see
//! `binary.rs` and `download.rs`) ship as standalone
//! library code.  `PiperEngine::new` still always returns
//! `Err(PiperUnavailable::NotImplemented)` so the `auto`
//! engine resolver continues to fall through to the 1.2.9
//! System backend.  The engine doesn't switch to using
//! the resolver until T.5 wires synthesis + playback.
//!
//! Cross-references:
//!   * `Documentation/PROPOSALS/1.2.17_PLAN.md` — full
//!     architecture + on-disk layout + phase plan.
//!   * `src/tui/tts.rs` — the `TtsEngine` enum that wraps
//!     this backend alongside the System backend.
//!   * `binary.rs` — `Platform`, `pick_piper_release_asset`,
//!     `resolve_piper_binary`, user-cache root.
//!   * `download.rs` — curl-subprocess fetch + tar/zip
//!     extraction + atomic install of the resolved
//!     binary.

// T.2 / T.3: each submodule lands as standalone library
// code that the production engine doesn't consume until
// T.5 wires synthesis.  Tests + the future T.5 engine
// use every public item; suppress the dead-code lint at
// the module level rather than scattering #[allow]
// across every function.
#[allow(dead_code)]
pub(crate) mod binary;
#[allow(dead_code)]
pub(crate) mod catalog;
#[allow(dead_code)]
pub(crate) mod download;
#[allow(dead_code)]
pub(crate) mod lru;
#[allow(dead_code)]
pub(crate) mod voice;

use std::path::{Path, PathBuf};

use crate::config::TtsConfig;

/// Why a Piper engine couldn't be constructed.  Each
/// variant maps to a user-facing diagnostic string so the
/// engine resolver can surface a clear reason rather than
/// a generic "Piper unavailable".
#[derive(Debug, Clone)]
pub(crate) enum PiperUnavailable {
    /// Engine wiring not yet complete.  The resolver +
    /// downloader land in T.2, the catalog in T.3, the
    /// voice store in T.4, synthesis itself in T.5.
    /// Until T.5, `PiperEngine::new` always reports this
    /// so `engine: "auto"` falls through to System.
    NotImplemented,
    /// The configured `tts.binary_path` doesn't exist or
    /// isn't executable, OR the resolver searched PATH +
    /// user-cache + auto-download and found nothing.
    BinaryNotFound(PathBuf),
    /// `std::env::consts::OS` / `ARCH` doesn't map to a
    /// (PiperOs, PiperArch) pair the downloader knows how
    /// to fetch.  Carries the offending identifier.
    UnsupportedPlatform(String),
    /// GitHub Releases responded but no asset matched the
    /// current platform.  Carries the release tag for
    /// diagnostics.
    AssetNotFound { tag: String, platform: String },
    /// The download itself failed (curl non-zero exit,
    /// HTTP non-200, partial transfer, etc).  Carries the
    /// curl/HTTP error verbatim.
    DownloadFailed(String),
    /// `tar -xzf` (or `tar -xf` for .zip on Windows)
    /// failed.  Carries the extractor's stderr.
    ExtractFailed(String),
    /// SHA256 of a downloaded artefact didn't match the
    /// expected value.  Indicates either a corrupted
    /// download or a tampered upstream.  T.4 enforces
    /// this for voices; T.2 doesn't (GitHub Releases
    /// doesn't ship per-asset SHA in the API surface we
    /// query).
    #[allow(dead_code)]
    ChecksumMismatch { expected: String, actual: String },
    /// The voices directory couldn't be resolved within
    /// the project root (path-traversal defence).  Set
    /// in T.4.
    #[allow(dead_code)]
    VoicesDirInvalid(String),
}

impl PiperUnavailable {
    pub(crate) fn to_user_message(&self) -> String {
        match self {
            Self::NotImplemented => {
                "Piper synthesis lands in 1.2.17 T.5.  T.2 wired \
                 binary resolution + auto-download; T.3/T.4 add the \
                 voice catalog + auto-download.  Until T.5, \
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
            Self::UnsupportedPlatform(detail) => format!(
                "Piper has no prebuilt binary for this host: {detail}.  \
                 Supported: macOS aarch64/x86_64, Linux \
                 x86_64/aarch64/armv7, Windows x86_64.  You can \
                 still build piper from source and set \
                 tts.binary_path.",
            ),
            Self::AssetNotFound { tag, platform } => format!(
                "Piper release {tag} has no asset for {platform}.  \
                 Try `inkhaven tts binary download --tag <older-tag>` \
                 (lands in T.7) or set tts.binary_path manually.",
            ),
            Self::DownloadFailed(detail) => format!(
                "Piper download failed: {detail}.  Check network + \
                 try again; the partial download is cleaned up.  If \
                 your network blocks GitHub, set tts.binary_path to \
                 a hand-installed copy.",
            ),
            Self::ExtractFailed(detail) => format!(
                "Piper archive extraction failed: {detail}.  Make \
                 sure `tar` is on PATH (universal on macOS / Linux; \
                 Windows 10 1803+ ships `tar.exe`).",
            ),
            Self::ChecksumMismatch { expected, actual } => format!(
                "Checksum mismatch — expected {expected}, got \
                 {actual}.  The download is corrupt or the upstream \
                 was tampered with; refusing to install.",
            ),
            Self::VoicesDirInvalid(detail) => format!(
                "tts.voices_dir is invalid: {detail}.  The path must \
                 resolve inside the project root (relative paths are \
                 joined to it).",
            ),
        }
    }
}

/// Piper TTS engine handle.  T.2 still stub — synthesis
/// methods land in T.5.
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
    /// `cfg`.  T.2 still returns
    /// `Err(PiperUnavailable::NotImplemented)`; T.5 will
    /// resolve the binary via `binary::resolve_piper_binary`
    /// and switch over.
    pub(crate) fn new(
        _cfg: &TtsConfig,
        _project_root: &Path,
    ) -> Result<Self, PiperUnavailable> {
        Err(PiperUnavailable::NotImplemented)
    }

    /// T.5 will implement.  Returns a clear stub error so a
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

    /// T.5 will implement.  Stub returns false.
    #[allow(dead_code)]
    pub(crate) fn is_speaking(&mut self) -> bool {
        false
    }

    /// T.5 will implement.  Stub is a no-op.
    #[allow(dead_code)]
    pub(crate) fn stop(&mut self) {}

    /// T.3 will implement — resolves a voice needle
    /// against the catalog (e.g. "irina" → "ru_RU-irina-
    /// medium").  T.2 stub returns the needle unchanged.
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
    fn stub_still_returns_not_implemented() {
        // T.2 invariant: PiperEngine::new continues to
        // return NotImplemented so the `auto` engine
        // resolver falls through to System.  T.5 will
        // remove this guard.
        let cfg = TtsConfig::default();
        let tmp = std::env::temp_dir();
        let result = PiperEngine::new(&cfg, &tmp);
        assert!(matches!(
            result,
            Err(PiperUnavailable::NotImplemented)
        ));
    }

    #[test]
    fn user_message_mentions_t5() {
        let msg = PiperUnavailable::NotImplemented.to_user_message();
        assert!(
            msg.contains("T.5"),
            "expected message to reference T.5, got: {msg}",
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
    fn unsupported_platform_message_lists_supported() {
        let msg = PiperUnavailable::UnsupportedPlatform("arch=mips".into())
            .to_user_message();
        assert!(msg.contains("mips"));
        assert!(msg.contains("aarch64"));
        assert!(msg.contains("x86_64"));
    }

    #[test]
    fn asset_not_found_message_carries_tag() {
        let msg = PiperUnavailable::AssetNotFound {
            tag: "2024.01.01".into(),
            platform: "darwin-aarch64".into(),
        }
        .to_user_message();
        assert!(msg.contains("2024.01.01"));
        assert!(msg.contains("darwin-aarch64"));
    }

    #[test]
    fn download_failed_message_suggests_recovery() {
        let msg = PiperUnavailable::DownloadFailed("curl: 7".into())
            .to_user_message();
        assert!(msg.contains("network"));
        assert!(msg.contains("tts.binary_path"));
    }

    #[test]
    fn extract_failed_message_mentions_tar() {
        let msg = PiperUnavailable::ExtractFailed("bad header".into())
            .to_user_message();
        assert!(msg.contains("tar"));
    }

    #[test]
    fn engine_struct_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PiperEngine>();
    }

    #[test]
    fn piper_unavailable_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PiperUnavailable>();
    }
}
