//! Per-voice download orchestration.
//!
//! T.4's job: take a `VoiceMeta` (from T.3's catalog) +
//! a `voices_dir` (resolved inside the project root via
//! the 1.2.15 `crate::path_safety`) and ensure the two
//! synthesis-critical files (`.onnx` + `.onnx.json`)
//! land in `<voices_dir>/<voice_key>.onnx{,.json}` —
//! atomically, with LRU bookkeeping, and with a
//! one-time `.gitignore` append so the project's git
//! tree doesn't bloat with 100 MB blobs.
//!
//! ## On-disk layout (flat)
//!
//! The catalog's deep Hugging Face path
//! (`en/en_US/lessac/medium/en_US-lessac-medium.onnx`)
//! is flattened to the voice-key prefix in
//! `voices_dir`:
//!
//! ```text
//! <project>/.inkhaven/voices/
//!   voices.json                          # catalog cache
//!   .lru                                  # LRU index
//!   en_US-lessac-medium.onnx
//!   en_US-lessac-medium.onnx.json
//!   ru_RU-irina-medium.onnx
//!   ru_RU-irina-medium.onnx.json
//! ```
//!
//! Flat is deliberate: a path-traversal attempt in the
//! voice key (e.g. `../../etc/passwd`) is rejected by
//! `safe_voice_filename` before we even touch the
//! filesystem.  Voice keys are sourced from the catalog,
//! but defence-in-depth matters — a future custom-
//! catalog feature could surface untrusted strings here.
//!
//! ## Skip-when-present
//!
//! `ensure_voice_downloaded` checks for both files
//! first.  When both are present + non-empty, the
//! download is skipped (we still `touch` the LRU so
//! "last used" reflects this access).  ONNX runtime
//! rejects corrupted models at load time, so the
//! catalog's `md5_digest` field is not yet enforced
//! here — that's a forward-looking T.4.b enhancement.
//!
//! ## Progress
//!
//! Every step calls back through `progress` so the
//! T.6 voice picker's status-bar chip can show
//! `[tts: en_US-lessac-medium 73%]`.  Stages are
//! discrete: downloading-onnx → downloading-config →
//! installing → updating-lru → done.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::catalog::VoiceMeta;
use super::lru::LruIndex;
use super::PiperUnavailable;

/// `.gitignore` line we append on first download.
pub(crate) const GITIGNORE_VOICES_LINE: &str = ".inkhaven/voices/";

/// Pair of synthesis files for a downloaded voice.
#[derive(Debug, Clone)]
pub(crate) struct VoiceFiles {
    pub onnx: PathBuf,
    pub onnx_json: PathBuf,
}

/// Stages emitted via the progress callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VoiceDownloadStage {
    /// Starting — files not on disk; will download.
    Starting,
    /// Fetching the .onnx model (the big one — 25–100 MB).
    DownloadingOnnx,
    /// Fetching the .onnx.json config (the small one — ~5 KB).
    DownloadingOnnxJson,
    /// Both downloaded; touching the LRU + evicting
    /// excess voices.
    Installing,
    /// Done — both files in place.
    Done,
    /// Skipped — files were already present + LRU
    /// touched.  Emitted just before `Done` when no
    /// network call happened.
    AlreadyPresent,
}

/// Progress callback payload.  Percent is a coarse
/// stage-level progress (we don't get byte counts from
/// curl-subprocess yet; T.6 wires real percent via
/// `curl --progress-bar` parsing in a follow-up).
#[derive(Debug, Clone, Copy)]
pub(crate) struct VoiceDownloadProgress {
    pub stage: VoiceDownloadStage,
    pub percent: u8,
}

/// Resolve the flat-layout `(onnx, onnx_json)` paths
/// for `voice_key` inside `voices_dir`.  Rejects keys
/// that would escape the directory.
pub(crate) fn voice_files_for(
    voices_dir: &Path,
    voice_key: &str,
) -> Result<VoiceFiles, PiperUnavailable> {
    let onnx_name = safe_voice_filename(voice_key, "onnx")?;
    let json_name = safe_voice_filename(voice_key, "onnx.json")?;
    Ok(VoiceFiles {
        onnx: voices_dir.join(onnx_name),
        onnx_json: voices_dir.join(json_name),
    })
}

/// Compose a filename from `voice_key` + `extension`,
/// rejecting any voice key that contains path
/// separators / `..` segments / NUL bytes.  These are
/// defensive checks: the catalog hands us clean keys,
/// but a future custom-catalog feature could surface
/// untrusted strings.
pub(crate) fn safe_voice_filename(
    voice_key: &str,
    extension: &str,
) -> Result<String, PiperUnavailable> {
    if voice_key.is_empty() {
        return Err(PiperUnavailable::VoicesDirInvalid(
            "voice key is empty".into(),
        ));
    }
    if voice_key.contains('/')
        || voice_key.contains('\\')
        || voice_key.contains('\0')
        || voice_key.split(['/', '\\']).any(|seg| seg == "..")
        || voice_key == "."
        || voice_key == ".."
    {
        return Err(PiperUnavailable::VoicesDirInvalid(format!(
            "voice key contains path-unsafe characters: {voice_key:?}",
        )));
    }
    // Also reject keys that would shadow the catalog
    // cache or the LRU index, even though those have
    // distinct names.  Belt + braces.
    if voice_key.starts_with('.') {
        return Err(PiperUnavailable::VoicesDirInvalid(format!(
            "voice key may not start with `.`: {voice_key:?}",
        )));
    }
    Ok(format!("{voice_key}.{extension}"))
}

/// True when both `.onnx` and `.onnx.json` are on disk
/// for `voice_key` + non-empty.  `safe_voice_filename`
/// is exercised here too, so an invalid key short-
/// circuits to false (a separate caller path will
/// surface the validation error).
pub(crate) fn voice_files_present(voices_dir: &Path, voice_key: &str) -> bool {
    let Ok(files) = voice_files_for(voices_dir, voice_key) else {
        return false;
    };
    is_nonempty_file(&files.onnx) && is_nonempty_file(&files.onnx_json)
}

fn is_nonempty_file(p: &Path) -> bool {
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

/// Ensure the synthesis files for `voice` are present
/// in `voices_dir`.  See module docs for the full
/// orchestration shape.
pub(crate) fn ensure_voice_downloaded(
    voice: &VoiceMeta,
    voices_dir: &Path,
    project_root: &Path,
    cache_max_voices: usize,
    auto_gitignore: bool,
    fetch_to_file: impl Fn(&str, &Path) -> Result<(), PiperUnavailable>,
    mut progress: impl FnMut(VoiceDownloadProgress),
) -> Result<VoiceFiles, PiperUnavailable> {
    ensure_voice_downloaded_with_clock(
        voice,
        voices_dir,
        project_root,
        cache_max_voices,
        auto_gitignore,
        SystemTime::now(),
        fetch_to_file,
        &mut progress,
    )
}

/// `ensure_voice_downloaded` with an injected clock for
/// deterministic LRU testing.
pub(crate) fn ensure_voice_downloaded_with_clock(
    voice: &VoiceMeta,
    voices_dir: &Path,
    project_root: &Path,
    cache_max_voices: usize,
    auto_gitignore: bool,
    now: SystemTime,
    fetch_to_file: impl Fn(&str, &Path) -> Result<(), PiperUnavailable>,
    progress: &mut dyn FnMut(VoiceDownloadProgress),
) -> Result<VoiceFiles, PiperUnavailable> {
    let files = voice_files_for(voices_dir, &voice.key)?;

    // 1. Skip if both present.
    if voice_files_present(voices_dir, &voice.key) {
        progress(VoiceDownloadProgress {
            stage: VoiceDownloadStage::AlreadyPresent,
            percent: 100,
        });
        touch_lru(voices_dir, &voice.key, cache_max_voices, now)?;
        progress(VoiceDownloadProgress {
            stage: VoiceDownloadStage::Done,
            percent: 100,
        });
        return Ok(files);
    }

    progress(VoiceDownloadProgress {
        stage: VoiceDownloadStage::Starting,
        percent: 0,
    });

    // 2. Download both files into a staging area so a
    //    partial transfer never poisons the final
    //    location.  Each goes to a sibling temp file +
    //    moves into place via io_atomic.
    std::fs::create_dir_all(voices_dir).map_err(|e| {
        PiperUnavailable::DownloadFailed(format!(
            "mkdir voices_dir {}: {e}",
            voices_dir.display(),
        ))
    })?;

    let onnx_url = voice.onnx_url().ok_or_else(|| {
        PiperUnavailable::DownloadFailed(format!(
            "voice {} has no .onnx file in the catalog",
            voice.key,
        ))
    })?;
    let onnx_json_url = voice.onnx_json_url().ok_or_else(|| {
        PiperUnavailable::DownloadFailed(format!(
            "voice {} has no .onnx.json file in the catalog",
            voice.key,
        ))
    })?;

    let staging_onnx = staging_path(voices_dir, &voice.key, "onnx.part")?;
    let staging_json =
        staging_path(voices_dir, &voice.key, "onnx.json.part")?;

    // Clean any leftover staging from a prior failed
    // attempt.
    let _ = std::fs::remove_file(&staging_onnx);
    let _ = std::fs::remove_file(&staging_json);

    progress(VoiceDownloadProgress {
        stage: VoiceDownloadStage::DownloadingOnnx,
        percent: 10,
    });
    fetch_to_file(&onnx_url, &staging_onnx).inspect_err(|_e| {
        let _ = std::fs::remove_file(&staging_onnx);
    })?;

    progress(VoiceDownloadProgress {
        stage: VoiceDownloadStage::DownloadingOnnxJson,
        percent: 70,
    });
    fetch_to_file(&onnx_json_url, &staging_json).inspect_err(|_e| {
        let _ = std::fs::remove_file(&staging_onnx);
        let _ = std::fs::remove_file(&staging_json);
    })?;

    progress(VoiceDownloadProgress {
        stage: VoiceDownloadStage::Installing,
        percent: 90,
    });

    // 3. Move into place via io_atomic.  Read the
    //    staged file + atomic-write to the final path
    //    — the staged file becomes irrelevant after a
    //    successful install.
    install_atomic(&staging_onnx, &files.onnx)?;
    install_atomic(&staging_json, &files.onnx_json)?;
    let _ = std::fs::remove_file(&staging_onnx);
    let _ = std::fs::remove_file(&staging_json);

    // 4. LRU touch + eviction.
    let evicted =
        touch_lru(voices_dir, &voice.key, cache_max_voices, now)?;
    for key in evicted {
        delete_voice_files(voices_dir, &key);
    }

    // 5. First-ever-download .gitignore append.
    if auto_gitignore {
        if let Err(e) = append_to_gitignore(project_root) {
            // .gitignore failure is non-fatal — the
            // user still got their voice; we just
            // couldn't update the ignore.  Log + carry
            // on.
            tracing::warn!(
                "auto_gitignore append failed for project {}: {}",
                project_root.display(),
                e.to_user_message(),
            );
        }
    }

    progress(VoiceDownloadProgress {
        stage: VoiceDownloadStage::Done,
        percent: 100,
    });
    Ok(files)
}

fn staging_path(
    voices_dir: &Path,
    voice_key: &str,
    suffix: &str,
) -> Result<PathBuf, PiperUnavailable> {
    let name = safe_voice_filename(voice_key, suffix)?;
    Ok(voices_dir.join(name))
}

fn install_atomic(src: &Path, dst: &Path) -> Result<(), PiperUnavailable> {
    let bytes = std::fs::read(src).map_err(|e| {
        PiperUnavailable::DownloadFailed(format!(
            "read staged {}: {e}",
            src.display(),
        ))
    })?;
    crate::io_atomic::write(dst, &bytes).map_err(|e| {
        PiperUnavailable::DownloadFailed(format!(
            "atomic write {}: {e}",
            dst.display(),
        ))
    })
}

fn touch_lru(
    voices_dir: &Path,
    voice_key: &str,
    cache_max_voices: usize,
    now: SystemTime,
) -> Result<Vec<String>, PiperUnavailable> {
    let mut idx = LruIndex::load(voices_dir);
    idx.touch(voice_key, now);
    let evicted = idx.evict_beyond(cache_max_voices);
    idx.save(voices_dir)?;
    Ok(evicted)
}

fn delete_voice_files(voices_dir: &Path, voice_key: &str) {
    // Best-effort: removed files become "needs
    // redownload" on the next ensure call.  Errors are
    // logged but not propagated — eviction shouldn't
    // fail a download.
    if let Ok(files) = voice_files_for(voices_dir, voice_key) {
        if files.onnx.exists() {
            if let Err(e) = std::fs::remove_file(&files.onnx) {
                tracing::warn!(
                    "LRU eviction couldn't remove {}: {e}",
                    files.onnx.display(),
                );
            }
        }
        if files.onnx_json.exists() {
            if let Err(e) = std::fs::remove_file(&files.onnx_json) {
                tracing::warn!(
                    "LRU eviction couldn't remove {}: {e}",
                    files.onnx_json.display(),
                );
            }
        }
    }
}

/// Idempotently append `GITIGNORE_VOICES_LINE` to
/// `<project_root>/.gitignore`.  Creates the file if
/// absent.  Skips when the line already appears
/// (with or without a trailing slash).
pub(crate) fn append_to_gitignore(
    project_root: &Path,
) -> Result<(), PiperUnavailable> {
    let path = project_root.join(".gitignore");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if gitignore_already_lists(&existing) {
        return Ok(());
    }
    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(GITIGNORE_VOICES_LINE);
    next.push('\n');
    crate::io_atomic::write(&path, next.as_bytes()).map_err(|e| {
        PiperUnavailable::DownloadFailed(format!(
            "write .gitignore: {e}",
        ))
    })
}

/// True when the .gitignore content already excludes
/// the voices dir (tolerant of trailing-slash variations
/// + leading whitespace).
fn gitignore_already_lists(content: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        let normalised = trimmed.trim_end_matches('/');
        if normalised == ".inkhaven/voices" {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::piper::catalog::{parse_voice_catalog, VoiceMeta};
    use std::collections::BTreeMap;
    use std::time::{Duration, UNIX_EPOCH};

    const FIXTURE: &str = r#"
    {
      "en_US-lessac-medium": {
        "key": "en_US-lessac-medium",
        "name": "lessac",
        "language": {
          "code": "en_US", "family": "en",
          "name_native": "English", "name_english": "English"
        },
        "quality": "medium",
        "num_speakers": 1,
        "files": {
          "en/en_US/lessac/medium/en_US-lessac-medium.onnx": {
            "size_bytes": 1234, "md5_digest": "x"
          },
          "en/en_US/lessac/medium/en_US-lessac-medium.onnx.json": {
            "size_bytes": 56, "md5_digest": "y"
          }
        },
        "aliases": []
      }
    }
    "#;

    fn lessac() -> VoiceMeta {
        parse_voice_catalog(FIXTURE.as_bytes())
            .unwrap()
            .voices
            .remove("en_US-lessac-medium")
            .unwrap()
    }

    // ── safe_voice_filename ───────────────────────────

    #[test]
    fn safe_voice_filename_composes() {
        let got = safe_voice_filename("en_US-lessac-medium", "onnx").unwrap();
        assert_eq!(got, "en_US-lessac-medium.onnx");
    }

    #[test]
    fn safe_voice_filename_rejects_slash() {
        let err =
            safe_voice_filename("../etc/passwd", "onnx").unwrap_err();
        assert!(matches!(err, PiperUnavailable::VoicesDirInvalid(_)));
    }

    #[test]
    fn safe_voice_filename_rejects_backslash() {
        let err =
            safe_voice_filename("..\\windows\\system32", "onnx").unwrap_err();
        assert!(matches!(err, PiperUnavailable::VoicesDirInvalid(_)));
    }

    #[test]
    fn safe_voice_filename_rejects_dotdot_segment() {
        let err = safe_voice_filename("..", "onnx").unwrap_err();
        assert!(matches!(err, PiperUnavailable::VoicesDirInvalid(_)));
    }

    #[test]
    fn safe_voice_filename_rejects_nul() {
        let err =
            safe_voice_filename("foo\0bar", "onnx").unwrap_err();
        assert!(matches!(err, PiperUnavailable::VoicesDirInvalid(_)));
    }

    #[test]
    fn safe_voice_filename_rejects_dotfile() {
        // .lru, .voices.json etc — keep voices keys
        // distinct from the cache namespace.
        let err = safe_voice_filename(".lru", "onnx").unwrap_err();
        assert!(matches!(err, PiperUnavailable::VoicesDirInvalid(_)));
    }

    #[test]
    fn safe_voice_filename_rejects_empty() {
        let err = safe_voice_filename("", "onnx").unwrap_err();
        assert!(matches!(err, PiperUnavailable::VoicesDirInvalid(_)));
    }

    // ── voice_files_present ───────────────────────────

    #[test]
    fn files_present_false_when_neither_exists() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!voice_files_present(tmp.path(), "en_US-lessac-medium"));
    }

    #[test]
    fn files_present_false_when_only_onnx_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("en_US-lessac-medium.onnx"),
            b"x",
        )
        .unwrap();
        assert!(!voice_files_present(tmp.path(), "en_US-lessac-medium"));
    }

    #[test]
    fn files_present_true_when_both_nonempty() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("en_US-lessac-medium.onnx"),
            b"x",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("en_US-lessac-medium.onnx.json"),
            b"y",
        )
        .unwrap();
        assert!(voice_files_present(tmp.path(), "en_US-lessac-medium"));
    }

    #[test]
    fn files_present_false_when_files_are_zero_bytes() {
        // A zero-byte file from an interrupted transfer
        // must not be treated as "present".  This
        // catches an old failure mode where curl was
        // killed and left a 0-byte placeholder.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("en_US-lessac-medium.onnx"),
            b"",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("en_US-lessac-medium.onnx.json"),
            b"",
        )
        .unwrap();
        assert!(!voice_files_present(tmp.path(), "en_US-lessac-medium"));
    }

    // ── ensure_voice_downloaded ───────────────────────

    fn pass_fetch(
        onnx_body: &[u8],
        json_body: &[u8],
    ) -> impl Fn(&str, &Path) -> Result<(), PiperUnavailable> {
        let onnx_body = onnx_body.to_vec();
        let json_body = json_body.to_vec();
        move |url: &str, dest: &Path| -> Result<(), PiperUnavailable> {
            let body = if url.ends_with(".onnx") {
                &onnx_body[..]
            } else if url.ends_with(".onnx.json") {
                &json_body[..]
            } else {
                return Err(PiperUnavailable::DownloadFailed(format!(
                    "unexpected url: {url}",
                )));
            };
            std::fs::write(dest, body).map_err(|e| {
                PiperUnavailable::DownloadFailed(format!("write dest: {e}"))
            })
        }
    }

    fn never_fetch() -> impl Fn(&str, &Path) -> Result<(), PiperUnavailable> {
        |_url: &str, _dest: &Path| -> Result<(), PiperUnavailable> {
            panic!("fetch must not be called in this test");
        }
    }

    #[test]
    fn ensure_downloads_when_files_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tempfile::tempdir().unwrap();
        let v = lessac();
        let mut stages = Vec::new();
        let files = ensure_voice_downloaded_with_clock(
            &v,
            tmp.path(),
            project_root.path(),
            5,
            true,
            UNIX_EPOCH + Duration::from_secs(1_000_000),
            pass_fetch(b"ONNX-bytes", b"{\"json\":true}"),
            &mut |p| stages.push(p.stage),
        )
        .unwrap();
        assert!(files.onnx.exists());
        assert!(files.onnx_json.exists());
        assert_eq!(std::fs::read(&files.onnx).unwrap(), b"ONNX-bytes");
        assert_eq!(
            std::fs::read(&files.onnx_json).unwrap(),
            b"{\"json\":true}"
        );
        assert!(stages.contains(&VoiceDownloadStage::DownloadingOnnx));
        assert!(stages.contains(&VoiceDownloadStage::DownloadingOnnxJson));
        assert!(stages.contains(&VoiceDownloadStage::Installing));
        assert!(stages.contains(&VoiceDownloadStage::Done));
        assert!(!stages.contains(&VoiceDownloadStage::AlreadyPresent));
    }

    #[test]
    fn ensure_skips_download_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("en_US-lessac-medium.onnx"),
            b"pre-existing-onnx",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("en_US-lessac-medium.onnx.json"),
            b"pre-existing-json",
        )
        .unwrap();
        let v = lessac();
        let mut stages = Vec::new();
        let files = ensure_voice_downloaded_with_clock(
            &v,
            tmp.path(),
            project_root.path(),
            5,
            false,
            UNIX_EPOCH + Duration::from_secs(1_000_000),
            never_fetch(),
            &mut |p| stages.push(p.stage),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(&files.onnx).unwrap(),
            b"pre-existing-onnx",
        );
        assert!(stages.contains(&VoiceDownloadStage::AlreadyPresent));
        assert!(stages.contains(&VoiceDownloadStage::Done));
        assert!(!stages.contains(&VoiceDownloadStage::DownloadingOnnx));
    }

    #[test]
    fn ensure_atomic_install_no_partial_leaks() {
        // After a successful install, no .onnx.part or
        // .onnx.json.part files should remain.
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tempfile::tempdir().unwrap();
        let v = lessac();
        ensure_voice_downloaded_with_clock(
            &v,
            tmp.path(),
            project_root.path(),
            5,
            false,
            UNIX_EPOCH + Duration::from_secs(1_000_000),
            pass_fetch(b"X", b"Y"),
            &mut |_p| {},
        )
        .unwrap();
        let stray: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .contains(".part")
            })
            .collect();
        assert!(
            stray.is_empty(),
            "expected no .part leftovers, got: {stray:?}",
        );
    }

    #[test]
    fn ensure_cleans_partial_on_second_file_failure() {
        // Onnx succeeds, json fails → both staging
        // files cleaned up + neither final file
        // present.
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tempfile::tempdir().unwrap();
        let v = lessac();
        let fetcher = |url: &str, dest: &Path| -> Result<(), PiperUnavailable> {
            if url.ends_with(".onnx.json") {
                Err(PiperUnavailable::DownloadFailed("network".into()))
            } else {
                std::fs::write(dest, b"OK").map_err(|e| {
                    PiperUnavailable::DownloadFailed(format!("write: {e}"))
                })
            }
        };
        let err = ensure_voice_downloaded_with_clock(
            &v,
            tmp.path(),
            project_root.path(),
            5,
            false,
            UNIX_EPOCH + Duration::from_secs(1_000_000),
            fetcher,
            &mut |_p| {},
        )
        .unwrap_err();
        assert!(matches!(err, PiperUnavailable::DownloadFailed(_)));
        let stray: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                name.contains(".part") || name.ends_with(".onnx")
            })
            .collect();
        assert!(
            stray.is_empty(),
            "expected no leftover artefacts after failed download, got: {stray:?}",
        );
    }

    #[test]
    fn ensure_lru_eviction_removes_oldest_voice_files() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tempfile::tempdir().unwrap();
        // Pre-populate two older voices.
        for older in ["fr_FR-tom-low", "es_ES-juan-low"] {
            std::fs::write(
                tmp.path().join(format!("{older}.onnx")),
                b"old",
            )
            .unwrap();
            std::fs::write(
                tmp.path().join(format!("{older}.onnx.json")),
                b"old-json",
            )
            .unwrap();
        }
        // Touch them so they exist in LRU.
        let mut idx = LruIndex::load(tmp.path());
        idx.touch("fr_FR-tom-low", UNIX_EPOCH + Duration::from_secs(100));
        idx.touch(
            "es_ES-juan-low",
            UNIX_EPOCH + Duration::from_secs(200),
        );
        idx.save(tmp.path()).unwrap();

        let v = lessac();
        ensure_voice_downloaded_with_clock(
            &v,
            tmp.path(),
            project_root.path(),
            // Cap at 2 → newcomer pushes out the oldest.
            2,
            false,
            UNIX_EPOCH + Duration::from_secs(1_000_000),
            pass_fetch(b"NEW-onnx", b"NEW-json"),
            &mut |_p| {},
        )
        .unwrap();

        // fr_FR-tom-low was older → evicted.
        assert!(
            !tmp.path().join("fr_FR-tom-low.onnx").exists(),
            "expected oldest voice's .onnx removed",
        );
        assert!(
            !tmp.path().join("fr_FR-tom-low.onnx.json").exists(),
            "expected oldest voice's .onnx.json removed",
        );
        // es_ES kept.
        assert!(tmp.path().join("es_ES-juan-low.onnx").exists());
        // New voice landed.
        assert!(
            tmp.path().join("en_US-lessac-medium.onnx").exists(),
            "expected newly downloaded voice present",
        );
    }

    #[test]
    fn ensure_skipped_download_still_touches_lru() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("en_US-lessac-medium.onnx"),
            b"x",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("en_US-lessac-medium.onnx.json"),
            b"y",
        )
        .unwrap();
        let v = lessac();
        ensure_voice_downloaded_with_clock(
            &v,
            tmp.path(),
            project_root.path(),
            5,
            false,
            UNIX_EPOCH + Duration::from_secs(1_000_000),
            never_fetch(),
            &mut |_p| {},
        )
        .unwrap();
        let idx = LruIndex::load(tmp.path());
        assert!(idx.contains("en_US-lessac-medium"));
    }

    #[test]
    fn ensure_rejects_path_traversal_voice_key() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tempfile::tempdir().unwrap();
        // Build a voice with a malicious key.
        let mut v = lessac();
        v.key = "../../etc/passwd".to_string();
        let err = ensure_voice_downloaded_with_clock(
            &v,
            tmp.path(),
            project_root.path(),
            5,
            false,
            UNIX_EPOCH + Duration::from_secs(1_000_000),
            never_fetch(),
            &mut |_p| {},
        )
        .unwrap_err();
        assert!(matches!(err, PiperUnavailable::VoicesDirInvalid(_)));
    }

    #[test]
    fn ensure_rejects_voice_without_onnx_in_catalog() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tempfile::tempdir().unwrap();
        let mut v = lessac();
        v.files = BTreeMap::new(); // strip files
        let err = ensure_voice_downloaded_with_clock(
            &v,
            tmp.path(),
            project_root.path(),
            5,
            false,
            UNIX_EPOCH + Duration::from_secs(1_000_000),
            never_fetch(),
            &mut |_p| {},
        )
        .unwrap_err();
        assert!(matches!(err, PiperUnavailable::DownloadFailed(_)));
    }

    // ── .gitignore append ─────────────────────────────

    #[test]
    fn gitignore_creates_file_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        append_to_gitignore(tmp.path()).unwrap();
        let content =
            std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains(".inkhaven/voices/"));
    }

    #[test]
    fn gitignore_append_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        append_to_gitignore(tmp.path()).unwrap();
        append_to_gitignore(tmp.path()).unwrap();
        let content =
            std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        let count = content.matches(".inkhaven/voices/").count();
        assert_eq!(count, 1, "expected exactly one occurrence, got {count}");
    }

    #[test]
    fn gitignore_recognises_without_trailing_slash() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(".gitignore"),
            "build/\n.inkhaven/voices\nsomething-else\n",
        )
        .unwrap();
        append_to_gitignore(tmp.path()).unwrap();
        let content =
            std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        // No duplicate appended because the slash-less
        // form is already present.
        let count = content
            .lines()
            .filter(|l| l.trim().trim_end_matches('/') == ".inkhaven/voices")
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn gitignore_appends_after_other_lines() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(".gitignore"),
            "target/\nnode_modules/\n",
        )
        .unwrap();
        append_to_gitignore(tmp.path()).unwrap();
        let content =
            std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("target/"));
        assert!(content.contains("node_modules/"));
        assert!(content.contains(".inkhaven/voices/"));
    }

    #[test]
    fn gitignore_handles_no_trailing_newline() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".gitignore"), "build/").unwrap();
        append_to_gitignore(tmp.path()).unwrap();
        let content =
            std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        // The new content must include both lines on
        // separate lines.
        assert!(content.starts_with("build/\n"));
        assert!(content.contains(".inkhaven/voices/"));
    }

    #[test]
    fn ensure_skips_gitignore_when_auto_off() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tempfile::tempdir().unwrap();
        let v = lessac();
        ensure_voice_downloaded_with_clock(
            &v,
            tmp.path(),
            project_root.path(),
            5,
            false, // auto_gitignore off
            UNIX_EPOCH + Duration::from_secs(1_000_000),
            pass_fetch(b"X", b"Y"),
            &mut |_p| {},
        )
        .unwrap();
        assert!(
            !project_root.path().join(".gitignore").exists(),
            "auto_gitignore=false must not create .gitignore",
        );
    }

    #[test]
    fn ensure_writes_gitignore_when_auto_on() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tempfile::tempdir().unwrap();
        let v = lessac();
        ensure_voice_downloaded_with_clock(
            &v,
            tmp.path(),
            project_root.path(),
            5,
            true,
            UNIX_EPOCH + Duration::from_secs(1_000_000),
            pass_fetch(b"X", b"Y"),
            &mut |_p| {},
        )
        .unwrap();
        let gitignore_path = project_root.path().join(".gitignore");
        assert!(gitignore_path.exists());
        let content = std::fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains(".inkhaven/voices/"));
    }
}
