//! Piper TTS backend — cross-platform neural synthesis.
//!
//! **T.5.**  Engine fully wired end-to-end: `PiperEngine::new`
//! resolves the binary via PATH + user cache (no startup
//! auto-download — that's an explicit user action via T.7);
//! the first `speak()` lazily loads the catalog + ensures
//! the requested voice is downloaded; synthesis runs
//! Piper as a subprocess (stdin text, WAV stdout) and
//! playback dispatches via the platform default
//! (`afplay` / `paplay` → `aplay` / PowerShell SoundPlayer)
//! or `tts.play_command` override.
//!
//! With T.5 landed, `tts.engine = "auto"` now routes to
//! Piper on hosts where the binary resolves and falls
//! through to System otherwise.  On macOS without Piper
//! installed the user still gets `say` (no regression);
//! T.6/T.7 give them surfaces to bring Piper online.
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
//!   * `catalog.rs` — voice catalog parse + cache +
//!     freshness-first load policy.
//!   * `lru.rs` — least-recently-used eviction index.
//!   * `voice.rs` — per-voice download + atomic install
//!     + .gitignore append.
//!   * `synth.rs` — synthesis subprocess + playback
//!     dispatch + WPM→length_scale mapping.

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
pub(crate) mod synth;
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

/// Piper TTS engine handle.
///
/// Resolved at TUI startup.  Holds the binary path
/// (looked up from PATH + user cache at construction —
/// no network call at startup; explicit download is
/// triggered by `inkhaven tts binary download` in T.7
/// or the voice picker in T.6), the voices directory
/// (path-safety-resolved under the project root), and
/// the platform identifier.
///
/// Catalog + per-voice download happen lazily on the
/// first `speak()` call so the TUI starts fast even on
/// a project that doesn't end up using TTS.
///
/// Playback subprocess + last-synthesised WAV are
/// tracked so `is_speaking` / `stop` work, and so the
/// next `speak` can clean up the prior WAV file.
#[derive(Debug)]
pub(crate) struct PiperEngine {
    cfg: TtsConfig,
    project_root: PathBuf,
    voices_dir: PathBuf,
    platform: binary::Platform,
    /// Resolved at `new()` time.  No more I/O needed
    /// for speak's binary lookup.
    binary: PathBuf,
    /// Loaded on first `speak()`.  None initially.
    catalog: Option<catalog::Catalog>,
    /// Last `speak()` synth output — kept so the next
    /// `speak()` can clean it up + `Drop` removes it.
    last_wav: Option<PathBuf>,
    /// Current playback subprocess.  None when nothing
    /// is playing.
    playback: Option<std::process::Child>,
}

impl PiperEngine {
    /// Construct a Piper engine for `project_root` using
    /// `cfg`.
    ///
    /// Resolution order at `new()` (no network):
    ///
    ///   1. Validate platform — bail if `(os, arch)`
    ///      isn't a Piper-supported pair.
    ///   2. Resolve `voices_dir` under the project root
    ///      via `crate::path_safety::resolve_within_str`.
    ///   3. Resolve the Piper binary via `binary::resolve_piper_binary`
    ///      with auto-download disabled — only PATH +
    ///      user cache are checked.  A missing binary
    ///      makes us fall through to System under
    ///      `engine: "auto"`; users explicitly trigger
    ///      a download via T.7's CLI or T.6's picker.
    ///
    /// The catalog + per-voice files are deferred to
    /// `speak()` — TUI startup must not block on
    /// network.
    pub(crate) fn new(
        cfg: &TtsConfig,
        project_root: &Path,
    ) -> Result<Self, PiperUnavailable> {
        let platform = binary::Platform::detect()?;
        let voices_dir = resolve_voices_dir(project_root, &cfg.voices_dir)?;
        let cache_root = binary::user_cache_root();
        let binary_path = binary::resolve_piper_binary(
            cfg,
            &platform,
            &cache_root,
            // Startup auto-download disabled — the
            // user explicitly triggers via the T.7 CLI
            // or T.6 picker so they're not blindsided
            // by a ~30s pause on first launch.
            |plat, _cache| {
                Err(PiperUnavailable::BinaryNotFound(
                    cache_root_binary_hint(&cache_root, plat),
                ))
            },
        )?;
        Ok(Self {
            cfg: cfg.clone(),
            project_root: project_root.to_path_buf(),
            voices_dir,
            platform,
            binary: binary_path,
            catalog: None,
            last_wav: None,
            playback: None,
        })
    }

    /// Synthesise `text` with `voice_needle` and start
    /// playback.  Non-blocking — returns once the
    /// playback subprocess is spawned.  Stops any
    /// prior playback first.
    pub(crate) fn speak(
        &mut self,
        text: &str,
        voice_needle: &str,
        rate_wpm: Option<u16>,
    ) -> Result<(), String> {
        self.stop();
        let voice_files = self
            .ensure_voice_ready(voice_needle)
            .map_err(|e| e.to_user_message())?;
        let dest = synth::synth_wav_path(&self.voices_dir, voice_needle);
        synth::synth_to_wav(
            &self.binary,
            &voice_files,
            text,
            rate_wpm,
            &dest,
            synth::DEFAULT_SYNTH_TIMEOUT,
        )
        .map_err(|e| e.to_user_message())?;
        let player = synth::spawn_playback(
            self.cfg.play_command.as_deref(),
            &dest,
            self.platform,
        )
        .map_err(|e| e.to_user_message())?;
        self.playback = Some(player);
        self.last_wav = Some(dest);
        Ok(())
    }

    /// Synthesise to `dest` (no playback).  Blocks
    /// until the subprocess exits or `timeout` fires.
    /// Returns the bytes written.
    pub(crate) fn speak_to_file_blocking(
        &mut self,
        text: &str,
        voice_needle: &str,
        rate_wpm: Option<u16>,
        dest: &Path,
        timeout: std::time::Duration,
    ) -> Result<u64, String> {
        let voice_files = self
            .ensure_voice_ready(voice_needle)
            .map_err(|e| e.to_user_message())?;
        synth::synth_to_wav(
            &self.binary,
            &voice_files,
            text,
            rate_wpm,
            dest,
            timeout,
        )
        .map_err(|e| e.to_user_message())
    }

    /// True while a playback subprocess is running.
    pub(crate) fn is_speaking(&mut self) -> bool {
        let Some(child) = self.playback.as_mut() else {
            return false;
        };
        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => {
                self.playback = None;
                self.cleanup_last_wav();
                false
            }
            Err(_) => false,
        }
    }

    /// Stop any in-flight playback.  Idempotent.
    pub(crate) fn stop(&mut self) {
        if let Some(mut child) = self.playback.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.cleanup_last_wav();
    }

    /// Resolve a voice needle against the cache + the
    /// catalog.  Cache hit (both files already on disk
    /// under the canonical key) skips the catalog
    /// entirely so offline users with downloaded
    /// voices keep working.
    pub(crate) fn resolve_voice(&self, needle: &str) -> String {
        // Try cache first.
        if voice::voice_files_present(&self.voices_dir, needle) {
            return needle.to_string();
        }
        // Catalog lookup (if loaded).  We don't load
        // it here — that'd require &mut self.  The
        // catalog-aware path runs through speak().
        if let Some(catalog) = self.catalog.as_ref() {
            if let Some(v) = catalog.voice(needle) {
                return v.key.clone();
            }
        }
        needle.to_string()
    }

    /// Ensure the voice's onnx + onnx.json are on disk
    /// for `voice_needle`.  Cache-fast-path when both
    /// files are already present + nonempty.  Otherwise
    /// loads the catalog, looks up the voice, and runs
    /// the T.4 downloader.
    fn ensure_voice_ready(
        &mut self,
        voice_needle: &str,
    ) -> Result<voice::VoiceFiles, PiperUnavailable> {
        // Cache hit — skip catalog entirely.
        if voice::voice_files_present(&self.voices_dir, voice_needle) {
            return voice::voice_files_for(&self.voices_dir, voice_needle);
        }
        // Need the catalog.
        self.ensure_catalog_loaded()?;
        let catalog = self.catalog.as_ref().expect(
            "catalog loaded in ensure_catalog_loaded but missing here",
        );
        let voice_meta = catalog.voice(voice_needle).ok_or_else(|| {
            PiperUnavailable::DownloadFailed(format!(
                "voice `{voice_needle}` not found in catalog",
            ))
        })?;
        if !self.cfg.auto_download {
            return Err(PiperUnavailable::DownloadFailed(format!(
                "voice `{}` is not downloaded and \
                 tts.auto_download = false.  Run \
                 `inkhaven tts voice download {}` to \
                 fetch it explicitly.",
                voice_meta.key, voice_meta.key,
            )));
        }
        let voice_meta_owned = voice_meta.clone();
        let project_root = self.project_root.clone();
        let voices_dir = self.voices_dir.clone();
        let cache_max = self.cfg.cache_max_voices;
        let auto_gitignore = self.cfg.auto_gitignore;
        voice::ensure_voice_downloaded(
            &voice_meta_owned,
            &voices_dir,
            &project_root,
            cache_max,
            auto_gitignore,
            download::curl_get_to_file,
            // T.6 will wire a real progress channel
            // through to the status-bar chip; T.5
            // ships with a noop callback so synthesis
            // works headlessly.
            |_progress| {},
        )
    }

    fn ensure_catalog_loaded(&mut self) -> Result<(), PiperUnavailable> {
        if self.catalog.is_some() {
            return Ok(());
        }
        let ttl = std::time::Duration::from_secs(
            self.cfg.catalog_ttl_hours as u64 * 3600,
        );
        let catalog = catalog::Catalog::load(
            &self.voices_dir,
            &self.cfg.catalog_url,
            ttl,
            download::curl_get_json,
        )?;
        self.catalog = Some(catalog);
        Ok(())
    }

    fn cleanup_last_wav(&mut self) {
        if let Some(path) = self.last_wav.take() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

impl Drop for PiperEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Resolve `<project_root>/<configured>` with
/// path-traversal defence.  Rejects absolute paths +
/// any `..` segment that escapes the project root.
fn resolve_voices_dir(
    project_root: &Path,
    configured: &str,
) -> Result<PathBuf, PiperUnavailable> {
    let configured = configured.trim();
    if configured.is_empty() {
        return Err(PiperUnavailable::VoicesDirInvalid(
            "tts.voices_dir is empty".to_string(),
        ));
    }
    crate::path_safety::resolve_within_str(project_root, configured).map_err(
        |e| PiperUnavailable::VoicesDirInvalid(format!("{e}")),
    )
}

/// Compose the expected install path for the auto-
/// downloader's `BinaryNotFound` error so the user can
/// see where the binary *would* land if they ran the
/// explicit download CLI.
fn cache_root_binary_hint(cache_root: &Path, plat: &binary::Platform) -> PathBuf {
    cache_root
        .join(plat.cache_subdir())
        .join(plat.binary_filename())
}

/// Convenience access for callers that need to peek at
/// the resolved binary (T.7's `inkhaven tts binary
/// status`).  Marked `#[allow(dead_code)]` until T.7
/// wires the CLI surface.
#[allow(dead_code)]
impl PiperEngine {
    pub(crate) fn binary_path(&self) -> &Path {
        &self.binary
    }
    pub(crate) fn voices_dir(&self) -> &Path {
        &self.voices_dir
    }
    pub(crate) fn platform_label(&self) -> String {
        self.platform.label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TtsConfig;

    // ── PiperUnavailable diagnostics ──────────────────

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
    fn voices_dir_invalid_message_explains_constraint() {
        let msg =
            PiperUnavailable::VoicesDirInvalid("absolute path".into())
                .to_user_message();
        assert!(msg.contains("project root"));
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

    // ── resolve_voices_dir ────────────────────────────

    #[test]
    fn voices_dir_resolves_relative_under_project() {
        let project = tempfile::tempdir().unwrap();
        let resolved =
            resolve_voices_dir(project.path(), ".inkhaven/voices").unwrap();
        assert!(resolved.starts_with(project.path()));
        assert!(resolved.ends_with(".inkhaven/voices"));
    }

    #[test]
    fn voices_dir_rejects_absolute() {
        let project = tempfile::tempdir().unwrap();
        let err =
            resolve_voices_dir(project.path(), "/etc/shadow").unwrap_err();
        assert!(matches!(err, PiperUnavailable::VoicesDirInvalid(_)));
    }

    #[test]
    fn voices_dir_rejects_traversal() {
        let project = tempfile::tempdir().unwrap();
        let err =
            resolve_voices_dir(project.path(), "../escaping").unwrap_err();
        assert!(matches!(err, PiperUnavailable::VoicesDirInvalid(_)));
    }

    #[test]
    fn voices_dir_rejects_empty_string() {
        let project = tempfile::tempdir().unwrap();
        let err = resolve_voices_dir(project.path(), "").unwrap_err();
        assert!(matches!(err, PiperUnavailable::VoicesDirInvalid(_)));
    }

    // ── PiperEngine::new (path safety + binary
    //    resolution).  These avoid touching real PATH /
    //    user-cache contents by pointing binary_path
    //    explicitly. ────────────────────────────────────

    fn make_executable(path: &Path) {
        std::fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn new_succeeds_with_explicit_binary_path() {
        let project = tempfile::tempdir().unwrap();
        let bin_dir = tempfile::tempdir().unwrap();
        let bin = bin_dir.path().join("piper");
        make_executable(&bin);
        let mut cfg = TtsConfig::default();
        cfg.binary_path = Some(bin.to_string_lossy().to_string());
        cfg.auto_download_binary = false;
        let engine = PiperEngine::new(&cfg, project.path()).unwrap();
        assert_eq!(engine.binary_path(), bin);
        assert!(
            engine.voices_dir().starts_with(project.path()),
            "voices_dir must resolve under project root, got {}",
            engine.voices_dir().display(),
        );
    }

    #[test]
    fn new_rejects_invalid_voices_dir() {
        let project = tempfile::tempdir().unwrap();
        let bin_dir = tempfile::tempdir().unwrap();
        let bin = bin_dir.path().join("piper");
        make_executable(&bin);
        let mut cfg = TtsConfig::default();
        cfg.binary_path = Some(bin.to_string_lossy().to_string());
        cfg.voices_dir = "/etc/passwd-dir".to_string();
        cfg.auto_download_binary = false;
        let err = PiperEngine::new(&cfg, project.path()).unwrap_err();
        assert!(matches!(err, PiperUnavailable::VoicesDirInvalid(_)));
    }

    #[test]
    fn new_rejects_missing_explicit_binary_path() {
        let project = tempfile::tempdir().unwrap();
        let mut cfg = TtsConfig::default();
        cfg.binary_path = Some("/nowhere/piper-binary".to_string());
        cfg.auto_download_binary = false;
        let err = PiperEngine::new(&cfg, project.path()).unwrap_err();
        assert!(matches!(err, PiperUnavailable::BinaryNotFound(_)));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_voice_returns_canonical_when_files_present() {
        // With voice files pre-populated under the
        // voices dir, resolve_voice returns the needle
        // verbatim (cache hit, no catalog needed).
        let project = tempfile::tempdir().unwrap();
        let bin_dir = tempfile::tempdir().unwrap();
        let bin = bin_dir.path().join("piper");
        make_executable(&bin);
        let mut cfg = TtsConfig::default();
        cfg.binary_path = Some(bin.to_string_lossy().to_string());
        cfg.auto_download_binary = false;
        let engine = PiperEngine::new(&cfg, project.path()).unwrap();
        let voices_dir = engine.voices_dir().to_path_buf();
        std::fs::create_dir_all(&voices_dir).unwrap();
        std::fs::write(
            voices_dir.join("en_US-lessac-medium.onnx"),
            b"x",
        )
        .unwrap();
        std::fs::write(
            voices_dir.join("en_US-lessac-medium.onnx.json"),
            b"y",
        )
        .unwrap();
        assert_eq!(
            engine.resolve_voice("en_US-lessac-medium"),
            "en_US-lessac-medium",
        );
    }

    #[cfg(unix)]
    #[test]
    fn resolve_voice_returns_needle_when_no_match() {
        // No catalog loaded + no cache → needle as-is
        // (the speak() path will surface a clearer
        // error when it actually tries to load).
        let project = tempfile::tempdir().unwrap();
        let bin_dir = tempfile::tempdir().unwrap();
        let bin = bin_dir.path().join("piper");
        make_executable(&bin);
        let mut cfg = TtsConfig::default();
        cfg.binary_path = Some(bin.to_string_lossy().to_string());
        cfg.auto_download_binary = false;
        let engine = PiperEngine::new(&cfg, project.path()).unwrap();
        assert_eq!(engine.resolve_voice("not-installed"), "not-installed");
    }

    #[cfg(unix)]
    #[test]
    fn is_speaking_false_after_construction() {
        let project = tempfile::tempdir().unwrap();
        let bin_dir = tempfile::tempdir().unwrap();
        let bin = bin_dir.path().join("piper");
        make_executable(&bin);
        let mut cfg = TtsConfig::default();
        cfg.binary_path = Some(bin.to_string_lossy().to_string());
        cfg.auto_download_binary = false;
        let mut engine = PiperEngine::new(&cfg, project.path()).unwrap();
        assert!(!engine.is_speaking());
    }

    #[cfg(unix)]
    #[test]
    fn stop_is_idempotent_on_idle_engine() {
        let project = tempfile::tempdir().unwrap();
        let bin_dir = tempfile::tempdir().unwrap();
        let bin = bin_dir.path().join("piper");
        make_executable(&bin);
        let mut cfg = TtsConfig::default();
        cfg.binary_path = Some(bin.to_string_lossy().to_string());
        cfg.auto_download_binary = false;
        let mut engine = PiperEngine::new(&cfg, project.path()).unwrap();
        engine.stop();
        engine.stop(); // second call must not panic.
    }

    #[cfg(unix)]
    #[test]
    fn platform_label_round_trips() {
        let project = tempfile::tempdir().unwrap();
        let bin_dir = tempfile::tempdir().unwrap();
        let bin = bin_dir.path().join("piper");
        make_executable(&bin);
        let mut cfg = TtsConfig::default();
        cfg.binary_path = Some(bin.to_string_lossy().to_string());
        cfg.auto_download_binary = false;
        let engine = PiperEngine::new(&cfg, project.path()).unwrap();
        let label = engine.platform_label();
        assert!(
            label.contains("darwin")
                || label.contains("linux")
                || label.contains("windows"),
            "got: {label}",
        );
    }
}
