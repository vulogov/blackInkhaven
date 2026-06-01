//! Piper voice catalog.
//!
//! Piper's voice catalog lives at
//! `https://huggingface.co/rhasspy/piper-voices/raw/main/voices.json`
//! as a single ~700 KB JSON document.  It maps voice
//! keys (e.g. `en_US-lessac-medium`) to metadata
//! including language, quality tier, speaker count,
//! aliases, and the per-file map (`.onnx` model + `.onnx.json`
//! config + a `MODEL_CARD` + a `COPYING` license blob).
//!
//! ## Load policy
//!
//! `Catalog::load(voices_dir, catalog_url, ttl, fetch_json)`
//! follows the freshness-first policy laid out in the
//! 1.2.17 proposal:
//!
//!   1. If `<voices_dir>/voices.json` exists and its
//!      mtime is younger than `ttl`, parse + return.
//!   2. Otherwise call `fetch_json(catalog_url)`, parse
//!      the response, write atomically to the cache,
//!      and return the fresh catalog.
//!   3. If step 2 fails *and* a cached copy exists,
//!      return the stale cache with `Catalog.stale =
//!      true` + a `tracing::warn!`.  Synthesis with
//!      already-downloaded voices must not break when
//!      the network goes away.
//!   4. If step 2 fails and no cache exists, surface
//!      the network error.
//!
//! ## Voice URL composition
//!
//! The catalog stores voice-file paths as relative
//! Hugging Face paths, e.g.
//! `en/en_US/lessac/medium/en_US-lessac-medium.onnx`.
//! `VoiceMeta::onnx_url` / `onnx_json_url` join those to
//! the resolve-base URL so T.4 has a clean download
//! source.
//!
//! ## Parser tolerance
//!
//! Real-world `voices.json` versions have shifted
//! schema over time — some have extra keys, some omit
//! `aliases`, some have a `speaker_id_map` we don't
//! care about.  `parse_voice_catalog` is tolerant:
//! unknown keys are ignored, missing optional fields
//! become empty strings / vecs / maps, but a missing
//! `language.code` drops the voice entirely (we can't
//! match users to a voice without knowing its
//! language).

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::PiperUnavailable;

/// Hugging Face base URL for resolving voice files.
/// Combined with the relative paths from `voices.json`'s
/// `files` map to produce direct download URLs.
pub(crate) const VOICES_BASE_URL: &str =
    "https://huggingface.co/rhasspy/piper-voices/resolve/main/";

/// Catalog cache filename inside `voices_dir`.
pub(crate) const CATALOG_FILENAME: &str = "voices.json";

/// Default catalog TTL when `tts.catalog_ttl_hours` is
/// missing (24 hours).  Mirrors the HJSON default.
#[allow(dead_code)]
pub(crate) const DEFAULT_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Parsed voice catalog.
#[derive(Debug, Clone)]
pub(crate) struct Catalog {
    pub voices: BTreeMap<String, VoiceMeta>,
    /// True when the catalog was returned from a stale
    /// cache because the network refresh failed.  The
    /// voice picker surfaces this with a warning chip
    /// in T.6 so users know they may be looking at out-
    /// of-date metadata.
    pub stale: bool,
}

/// Per-voice metadata pulled from the catalog.  Field
/// names match Piper's `voices.json` shape, flattened so
/// the language block becomes scalar fields and the
/// files map becomes a `BTreeMap<relative_path,
/// CatalogFile>`.
#[derive(Debug, Clone)]
pub(crate) struct VoiceMeta {
    /// Canonical voice key, e.g. `en_US-lessac-medium`.
    pub key: String,
    /// Speaker name (`lessac`, `irina`, `ryan`, etc.) —
    /// the part of the key between the locale and the
    /// quality tier.
    pub name: String,
    /// BCP-47-ish language tag, e.g. `en_US`, `ru_RU`,
    /// `fr_FR`.  Note Piper uses underscores, not the
    /// canonical `-`.
    pub language_code: String,
    /// Two-letter language family code, e.g. `en`.
    pub language_family: String,
    /// Native-script name, e.g. "Русский", "中文".
    pub language_native: String,
    /// English name, e.g. "Russian", "Chinese".
    pub language_english: String,
    /// Quality tier: `"x_low"` / `"low"` / `"medium"` /
    /// `"high"`.  Drives the size-vs-quality tradeoff
    /// the voice picker surfaces.
    pub quality: String,
    /// Speaker count; >1 for multi-speaker models.
    pub num_speakers: u32,
    /// Alternative names this voice can be looked up
    /// under.  Usually empty.
    pub aliases: Vec<String>,
    /// Per-file metadata.  The two synthesis-critical
    /// files are `.onnx` + `.onnx.json`; the catalog
    /// also lists `MODEL_CARD` + `COPYING` which we
    /// skip at download time but keep here for
    /// completeness.
    pub files: BTreeMap<String, CatalogFile>,
}

/// One file under a voice's Hugging Face path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogFile {
    pub relative_path: String,
    pub size_bytes: u64,
    pub md5_digest: String,
}

impl VoiceMeta {
    /// The two files (.onnx + .onnx.json) that voice
    /// synthesis requires.  T.4's downloader fetches
    /// these + verifies the MD5.  Order is `[onnx,
    /// onnx_json]` when both are present.
    pub fn synthesis_files(&self) -> Vec<&CatalogFile> {
        let mut out = Vec::new();
        if let Some(f) = self.onnx_file() {
            out.push(f);
        }
        if let Some(f) = self.onnx_json_file() {
            out.push(f);
        }
        out
    }

    pub fn onnx_file(&self) -> Option<&CatalogFile> {
        self.files
            .values()
            .find(|f| is_onnx_path(&f.relative_path))
    }

    pub fn onnx_json_file(&self) -> Option<&CatalogFile> {
        self.files
            .values()
            .find(|f| is_onnx_json_path(&f.relative_path))
    }

    /// Direct download URL for the `.onnx` model.  None
    /// when the catalog entry has no .onnx file (should
    /// never happen for a synthesis-capable voice; the
    /// catalog ships .onnx for every voice).
    pub fn onnx_url(&self) -> Option<String> {
        self.onnx_file()
            .map(|f| format!("{VOICES_BASE_URL}{}", f.relative_path))
    }

    /// Direct download URL for the `.onnx.json` config.
    pub fn onnx_json_url(&self) -> Option<String> {
        self.onnx_json_file()
            .map(|f| format!("{VOICES_BASE_URL}{}", f.relative_path))
    }

    /// Total bytes for the synthesis-critical files
    /// (.onnx + .onnx.json).  Used by the picker to
    /// show download size before the user commits.
    pub fn synthesis_size_bytes(&self) -> u64 {
        self.synthesis_files()
            .iter()
            .map(|f| f.size_bytes)
            .sum()
    }
}

fn is_onnx_path(p: &str) -> bool {
    p.ends_with(".onnx") && !p.ends_with(".onnx.json")
}

fn is_onnx_json_path(p: &str) -> bool {
    p.ends_with(".onnx.json")
}

impl Catalog {
    /// See the module docs for the load policy.
    pub fn load(
        voices_dir: &Path,
        catalog_url: &str,
        ttl: Duration,
        fetch_json: impl Fn(&str) -> Result<Vec<u8>, PiperUnavailable>,
    ) -> Result<Self, PiperUnavailable> {
        Self::load_with_clock(
            voices_dir,
            catalog_url,
            ttl,
            SystemTime::now(),
            fetch_json,
        )
    }

    /// `load` with an injected `now` for deterministic
    /// TTL testing.  Production calls `load` (which
    /// passes `SystemTime::now()`); tests can pin time
    /// to make TTL boundaries reproducible.
    pub fn load_with_clock(
        voices_dir: &Path,
        catalog_url: &str,
        ttl: Duration,
        now: SystemTime,
        fetch_json: impl Fn(&str) -> Result<Vec<u8>, PiperUnavailable>,
    ) -> Result<Self, PiperUnavailable> {
        let cache_path = voices_dir.join(CATALOG_FILENAME);
        // 1. Fresh cache — skip the network entirely.
        if is_cache_fresh(&cache_path, ttl, now) {
            let bytes = std::fs::read(&cache_path).map_err(|e| {
                PiperUnavailable::DownloadFailed(format!(
                    "read fresh catalog cache {}: {e}",
                    cache_path.display(),
                ))
            })?;
            return parse_voice_catalog(&bytes);
        }

        // 2. Refresh attempt.
        match fetch_json(catalog_url) {
            Ok(bytes) => {
                // Parse before writing so a corrupted
                // upstream doesn't poison the cache.
                let mut catalog = parse_voice_catalog(&bytes)?;
                std::fs::create_dir_all(voices_dir).map_err(|e| {
                    PiperUnavailable::DownloadFailed(format!(
                        "mkdir voices_dir {}: {e}",
                        voices_dir.display(),
                    ))
                })?;
                crate::io_atomic::write(&cache_path, &bytes).map_err(
                    |e| {
                        PiperUnavailable::DownloadFailed(format!(
                            "atomic write catalog cache {}: {e}",
                            cache_path.display(),
                        ))
                    },
                )?;
                catalog.stale = false;
                Ok(catalog)
            }
            // 3. Stale fallback.
            Err(net_err) => {
                if cache_path.exists() {
                    tracing::warn!(
                        "voice catalog refresh failed ({}); falling back to stale cache at {}",
                        net_err.to_user_message(),
                        cache_path.display(),
                    );
                    let bytes = std::fs::read(&cache_path).map_err(|e| {
                        PiperUnavailable::DownloadFailed(format!(
                            "read stale catalog cache {}: {e}",
                            cache_path.display(),
                        ))
                    })?;
                    let mut catalog = parse_voice_catalog(&bytes)?;
                    catalog.stale = true;
                    Ok(catalog)
                } else {
                    // 4. No cache, no network — surface
                    // the original error.
                    Err(net_err)
                }
            }
        }
    }

    /// Look up a voice by canonical key first
    /// (`en_US-lessac-medium`); falls back to alias
    /// matching.  `needle` is matched case-sensitively
    /// — Piper's voice keys are stable identifiers, not
    /// human-friendly names.
    pub fn voice(&self, needle: &str) -> Option<&VoiceMeta> {
        if let Some(v) = self.voices.get(needle) {
            return Some(v);
        }
        self.voices
            .values()
            .find(|v| v.aliases.iter().any(|a| a == needle))
    }

    /// All distinct language codes in the catalog,
    /// sorted lexicographically.  Used by the voice
    /// picker's language filter.
    pub fn languages(&self) -> Vec<String> {
        let mut langs: Vec<String> = self
            .voices
            .values()
            .map(|v| v.language_code.clone())
            .collect();
        langs.sort();
        langs.dedup();
        langs
    }

    /// All voices whose `language_code` exactly matches
    /// `code`.  Order: by quality tier descending then
    /// key ascending, so the highest-quality voice for
    /// a language lands first in the picker.
    pub fn voices_for_language(&self, code: &str) -> Vec<&VoiceMeta> {
        let mut voices: Vec<&VoiceMeta> = self
            .voices
            .values()
            .filter(|v| v.language_code == code)
            .collect();
        voices.sort_by(|a, b| {
            quality_rank(&b.quality)
                .cmp(&quality_rank(&a.quality))
                .then(a.key.cmp(&b.key))
        });
        voices
    }

    /// Total voice count, for the diagnostic chip.
    pub fn len(&self) -> usize {
        self.voices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.voices.is_empty()
    }
}

/// Numeric quality rank (higher = better).  Unknown
/// quality strings sort last (rank 0).
fn quality_rank(q: &str) -> u8 {
    match q {
        "high" => 4,
        "medium" => 3,
        "low" => 2,
        "x_low" => 1,
        _ => 0,
    }
}

/// True when `path` exists and its mtime is within
/// `ttl` of `now`.  Errors (no metadata, no mtime,
/// negative duration) all classify as "not fresh" so
/// the caller refetches.
fn is_cache_fresh(path: &Path, ttl: Duration, now: SystemTime) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(mtime) = meta.modified() else {
        return false;
    };
    match now.duration_since(mtime) {
        Ok(age) => age < ttl,
        // mtime is in the future (clock skew); treat
        // as fresh.
        Err(_) => true,
    }
}

/// Parse `voices.json` bytes into a `Catalog`.  Pure —
/// no I/O.  Tolerant of unknown / missing optional
/// fields; voices missing `language.code` are silently
/// dropped (we can't classify them).
pub(crate) fn parse_voice_catalog(bytes: &[u8]) -> Result<Catalog, PiperUnavailable> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| {
            PiperUnavailable::DownloadFailed(format!(
                "parse voices.json: {e}",
            ))
        })?;
    let obj = value.as_object().ok_or_else(|| {
        PiperUnavailable::DownloadFailed(
            "voices.json root is not an object".to_string(),
        )
    })?;
    let mut voices = BTreeMap::new();
    for (key, voice_value) in obj {
        if let Some(meta) = parse_voice_meta(key, voice_value) {
            voices.insert(key.clone(), meta);
        }
    }
    Ok(Catalog {
        voices,
        stale: false,
    })
}

fn parse_voice_meta(key: &str, value: &serde_json::Value) -> Option<VoiceMeta> {
    let obj = value.as_object()?;
    let lang = obj.get("language")?.as_object()?;
    let language_code = lang.get("code")?.as_str()?.to_string();
    let language_family = lang
        .get("family")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let language_native = lang
        .get("name_native")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let language_english = lang
        .get("name_english")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let quality = obj
        .get("quality")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let num_speakers =
        obj.get("num_speakers").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let aliases: Vec<String> = obj
        .get("aliases")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let files = obj
        .get("files")
        .and_then(|v| v.as_object())
        .map(|files_obj| {
            let mut map = BTreeMap::new();
            for (path, file_value) in files_obj {
                let Some(f_obj) = file_value.as_object() else {
                    continue;
                };
                let size_bytes = f_obj
                    .get("size_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let md5_digest = f_obj
                    .get("md5_digest")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                map.insert(
                    path.clone(),
                    CatalogFile {
                        relative_path: path.clone(),
                        size_bytes,
                        md5_digest,
                    },
                );
            }
            map
        })
        .unwrap_or_default();
    Some(VoiceMeta {
        key: key.to_string(),
        name,
        language_code,
        language_family,
        language_native,
        language_english,
        quality,
        num_speakers,
        aliases,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const FIXTURE: &str = r#"
    {
      "en_US-lessac-medium": {
        "key": "en_US-lessac-medium",
        "name": "lessac",
        "language": {
          "code": "en_US",
          "family": "en",
          "region": "US",
          "name_native": "English",
          "name_english": "English",
          "country_english": "United States"
        },
        "quality": "medium",
        "num_speakers": 1,
        "speaker_id_map": {},
        "files": {
          "en/en_US/lessac/medium/en_US-lessac-medium.onnx": {
            "size_bytes": 63201294,
            "md5_digest": "deadbeef0001"
          },
          "en/en_US/lessac/medium/en_US-lessac-medium.onnx.json": {
            "size_bytes": 4995,
            "md5_digest": "deadbeef0002"
          },
          "en/en_US/lessac/medium/MODEL_CARD": {
            "size_bytes": 1024,
            "md5_digest": "deadbeef0003"
          }
        },
        "aliases": ["lessac"]
      },
      "en_US-ryan-high": {
        "key": "en_US-ryan-high",
        "name": "ryan",
        "language": {
          "code": "en_US",
          "family": "en",
          "name_native": "English",
          "name_english": "English"
        },
        "quality": "high",
        "num_speakers": 1,
        "files": {
          "en/en_US/ryan/high/en_US-ryan-high.onnx": {
            "size_bytes": 109001000,
            "md5_digest": "cafe0001"
          },
          "en/en_US/ryan/high/en_US-ryan-high.onnx.json": {
            "size_bytes": 5100,
            "md5_digest": "cafe0002"
          }
        },
        "aliases": []
      },
      "ru_RU-irina-medium": {
        "key": "ru_RU-irina-medium",
        "name": "irina",
        "language": {
          "code": "ru_RU",
          "family": "ru",
          "name_native": "Русский",
          "name_english": "Russian"
        },
        "quality": "medium",
        "num_speakers": 1,
        "files": {
          "ru/ru_RU/irina/medium/ru_RU-irina-medium.onnx": {
            "size_bytes": 63100000,
            "md5_digest": "beef0001"
          },
          "ru/ru_RU/irina/medium/ru_RU-irina-medium.onnx.json": {
            "size_bytes": 4800,
            "md5_digest": "beef0002"
          }
        },
        "aliases": []
      },
      "broken-no-language": {
        "key": "broken",
        "name": "broken",
        "quality": "low"
      }
    }
    "#;

    // ── parse_voice_catalog ───────────────────────────

    #[test]
    fn parse_extracts_all_well_formed_voices() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        // 3 well-formed + 1 broken (dropped).
        assert_eq!(cat.voices.len(), 3);
        assert!(cat.voices.contains_key("en_US-lessac-medium"));
        assert!(cat.voices.contains_key("en_US-ryan-high"));
        assert!(cat.voices.contains_key("ru_RU-irina-medium"));
        assert!(!cat.voices.contains_key("broken-no-language"));
        assert!(!cat.stale);
    }

    #[test]
    fn parse_voice_metadata_round_trips() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let v = cat.voices.get("en_US-lessac-medium").unwrap();
        assert_eq!(v.key, "en_US-lessac-medium");
        assert_eq!(v.name, "lessac");
        assert_eq!(v.language_code, "en_US");
        assert_eq!(v.language_family, "en");
        assert_eq!(v.language_english, "English");
        assert_eq!(v.quality, "medium");
        assert_eq!(v.num_speakers, 1);
        assert_eq!(v.aliases, vec!["lessac".to_string()]);
        assert_eq!(v.files.len(), 3);
    }

    #[test]
    fn parse_handles_missing_optional_fields() {
        // ryan has no `aliases`; that field defaults to
        // empty rather than failing the voice.
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let v = cat.voices.get("en_US-ryan-high").unwrap();
        assert!(v.aliases.is_empty());
    }

    #[test]
    fn parse_rejects_bad_json() {
        let err = parse_voice_catalog(b"not json").unwrap_err();
        assert!(matches!(err, PiperUnavailable::DownloadFailed(_)));
    }

    #[test]
    fn parse_rejects_non_object_root() {
        let err = parse_voice_catalog(b"[1, 2, 3]").unwrap_err();
        assert!(matches!(err, PiperUnavailable::DownloadFailed(_)));
        assert!(err.to_user_message().contains("object"));
    }

    #[test]
    fn parse_empty_object_yields_empty_catalog() {
        let cat = parse_voice_catalog(b"{}").unwrap();
        assert!(cat.voices.is_empty());
        assert!(cat.is_empty());
    }

    // ── VoiceMeta URL composition ─────────────────────

    #[test]
    fn onnx_url_joins_base_and_relative_path() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let v = cat.voices.get("en_US-lessac-medium").unwrap();
        let url = v.onnx_url().unwrap();
        assert_eq!(
            url,
            "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium/en_US-lessac-medium.onnx",
        );
    }

    #[test]
    fn onnx_json_url_distinguishes_from_onnx() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let v = cat.voices.get("en_US-lessac-medium").unwrap();
        let url = v.onnx_json_url().unwrap();
        assert!(url.ends_with(".onnx.json"));
        assert_ne!(v.onnx_url().unwrap(), url);
    }

    #[test]
    fn synthesis_files_returns_two_files() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let v = cat.voices.get("en_US-lessac-medium").unwrap();
        let files = v.synthesis_files();
        assert_eq!(files.len(), 2);
        // First is .onnx, second is .onnx.json (per
        // synthesis_files contract).
        assert!(files[0].relative_path.ends_with(".onnx"));
        assert!(!files[0].relative_path.ends_with(".onnx.json"));
        assert!(files[1].relative_path.ends_with(".onnx.json"));
    }

    #[test]
    fn synthesis_size_sums_both_files() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let v = cat.voices.get("en_US-lessac-medium").unwrap();
        // 63201294 + 4995 = 63206289
        assert_eq!(v.synthesis_size_bytes(), 63_206_289);
    }

    // ── Catalog lookups ───────────────────────────────

    #[test]
    fn voice_lookup_by_canonical_key() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        assert!(cat.voice("en_US-lessac-medium").is_some());
        assert!(cat.voice("does-not-exist").is_none());
    }

    #[test]
    fn voice_lookup_by_alias() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let v = cat.voice("lessac").unwrap();
        assert_eq!(v.key, "en_US-lessac-medium");
    }

    #[test]
    fn languages_dedupes_and_sorts() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let langs = cat.languages();
        // Two voices share en_US; one is ru_RU.  After
        // dedup: ["en_US", "ru_RU"].
        assert_eq!(langs, vec!["en_US".to_string(), "ru_RU".to_string()]);
    }

    #[test]
    fn voices_for_language_filters_and_sorts_by_quality() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let en = cat.voices_for_language("en_US");
        // Both English voices; high ranks above medium.
        assert_eq!(en.len(), 2);
        assert_eq!(en[0].key, "en_US-ryan-high"); // high first
        assert_eq!(en[1].key, "en_US-lessac-medium");
        let ru = cat.voices_for_language("ru_RU");
        assert_eq!(ru.len(), 1);
    }

    #[test]
    fn voices_for_unknown_language_is_empty() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        assert!(cat.voices_for_language("xx_YY").is_empty());
    }

    #[test]
    fn quality_rank_orders_tiers() {
        assert!(quality_rank("high") > quality_rank("medium"));
        assert!(quality_rank("medium") > quality_rank("low"));
        assert!(quality_rank("low") > quality_rank("x_low"));
        assert_eq!(quality_rank("unknown-tier"), 0);
    }

    // ── Catalog::load (fresh / refresh / stale / fail) ─

    fn dir_with_cache(bytes: &[u8]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(CATALOG_FILENAME), bytes).unwrap();
        tmp
    }

    #[test]
    fn load_uses_fresh_cache_without_network() {
        let tmp = dir_with_cache(FIXTURE.as_bytes());
        let call_count = AtomicUsize::new(0);
        let fetch = |_url: &str| -> Result<Vec<u8>, PiperUnavailable> {
            call_count.fetch_add(1, Ordering::Relaxed);
            panic!("network must not be hit for fresh cache");
        };
        let cat = Catalog::load_with_clock(
            tmp.path(),
            "https://example.test/voices.json",
            Duration::from_secs(3600),
            SystemTime::now(),
            fetch,
        )
        .unwrap();
        assert!(!cat.stale);
        assert_eq!(cat.voices.len(), 3);
        assert_eq!(call_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn load_fetches_when_no_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let fetch =
            |_url: &str| -> Result<Vec<u8>, PiperUnavailable> { Ok(FIXTURE.as_bytes().to_vec()) };
        let cat = Catalog::load_with_clock(
            tmp.path(),
            "https://example.test/voices.json",
            Duration::from_secs(3600),
            SystemTime::now(),
            fetch,
        )
        .unwrap();
        assert!(!cat.stale);
        // Cache should now exist on disk.
        assert!(tmp.path().join(CATALOG_FILENAME).exists());
    }

    #[test]
    fn load_fetches_when_cache_expired() {
        let tmp = dir_with_cache(b"{\"corrupt\": true}");
        // ttl = 1s, but "now" is 1 hour after the
        // file's mtime → expired.
        let in_the_future =
            SystemTime::now() + Duration::from_secs(3600);
        let fetched = AtomicUsize::new(0);
        let fetch = |_url: &str| -> Result<Vec<u8>, PiperUnavailable> {
            fetched.fetch_add(1, Ordering::Relaxed);
            Ok(FIXTURE.as_bytes().to_vec())
        };
        let cat = Catalog::load_with_clock(
            tmp.path(),
            "https://example.test/voices.json",
            Duration::from_secs(1),
            in_the_future,
            fetch,
        )
        .unwrap();
        assert_eq!(fetched.load(Ordering::Relaxed), 1);
        assert_eq!(cat.voices.len(), 3);
        assert!(!cat.stale);
    }

    #[test]
    fn load_falls_back_to_stale_cache_on_network_failure() {
        let tmp = dir_with_cache(FIXTURE.as_bytes());
        let in_the_future =
            SystemTime::now() + Duration::from_secs(3600);
        let fetch = |_url: &str| -> Result<Vec<u8>, PiperUnavailable> {
            Err(PiperUnavailable::DownloadFailed("curl 7".into()))
        };
        let cat = Catalog::load_with_clock(
            tmp.path(),
            "https://example.test/voices.json",
            Duration::from_secs(1),
            in_the_future,
            fetch,
        )
        .unwrap();
        assert!(cat.stale);
        assert_eq!(cat.voices.len(), 3);
    }

    #[test]
    fn load_surfaces_error_when_no_cache_and_no_network() {
        let tmp = tempfile::tempdir().unwrap();
        let fetch = |_url: &str| -> Result<Vec<u8>, PiperUnavailable> {
            Err(PiperUnavailable::DownloadFailed("curl 7".into()))
        };
        let err = Catalog::load_with_clock(
            tmp.path(),
            "https://example.test/voices.json",
            Duration::from_secs(3600),
            SystemTime::now(),
            fetch,
        )
        .unwrap_err();
        assert!(matches!(err, PiperUnavailable::DownloadFailed(_)));
    }

    #[test]
    fn load_rejects_corrupted_fetch_response() {
        let tmp = tempfile::tempdir().unwrap();
        let fetch = |_url: &str| -> Result<Vec<u8>, PiperUnavailable> {
            Ok(b"not json".to_vec())
        };
        let err = Catalog::load_with_clock(
            tmp.path(),
            "https://example.test/voices.json",
            Duration::from_secs(3600),
            SystemTime::now(),
            fetch,
        )
        .unwrap_err();
        assert!(matches!(err, PiperUnavailable::DownloadFailed(_)));
        // Cache must not be written with garbage.
        assert!(!tmp.path().join(CATALOG_FILENAME).exists());
    }

    #[test]
    fn load_writes_cache_atomically_on_refresh() {
        let tmp = tempfile::tempdir().unwrap();
        let fetch =
            |_url: &str| -> Result<Vec<u8>, PiperUnavailable> { Ok(FIXTURE.as_bytes().to_vec()) };
        Catalog::load_with_clock(
            tmp.path(),
            "https://example.test/voices.json",
            Duration::from_secs(3600),
            SystemTime::now(),
            fetch,
        )
        .unwrap();
        let cache = tmp.path().join(CATALOG_FILENAME);
        let bytes = std::fs::read(&cache).unwrap();
        // Atomic write should produce the same bytes
        // we passed in.
        assert_eq!(bytes, FIXTURE.as_bytes());
        // No leftover temp files in the parent.
        let tmp_count = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".voices.json")
            })
            .count();
        assert_eq!(tmp_count, 0, "no leftover atomic-temp files");
    }

    #[test]
    fn is_cache_fresh_handles_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_cache_fresh(
            &tmp.path().join("nope.json"),
            Duration::from_secs(3600),
            SystemTime::now(),
        ));
    }

    #[test]
    fn is_cache_fresh_returns_true_within_ttl() {
        let tmp = dir_with_cache(b"x");
        assert!(is_cache_fresh(
            &tmp.path().join(CATALOG_FILENAME),
            Duration::from_secs(3600),
            SystemTime::now(),
        ));
    }

    #[test]
    fn is_cache_fresh_returns_false_past_ttl() {
        let tmp = dir_with_cache(b"x");
        let in_the_future =
            SystemTime::now() + Duration::from_secs(3600);
        assert!(!is_cache_fresh(
            &tmp.path().join(CATALOG_FILENAME),
            Duration::from_secs(1),
            in_the_future,
        ));
    }

    #[test]
    fn catalog_struct_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Catalog>();
        assert_send_sync::<VoiceMeta>();
        assert_send_sync::<CatalogFile>();
    }
}
