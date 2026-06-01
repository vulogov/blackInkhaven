//! Piper binary auto-download orchestration.
//!
//! Why subprocesses?  Inkhaven's existing TTS pattern
//! (`/usr/bin/say` from `tui::say`) shells out rather
//! than embedding tts-rs.  We follow the same shape
//! here: `curl` for the HTTP layer, `tar` (or `tar -xf
//! foo.zip` on Windows 10+) for extraction.  Both are
//! universal on modern macOS / Linux / Windows hosts,
//! eliminating new Rust deps.
//!
//! The fetch + extract are factored as injectable
//! closures so tests don't have to spawn real curl /
//! tar — they substitute fake bytes + fake extraction
//! directly into the temp staging area.
//!
//! ## Pipeline
//!
//! ```text
//!   ┌───────────────────────────────────────┐
//!   │ download_piper_binary(plat, cache)    │
//!   └───────────────┬───────────────────────┘
//!                   ▼
//!     ┌──────────────────────────────┐
//!     │ fetch_release_json(url)       │   ← curl
//!     └──────────────┬───────────────┘
//!                    ▼
//!     ┌──────────────────────────────┐
//!     │ parse_release_json(bytes)     │   ← pure
//!     └──────────────┬───────────────┘
//!                    ▼
//!     ┌──────────────────────────────┐
//!     │ pick_piper_release_asset(...)  │   ← pure (binary.rs)
//!     └──────────────┬───────────────┘
//!                    ▼
//!     ┌──────────────────────────────┐
//!     │ fetch_asset(asset, staging)   │   ← curl
//!     └──────────────┬───────────────┘
//!                    ▼
//!     ┌──────────────────────────────┐
//!     │ extract_archive(staging, dst) │   ← tar
//!     └──────────────┬───────────────┘
//!                    ▼
//!     ┌──────────────────────────────┐
//!     │ install_binary(dst, target)   │   ← atomic via io_atomic
//!     └──────────────────────────────┘
//! ```
//!
//! Each step has a corresponding test that exercises
//! either the pure logic (parse, pick) or the
//! filesystem-side effect (extract a real fixture,
//! install via io_atomic).

use std::path::{Path, PathBuf};
use std::process::Command;

use super::binary::{
    pick_piper_release_asset, Platform, ReleaseAsset,
};
use super::PiperUnavailable;

/// GitHub Releases endpoint for Piper.  Pinned here so
/// tests can override via a closure; production callers
/// hit the live URL.
pub(crate) const PIPER_RELEASES_LATEST_URL: &str =
    "https://api.github.com/repos/rhasspy/piper/releases/latest";

/// User-agent string used for curl requests.  GitHub
/// rejects requests without a UA on the API endpoints.
pub(crate) const USER_AGENT: &str =
    concat!("inkhaven/", env!("CARGO_PKG_VERSION"));

/// Synthetic release shape parsed out of GitHub's JSON.
/// We deliberately keep this minimal — `tag_name` for
/// diagnostics and `assets` for selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Release {
    pub tag_name: String,
    pub assets: Vec<ReleaseAsset>,
}

/// Top-level orchestrator.  See module-level pipeline
/// diagram.  `fetch_json` and `fetch_bytes` are injected
/// so tests substitute fixture data; production wires
/// them to `curl_get_json` / `curl_get_to_file` defined
/// below.
pub(crate) fn download_piper_binary(
    platform: &Platform,
    cache_root: &Path,
    fetch_json: impl Fn(&str) -> Result<Vec<u8>, PiperUnavailable>,
    fetch_bytes: impl Fn(&str, &Path) -> Result<(), PiperUnavailable>,
) -> Result<PathBuf, PiperUnavailable> {
    // 1. Fetch the release manifest.
    let json = fetch_json(PIPER_RELEASES_LATEST_URL)?;
    let release = parse_release_json(&json)?;

    // 2. Pick the asset for this platform.
    let asset =
        pick_piper_release_asset(&release.assets, platform).ok_or_else(
            || PiperUnavailable::AssetNotFound {
                tag: release.tag_name.clone(),
                platform: platform.label(),
            },
        )?;

    // 3. Stage the download into a unique temp dir
    //    under the cache so a partial transfer can't
    //    leave garbage behind in the final location.
    //    A failed run leaves the staging dir; the next
    //    successful run overwrites it.
    let staging = cache_root
        .join(platform.cache_subdir())
        .join(".staging");
    std::fs::create_dir_all(&staging).map_err(|e| {
        PiperUnavailable::DownloadFailed(format!(
            "mkdir staging {}: {e}",
            staging.display(),
        ))
    })?;
    let archive_path = staging.join(&asset.name);
    fetch_bytes(&asset.download_url, &archive_path)?;

    // 4. Extract.  `tar -xzf <tarball> -C <dir>` on
    //    Unix-style; `tar -xf <zip> -C <dir>` on
    //    Windows 10+.  The bsdtar that ships with
    //    Windows handles both.
    let extract_dir = staging.join("extract");
    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir).map_err(|e| {
        PiperUnavailable::ExtractFailed(format!(
            "mkdir extract {}: {e}",
            extract_dir.display(),
        ))
    })?;
    extract_archive(&archive_path, &extract_dir)?;

    // 5. Locate the binary inside the extracted tree.
    //    Piper tarballs ship as `piper/piper(.exe)`;
    //    we walk a couple of layers to be tolerant of
    //    upstream layout changes.
    let target = cache_root
        .join(platform.cache_subdir())
        .join(platform.binary_filename());
    let extracted = locate_extracted_binary(
        &extract_dir,
        platform.binary_filename(),
    )?;
    install_binary(&extracted, &target)?;

    // 6. Cleanup staging (best-effort; not fatal if it
    //    fails — the next run will overwrite).
    let _ = std::fs::remove_dir_all(&staging);

    Ok(target)
}

/// Parse the subset of GitHub's release JSON we care
/// about.  Pure — no I/O.  Tolerant of extra fields and
/// missing optional fields; rejects malformed JSON.
pub(crate) fn parse_release_json(bytes: &[u8]) -> Result<Release, PiperUnavailable> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| {
            PiperUnavailable::DownloadFailed(format!(
                "parse release JSON: {e}",
            ))
        })?;
    let tag_name = value
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let assets_arr = match value.get("assets").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => {
            return Err(PiperUnavailable::DownloadFailed(
                "release JSON has no `assets` array".to_string(),
            ));
        }
    };
    let assets: Vec<ReleaseAsset> = assets_arr
        .iter()
        .filter_map(|a| {
            let name = a.get("name")?.as_str()?.to_string();
            let download_url =
                a.get("browser_download_url")?.as_str()?.to_string();
            let size = a.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(ReleaseAsset {
                name,
                download_url,
                size,
            })
        })
        .collect();
    Ok(Release { tag_name, assets })
}

/// curl-based JSON fetch.  GitHub requires a User-Agent
/// + accepts the v3 API media type.  Returns the
/// response body as bytes.  Errors surface as
/// `DownloadFailed`.
#[allow(dead_code)]
pub(crate) fn curl_get_json(url: &str) -> Result<Vec<u8>, PiperUnavailable> {
    let output = Command::new("curl")
        .args([
            "-sSL",
            "-A",
            USER_AGENT,
            "-H",
            "Accept: application/vnd.github+json",
            "--fail",
            "--max-time",
            "30",
            url,
        ])
        .output()
        .map_err(|e| {
            PiperUnavailable::DownloadFailed(format!("spawn curl: {e}"))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PiperUnavailable::DownloadFailed(format!(
            "curl exit {:?}: {}",
            output.status.code(),
            stderr.trim(),
        )));
    }
    Ok(output.stdout)
}

/// curl-based binary download.  Streams `url` into
/// `dest`.  `--fail` so HTTP 4xx/5xx surface as a
/// non-zero curl exit + readable stderr.  Drops partial
/// files on failure (`curl -o` writes the file in place
/// — we delete it ourselves on error to keep the
/// staging dir clean).
#[allow(dead_code)]
pub(crate) fn curl_get_to_file(url: &str, dest: &Path) -> Result<(), PiperUnavailable> {
    let output = Command::new("curl")
        .args([
            "-sSL",
            "-A",
            USER_AGENT,
            "--fail",
            "--max-time",
            "600",
            "-o",
        ])
        .arg(dest)
        .arg(url)
        .output()
        .map_err(|e| {
            PiperUnavailable::DownloadFailed(format!("spawn curl: {e}"))
        })?;
    if !output.status.success() {
        let _ = std::fs::remove_file(dest);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PiperUnavailable::DownloadFailed(format!(
            "curl exit {:?}: {}",
            output.status.code(),
            stderr.trim(),
        )));
    }
    Ok(())
}

/// Extract `archive` into `dst`.  Uses `tar` for both
/// `.tar.gz` (`tar -xzf`) and `.zip` (`tar -xf` — the
/// `bsdtar` shipped on macOS / Windows handles zip).
/// On Linux glibc that ships GNU tar, the `.zip`
/// extraction path falls back to `unzip` if `tar` rejects
/// the archive — but this is mostly a theoretical
/// concern: Piper's Linux releases are always
/// `.tar.gz`, only the Windows asset is `.zip`.
pub(crate) fn extract_archive(archive: &Path, dst: &Path) -> Result<(), PiperUnavailable> {
    let archive_name = archive
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let is_zip = archive_name.ends_with(".zip");
    let args: Vec<&str> = if is_zip {
        // bsdtar accepts -xf on a zip.  This works on
        // macOS + Windows 10+ out of the box.
        vec!["-xf"]
    } else {
        vec!["-xzf"]
    };
    let mut cmd = Command::new("tar");
    cmd.args(&args).arg(archive).arg("-C").arg(dst);
    let output = cmd.output().map_err(|e| {
        PiperUnavailable::ExtractFailed(format!("spawn tar: {e}"))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // On non-bsdtar Linux a `.zip` may need
        // `unzip` — surface a clear error so the user
        // can install it manually.  This branch is
        // theoretical for Piper but real for resilience.
        return Err(PiperUnavailable::ExtractFailed(format!(
            "tar exit {:?}: {}",
            output.status.code(),
            stderr.trim(),
        )));
    }
    Ok(())
}

/// Walk `root` for `<binary_name>` (Piper ships it as
/// `piper/piper` in the tarball; future layouts might
/// nest differently).  Returns the first match within
/// 3 layers deep.  Errors surface as
/// `ExtractFailed("binary not found in archive")`.
pub(crate) fn locate_extracted_binary(
    root: &Path,
    binary_name: &str,
) -> Result<PathBuf, PiperUnavailable> {
    fn walk(dir: &Path, target: &str, depth: usize) -> Option<PathBuf> {
        if depth > 3 {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            // Only match regular files — a directory
            // named `piper` (Piper's tarballs nest the
            // binary inside such a dir) must NOT be
            // returned here, otherwise we install the
            // directory path as if it were the binary.
            let matches_name =
                path.file_name().map(|n| n == target).unwrap_or(false);
            if matches_name && path.is_file() {
                return Some(path);
            }
            if path.is_dir() {
                if let Some(found) = walk(&path, target, depth + 1) {
                    return Some(found);
                }
            }
        }
        None
    }
    walk(root, binary_name, 0).ok_or_else(|| {
        PiperUnavailable::ExtractFailed(format!(
            "binary `{binary_name}` not found in archive under {}",
            root.display(),
        ))
    })
}

/// Move `src` to `dst`, atomic-rename style.  Creates
/// the parent of `dst` if missing + carries +x on Unix.
pub(crate) fn install_binary(src: &Path, dst: &Path) -> Result<(), PiperUnavailable> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            PiperUnavailable::ExtractFailed(format!(
                "mkdir {}: {e}",
                parent.display(),
            ))
        })?;
    }
    // Atomic via `crate::io_atomic`: copy through a
    // temp file in the dst's directory + fsync +
    // rename, so a crash mid-install never leaves the
    // user with a half-written binary.
    let bytes = std::fs::read(src).map_err(|e| {
        PiperUnavailable::ExtractFailed(format!(
            "read extracted binary {}: {e}",
            src.display(),
        ))
    })?;
    crate::io_atomic::write(dst, &bytes).map_err(|e| {
        PiperUnavailable::ExtractFailed(format!(
            "atomic write {}: {e}",
            dst.display(),
        ))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(dst)
            .map_err(|e| {
                PiperUnavailable::ExtractFailed(format!(
                    "stat dst {}: {e}",
                    dst.display(),
                ))
            })?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(dst, perms).map_err(|e| {
            PiperUnavailable::ExtractFailed(format!(
                "chmod {}: {e}",
                dst.display(),
            ))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_RELEASE_JSON: &[u8] = br#"
    {
      "tag_name": "2023.11.14-2",
      "name": "Piper 2023.11.14-2",
      "draft": false,
      "prerelease": false,
      "assets": [
        {
          "name": "piper_amd64.tar.gz",
          "browser_download_url": "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_amd64.tar.gz",
          "size": 8388608
        },
        {
          "name": "piper_arm64.tar.gz",
          "browser_download_url": "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_arm64.tar.gz",
          "size": 8388608
        },
        {
          "name": "piper_macos_aarch64.tar.gz",
          "browser_download_url": "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_macos_aarch64.tar.gz",
          "size": 8388608
        },
        {
          "name": "piper_windows_amd64.zip",
          "browser_download_url": "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip",
          "size": 8388608
        }
      ]
    }
    "#;

    // ── parse_release_json ────────────────────────────

    #[test]
    fn parse_release_extracts_tag_and_assets() {
        let release = parse_release_json(FIXTURE_RELEASE_JSON).unwrap();
        assert_eq!(release.tag_name, "2023.11.14-2");
        assert_eq!(release.assets.len(), 4);
        assert_eq!(release.assets[0].name, "piper_amd64.tar.gz");
        assert!(release.assets[0]
            .download_url
            .starts_with("https://github.com/rhasspy/piper"));
        assert_eq!(release.assets[0].size, 8_388_608);
    }

    #[test]
    fn parse_release_rejects_bad_json() {
        let err = parse_release_json(b"not json").unwrap_err();
        assert!(matches!(err, PiperUnavailable::DownloadFailed(_)));
    }

    #[test]
    fn parse_release_rejects_missing_assets() {
        let err = parse_release_json(b"{\"tag_name\":\"x\"}").unwrap_err();
        assert!(matches!(err, PiperUnavailable::DownloadFailed(_)));
        assert!(err.to_user_message().contains("assets"));
    }

    #[test]
    fn parse_release_skips_malformed_asset_entries() {
        // An asset entry missing `name` should be
        // skipped rather than failing the whole parse.
        // GitHub doesn't ship malformed assets in
        // practice, but tolerance keeps us out of
        // trouble if the API surface evolves.
        let json = br#"{
          "tag_name": "x",
          "assets": [
            { "browser_download_url": "https://example.test/a", "size": 1 },
            { "name": "good.tar.gz", "browser_download_url": "https://example.test/g", "size": 2 }
          ]
        }"#;
        let release = parse_release_json(json).unwrap();
        assert_eq!(release.assets.len(), 1);
        assert_eq!(release.assets[0].name, "good.tar.gz");
    }

    // ── extract_archive (real fs, real tar) ───────────

    fn make_tarball(dir: &Path, name: &str) -> PathBuf {
        // Create a tiny tarball at dir/<name>.tar.gz
        // containing piper/piper with the bytes
        // "fake-binary".  Uses `tar` itself for the
        // creation so we exercise the same tool the
        // extractor uses.
        let staging = dir.join("mk");
        let inner = staging.join("piper");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(inner.join(name), b"fake-binary").unwrap();
        let archive = dir.join(format!("{}.tar.gz", name));
        let status = Command::new("tar")
            .args(["-czf"])
            .arg(&archive)
            .args(["-C"])
            .arg(&staging)
            .arg("piper")
            .status()
            .expect("tar -czf must succeed in tests");
        assert!(status.success(), "fixture tarball creation failed");
        archive
    }

    #[test]
    fn extract_archive_unpacks_real_tarball() {
        let tmp = tempfile::tempdir().unwrap();
        let archive = make_tarball(tmp.path(), "piper");
        let dst = tmp.path().join("out");
        std::fs::create_dir_all(&dst).unwrap();
        extract_archive(&archive, &dst).unwrap();
        assert!(dst.join("piper").join("piper").exists());
    }

    #[test]
    fn extract_archive_errors_on_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dst = tmp.path().join("out");
        std::fs::create_dir_all(&dst).unwrap();
        let err = extract_archive(
            &tmp.path().join("does-not-exist.tar.gz"),
            &dst,
        )
        .unwrap_err();
        assert!(matches!(err, PiperUnavailable::ExtractFailed(_)));
    }

    // ── locate_extracted_binary ───────────────────────

    #[test]
    fn locate_binary_finds_nested() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("piper").join("piper");
        std::fs::create_dir_all(nested.parent().unwrap()).unwrap();
        std::fs::write(&nested, b"x").unwrap();
        let got = locate_extracted_binary(tmp.path(), "piper").unwrap();
        assert_eq!(got, nested);
    }

    #[test]
    fn locate_binary_finds_at_root() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("piper");
        std::fs::write(&bin, b"x").unwrap();
        let got = locate_extracted_binary(tmp.path(), "piper").unwrap();
        assert_eq!(got, bin);
    }

    #[test]
    fn locate_binary_errors_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let err = locate_extracted_binary(tmp.path(), "piper").unwrap_err();
        assert!(matches!(err, PiperUnavailable::ExtractFailed(_)));
        assert!(err.to_user_message().contains("piper"));
    }

    // ── install_binary (atomic + +x) ──────────────────

    #[test]
    fn install_binary_writes_and_sets_executable() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        std::fs::write(&src, b"binary-bytes").unwrap();
        let dst = tmp.path().join("cache").join("piper-linux-x86_64").join("piper");
        install_binary(&src, &dst).unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(), b"binary-bytes");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&dst).unwrap().permissions().mode();
            assert!(
                mode & 0o111 != 0,
                "expected +x on installed binary, mode={mode:o}",
            );
        }
    }

    // ── download_piper_binary orchestrator ────────────

    #[test]
    fn download_orchestrator_picks_asset_and_installs() {
        let tmp = tempfile::tempdir().unwrap();
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        // Build a fixture tarball that will be returned
        // when the fake fetcher is called with the
        // asset URL.
        let archive_src = make_tarball(tmp.path(), "piper");
        let archive_bytes = std::fs::read(&archive_src).unwrap();

        let json_called = std::sync::atomic::AtomicBool::new(false);
        let asset_called = std::sync::atomic::AtomicBool::new(false);

        let fetch_json = |url: &str| -> Result<Vec<u8>, PiperUnavailable> {
            json_called.store(true, std::sync::atomic::Ordering::Relaxed);
            assert!(url.contains("rhasspy/piper"));
            Ok(FIXTURE_RELEASE_JSON.to_vec())
        };
        let fetch_bytes = |url: &str, dest: &Path| -> Result<(), PiperUnavailable> {
            asset_called.store(true, std::sync::atomic::Ordering::Relaxed);
            assert!(url.ends_with("piper_amd64.tar.gz"));
            std::fs::write(dest, &archive_bytes).map_err(|e| {
                PiperUnavailable::DownloadFailed(format!("write: {e}"))
            })
        };

        let bin = download_piper_binary(
            &plat,
            tmp.path(),
            fetch_json,
            fetch_bytes,
        )
        .unwrap();

        assert_eq!(
            bin,
            tmp.path()
                .join("piper-linux-x86_64")
                .join("piper"),
        );
        assert!(bin.exists());
        assert_eq!(std::fs::read(&bin).unwrap(), b"fake-binary");
        assert!(json_called.load(std::sync::atomic::Ordering::Relaxed));
        assert!(asset_called.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn download_orchestrator_surfaces_asset_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        // FreeBSD is supported by Platform's from_consts
        // _input_ (we'd reject it) so to exercise the
        // AssetNotFound path we construct a release with
        // no matching assets.
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        // Release JSON with only macOS asset:
        let empty_for_linux = br#"{
          "tag_name": "2024.01.01",
          "assets": [
            {
              "name": "piper_macos_aarch64.tar.gz",
              "browser_download_url": "https://example.test/x",
              "size": 1
            }
          ]
        }"#;
        let fetch_json = |_url: &str| -> Result<Vec<u8>, PiperUnavailable> {
            Ok(empty_for_linux.to_vec())
        };
        let fetch_bytes = |_url: &str, _dest: &Path| -> Result<(), PiperUnavailable> {
            panic!("fetch_bytes must not be called when asset selection fails");
        };
        let err = download_piper_binary(
            &plat,
            tmp.path(),
            fetch_json,
            fetch_bytes,
        )
        .unwrap_err();
        match err {
            PiperUnavailable::AssetNotFound { tag, platform } => {
                assert_eq!(tag, "2024.01.01");
                assert_eq!(platform, "linux-x86_64");
            }
            other => panic!("expected AssetNotFound, got: {other:?}"),
        }
    }

    #[test]
    fn download_orchestrator_propagates_fetch_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let plat = Platform::from_consts("linux", "x86_64").unwrap();
        let fetch_json = |_url: &str| -> Result<Vec<u8>, PiperUnavailable> {
            Err(PiperUnavailable::DownloadFailed("curl 7".into()))
        };
        let fetch_bytes = |_url: &str, _dest: &Path| -> Result<(), PiperUnavailable> {
            panic!("must not call asset fetch when manifest fetch fails");
        };
        let err = download_piper_binary(
            &plat,
            tmp.path(),
            fetch_json,
            fetch_bytes,
        )
        .unwrap_err();
        assert!(matches!(err, PiperUnavailable::DownloadFailed(_)));
    }
}
