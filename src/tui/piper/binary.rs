//! Piper binary resolution + auto-download orchestration.
//!
//! Resolution order (`resolve_piper_binary`):
//!
//!   1. `tts.binary_path` (explicit HJSON override) — fail
//!      if set but not executable.  Treat the explicit
//!      override as authoritative; don't silently fall
//!      back to PATH.
//!   2. `PATH` lookup — `piper` (or `piper.exe` on
//!      Windows).
//!   3. User cache — `<cache_root>/piper-<plat>/piper`
//!      from a prior auto-download.
//!   4. Auto-download from GitHub Releases (T.2 +
//!      download.rs).  Disabled when
//!      `tts.auto_download_binary = false`.
//!   5. Give up — `BinaryNotFound(<cache_path>)` so the
//!      diagnostic points at where the binary should
//!      land.
//!
//! ## Platform identifiers
//!
//! `Platform` carries `(PiperOs, PiperArch)`.  We map
//! `std::env::consts::{OS, ARCH}` to these via
//! `Platform::detect()`.  The set of supported platforms
//! matches Piper's prebuilt release assets — anything
//! outside that set surfaces as
//! `UnsupportedPlatform(<id>)`.
//!
//! ## Asset selection
//!
//! `pick_piper_release_asset(assets, platform)` is a pure
//! function.  Piper's release-naming convention is stable
//! enough that we can match by filename, but the matcher
//! holds a *list* of acceptable filenames per platform so
//! upstream naming churn (it has happened: `piper_amd64`
//! ↔ `piper_linux_x86_64`) doesn't immediately break the
//! installer.
//!
//! ## Test-friendly factoring
//!
//! Every pure function in here is exercised by tests
//! directly.  The orchestrator `resolve_piper_binary`
//! takes a `cache_root` parameter so tests pin it to a
//! temp dir; in production the caller resolves it via
//! `user_cache_root()`.

use std::path::{Path, PathBuf};

use super::PiperUnavailable;
use crate::config::TtsConfig;

/// macOS / Linux / Windows.  Maps to the OS half of
/// Piper's release-asset filenames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PiperOs {
    Darwin,
    Linux,
    Windows,
}

/// Supported CPU architectures.  Maps to the ARCH half of
/// Piper's release-asset filenames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PiperArch {
    /// 64-bit x86.  Piper labels it `amd64` (Linux),
    /// `x64` (macOS), or `amd64` (Windows .zip).
    X86_64,
    /// 64-bit ARM.  Piper labels it `aarch64` (Linux /
    /// macOS) or `arm64` on some releases.
    Aarch64,
    /// 32-bit ARM (Raspberry Pi 2/3 etc).  Piper labels
    /// it `armv7` (or `armv7l` on some releases).  Linux
    /// only.
    Armv7,
}

/// Full `(OS, ARCH)` tuple plus rendering helpers for
/// cache-directory naming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Platform {
    pub os: PiperOs,
    pub arch: PiperArch,
}

impl Platform {
    /// Detect the current host's platform.  Returns
    /// `UnsupportedPlatform` for `(os, arch)` pairs that
    /// don't have a Piper prebuilt binary (e.g. FreeBSD,
    /// MIPS).
    pub(crate) fn detect() -> Result<Self, PiperUnavailable> {
        Self::from_consts(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// Same as `detect()` but takes the OS + ARCH
    /// strings explicitly.  Pure; used by tests to
    /// exercise non-host platforms.
    pub(crate) fn from_consts(os: &str, arch: &str) -> Result<Self, PiperUnavailable> {
        let os = match os {
            "macos" => PiperOs::Darwin,
            "linux" => PiperOs::Linux,
            "windows" => PiperOs::Windows,
            other => {
                return Err(PiperUnavailable::UnsupportedPlatform(format!(
                    "os={other}",
                )));
            }
        };
        let arch = match arch {
            "x86_64" => PiperArch::X86_64,
            "aarch64" => PiperArch::Aarch64,
            // `arm` is std's identifier for 32-bit ARM
            // (armv6 / armv7).  Piper only ships armv7
            // binaries, but most modern 32-bit ARM
            // distros are armv7 anyway, so this is the
            // pragmatic mapping.
            "arm" => {
                if !matches!(os, PiperOs::Linux) {
                    return Err(PiperUnavailable::UnsupportedPlatform(
                        format!("os={os:?} arch=armv7 (Linux only)"),
                    ));
                }
                PiperArch::Armv7
            }
            other => {
                return Err(PiperUnavailable::UnsupportedPlatform(format!(
                    "arch={other}",
                )));
            }
        };
        Ok(Self { os, arch })
    }

    /// Subdirectory name under `~/.cache/inkhaven/` where
    /// this platform's binary lives.  Stable across
    /// inkhaven versions so a binary downloaded under one
    /// release stays usable under the next.
    pub(crate) fn cache_subdir(&self) -> String {
        let os = match self.os {
            PiperOs::Darwin => "darwin",
            PiperOs::Linux => "linux",
            PiperOs::Windows => "windows",
        };
        let arch = match self.arch {
            PiperArch::X86_64 => "x86_64",
            PiperArch::Aarch64 => "aarch64",
            PiperArch::Armv7 => "armv7",
        };
        format!("piper-{os}-{arch}")
    }

    /// `piper.exe` on Windows, `piper` everywhere else.
    pub(crate) fn binary_filename(&self) -> &'static str {
        match self.os {
            PiperOs::Windows => "piper.exe",
            _ => "piper",
        }
    }

    /// Short label for diagnostics, e.g.
    /// `darwin-aarch64`.
    pub(crate) fn label(&self) -> String {
        self.cache_subdir().trim_start_matches("piper-").to_string()
    }
}

/// Lightweight release-asset descriptor (subset of the
/// GitHub Releases JSON we care about).  The downloader
/// passes these through verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
}

/// Pick the right Piper release asset for the given
/// platform.  Pure; no I/O.  Returns `None` when no
/// asset matches.
///
/// Naming conventions are matched in order of priority —
/// the first pattern that lands in `assets` wins.  This
/// gives us a small headroom for upstream renaming: a
/// new release that drops the historical name still
/// works as long as we add the new name first.
pub(crate) fn pick_piper_release_asset(
    assets: &[ReleaseAsset],
    platform: &Platform,
) -> Option<ReleaseAsset> {
    let candidates: &[&str] = match (platform.os, platform.arch) {
        (PiperOs::Linux, PiperArch::X86_64) => &[
            "piper_amd64.tar.gz",
            "piper_linux_x86_64.tar.gz",
            "piper_x86_64.tar.gz",
        ],
        (PiperOs::Linux, PiperArch::Aarch64) => &[
            "piper_arm64.tar.gz",
            "piper_linux_aarch64.tar.gz",
            "piper_aarch64.tar.gz",
        ],
        (PiperOs::Linux, PiperArch::Armv7) => &[
            "piper_armv7.tar.gz",
            "piper_armv7l.tar.gz",
            "piper_linux_armv7.tar.gz",
        ],
        (PiperOs::Darwin, PiperArch::X86_64) => &[
            "piper_macos_x64.tar.gz",
            "piper_macos_x86_64.tar.gz",
            "piper_darwin_x86_64.tar.gz",
        ],
        (PiperOs::Darwin, PiperArch::Aarch64) => &[
            "piper_macos_aarch64.tar.gz",
            "piper_macos_arm64.tar.gz",
            "piper_darwin_aarch64.tar.gz",
        ],
        (PiperOs::Windows, PiperArch::X86_64) => &[
            "piper_windows_amd64.zip",
            "piper_windows_x64.zip",
            "piper_windows_x86_64.zip",
        ],
        // ARM Windows / ARM macOS Intel-only crosses
        // aren't shipped by Piper.  Return None and let
        // the caller surface `AssetNotFound`.
        _ => return None,
    };
    for needle in candidates {
        if let Some(asset) = assets.iter().find(|a| a.name == *needle) {
            return Some(asset.clone());
        }
    }
    None
}

/// Resolve the binary path.  Production callers use
/// `resolve_piper_binary` (which reads the process
/// environment); tests call the lower-level
/// `resolve_piper_binary_with` with an explicit
/// `path_env` so they don't mutate the shared process
/// PATH (cargo runs tests in parallel; `set_var` is
/// racy across threads).
pub(crate) fn resolve_piper_binary(
    cfg: &TtsConfig,
    platform: &Platform,
    cache_root: &Path,
    auto_download: impl FnOnce(&Platform, &Path) -> Result<(), PiperUnavailable>,
) -> Result<PathBuf, PiperUnavailable> {
    let path_env = std::env::var_os("PATH");
    resolve_piper_binary_with(
        cfg,
        platform,
        cache_root,
        path_env.as_deref(),
        auto_download,
    )
}

/// See `resolve_piper_binary`.  `path_env` is the
/// PATH-style search list (`None` → no PATH lookup).
pub(crate) fn resolve_piper_binary_with(
    cfg: &TtsConfig,
    platform: &Platform,
    cache_root: &Path,
    path_env: Option<&std::ffi::OsStr>,
    auto_download: impl FnOnce(&Platform, &Path) -> Result<(), PiperUnavailable>,
) -> Result<PathBuf, PiperUnavailable> {
    // 1. Explicit override.  Treat as authoritative: if
    //    the user set the path, don't silently fall back
    //    to PATH lookup.  That'd hide bugs ("why is my
    //    binary_path being ignored?").
    if let Some(path) = cfg.binary_path.as_ref().filter(|p| !p.trim().is_empty()) {
        let p = PathBuf::from(path);
        if is_executable(&p) {
            return Ok(p);
        }
        return Err(PiperUnavailable::BinaryNotFound(p));
    }

    // 2. PATH lookup.
    if let Some(paths) = path_env {
        if let Some(p) = which_piper_in(platform, paths) {
            return Ok(p);
        }
    }

    // 3. User cache.
    let cache_path = cache_root
        .join(platform.cache_subdir())
        .join(platform.binary_filename());
    if is_executable(&cache_path) {
        return Ok(cache_path);
    }

    // 4. Auto-download.
    if cfg.auto_download_binary {
        auto_download(platform, cache_root)?;
        if is_executable(&cache_path) {
            return Ok(cache_path);
        }
        // Downloader claimed success but the binary
        // isn't where we expected — surface a clear
        // error rather than silently fall through.
        return Err(PiperUnavailable::BinaryNotFound(cache_path));
    }

    // 5. Give up.
    Err(PiperUnavailable::BinaryNotFound(cache_path))
}

/// User-cache root.  Honours `XDG_CACHE_HOME` first
/// (Linux + opt-in macOS users), then `HOME/.cache` (the
/// XDG default and matches the user's stated preference
/// for `~/.cache/inkhaven/piper-<plat>/`), then platform
/// fallbacks on Windows / unset-HOME systems.
pub(crate) fn user_cache_root() -> PathBuf {
    user_cache_root_with(&|k| std::env::var_os(k))
}

/// See `user_cache_root`.  `env` is the env-var lookup
/// function so tests can inject without mutating the
/// shared process env.
pub(crate) fn user_cache_root_with(
    env: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> PathBuf {
    if let Some(xdg) = env("XDG_CACHE_HOME") {
        let xdg = PathBuf::from(xdg);
        if !xdg.as_os_str().is_empty() {
            return xdg.join("inkhaven");
        }
    }
    if let Some(home) = env("HOME") {
        let home = PathBuf::from(home);
        if !home.as_os_str().is_empty() {
            return home.join(".cache").join("inkhaven");
        }
    }
    // Windows + no HOME — try LOCALAPPDATA, then APPDATA.
    if let Some(local) = env("LOCALAPPDATA") {
        return PathBuf::from(local).join("inkhaven").join("Cache");
    }
    if let Some(appdata) = env("APPDATA") {
        return PathBuf::from(appdata).join("inkhaven").join("Cache");
    }
    // Absolute last resort: a deterministic per-user
    // location under /tmp.  This branch only triggers on
    // very stripped CI containers; the cache won't
    // survive a reboot but that's acceptable for the
    // fallback.
    std::env::temp_dir().join("inkhaven-cache")
}

fn which_piper_in(platform: &Platform, paths: &std::ffi::OsStr) -> Option<PathBuf> {
    let needle = platform.binary_filename();
    for dir in std::env::split_paths(paths) {
        let candidate = dir.join(needle);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(p: &Path) -> bool {
    // Windows doesn't carry a +x bit; .exe is the
    // canonical executable marker.  We've already
    // disambiguated via Platform::binary_filename so a
    // .exe-suffixed file that exists + is-a-file is
    // sufficient.
    std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Platform detection / mapping ──────────────────

    #[test]
    fn from_consts_maps_macos_aarch64() {
        let p = Platform::from_consts("macos", "aarch64").unwrap();
        assert_eq!(p.os, PiperOs::Darwin);
        assert_eq!(p.arch, PiperArch::Aarch64);
        assert_eq!(p.cache_subdir(), "piper-darwin-aarch64");
        assert_eq!(p.binary_filename(), "piper");
        assert_eq!(p.label(), "darwin-aarch64");
    }

    #[test]
    fn from_consts_maps_linux_x86_64() {
        let p = Platform::from_consts("linux", "x86_64").unwrap();
        assert_eq!(p.os, PiperOs::Linux);
        assert_eq!(p.arch, PiperArch::X86_64);
        assert_eq!(p.cache_subdir(), "piper-linux-x86_64");
        assert_eq!(p.binary_filename(), "piper");
    }

    #[test]
    fn from_consts_maps_linux_armv7() {
        let p = Platform::from_consts("linux", "arm").unwrap();
        assert_eq!(p.arch, PiperArch::Armv7);
        assert_eq!(p.cache_subdir(), "piper-linux-armv7");
    }

    #[test]
    fn from_consts_maps_windows_x86_64() {
        let p = Platform::from_consts("windows", "x86_64").unwrap();
        assert_eq!(p.os, PiperOs::Windows);
        assert_eq!(p.binary_filename(), "piper.exe");
    }

    #[test]
    fn from_consts_rejects_unknown_os() {
        let err = Platform::from_consts("plan9", "x86_64").unwrap_err();
        assert!(matches!(err, PiperUnavailable::UnsupportedPlatform(_)));
        assert!(err.to_user_message().contains("plan9"));
    }

    #[test]
    fn from_consts_rejects_unknown_arch() {
        let err = Platform::from_consts("linux", "mips").unwrap_err();
        assert!(matches!(err, PiperUnavailable::UnsupportedPlatform(_)));
        assert!(err.to_user_message().contains("mips"));
    }

    #[test]
    fn from_consts_rejects_armv7_on_non_linux() {
        let err = Platform::from_consts("macos", "arm").unwrap_err();
        assert!(matches!(err, PiperUnavailable::UnsupportedPlatform(_)));
    }

    #[test]
    fn detect_on_current_host_succeeds_or_unsupported() {
        // Whatever the host is, detect() must either
        // succeed or report UnsupportedPlatform — never
        // produce a different error class.
        match Platform::detect() {
            Ok(_) => {}
            Err(PiperUnavailable::UnsupportedPlatform(_)) => {}
            Err(other) => panic!(
                "detect() returned non-UnsupportedPlatform error: {other:?}",
            ),
        }
    }

    // ── Asset selection (pure) ─────────────────────────

    fn asset(name: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_string(),
            download_url: format!(
                "https://example.test/releases/{name}",
            ),
            size: 1234,
        }
    }

    #[test]
    fn pick_asset_linux_x86_64_primary_name() {
        let assets = vec![
            asset("piper_amd64.tar.gz"),
            asset("piper_arm64.tar.gz"),
        ];
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let got = pick_piper_release_asset(&assets, &plat).unwrap();
        assert_eq!(got.name, "piper_amd64.tar.gz");
    }

    #[test]
    fn pick_asset_linux_x86_64_alt_name() {
        // Upstream rename: the primary name is gone but
        // an alternate from our priority list is present.
        let assets = vec![asset("piper_linux_x86_64.tar.gz")];
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let got = pick_piper_release_asset(&assets, &plat).unwrap();
        assert_eq!(got.name, "piper_linux_x86_64.tar.gz");
    }

    #[test]
    fn pick_asset_macos_aarch64() {
        let assets = vec![asset("piper_macos_aarch64.tar.gz")];
        let plat = Platform::from_consts("macos", "aarch64").unwrap();
        let got = pick_piper_release_asset(&assets, &plat).unwrap();
        assert_eq!(got.name, "piper_macos_aarch64.tar.gz");
    }

    #[test]
    fn pick_asset_macos_intel() {
        let assets = vec![asset("piper_macos_x64.tar.gz")];
        let plat = Platform::from_consts("macos", "x86_64").unwrap();
        let got = pick_piper_release_asset(&assets, &plat).unwrap();
        assert_eq!(got.name, "piper_macos_x64.tar.gz");
    }

    #[test]
    fn pick_asset_windows_zip() {
        let assets = vec![asset("piper_windows_amd64.zip")];
        let plat = Platform::from_consts("windows", "x86_64").unwrap();
        let got = pick_piper_release_asset(&assets, &plat).unwrap();
        assert_eq!(got.name, "piper_windows_amd64.zip");
    }

    #[test]
    fn pick_asset_armv7() {
        let assets = vec![asset("piper_armv7.tar.gz")];
        let plat = Platform::from_consts("linux", "arm").unwrap();
        let got = pick_piper_release_asset(&assets, &plat).unwrap();
        assert_eq!(got.name, "piper_armv7.tar.gz");
    }

    #[test]
    fn pick_asset_returns_none_for_empty_release() {
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let got = pick_piper_release_asset(&[], &plat);
        assert!(got.is_none());
    }

    #[test]
    fn pick_asset_returns_none_when_only_wrong_assets_present() {
        let assets = vec![asset("piper_arm64.tar.gz")];
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let got = pick_piper_release_asset(&assets, &plat);
        assert!(got.is_none());
    }

    #[test]
    fn pick_asset_priority_first_match_wins() {
        // Both primary and alternate names present →
        // primary wins (priority list is ordered).
        let assets = vec![
            asset("piper_linux_x86_64.tar.gz"),
            asset("piper_amd64.tar.gz"),
        ];
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let got = pick_piper_release_asset(&assets, &plat).unwrap();
        assert_eq!(got.name, "piper_amd64.tar.gz");
    }

    // ── User-cache root ────────────────────────────────
    //
    // These tests inject env-lookup closures rather than
    // mutating the process env.  cargo runs tests in
    // parallel and `set_var` is racy across threads —
    // a sibling test that spawns `tar` (extract_archive)
    // would see a corrupted PATH if we mutated it here.

    #[test]
    fn user_cache_honours_xdg() {
        let env = |k: &str| -> Option<std::ffi::OsString> {
            match k {
                "XDG_CACHE_HOME" => Some("/tmp/xdg-cache-x".into()),
                _ => None,
            }
        };
        let root = user_cache_root_with(&env);
        assert_eq!(root, PathBuf::from("/tmp/xdg-cache-x/inkhaven"));
    }

    #[test]
    fn user_cache_falls_back_to_home_cache() {
        let env = |k: &str| -> Option<std::ffi::OsString> {
            match k {
                "HOME" => Some("/tmp/fake-home".into()),
                _ => None,
            }
        };
        let root = user_cache_root_with(&env);
        assert_eq!(root, PathBuf::from("/tmp/fake-home/.cache/inkhaven"));
    }

    #[test]
    fn user_cache_xdg_wins_over_home() {
        let env = |k: &str| -> Option<std::ffi::OsString> {
            match k {
                "XDG_CACHE_HOME" => Some("/xdg".into()),
                "HOME" => Some("/home".into()),
                _ => None,
            }
        };
        let root = user_cache_root_with(&env);
        assert_eq!(root, PathBuf::from("/xdg/inkhaven"));
    }

    #[test]
    fn user_cache_windows_localappdata_fallback() {
        let env = |k: &str| -> Option<std::ffi::OsString> {
            match k {
                "LOCALAPPDATA" => Some(r"C:\Users\X\AppData\Local".into()),
                _ => None,
            }
        };
        let root = user_cache_root_with(&env);
        assert!(
            root.to_string_lossy().contains("inkhaven"),
            "got: {}",
            root.display(),
        );
        assert!(
            root.to_string_lossy().ends_with("Cache"),
            "got: {}",
            root.display(),
        );
    }

    #[test]
    fn user_cache_uses_tempdir_when_nothing_set() {
        let env = |_k: &str| -> Option<std::ffi::OsString> { None };
        let root = user_cache_root_with(&env);
        assert!(
            root.file_name()
                .map(|n| n == "inkhaven-cache")
                .unwrap_or(false),
            "expected inkhaven-cache fallback, got: {}",
            root.display(),
        );
    }

    // ── resolve_piper_binary orchestration ────────────

    /// Fake auto-downloader: writes a synthetic
    /// executable into the expected cache path.  Used
    /// in tests to exercise the resolve flow without
    /// hitting the network.
    fn fake_downloader_installs_at(
        cache_root_for_install: PathBuf,
    ) -> impl FnOnce(&Platform, &Path) -> Result<(), PiperUnavailable> {
        move |plat, cache_root| {
            assert_eq!(cache_root, cache_root_for_install.as_path());
            let dir = cache_root.join(plat.cache_subdir());
            std::fs::create_dir_all(&dir).map_err(|e| {
                PiperUnavailable::DownloadFailed(format!("mkdir: {e}"))
            })?;
            let bin = dir.join(plat.binary_filename());
            std::fs::write(&bin, b"#!/bin/sh\nexit 0\n").map_err(|e| {
                PiperUnavailable::DownloadFailed(format!("write: {e}"))
            })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&bin).unwrap().permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&bin, perms).unwrap();
            }
            Ok(())
        }
    }

    fn never_downloader() -> impl FnOnce(&Platform, &Path) -> Result<(), PiperUnavailable> {
        |_plat, _cache| {
            panic!("auto-downloader must not be called in this test");
        }
    }

    fn cfg_with_path(path: Option<&str>, auto_download: bool) -> TtsConfig {
        let mut c = TtsConfig::default();
        c.binary_path = path.map(|s| s.to_string());
        c.auto_download_binary = auto_download;
        c
    }

    /// Tests pass an empty PATH string so `which_piper`
    /// finds nothing — guarantees the resolver moves on
    /// to the cache + auto-download stages even on a host
    /// that has piper installed system-wide.
    fn empty_path() -> std::ffi::OsString {
        std::ffi::OsString::new()
    }

    #[test]
    fn resolve_honours_explicit_binary_path_when_executable() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("my-piper");
        std::fs::write(&bin, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&bin).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&bin, perms).unwrap();
        }
        let cfg = cfg_with_path(Some(bin.to_str().unwrap()), false);
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let path_env = empty_path();
        let got = resolve_piper_binary_with(
            &cfg,
            &plat,
            tmp.path(),
            Some(path_env.as_os_str()),
            never_downloader(),
        )
        .unwrap();
        assert_eq!(got, bin);
    }

    #[test]
    fn resolve_rejects_explicit_binary_path_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg_with_path(Some("/nowhere/piper-binary"), false);
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let path_env = empty_path();
        let err = resolve_piper_binary_with(
            &cfg,
            &plat,
            tmp.path(),
            Some(path_env.as_os_str()),
            never_downloader(),
        )
        .unwrap_err();
        assert!(matches!(err, PiperUnavailable::BinaryNotFound(_)));
    }

    #[test]
    fn resolve_finds_binary_in_user_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let dir = tmp.path().join(plat.cache_subdir());
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join(plat.binary_filename());
        std::fs::write(&bin, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&bin).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&bin, perms).unwrap();
        }
        let cfg = cfg_with_path(None, false);
        let path_env = empty_path();
        let got = resolve_piper_binary_with(
            &cfg,
            &plat,
            tmp.path(),
            Some(path_env.as_os_str()),
            never_downloader(),
        )
        .unwrap();
        assert_eq!(got, bin);
    }

    #[test]
    fn resolve_triggers_auto_download_when_enabled() {
        let tmp = tempfile::tempdir().unwrap();
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let cfg = cfg_with_path(None, true);
        let path_env = empty_path();
        let got = resolve_piper_binary_with(
            &cfg,
            &plat,
            tmp.path(),
            Some(path_env.as_os_str()),
            fake_downloader_installs_at(tmp.path().to_path_buf()),
        )
        .unwrap();
        assert_eq!(
            got,
            tmp.path()
                .join(plat.cache_subdir())
                .join(plat.binary_filename()),
        );
    }

    #[test]
    fn resolve_skips_auto_download_when_disabled() {
        let tmp = tempfile::tempdir().unwrap();
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let cfg = cfg_with_path(None, false);
        let path_env = empty_path();
        let err = resolve_piper_binary_with(
            &cfg,
            &plat,
            tmp.path(),
            Some(path_env.as_os_str()),
            never_downloader(),
        )
        .unwrap_err();
        assert!(matches!(err, PiperUnavailable::BinaryNotFound(_)));
    }

    #[test]
    fn resolve_finds_binary_in_path() {
        // A PATH that contains a directory where the
        // binary lives.  Confirms the PATH lookup step
        // actually fires when present + executable.
        let tmp = tempfile::tempdir().unwrap();
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let path_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&path_dir).unwrap();
        let bin = path_dir.join(plat.binary_filename());
        std::fs::write(&bin, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&bin).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&bin, perms).unwrap();
        }
        let cfg = cfg_with_path(None, false);
        let path_env = path_dir.into_os_string();
        let cache_root = tempfile::tempdir().unwrap();
        let got = resolve_piper_binary_with(
            &cfg,
            &plat,
            cache_root.path(),
            Some(path_env.as_os_str()),
            never_downloader(),
        )
        .unwrap();
        assert_eq!(got, bin);
    }
}
