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

/// Stable GitHub release asset holding the packaged documentation corpus.
/// Overridable per-invocation with `rebuild-help --url`.
pub const DEFAULT_CORPUS_URL: &str =
    "https://github.com/vulogov/blackInkhaven/releases/download/help-corpus/help-corpus.json";

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

/// MAINTAINER: walk `docs_dir` and package every `.md` file (path relative to
/// `docs_dir` + content) into a [`HelpCorpus`].
pub fn package_from_dir(docs_dir: &Path, version: &str) -> Result<HelpCorpus> {
    if !docs_dir.is_dir() {
        return Err(Error::Store(format!(
            "`{}` is not a directory",
            docs_dir.display()
        )));
    }
    let mut files = Vec::new();
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
        let content = std::fs::read_to_string(path).map_err(Error::Io)?;
        files.push(HelpFile {
            path: rel_str,
            content,
        });
    }
    Ok(HelpCorpus {
        version: version.to_string(),
        files,
    })
}

/// Fetch the corpus: a readable cache short-circuits the network (unless
/// `force`); otherwise download it, parse, and cache atomically; on a network
/// error fall back to a stale cache when one exists.
pub fn fetch(url: &str, force: bool) -> Result<HelpCorpus> {
    let cache = cache_path();
    if !force {
        if let Some(cp) = &cache {
            if let Ok(bytes) = std::fs::read(cp) {
                if let Ok(corpus) = serde_json::from_slice::<HelpCorpus>(&bytes) {
                    return Ok(corpus);
                }
            }
        }
    }
    match crate::typst_universe::reqwest_fetch(url) {
        Ok(bytes) => {
            // Parse before writing so a corrupt download never poisons the cache.
            let corpus: HelpCorpus = serde_json::from_slice(&bytes)
                .map_err(|e| Error::Store(format!("parse help corpus: {e}")))?;
            if let Some(cp) = &cache {
                if let Some(parent) = cp.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = crate::io_atomic::write(cp, &bytes);
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
                "download help corpus: {net} — and no cached copy is available (are you offline?)"
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
    // Transient scratch — the importer copies content into the project store,
    // so this directory is discarded afterwards.
    let scratch =
        std::env::temp_dir().join(format!("inkhaven-help-corpus-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(Error::Io)?;
    unpack_to_dir(&corpus, &scratch)?;
    let res = super::import_help::run(project, &scratch);
    let _ = std::fs::remove_dir_all(&scratch);
    res?;
    eprintln!("help corpus rebuilt from {n} document(s)");
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
