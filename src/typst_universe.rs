//! TYPST-UNIVERSE (1.6.15+) — a cached index of Typst Universe packages, for
//! the `Ctrl+V #` import picker.
//!
//! Papers lean on community packages (`cetz`, `fletcher`, `unify`, journal
//! templates…). Rather than make the author remember `@preview/<name>:<ver>`
//! spellings, we fetch a package manifest (default: the community
//! "typst-universe-with-stars" list), cache it under `.inkhaven/`, and let a
//! fuzzy modal insert the `#import` line.
//!
//! Fetch policy mirrors the Piper voice-catalog cache: a TTL-checked on-disk
//! JSON, parsed-before-written (so a corrupt upstream never poisons the cache),
//! with a stale-copy fallback when the network is down. The HTTP fetch uses
//! `reqwest` (a crate — no external-binary dependency) on a dedicated thread
//! with its own runtime, so it is safe to call from the synchronous TUI without
//! risking a nested-runtime panic.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::Deserialize;

/// The default community manifest (package list with GitHub star counts).
pub const DEFAULT_URL: &str =
    "https://orangex4.github.io/typst-universe-with-stars/packages.json";

const CACHE_FILENAME: &str = "typst-universe.json";

/// One Typst Universe package (the fields we surface). Unknown JSON fields are
/// ignored; missing ones default, so the parser tolerates manifest drift.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct UniversePackage {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, deserialize_with = "de_lenient_u64")]
    pub stars: u64,
}

impl UniversePackage {
    /// The `@preview/<name>:<version>` spec used in a Typst import. The picker
    /// stores this as the row's payload; the modal wraps it in a `#import`
    /// line on insert.
    pub fn spec(&self) -> String {
        if self.version.trim().is_empty() {
            format!("@preview/{}", self.name.trim())
        } else {
            format!("@preview/{}:{}", self.name.trim(), self.version.trim())
        }
    }
}

/// Accept a star count as a JSON number, string, or null → `u64` (0 on miss).
fn de_lenient_u64<'de, D>(d: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_f64().map(|f| f.max(0.0) as u64))
            .unwrap_or(0),
        serde_json::Value::String(s) => s.trim().parse::<u64>().unwrap_or(0),
        _ => 0,
    })
}

/// Parse the manifest bytes, tolerating the three plausible top-level shapes: a
/// bare array, a `{ "packages": [...] }` wrapper, or a `{ name: {...} }` map.
pub fn parse_packages(bytes: &[u8]) -> Result<Vec<UniversePackage>, String> {
    if let Ok(v) = serde_json::from_slice::<Vec<UniversePackage>>(bytes) {
        return Ok(v);
    }

    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(default)]
        packages: Vec<UniversePackage>,
    }
    if let Ok(w) = serde_json::from_slice::<Wrapper>(bytes) {
        // A map-shaped manifest also parses as `Wrapper` (serde ignores the
        // unknown keys), yielding an empty list — so only accept a non-empty
        // `packages` here and let the map branch handle the rest.
        if !w.packages.is_empty() {
            return Ok(w.packages);
        }
    }

    #[derive(Deserialize)]
    struct Body {
        #[serde(default)]
        version: String,
        #[serde(default)]
        description: String,
        #[serde(default, deserialize_with = "de_lenient_u64")]
        stars: u64,
    }
    match serde_json::from_slice::<std::collections::BTreeMap<String, Body>>(bytes) {
        Ok(m) => Ok(m
            .into_iter()
            .map(|(name, b)| UniversePackage {
                name,
                version: b.version,
                description: b.description,
                stars: b.stars,
            })
            .collect()),
        Err(e) => Err(format!("unrecognized packages.json shape: {e}")),
    }
}

/// A semver-ish sort key: numeric components split on `.`, non-numeric parts
/// contribute 0. Enough to pick the newest version of a package.
fn version_key(v: &str) -> Vec<u64> {
    v.split('.')
        .map(|part| {
            part.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(0)
        })
        .collect()
}

/// Collapse multiple version rows per package to the highest version, drop
/// nameless rows, and sort by stars (desc) then name, so the picker's default
/// order surfaces the most-used packages first.
pub fn latest_per_package(pkgs: Vec<UniversePackage>) -> Vec<UniversePackage> {
    use std::collections::HashMap;
    let mut best: HashMap<String, UniversePackage> = HashMap::new();
    for p in pkgs {
        let name = p.name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        match best.get(&name) {
            Some(existing) if version_key(&existing.version) >= version_key(&p.version) => {}
            _ => {
                best.insert(name, p);
            }
        }
    }
    let mut out: Vec<UniversePackage> = best.into_values().collect();
    out.sort_by(|a, b| {
        b.stars
            .cmp(&a.stars)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

/// Whether the cache file exists and is younger than `ttl`.
fn is_cache_fresh(path: &Path, ttl: Duration, now: SystemTime) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(mtime) = meta.modified() else {
        return false;
    };
    now.duration_since(mtime)
        .map(|age| age < ttl)
        .unwrap_or(false)
}

/// Load the package list: fresh cache → refetch → stale fallback. `fetch` is
/// injected so tests can drive it without a network. `cache_dir` is the
/// project's `.inkhaven/` directory.
pub fn load(
    cache_dir: &Path,
    url: &str,
    ttl: Duration,
    fetch: impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<UniversePackage>, String> {
    load_with_clock(cache_dir, url, ttl, SystemTime::now(), false, fetch)
}

/// Like [`load`] but ignores cache freshness — always attempts a refetch,
/// rewriting the cache on success and falling back to the stale copy on a
/// network error. Backs the modal's `Ctrl+R` force-refresh.
pub fn load_forced(
    cache_dir: &Path,
    url: &str,
    fetch: impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<UniversePackage>, String> {
    load_with_clock(cache_dir, url, Duration::from_secs(0), SystemTime::now(), true, fetch)
}

/// `load` with an injected clock for deterministic TTL tests. `force` skips the
/// fresh-cache shortcut and goes straight to a refetch.
pub fn load_with_clock(
    cache_dir: &Path,
    url: &str,
    ttl: Duration,
    now: SystemTime,
    force: bool,
    fetch: impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<UniversePackage>, String> {
    let cache_path = cache_dir.join(CACHE_FILENAME);

    // 1. Fresh cache — skip the network (unless a refresh was forced).
    if !force && is_cache_fresh(&cache_path, ttl, now) {
        if let Ok(bytes) = std::fs::read(&cache_path) {
            if let Ok(pkgs) = parse_packages(&bytes) {
                return Ok(latest_per_package(pkgs));
            }
        }
        // Corrupt/unreadable fresh cache falls through to a refetch.
    }

    // 2. Refresh attempt.
    match fetch(url) {
        Ok(bytes) => {
            // Parse before writing so a corrupt upstream never poisons the cache.
            let pkgs = parse_packages(&bytes)?;
            let _ = std::fs::create_dir_all(cache_dir);
            let _ = crate::io_atomic::write(&cache_path, &bytes);
            Ok(latest_per_package(pkgs))
        }
        // 3. Stale fallback.
        Err(net_err) => {
            if cache_path.exists() {
                let bytes = std::fs::read(&cache_path)
                    .map_err(|e| format!("read stale cache: {e}"))?;
                let pkgs = parse_packages(&bytes)?;
                tracing::warn!("typst-universe refresh failed ({net_err}); using stale cache");
                Ok(latest_per_package(pkgs))
            } else {
                Err(format!("fetch typst universe: {net_err}"))
            }
        }
    }
}

/// Fetch a URL with `reqwest` on a dedicated thread + runtime. Safe to call
/// from the synchronous TUI (no nested-runtime panic).
pub fn reqwest_fetch(url: &str) -> Result<Vec<u8>, String> {
    let url = url.to_string();
    std::thread::scope(|s| {
        s.spawn(move || -> Result<Vec<u8>, String> {
            let rt = tokio::runtime::Runtime::new().map_err(|e| format!("tokio: {e}"))?;
            rt.block_on(async {
                let client = reqwest::Client::builder()
                    .user_agent("inkhaven")
                    .timeout(Duration::from_secs(30))
                    .build()
                    .map_err(|e| format!("http client: {e}"))?;
                let resp = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| format!("GET {url}: {e}"))?;
                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status()));
                }
                let bytes = resp.bytes().await.map_err(|e| format!("read body: {e}"))?;
                Ok(bytes.to_vec())
            })
        })
        .join()
        .map_err(|_| "fetch thread panicked".to_string())?
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARRAY_JSON: &[u8] = br#"[
        {"name":"cetz","version":"0.2.0","description":"draw","stars":900},
        {"name":"cetz","version":"0.3.0","description":"draw","stars":900},
        {"name":"fletcher","version":"0.5.1","description":"diagrams","stars":300},
        {"name":"","version":"1.0.0","description":"nameless"}
    ]"#;

    #[test]
    fn parses_array_and_keeps_latest_version() {
        let pkgs = latest_per_package(parse_packages(ARRAY_JSON).unwrap());
        // nameless dropped; cetz collapsed to newest.
        assert_eq!(pkgs.len(), 2);
        let cetz = pkgs.iter().find(|p| p.name == "cetz").unwrap();
        assert_eq!(cetz.version, "0.3.0");
        // Sorted by stars desc → cetz (900) before fletcher (300).
        assert_eq!(pkgs[0].name, "cetz");
    }

    #[test]
    fn spec_is_well_formed_with_and_without_version() {
        let p = UniversePackage {
            name: "cetz".into(),
            version: "0.3.0".into(),
            description: String::new(),
            stars: 0,
        };
        assert_eq!(p.spec(), "@preview/cetz:0.3.0");
        let no_ver = UniversePackage {
            name: "cetz".into(),
            version: String::new(),
            description: String::new(),
            stars: 0,
        };
        assert_eq!(no_ver.spec(), "@preview/cetz");
    }

    #[test]
    fn parses_wrapper_and_map_shapes() {
        let wrapper = br#"{"packages":[{"name":"unify","version":"0.6.0"}]}"#;
        assert_eq!(parse_packages(wrapper).unwrap()[0].name, "unify");
        let map = br#"{"unify":{"version":"0.6.0","stars":"42"}}"#;
        let m = parse_packages(map).unwrap();
        assert_eq!(m[0].name, "unify");
        assert_eq!(m[0].stars, 42); // string star count coerced
    }

    #[test]
    fn lenient_stars_accepts_number_string_and_null() {
        let json = br#"[
            {"name":"a","stars":5},
            {"name":"b","stars":"7"},
            {"name":"c","stars":null},
            {"name":"d"}
        ]"#;
        let pkgs = parse_packages(json).unwrap();
        let get = |n: &str| pkgs.iter().find(|p| p.name == n).unwrap().stars;
        assert_eq!(get("a"), 5);
        assert_eq!(get("b"), 7);
        assert_eq!(get("c"), 0);
        assert_eq!(get("d"), 0);
    }

    #[test]
    fn load_prefers_fresh_cache_over_fetch() {
        let dir = std::env::temp_dir().join(format!("ink-univ-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let cache = dir.join(CACHE_FILENAME);
        std::fs::write(&cache, ARRAY_JSON).unwrap();
        // Fresh (ttl huge) → fetch must NOT run.
        let pkgs = load_with_clock(&dir, "http://unused", Duration::from_secs(3600), SystemTime::now(), false, |_| {
            panic!("fetch should not be called for a fresh cache")
        })
        .unwrap();
        assert!(pkgs.iter().any(|p| p.name == "cetz"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_falls_back_to_stale_cache_on_fetch_error() {
        let dir = std::env::temp_dir().join(format!("ink-univ-stale-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join(CACHE_FILENAME), ARRAY_JSON).unwrap();
        // Expired cache (ttl 0) + failing fetch → stale fallback, not an error.
        let pkgs = load_with_clock(&dir, "http://unused", Duration::from_secs(0), SystemTime::now(), false, |_| {
            Err("network down".into())
        })
        .unwrap();
        assert!(pkgs.iter().any(|p| p.name == "fletcher"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn force_refetches_even_when_cache_is_fresh() {
        let dir = std::env::temp_dir().join(format!("ink-univ-force-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join(CACHE_FILENAME), ARRAY_JSON).unwrap();
        // Fresh cache (huge ttl) but force=true → fetch MUST run and its data wins.
        let fresh = br#"[{"name":"newpkg","version":"1.0.0"}]"#;
        let pkgs = load_with_clock(
            &dir,
            "http://unused",
            Duration::from_secs(3600),
            SystemTime::now(),
            true,
            |_| Ok(fresh.to_vec()),
        )
        .unwrap();
        assert!(pkgs.iter().any(|p| p.name == "newpkg"));
        assert!(!pkgs.iter().any(|p| p.name == "cetz"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
