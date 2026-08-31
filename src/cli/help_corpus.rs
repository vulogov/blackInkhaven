//! Help-corpus auto-build.
//!
//! The F1 Help feature is a RAG Q&A grounded in inkhaven's own documentation
//! (the "Help" system book). That book seeds **empty**, and populating it used
//! to require `import-help --documents-directory <PATH>` pointed at a docs
//! folder — which a `cargo install` user doesn't have (the source is compiled
//! and discarded). So F1 help was effectively empty for installed users.
//!
//! This module closes that gap by fetching a packaged documentation corpus
//! from the project's GitHub release at runtime and indexing it into the Help
//! book (reusing the [`super::import_help`] pipeline). The corpus is fetched,
//! **not** bundled — inkhaven already requires the network for AI inference, so
//! this adds no new class of dependency, and it keeps the crate/binary lean
//! (avoiding `cargo publish` size pressure).
//!
//! - `inkhaven rebuild-help` — download (cached) + index into the Help book.
//! - `inkhaven package-help-corpus` — MAINTAINER: package a docs directory into
//!   the `help-corpus.json` artifact uploaded to the GitHub release.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// The packaged documentation corpus, committed at the repo root and served
/// raw from GitHub (regenerated on every release cut so it tracks the docs).
/// It is excluded from the crate tarball (see Cargo.toml) to keep the published
/// crate lean — a `cargo install` user pulls it here on first `rebuild-help`.
/// Overridable per-invocation with `rebuild-help --url` (also accepts a local
/// path).
pub const DEFAULT_CORPUS_URL: &str =
    "https://raw.githubusercontent.com/vulogov/blackInkhaven/main/help-corpus.json";

const CACHE_FILENAME: &str = "help-corpus.json";

/// The packaged documentation corpus: a flat list of markdown files with their
/// paths relative to the docs root, so it can be unpacked back into a directory
/// tree and fed to the existing directory importer.
#[derive(Debug, Serialize, Deserialize)]
pub struct HelpCorpus {
    /// The inkhaven version the corpus was cut from (informational).
    #[serde(default)]
    pub version: String,
    pub files: Vec<HelpFile>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HelpFile {
    /// Path relative to the docs root, e.g. `Tutorials/01-getting-started.md`.
    pub path: String,
    pub content: String,
}

/// Shared on-disk cache for the downloaded corpus (across projects).
fn cache_path() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("dev", "inkhaven", "inkhaven")?;
    Some(dirs.cache_dir().join(CACHE_FILENAME))
}

/// True when a doc (path relative to the docs root) belongs in the F1 help
/// corpus. The corpus is USER how-to + reference, so this skips the meta /
/// internal material that would otherwise dominate retrieval:
///   - `RELEASE_NOTES/` — per-version changelogs (was ~43% of every file; the
///     reason F1 kept surfacing "what changed in 3.6" instead of how-tos),
///   - `PROPOSALS/` — internal RFCs / design docs / bugfix plans,
///   - top-level index / plan / maintenance files.
/// Everything else (Tutorials, KEYBINDING, CONFIGURATION, the feature guides,
/// Bund) is kept.
fn is_help_relevant(rel: &str) -> bool {
    let rel = rel.replace('\\', "/");
    let top = rel.split('/').next().unwrap_or("");
    if matches!(top, "RELEASE_NOTES" | "PROPOSALS") {
        return false;
    }
    let name = rel.rsplit('/').next().unwrap_or("").to_ascii_uppercase();
    // Indexes + internal planning/maintenance docs, wherever they sit.
    !(name == "README.MD"
        || name == "MAINTENANCE.MD"
        || name == "KEYS_REASSIGNMENT.MD"
        || name.starts_with("BUGFIX_PLAN")
        || name.ends_with("_PLAN.MD")
        || name.contains("READINESS"))
}

/// MAINTAINER: walk `docs_dir` and package every help-relevant `.md` file (path
/// relative to `docs_dir` + content) into a [`HelpCorpus`]. Meta / changelog /
/// internal docs are filtered out (see [`is_help_relevant`]).
pub fn package_from_dir(docs_dir: &Path, version: &str) -> Result<HelpCorpus> {
    if !docs_dir.is_dir() {
        return Err(Error::Store(format!(
            "`{}` is not a directory",
            docs_dir.display()
        )));
    }
    let mut files = Vec::new();
    let mut skipped = 0usize;
    for entry in walkdir::WalkDir::new(docs_dir)
        .sort_by_file_name()
        .follow_links(false)
    {
        let entry = entry.map_err(|e| Error::Store(format!("walk docs: {e}")))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let rel = path.strip_prefix(docs_dir).unwrap_or(path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if !is_help_relevant(&rel_str) {
            skipped += 1;
            continue;
        }
        let content = std::fs::read_to_string(path).map_err(Error::Io)?;
        files.push(HelpFile {
            path: rel_str,
            content,
        });
    }
    eprintln!(
        "included {} help doc(s); skipped {} meta/changelog doc(s)",
        files.len(),
        skipped
    );
    Ok(HelpCorpus {
        version: version.to_string(),
        files,
    })
}

/// Read the corpus bytes from `source`, which may be an `http(s)://` URL (fetched
/// with the shared reqwest helper), a `file://` URL, or a bare local filesystem
/// path — so `rebuild-help --url ./help-corpus.json` works offline and for a
/// user who packaged their own docs.
fn read_source(source: &str) -> std::result::Result<Vec<u8>, String> {
    if source.starts_with("http://") || source.starts_with("https://") {
        eprintln!("downloading help corpus from {source} …");
        let bytes = crate::typst_universe::reqwest_fetch(source)?;
        eprintln!("  downloaded {} KiB", bytes.len() / 1024);
        Ok(bytes)
    } else {
        let path = source.strip_prefix("file://").unwrap_or(source);
        eprintln!("reading help corpus from {path}");
        std::fs::read(path).map_err(|e| format!("read `{path}`: {e}"))
    }
}

/// Fetch the corpus: a readable cache short-circuits the network (unless
/// `force`); otherwise read it from `url` (remote or local), parse, and cache
/// atomically; on a fetch error fall back to a stale cache when one exists.
pub fn fetch(url: &str, force: bool) -> Result<HelpCorpus> {
    let cache = cache_path();
    if !force {
        if let Some(cp) = &cache {
            if let Ok(bytes) = std::fs::read(cp) {
                if let Ok(corpus) = serde_json::from_slice::<HelpCorpus>(&bytes) {
                    eprintln!("using cached help corpus ({})", cp.display());
                    return Ok(corpus);
                }
            }
        }
    }
    match read_source(url) {
        Ok(bytes) => {
            // Parse before writing so a corrupt download never poisons the cache.
            let corpus: HelpCorpus = serde_json::from_slice(&bytes)
                .map_err(|e| Error::Store(format!("parse help corpus: {e}")))?;
            if let Some(cp) = &cache {
                if let Some(parent) = cp.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if crate::io_atomic::write(cp, &bytes).is_ok() {
                    eprintln!("  cached to {} (offline next time)", cp.display());
                }
            }
            Ok(corpus)
        }
        Err(net) => {
            if let Some(cp) = &cache {
                if let Ok(bytes) = std::fs::read(cp) {
                    if let Ok(corpus) = serde_json::from_slice::<HelpCorpus>(&bytes) {
                        return Ok(corpus);
                    }
                }
            }
            Err(Error::Store(format!(
                "fetch help corpus: {net} — and no cached copy is available (offline? or the corpus isn't published yet — try `--url ./help-corpus.json` after `package-help-corpus`)"
            )))
        }
    }
}

/// Write the corpus back out to a directory tree, preserving relative paths.
/// Path-traversal segments (`..`, absolute paths) are skipped defensively.
fn unpack_to_dir(corpus: &HelpCorpus, dir: &Path) -> Result<()> {
    for f in &corpus.files {
        let rel = Path::new(&f.path);
        if rel.is_absolute()
            || rel
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            continue;
        }
        let dest = dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        std::fs::write(&dest, f.content.as_bytes()).map_err(Error::Io)?;
    }
    Ok(())
}

/// `inkhaven rebuild-help`: fetch the corpus (cached or from GitHub), unpack it
/// to a scratch directory, and import + index it into the project's Help book
/// via the existing directory importer. Eliminates the manual `import-help`
/// step and the need for a local docs folder.
pub fn rebuild(project: &Path, url: &str, force: bool) -> Result<()> {
    let corpus = fetch(url, force)?;
    let n = corpus.files.len();
    let ver = if corpus.version.is_empty() {
        String::new()
    } else {
        format!(" (built from inkhaven {})", corpus.version)
    };
    eprintln!("loaded {n} document(s){ver}");
    // Transient scratch — the importer copies content into the project store,
    // so this directory is discarded afterwards.
    let scratch =
        std::env::temp_dir().join(format!("inkhaven-help-corpus-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(Error::Io)?;
    eprintln!("unpacking to {} …", scratch.display());
    unpack_to_dir(&corpus, &scratch)?;
    let res = super::import_help::run(project, &scratch);
    let _ = std::fs::remove_dir_all(&scratch);
    res?;
    eprintln!("✓ help corpus rebuilt — {n} document(s) indexed into the Help book. F1 is ready.");
    Ok(())
}

/// MAINTAINER: `inkhaven package-help-corpus` — write a corpus artifact from a
/// local docs directory for upload to the GitHub release.
pub fn package(docs_dir: &Path, out: &Path, version: &str) -> Result<()> {
    let corpus = package_from_dir(docs_dir, version)?;
    let json = serde_json::to_vec_pretty(&corpus)
        .map_err(|e| Error::Store(format!("serialize help corpus: {e}")))?;
    std::fs::write(out, &json).map_err(Error::Io)?;
    eprintln!(
        "packaged {} document(s) into {}",
        corpus.files.len(),
        out.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_and_unpack_round_trip() {
        let dir = std::env::temp_dir().join(format!("inkhaven-help-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Tutorials")).unwrap();
        std::fs::write(dir.join("intro.md"), b"# Intro\nhello").unwrap();
        std::fs::write(dir.join("Tutorials/01.md"), b"tutorial one").unwrap();
        std::fs::write(dir.join("ignore.txt"), b"not markdown").unwrap();

        let corpus = package_from_dir(&dir, "9.9.9").unwrap();
        // Only the two .md files, txt skipped.
        assert_eq!(corpus.files.len(), 2);
        assert_eq!(corpus.version, "9.9.9");
        assert!(corpus.files.iter().any(|f| f.path == "intro.md"));
        assert!(corpus.files.iter().any(|f| f.path == "Tutorials/01.md"));

        let out = dir.join("unpacked");
        std::fs::create_dir_all(&out).unwrap();
        unpack_to_dir(&corpus, &out).unwrap();
        assert_eq!(
            std::fs::read_to_string(out.join("Tutorials/01.md")).unwrap(),
            "tutorial one"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn help_relevance_filters_meta_and_changelogs() {
        // Kept: user how-to + reference.
        assert!(is_help_relevant("Tutorials/03-the-editor.md"));
        assert!(is_help_relevant("KEYBINDING.md"));
        assert!(is_help_relevant("CONFIGURATION.md"));
        assert!(is_help_relevant("Bund/BUND_TUTORIAL.md"));
        // Skipped: changelogs, internal RFCs/plans, indexes, maintenance.
        assert!(!is_help_relevant("RELEASE_NOTES/3.6.0.md"));
        assert!(!is_help_relevant("RELEASE_NOTES/README.md"));
        assert!(!is_help_relevant("PROPOSALS/SEMNET-1_PLAN.md"));
        assert!(!is_help_relevant("README.md"));
        assert!(!is_help_relevant("MAINTENANCE.md"));
        assert!(!is_help_relevant("KEYS_REASSIGNMENT.md"));
        assert!(!is_help_relevant("BUGFIX_PLAN_1.5.9.md"));
        assert!(!is_help_relevant("2.0_READINESS.md"));
    }

    #[test]
    fn unpack_skips_path_traversal() {
        let dir =
            std::env::temp_dir().join(format!("inkhaven-help-trav-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let corpus = HelpCorpus {
            version: String::new(),
            files: vec![
                HelpFile {
                    path: "../escape.md".into(),
                    content: "nope".into(),
                },
                HelpFile {
                    path: "ok.md".into(),
                    content: "yes".into(),
                },
            ],
        };
        unpack_to_dir(&corpus, &dir).unwrap();
        assert!(dir.join("ok.md").exists());
        assert!(!dir.parent().unwrap().join("escape.md").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
