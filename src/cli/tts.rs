//! 1.2.17+ T.7 — `inkhaven tts …` subcommand family.
//!
//! Headless management of the Piper TTS stack.  Every
//! subcommand mirrors a TUI surface so projects can be
//! managed from scripts, CI gates, and remote-shell
//! sessions:
//!
//!   * `tts engine` — engine resolution + readiness
//!     (mirrors the status-bar chip).
//!   * `tts binary status / download` — Piper binary
//!     management (mirrors the lazy resolution +
//!     auto-download flow from the engine).
//!   * `tts voice list / download / remove` —
//!     per-voice CRUD (mirrors `Ctrl+B Shift+V`).
//!   * `tts catalog refresh` — voice catalog cache
//!     reset.
//!   * `tts test "<phrase>"` — cross-platform synth +
//!     playback diagnostic.
//!
//! Each subcommand loads the project's HJSON config
//! (falling back to defaults when the project doesn't
//! have one), runs through the same building blocks
//! the engine uses at runtime, and prints a stable
//! line-oriented summary suitable for grep / awk
//! piping.
//!
//! ## Test factoring
//!
//! Output is built by pure `format_*` functions so the
//! test suite can assert exact rendering without
//! touching the filesystem or the network.  Filesystem
//! + subprocess side effects are exercised through
//! integration-flavoured tests that pre-populate
//! voices_dir + run the subcommand against fake-piper
//! binaries.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::{Config, TtsConfig};
use crate::tui::piper::binary::{
    self, Platform,
};
use crate::tui::piper::catalog::{
    Catalog, CatalogFile, VoiceMeta, CATALOG_FILENAME,
};
use crate::tui::piper::download::{
    self, curl_get_json, curl_get_to_file,
};
use crate::tui::piper::lru::LruIndex;
use crate::tui::piper::voice::{
    self, ensure_voice_downloaded, voice_files_for, voice_files_present,
};
use crate::tui::piper::PiperUnavailable;

use super::{
    TtsBinarySubcommand, TtsCatalogSubcommand, TtsCommand,
    TtsVoiceSubcommand,
};

pub fn run(project: &Path, cmd: TtsCommand) -> Result<()> {
    match cmd {
        TtsCommand::Engine => engine_status(project),
        TtsCommand::Binary(TtsBinarySubcommand::Status) => {
            binary_status(project)
        }
        TtsCommand::Binary(TtsBinarySubcommand::Download) => {
            binary_download(project)
        }
        TtsCommand::Voice(TtsVoiceSubcommand::List { filter, downloaded }) => {
            voice_list(project, filter.as_deref(), downloaded)
        }
        TtsCommand::Voice(TtsVoiceSubcommand::Download { name }) => {
            voice_download(project, &name)
        }
        TtsCommand::Voice(TtsVoiceSubcommand::Remove { name }) => {
            voice_remove(project, &name)
        }
        TtsCommand::Catalog(TtsCatalogSubcommand::Refresh) => {
            catalog_refresh(project)
        }
        TtsCommand::Test { phrase, voice, output } => {
            test_phrase(project, &phrase, voice.as_deref(), output.as_deref())
        }
    }
}

// ── engine status ────────────────────────────────────

fn engine_status(project: &Path) -> Result<()> {
    let cfg = load_config(project);
    let tts_cfg = &cfg.editor.tts;

    print!("{}", format_engine_status(tts_cfg, project));
    Ok(())
}

/// Pure formatter so tests can assert the output
/// shape without resolving a real engine.
pub(crate) fn format_engine_status(
    cfg: &TtsConfig,
    project: &Path,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("inkhaven TTS engine — v{}\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!("project:      {}\n", project.display()));
    out.push_str(&format!("master switch: {}\n", if cfg.enabled { "enabled" } else { "disabled" }));
    out.push_str(&format!("requested:    tts.engine = {:?}\n", cfg.engine));
    out.push_str(&format!("voice:        {}\n", cfg.voice));
    out.push_str(&format!("speed:        {}\n", cfg.speed));

    if !cfg.enabled {
        out.push_str("\n→ effective backend: disabled (master switch off)\n");
        return out;
    }

    let platform = match Platform::detect() {
        Ok(p) => p,
        Err(e) => {
            out.push_str(&format!(
                "\n→ effective backend: System (Piper not supported on this platform — {})\n",
                e.to_user_message(),
            ));
            return out;
        }
    };
    out.push_str(&format!("platform:     {}\n", platform.label()));

    let cache_root = binary::user_cache_root();
    out.push_str(&format!("piper cache:  {}\n", cache_root.display()));

    let binary_result = binary::resolve_piper_binary(
        cfg,
        &platform,
        &cache_root,
        |_p, _r| Err(PiperUnavailable::BinaryNotFound(PathBuf::new())),
    );

    match binary_result {
        Ok(bin) => {
            out.push_str(&format!("piper binary: {}\n", bin.display()));
            let resolved = match cfg.engine.trim() {
                "system" => "System (forced)",
                "piper" => "Piper",
                _ => "Piper (auto)",
            };
            out.push_str(&format!(
                "\n→ effective backend: {resolved}\n",
            ));
        }
        Err(_) => {
            out.push_str("piper binary: not found on PATH or in cache\n");
            let resolved = match cfg.engine.trim() {
                "system" => "System",
                "piper" => "Disabled (forced Piper but binary not found)",
                _ => "System (auto fall-through; run `inkhaven tts binary download` to bring Piper online)",
            };
            out.push_str(&format!(
                "\n→ effective backend: {resolved}\n",
            ));
        }
    }
    out
}

// ── binary status / download ─────────────────────────

fn binary_status(project: &Path) -> Result<()> {
    let cfg = load_config(project);
    print!("{}", format_binary_status(&cfg.editor.tts));
    Ok(())
}

pub(crate) fn format_binary_status(cfg: &TtsConfig) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "inkhaven Piper binary — v{}\n",
        env!("CARGO_PKG_VERSION"),
    ));
    let platform = match Platform::detect() {
        Ok(p) => p,
        Err(e) => {
            out.push_str(&format!(
                "platform:    unsupported ({})\n",
                e.to_user_message(),
            ));
            return out;
        }
    };
    out.push_str(&format!("platform:    {}\n", platform.label()));
    let cache_root = binary::user_cache_root();
    let expected = cache_root
        .join(platform.cache_subdir())
        .join(platform.binary_filename());
    out.push_str(&format!("cache root:  {}\n", cache_root.display()));
    out.push_str(&format!("expected at: {}\n", expected.display()));
    out.push_str(&format!(
        "auto_download_binary: {}\n",
        cfg.auto_download_binary,
    ));
    if let Some(p) = cfg.binary_path.as_ref() {
        out.push_str(&format!("override:    tts.binary_path = {p}\n"));
    } else {
        out.push_str("override:    (none)\n");
    }
    let resolved = binary::resolve_piper_binary(
        cfg,
        &platform,
        &cache_root,
        |_p, _r| Err(PiperUnavailable::BinaryNotFound(PathBuf::new())),
    );
    match resolved {
        Ok(bin) => {
            let size = std::fs::metadata(&bin).map(|m| m.len()).unwrap_or(0);
            out.push_str(&format!(
                "resolved:    {} ({} bytes)\n",
                bin.display(),
                size,
            ));
        }
        Err(_) => {
            out.push_str(
                "resolved:    NOT FOUND — run `inkhaven tts binary download`\n",
            );
        }
    }
    out
}

fn binary_download(_project: &Path) -> Result<()> {
    println!("inkhaven Piper binary download — v{}", env!("CARGO_PKG_VERSION"));
    let platform = Platform::detect()
        .map_err(|e| anyhow::anyhow!(e.to_user_message()))?;
    println!("platform:   {}", platform.label());
    let cache_root = binary::user_cache_root();
    println!("cache root: {}", cache_root.display());
    print!("fetching from GitHub Releases ... ");
    let bin = download::download_piper_binary(
        &platform,
        &cache_root,
        curl_get_json,
        curl_get_to_file,
    )
    .map_err(|e| anyhow::anyhow!(e.to_user_message()))?;
    let size = std::fs::metadata(&bin).map(|m| m.len()).unwrap_or(0);
    println!("OK");
    println!("installed:  {} ({} bytes)", bin.display(), size);
    Ok(())
}

// ── voice list / download / remove ───────────────────

fn voice_list(
    project: &Path,
    filter: Option<&str>,
    downloaded_only: bool,
) -> Result<()> {
    let cfg = load_config(project);
    let voices_dir = match resolve_voices_dir(project, &cfg.editor.tts) {
        Ok(p) => p,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "tts.voices_dir invalid: {}",
                e.to_user_message(),
            ));
        }
    };
    let _ = std::fs::create_dir_all(&voices_dir);
    let ttl = std::time::Duration::from_secs(
        cfg.editor.tts.catalog_ttl_hours as u64 * 3600,
    );
    let catalog_result = Catalog::load(
        &voices_dir,
        &cfg.editor.tts.catalog_url,
        ttl,
        curl_get_json,
    );
    print!(
        "{}",
        format_voice_list(catalog_result, &voices_dir, filter, downloaded_only),
    );
    Ok(())
}

pub(crate) fn format_voice_list(
    catalog: std::result::Result<Catalog, PiperUnavailable>,
    voices_dir: &Path,
    filter: Option<&str>,
    downloaded_only: bool,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("inkhaven voices — {}\n", voices_dir.display()));
    let needle = filter.map(|s| s.to_lowercase());

    let mut rows: Vec<VoiceRow> = Vec::new();
    let header_status: String;
    match catalog {
        Ok(cat) => {
            header_status = if cat.stale {
                "catalog: stale (cached)".into()
            } else {
                "catalog: fresh".into()
            };
            for v in cat.voices.values() {
                let downloaded =
                    voice_files_present(voices_dir, &v.key);
                rows.push(VoiceRow {
                    key: v.key.clone(),
                    language: v.language_code.clone(),
                    quality: v.quality.clone(),
                    size_mb: v.synthesis_size_bytes() / 1_048_576,
                    downloaded,
                });
            }
        }
        Err(e) => {
            header_status = format!(
                "catalog: offline ({})",
                e.to_user_message().chars().take(60).collect::<String>(),
            );
            // List dir-only voices.
            if let Ok(read) = std::fs::read_dir(voices_dir) {
                for entry in read.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if let Some(key) = name.strip_suffix(".onnx") {
                        if key.ends_with(".onnx") {
                            continue;
                        }
                        if key.starts_with('.') {
                            continue;
                        }
                        let downloaded =
                            voice_files_present(voices_dir, key);
                        if !downloaded {
                            continue;
                        }
                        let (lang, quality) =
                            split_canonical_key(key);
                        rows.push(VoiceRow {
                            key: key.to_string(),
                            language: lang,
                            quality,
                            size_mb: 0,
                            downloaded,
                        });
                    }
                }
            }
        }
    }
    out.push_str(&format!("{header_status}\n"));

    // Apply filter + downloaded_only.
    rows.retain(|r| {
        let lang_match = needle
            .as_ref()
            .map(|n| {
                r.key.to_lowercase().contains(n)
                    || r.language.to_lowercase().contains(n)
            })
            .unwrap_or(true);
        let dl_match = !downloaded_only || r.downloaded;
        lang_match && dl_match
    });

    // Sort.
    rows.sort_by(|a, b| {
        a.language.cmp(&b.language).then(a.key.cmp(&b.key))
    });
    out.push_str(&format!("count:       {}\n\n", rows.len()));
    if rows.is_empty() {
        out.push_str("(no voices to show)\n");
        return out;
    }
    out.push_str(&format!(
        "{:<32} {:<10} {:<8} {:<10} {}\n",
        "key", "language", "quality", "size", "status",
    ));
    for r in &rows {
        let chip = if r.downloaded { "[✓ downloaded]" } else { "[⬇ available]" };
        let size = if r.size_mb > 0 {
            format!("{}MB", r.size_mb)
        } else {
            "?".to_string()
        };
        out.push_str(&format!(
            "{:<32} {:<10} {:<8} {:<10} {}\n",
            r.key, r.language, r.quality, size, chip,
        ));
    }
    out
}

#[derive(Debug)]
struct VoiceRow {
    key: String,
    language: String,
    quality: String,
    size_mb: u64,
    downloaded: bool,
}

fn split_canonical_key(key: &str) -> (String, String) {
    let parts: Vec<&str> = key.splitn(3, '-').collect();
    if parts.len() == 3 {
        (parts[0].to_string(), parts[2].to_string())
    } else {
        ("?".into(), "?".into())
    }
}

fn voice_download(project: &Path, name: &str) -> Result<()> {
    let cfg = load_config(project);
    let voices_dir = resolve_voices_dir(project, &cfg.editor.tts)
        .map_err(|e| anyhow::anyhow!(e.to_user_message()))?;
    let _ = std::fs::create_dir_all(&voices_dir);
    println!("inkhaven voice download — {name}");
    println!("voices_dir: {}", voices_dir.display());
    let ttl = std::time::Duration::from_secs(
        cfg.editor.tts.catalog_ttl_hours as u64 * 3600,
    );
    let catalog = Catalog::load(
        &voices_dir,
        &cfg.editor.tts.catalog_url,
        ttl,
        curl_get_json,
    )
    .map_err(|e| anyhow::anyhow!(e.to_user_message()))?;
    let voice = catalog
        .voice(name)
        .ok_or_else(|| anyhow::anyhow!("voice `{name}` not found in catalog"))?;
    if voice_files_present(&voices_dir, &voice.key) {
        let files = voice_files_for(&voices_dir, &voice.key)
            .map_err(|e| anyhow::anyhow!(e.to_user_message()))?;
        println!("status:     already present");
        println!("onnx:       {}", files.onnx.display());
        println!("onnx.json:  {}", files.onnx_json.display());
        return Ok(());
    }
    print!("downloading ... ");
    let files = ensure_voice_downloaded(
        voice,
        &voices_dir,
        project,
        cfg.editor.tts.cache_max_voices,
        cfg.editor.tts.auto_gitignore,
        curl_get_to_file,
        |_progress| {},
    )
    .map_err(|e| anyhow::anyhow!(e.to_user_message()))?;
    println!("OK");
    println!("onnx:       {}", files.onnx.display());
    println!("onnx.json:  {}", files.onnx_json.display());
    Ok(())
}

fn voice_remove(project: &Path, name: &str) -> Result<()> {
    let cfg = load_config(project);
    let voices_dir = resolve_voices_dir(project, &cfg.editor.tts)
        .map_err(|e| anyhow::anyhow!(e.to_user_message()))?;
    let files = voice_files_for(&voices_dir, name)
        .map_err(|e| anyhow::anyhow!(e.to_user_message()))?;
    let mut removed = 0;
    if files.onnx.exists() {
        std::fs::remove_file(&files.onnx)?;
        removed += 1;
    }
    if files.onnx_json.exists() {
        std::fs::remove_file(&files.onnx_json)?;
        removed += 1;
    }
    // LRU update.
    let mut idx = LruIndex::load(&voices_dir);
    let kept: Vec<(String, std::time::Duration)> = idx
        .entries()
        .iter()
        .filter(|e| e.voice_key != name)
        .map(|e| (e.voice_key.clone(), e.last_touched))
        .collect();
    idx = LruIndex::default();
    for (key, when) in kept {
        idx.touch(&key, std::time::SystemTime::UNIX_EPOCH + when);
    }
    let _ = idx.save(&voices_dir);
    if removed == 0 {
        println!("voice `{name}` was not on disk; nothing removed");
    } else {
        println!("removed {removed} file(s) for voice `{name}`");
    }
    Ok(())
}

// ── catalog refresh ──────────────────────────────────

fn catalog_refresh(project: &Path) -> Result<()> {
    let cfg = load_config(project);
    let voices_dir = resolve_voices_dir(project, &cfg.editor.tts)
        .map_err(|e| anyhow::anyhow!(e.to_user_message()))?;
    let _ = std::fs::create_dir_all(&voices_dir);
    let cache = voices_dir.join(CATALOG_FILENAME);
    if cache.exists() {
        std::fs::remove_file(&cache)?;
        println!("removed cached catalog: {}", cache.display());
    }
    // Force a refetch.
    let ttl = std::time::Duration::from_secs(
        cfg.editor.tts.catalog_ttl_hours as u64 * 3600,
    );
    let catalog = Catalog::load(
        &voices_dir,
        &cfg.editor.tts.catalog_url,
        ttl,
        curl_get_json,
    )
    .map_err(|e| anyhow::anyhow!(e.to_user_message()))?;
    println!(
        "fetched: {} voice(s) across {} language(s)",
        catalog.voices.len(),
        catalog.languages().len(),
    );
    Ok(())
}

// ── test phrase ──────────────────────────────────────

fn test_phrase(
    project: &Path,
    phrase: &str,
    voice_override: Option<&str>,
    output: Option<&Path>,
) -> Result<()> {
    let mut cfg = load_config(project);
    if let Some(v) = voice_override {
        cfg.editor.tts.voice = v.to_string();
    }
    println!("inkhaven TTS test (engine-routed) — v{}", env!("CARGO_PKG_VERSION"));
    println!("project:  {}", project.display());
    println!("phrase:   {phrase:?}");
    println!("voice:    {}", cfg.editor.tts.voice);
    println!("engine:   {}", cfg.editor.tts.engine);

    if !cfg.editor.tts.enabled {
        println!("→ disabled (set editor.tts.enabled = true)");
        return Err(anyhow::anyhow!("TTS disabled"));
    }

    let engine = crate::tui::tts::TtsEngine::resolve(&cfg.editor.tts, project);
    println!("resolved: {}", engine.label());
    engine
        .is_ready()
        .map_err(|e| anyhow::anyhow!(e))?;
    let mut engine = engine;

    if let Some(out) = output {
        print!("synthesising to {} ... ", out.display());
        let resolved_voice =
            engine.resolve_voice(&cfg.editor.tts.voice).unwrap_or_default();
        let bytes = engine
            .speak_to_file_blocking(
                phrase,
                &resolved_voice,
                Some(180),
                out,
                std::time::Duration::from_secs(60),
            )
            .map_err(|e| anyhow::anyhow!(e))?;
        println!("OK ({} bytes)", bytes);
        return Ok(());
    }
    print!("synthesising + playing ... ");
    let resolved_voice =
        engine.resolve_voice(&cfg.editor.tts.voice).unwrap_or_default();
    engine
        .speak(phrase, &resolved_voice, Some(180))
        .map_err(|e| anyhow::anyhow!(e))?;
    // Wait for playback to finish — this CLI is
    // intended to be synchronous from the caller's
    // perspective.
    while engine.is_speaking() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    println!("OK");
    Ok(())
}

// ── helpers ──────────────────────────────────────────

fn load_config(project: &Path) -> Config {
    let cfg_path = project.join("inkhaven.hjson");
    Config::load(&cfg_path).unwrap_or_else(|_| Config::default())
}

fn resolve_voices_dir(
    project: &Path,
    cfg: &TtsConfig,
) -> std::result::Result<PathBuf, PiperUnavailable> {
    crate::path_safety::resolve_within_str(project, cfg.voices_dir.trim())
        .map_err(|e| PiperUnavailable::VoicesDirInvalid(format!("{e}")))
}

// Surface a few unused-by-cli imports for the
// re-export-style `tts.rs::voice` access pattern.
#[allow(dead_code)]
fn _keep_imports_alive(
    _: &VoiceMeta,
    _: &CatalogFile,
    _: &voice::VoiceFiles,
) {
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TtsConfig;

    fn cfg_default() -> TtsConfig {
        TtsConfig::default()
    }

    // ── format_engine_status ──────────────────────────

    #[test]
    fn engine_status_reports_disabled_when_master_off() {
        let mut cfg = cfg_default();
        cfg.enabled = false;
        let out = format_engine_status(&cfg, Path::new("/tmp/proj"));
        assert!(out.contains("master switch: disabled"));
        assert!(out.contains("effective backend: disabled"));
    }

    #[test]
    fn engine_status_shows_engine_field_verbatim() {
        let mut cfg = cfg_default();
        cfg.enabled = true;
        cfg.engine = "system".into();
        let out = format_engine_status(&cfg, Path::new("/tmp/proj"));
        assert!(out.contains("tts.engine = \"system\""));
    }

    #[test]
    fn engine_status_advises_binary_download_when_missing() {
        let mut cfg = cfg_default();
        cfg.enabled = true;
        cfg.engine = "auto".into();
        cfg.binary_path =
            Some("/__inkhaven_test_no_such_piper__".into());
        cfg.auto_download_binary = false;
        let out = format_engine_status(&cfg, Path::new("/tmp/proj"));
        assert!(
            out.contains("piper binary: not found") || out.contains("auto fall-through"),
            "got: {out}",
        );
    }

    // ── format_binary_status ──────────────────────────

    #[test]
    fn binary_status_emits_platform_line() {
        let cfg = cfg_default();
        let out = format_binary_status(&cfg);
        assert!(out.contains("platform:"));
    }

    #[test]
    fn binary_status_shows_explicit_override() {
        let mut cfg = cfg_default();
        cfg.binary_path = Some("/usr/local/bin/piper".into());
        let out = format_binary_status(&cfg);
        assert!(
            out.contains("/usr/local/bin/piper"),
            "expected override path in output, got: {out}",
        );
    }

    #[test]
    fn binary_status_no_override_label() {
        let cfg = cfg_default();
        let out = format_binary_status(&cfg);
        assert!(out.contains("override:    (none)"));
    }

    // ── format_voice_list ─────────────────────────────

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
          "p/en_US-lessac-medium.onnx": {
            "size_bytes": 63206289, "md5_digest": "x"
          },
          "p/en_US-lessac-medium.onnx.json": {
            "size_bytes": 5000, "md5_digest": "y"
          }
        },
        "aliases": []
      },
      "ru_RU-irina-medium": {
        "key": "ru_RU-irina-medium",
        "name": "irina",
        "language": {
          "code": "ru_RU", "family": "ru",
          "name_native": "Russian", "name_english": "Russian"
        },
        "quality": "medium",
        "num_speakers": 1,
        "files": {
          "p/ru_RU-irina-medium.onnx": {
            "size_bytes": 63100000, "md5_digest": "a"
          },
          "p/ru_RU-irina-medium.onnx.json": {
            "size_bytes": 4800, "md5_digest": "b"
          }
        },
        "aliases": []
      }
    }
    "#;

    fn parsed_catalog() -> Catalog {
        crate::tui::piper::catalog::parse_voice_catalog(FIXTURE.as_bytes())
            .unwrap()
    }

    #[test]
    fn voice_list_renders_each_row() {
        let tmp = tempfile::tempdir().unwrap();
        let out = format_voice_list(
            Ok(parsed_catalog()),
            tmp.path(),
            None,
            false,
        );
        assert!(out.contains("en_US-lessac-medium"));
        assert!(out.contains("ru_RU-irina-medium"));
        assert!(out.contains("count:       2"));
        assert!(out.contains("[⬇ available]"));
        // Neither voice is downloaded → no
        // [✓ downloaded] chip.
        assert!(!out.contains("[✓ downloaded]"));
    }

    #[test]
    fn voice_list_marks_downloaded_with_chip() {
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
        let out = format_voice_list(
            Ok(parsed_catalog()),
            tmp.path(),
            None,
            false,
        );
        // One downloaded row plus one available row.
        let dl_count = out.matches("[✓ downloaded]").count();
        let av_count = out.matches("[⬇ available]").count();
        assert_eq!(dl_count, 1);
        assert_eq!(av_count, 1);
    }

    #[test]
    fn voice_list_filter_narrows_rows() {
        let tmp = tempfile::tempdir().unwrap();
        let out = format_voice_list(
            Ok(parsed_catalog()),
            tmp.path(),
            Some("ru"),
            false,
        );
        assert!(out.contains("ru_RU-irina-medium"));
        assert!(!out.contains("en_US-lessac-medium"));
        assert!(out.contains("count:       1"));
    }

    #[test]
    fn voice_list_downloaded_only_hides_undownloaded() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("ru_RU-irina-medium.onnx"),
            b"x",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("ru_RU-irina-medium.onnx.json"),
            b"y",
        )
        .unwrap();
        let out = format_voice_list(
            Ok(parsed_catalog()),
            tmp.path(),
            None,
            true,
        );
        assert!(out.contains("ru_RU-irina-medium"));
        assert!(!out.contains("en_US-lessac-medium"));
    }

    #[test]
    fn voice_list_offline_fallback_shows_disk_only() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("es_ES-juan-low.onnx"),
            b"x",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("es_ES-juan-low.onnx.json"),
            b"y",
        )
        .unwrap();
        let out = format_voice_list(
            Err(PiperUnavailable::DownloadFailed("network".into())),
            tmp.path(),
            None,
            false,
        );
        assert!(out.contains("catalog: offline"));
        assert!(out.contains("es_ES-juan-low"));
        assert!(out.contains("[✓ downloaded]"));
    }

    #[test]
    fn voice_list_empty_when_nothing_matches_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let out = format_voice_list(
            Ok(parsed_catalog()),
            tmp.path(),
            Some("japanese"),
            false,
        );
        assert!(out.contains("count:       0"));
        assert!(out.contains("(no voices to show)"));
    }

    #[test]
    fn voice_list_stale_catalog_chip() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cat = parsed_catalog();
        cat.stale = true;
        let out = format_voice_list(Ok(cat), tmp.path(), None, false);
        assert!(out.contains("catalog: stale"));
    }

    #[test]
    fn voice_list_sorts_by_language_then_key() {
        let tmp = tempfile::tempdir().unwrap();
        let out = format_voice_list(
            Ok(parsed_catalog()),
            tmp.path(),
            None,
            false,
        );
        // en_US before ru_RU.
        let en_pos = out.find("en_US-lessac-medium").unwrap();
        let ru_pos = out.find("ru_RU-irina-medium").unwrap();
        assert!(en_pos < ru_pos);
    }

    #[test]
    fn split_canonical_key_parses_three_parts() {
        let (lang, q) = split_canonical_key("en_US-lessac-medium");
        assert_eq!(lang, "en_US");
        assert_eq!(q, "medium");
    }

    #[test]
    fn split_canonical_key_unknown_shape_falls_back() {
        let (lang, q) = split_canonical_key("strange");
        assert_eq!(lang, "?");
        assert_eq!(q, "?");
    }
}
