//! TTS engine dispatcher.
//!
//! The 1.2.9 TTS surface was wired directly to a single
//! `Say` struct that wrapped macOS `/usr/bin/say`.  In
//! 1.2.17 we shift to an enum dispatcher so the same
//! call sites (the `Ctrl+B S` read-aloud chord,
//! `Ctrl+B Shift+R` save-as-audio, the `tts.greeting` /
//! `tts.goodbye` paths, and the `ink.tts.speak` Bund
//! word) can route to either the legacy `System` backend
//! or the new `Piper` neural backend.
//!
//! T.1 ships the abstraction + dispatch + resolver only.
//! The `Piper` variant's constructor is a stub that
//! always errors so `engine: "auto"` falls through to
//! `system` on every host — no behavioural change vs.
//! 1.2.16 on macOS, and the same "TTS unavailable on this
//! platform" copy on Linux / Windows.  The real Piper
//! pipeline lands across T.2–T.5.
//!
//! ## Design
//!
//! The enum carries variant-specific state (`Say` for
//! `System`, `PiperEngine` for `Piper`), but the public
//! API surface is uniform: every variant implements
//! `speak`, `speak_to_file_blocking`, `is_speaking`,
//! `stop`, `is_ready`, `resolve_voice`, `label`, and
//! `kind`.  Callers don't pattern-match on the variant —
//! they call through the wrapper methods.
//!
//! ## Resolution
//!
//! `TtsEngine::resolve(cfg, project_root)` honours
//! `tts.engine` ∈ {"auto", "piper", "system"}:
//!
//!   * `"system"` — always System(Say), even on non-macOS
//!     (the resulting engine errors at `is_ready` time).
//!   * `"piper"` — always Piper(PiperEngine); the
//!     constructor's error propagates through `is_ready`.
//!   * `"auto"` (default) — try Piper first; fall back
//!     to System if Piper's constructor errors.
//!
//! A disabled config (`tts.enabled = false`) collapses
//! every variant to `Disabled` so all the speak paths
//! short-circuit uniformly.

use std::path::Path;

use crate::config::TtsConfig;

use super::piper::PiperEngine;
use super::say::Say;

/// Identifies which backend the engine is using.  Used by
/// the diagnostic status-bar chip + the engine-status CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EngineKind {
    System,
    Piper,
    Disabled,
}

impl EngineKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Piper => "piper",
            Self::Disabled => "disabled",
        }
    }
}

/// TTS engine dispatcher.  See module docs for shape +
/// resolution semantics.
#[derive(Debug)]
pub(crate) enum TtsEngine {
    System(Say),
    #[allow(dead_code)] // T.5 will wire synthesis through this
    Piper(PiperEngine),
    /// Either `tts.enabled = false` or no backend
    /// resolved.  Carries a diagnostic string for the
    /// "why" so the UI can surface it.
    Disabled(String),
}

impl Default for TtsEngine {
    fn default() -> Self {
        // Construction without a config: a sane no-op
        // default that errors on first speak.  Used by
        // `App`'s field-default; replaced by the real
        // resolved engine before the first frame renders.
        Self::Disabled("TTS not yet initialised".into())
    }
}

impl TtsEngine {
    /// Resolve a `TtsEngine` from configuration.  See
    /// module docs for the precedence rules.
    pub(crate) fn resolve(cfg: &TtsConfig, project_root: &Path) -> Self {
        if !cfg.enabled {
            return Self::Disabled(
                "TTS is disabled (set editor.tts.enabled = true)".into(),
            );
        }
        let engine = cfg.engine.trim().to_ascii_lowercase();
        match engine.as_str() {
            "system" => Self::System(Say::default()),
            "piper" => match PiperEngine::new(cfg, project_root) {
                Ok(piper) => Self::Piper(piper),
                Err(reason) => Self::Disabled(reason.to_user_message()),
            },
            // "auto" + anything unknown (with a warning logged elsewhere)
            _ => match PiperEngine::new(cfg, project_root) {
                Ok(piper) => Self::Piper(piper),
                Err(_piper_err) => {
                    // Piper unavailable — fall through to
                    // the 1.2.9 macOS backend.  On non-
                    // macOS hosts this `Say` will report
                    // unready at `is_ready()` time, which
                    // is the right outcome.
                    Self::System(Say::default())
                }
            },
        }
    }

    /// Identifier of the active backend for diagnostics.
    pub(crate) fn kind(&self) -> EngineKind {
        match self {
            Self::System(_) => EngineKind::System,
            Self::Piper(_) => EngineKind::Piper,
            Self::Disabled(_) => EngineKind::Disabled,
        }
    }

    /// Short human-readable label for the status bar /
    /// diagnostic chip.
    pub(crate) fn label(&self) -> &'static str {
        self.kind().label()
    }

    /// Returns `Ok(())` when the engine is ready to
    /// synthesise.  Errors carry a user-facing reason
    /// suitable for surfacing in a modal or the status
    /// bar.
    pub(crate) fn is_ready(&self) -> Result<(), String> {
        match self {
            Self::System(_) => Say::available().map_err(|s| s.to_string()),
            Self::Piper(_) => {
                // T.5 will replace this with a real
                // readiness check (binary present + voice
                // downloadable / downloaded).  T.1 stub
                // never constructs Piper successfully, so
                // this branch is unreachable — but we
                // give it a reasonable answer for the
                // tests that construct PiperEngine
                // directly.
                Ok(())
            }
            Self::Disabled(reason) => Err(reason.clone()),
        }
    }

    /// Resolve a user-supplied voice needle to the actual
    /// voice identifier the engine wants.
    ///
    ///   * System — substring match against `say -v "?"`,
    ///     preferring Enhanced / Premium variants.
    ///     Returns `None` when nothing matches; caller
    ///     falls back to "system default".
    ///   * Piper — needle is the canonical voice id
    ///     (`en_US-lessac-medium`); returned verbatim.
    ///     T.3 catalog lookups + fuzzy matching land
    ///     later.
    ///   * Disabled — `None`.
    pub(crate) fn resolve_voice(&self, needle: &str) -> Option<String> {
        match self {
            Self::System(_) => Say::pick_voice(needle),
            Self::Piper(p) => Some(p.resolve_voice(needle)),
            Self::Disabled(_) => None,
        }
    }

    /// Speak `text`.  Non-blocking: returns once the
    /// playback subprocess (or future async task) is
    /// launched; audio continues in parallel.  Any prior
    /// in-flight playback this engine owns is stopped
    /// first.
    pub(crate) fn speak(
        &mut self,
        text: &str,
        voice: &str,
        rate_wpm: Option<u16>,
    ) -> Result<(), String> {
        match self {
            Self::System(say) => say
                .speak(text, voice, rate_wpm)
                .map_err(|e| format!("subprocess spawn failed: {e}")),
            Self::Piper(p) => p.speak(text, voice, rate_wpm),
            Self::Disabled(reason) => Err(reason.clone()),
        }
    }

    /// Speak `text` into a file at `dest`.  Blocks until
    /// the subprocess exits (or `timeout` fires).  Returns
    /// the number of bytes written on success.
    pub(crate) fn speak_to_file_blocking(
        &mut self,
        text: &str,
        voice: &str,
        rate_wpm: Option<u16>,
        dest: &Path,
        timeout: std::time::Duration,
    ) -> Result<u64, String> {
        match self {
            Self::System(_) => Say::speak_to_file_blocking(
                text, voice, rate_wpm, dest, timeout,
            ),
            Self::Piper(p) => p.speak_to_file_blocking(
                text, voice, rate_wpm, dest, timeout,
            ),
            Self::Disabled(reason) => Err(reason.clone()),
        }
    }

    /// True while a `speak` subprocess is still running.
    /// Cheap; non-blocking.
    pub(crate) fn is_speaking(&mut self) -> bool {
        match self {
            Self::System(say) => say.is_speaking(),
            Self::Piper(p) => p.is_speaking(),
            Self::Disabled(_) => false,
        }
    }

    /// Stop any in-flight playback.  Idempotent.
    pub(crate) fn stop(&mut self) {
        match self {
            Self::System(say) => say.stop(),
            Self::Piper(p) => p.stop(),
            Self::Disabled(_) => {}
        }
    }
}

// Cross-thread invariant: the engine sits on `App`, which
// the 1.2.16 P.4-pre pass made Send + Sync so the health
// monitor's tokio task can hold a clone of `Store`.  If a
// future refactor adds non-Send state into any TTS
// backend, this assertion fails the build, not the
// runtime.
const _ASSERT_SEND_SYNC: fn() = || {
    fn assert<T: Send + Sync>() {}
    assert::<TtsEngine>();
    assert::<EngineKind>();
};

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_with(engine: &str, enabled: bool) -> TtsConfig {
        let mut c = TtsConfig::default();
        c.enabled = enabled;
        c.engine = engine.into();
        // Force Piper to fail resolution so the
        // auto/piper resolver paths reach their
        // fallback behaviour deterministically — even
        // on dev machines where `piper` is on PATH.
        c.binary_path = Some("/__inkhaven_test_no_such_piper__".into());
        c.auto_download_binary = false;
        c
    }

    #[test]
    fn disabled_when_master_switch_off() {
        let cfg = cfg_with("auto", false);
        let eng = TtsEngine::resolve(&cfg, std::env::temp_dir().as_path());
        assert_eq!(eng.kind(), EngineKind::Disabled);
        assert!(eng.is_ready().is_err());
        let msg = eng.is_ready().unwrap_err();
        assert!(
            msg.contains("editor.tts.enabled"),
            "expected disabled reason to reference the master switch, got: {msg}",
        );
    }

    #[test]
    fn auto_falls_through_to_system_when_piper_unresolvable() {
        // T.5+: "auto" tries Piper first; falls back to
        // System when Piper construction errors (binary
        // missing in this test harness).  cfg_with()
        // points binary_path at a guaranteed-absent
        // path so this test is host-independent.
        let cfg = cfg_with("auto", true);
        let eng = TtsEngine::resolve(&cfg, std::env::temp_dir().as_path());
        assert_eq!(eng.kind(), EngineKind::System);
    }

    #[test]
    fn forced_system_resolves_to_system() {
        let cfg = cfg_with("system", true);
        let eng = TtsEngine::resolve(&cfg, std::env::temp_dir().as_path());
        assert_eq!(eng.kind(), EngineKind::System);
    }

    #[test]
    fn forced_piper_collapses_to_disabled_when_binary_missing() {
        // Explicit `engine: "piper"` with a missing
        // binary surfaces as Disabled, carrying the
        // BinaryNotFound diagnostic so the user can
        // see the expected install path.
        let cfg = cfg_with("piper", true);
        let eng = TtsEngine::resolve(&cfg, std::env::temp_dir().as_path());
        assert_eq!(eng.kind(), EngineKind::Disabled);
        let msg = eng.is_ready().unwrap_err();
        assert!(
            msg.contains("tts.binary_path") || msg.contains("Piper binary"),
            "expected diagnostic to reference binary path, got: {msg}",
        );
    }

    /// New T.5 invariant: when Piper IS resolvable
    /// (binary_path points at an executable file), the
    /// auto + piper resolvers both produce a Piper
    /// variant.  Exercised on Unix only because we
    /// rely on chmod +x; Windows test would need a
    /// .exe with the right ACL.
    #[cfg(unix)]
    #[test]
    fn auto_resolves_to_piper_when_binary_present() {
        let bin_dir = tempfile::tempdir().unwrap();
        let bin = bin_dir.path().join("piper");
        std::fs::write(&bin, b"#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&bin).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin, perms).unwrap();
        let project = tempfile::tempdir().unwrap();
        let mut cfg = TtsConfig::default();
        cfg.enabled = true;
        cfg.engine = "auto".into();
        cfg.binary_path = Some(bin.to_string_lossy().to_string());
        cfg.auto_download_binary = false;
        let eng = TtsEngine::resolve(&cfg, project.path());
        assert_eq!(eng.kind(), EngineKind::Piper);
    }

    #[test]
    fn unknown_engine_string_treated_as_auto() {
        let cfg = cfg_with("nonsense-value", true);
        let eng = TtsEngine::resolve(&cfg, std::env::temp_dir().as_path());
        // Unknown values are tolerated to auto — same
        // safety net the rest of the config has for
        // string-typed enums.  Documented in
        // TtsConfig::engine doc.
        assert_eq!(eng.kind(), EngineKind::System);
    }

    #[test]
    fn engine_kind_label_round_trip() {
        assert_eq!(EngineKind::System.label(), "system");
        assert_eq!(EngineKind::Piper.label(), "piper");
        assert_eq!(EngineKind::Disabled.label(), "disabled");
    }

    #[test]
    fn label_on_engine_matches_kind() {
        let cfg = cfg_with("auto", true);
        let eng = TtsEngine::resolve(&cfg, std::env::temp_dir().as_path());
        assert_eq!(eng.label(), eng.kind().label());
    }

    #[test]
    fn disabled_engine_short_circuits_speak() {
        let cfg = cfg_with("auto", false);
        let mut eng = TtsEngine::resolve(&cfg, std::env::temp_dir().as_path());
        let err = eng
            .speak("hello", "", None)
            .expect_err("disabled engine must not speak");
        assert!(err.contains("editor.tts.enabled"), "got: {err}");
        assert!(!eng.is_speaking());
        // stop() must be idempotent on a disabled engine.
        eng.stop();
    }

    #[test]
    fn disabled_engine_short_circuits_save() {
        let cfg = cfg_with("auto", false);
        let mut eng = TtsEngine::resolve(&cfg, std::env::temp_dir().as_path());
        let dest = std::env::temp_dir().join("inkhaven-tts-t1-disabled.wav");
        let err = eng
            .speak_to_file_blocking(
                "hello",
                "",
                None,
                &dest,
                std::time::Duration::from_secs(1),
            )
            .expect_err("disabled engine must not save");
        assert!(err.contains("editor.tts.enabled"), "got: {err}");
    }

    #[test]
    fn default_engine_is_disabled() {
        let eng = TtsEngine::default();
        assert_eq!(eng.kind(), EngineKind::Disabled);
    }

    #[test]
    fn voice_resolution_for_disabled_returns_none() {
        let cfg = cfg_with("auto", false);
        let eng = TtsEngine::resolve(&cfg, std::env::temp_dir().as_path());
        assert!(eng.resolve_voice("anything").is_none());
    }
}
