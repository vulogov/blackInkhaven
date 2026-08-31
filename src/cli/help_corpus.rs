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

/// A section heading turned into a filesystem-safe filename segment, so each
/// chunk imports as a paragraph *titled by its section* (not by the parent
/// file). Strips markdown formatting + path separators; caps the length.
fn section_slug(heading: &str) -> String {
    let cleaned: String = heading
        .chars()
        .map(|c| match c {
            '/' | '\\' => '-',
            '`' | '*' | '#' | '[' | ']' => ' ',
            c => c,
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let capped: String = cleaned.chars().take(80).collect();
    let capped = capped.trim().to_string();
    if capped.is_empty() {
        "section".to_string()
    } else {
        capped
    }
}

/// Append one chunk to `chunks`, prefixed with the document title + section
/// label for retrieval context. The chunk's path nests the section under a
/// directory named for the doc (`<doc>/<section>.md`) so it imports as a
/// distinctly-titled paragraph. Empty bodies are skipped.
fn push_chunk(
    chunks: &mut Vec<HelpFile>,
    doc_base: &str,
    doc_title: &str,
    heading: &Option<String>,
    body: &[&str],
) {
    let text = body.join("\n");
    if text.trim().is_empty() {
        return;
    }
    let (label, path) = match heading {
        Some(h) => (h.clone(), format!("{doc_base}/{}.md", section_slug(h))),
        None => ("overview".to_string(), format!("{doc_base}/overview.md")),
    };
    chunks.push(HelpFile {
        path,
        content: format!("# {doc_title} — {label}\n\n{text}"),
    });
}

/// Split a large markdown doc into retrievable chunks — one per `## ` / `### `
/// section — so a big reference (a 160 KB `KEYBINDING.md`) isn't one paragraph
/// truncated to ~2 KB by the F1 retrieval cap, and short-but-distinct sections
/// (e.g. "10. Quit") stay independently retrievable. Small docs (and docs with
/// no `## `/`### ` headings) pass through whole.
fn chunk_markdown(rel_path: &str, content: &str) -> Vec<HelpFile> {
    const WHOLE_MAX: usize = 2000;
    if content.chars().count() <= WHOLE_MAX {
        return vec![HelpFile {
            path: rel_path.to_string(),
            content: content.to_string(),
        }];
    }
    let doc_title = content
        .lines()
        .find_map(|l| l.strip_prefix("# ").map(|s| s.trim().to_string()))
        .unwrap_or_else(|| {
            rel_path
                .rsplit('/')
                .next()
                .unwrap_or(rel_path)
                .trim_end_matches(".md")
                .to_string()
        });

    // Sections nest under a directory named for the doc, so each imports as a
    // distinctly-titled paragraph (`<doc>/<section>.md`).
    let doc_base = rel_path.trim_end_matches(".md");

    let mut chunks: Vec<HelpFile> = Vec::new();
    let mut heading: Option<String> = None;
    let mut body: Vec<&str> = Vec::new();
    for line in content.lines() {
        // One chunk per `## ` / `### ` section — keeps short-but-distinct
        // sections (e.g. "10. Quit") independently retrievable. (Coalescing to
        // reduce the vector count diluted retrieval — a quit query stopped
        // surfacing the quit section — so precision wins here; index-open speed
        // is addressed structurally, not by merging chunks.)
        let hashes = line.chars().take_while(|c| *c == '#').count();
        let is_section = (hashes == 2 || hashes == 3) && line[hashes..].starts_with(' ');
        if is_section {
            push_chunk(&mut chunks, doc_base, &doc_title, &heading, &body);
            heading = Some(line[hashes + 1..].trim().to_string());
            body.clear();
            body.push(line);
        } else {
            body.push(line);
        }
    }
    push_chunk(&mut chunks, doc_base, &doc_title, &heading, &body);

    if chunks.is_empty() {
        return vec![HelpFile {
            path: rel_path.to_string(),
            content: content.to_string(),
        }];
    }
    // Disambiguate any two sections that slug to the same filename (rare, but a
    // collision would otherwise drop a chunk when unpacked to disk).
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for chunk in &mut chunks {
        let count = seen.entry(chunk.path.clone()).or_insert(0);
        if *count > 0 {
            let n = *count + 1;
            chunk.path = format!("{} ({n}).md", chunk.path.trim_end_matches(".md"));
        }
        *count += 1;
    }
    chunks
}

/// MAINTAINER: walk `docs_dir` and package every help-relevant `.md` file (path
/// relative to `docs_dir` + content) into a [`HelpCorpus`]. Meta / changelog /
/// internal docs are filtered out (see [`is_help_relevant`]); large docs are
/// split into section chunks (see [`chunk_markdown`]).
pub fn package_from_dir(docs_dir: &Path, version: &str) -> Result<HelpCorpus> {
    if !docs_dir.is_dir() {
        return Err(Error::Store(format!(
            "`{}` is not a directory",
            docs_dir.display()
        )));
    }
    let mut files = Vec::new();
    let mut skipped = 0usize;
    let mut docs = 0usize;
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
        files.extend(chunk_markdown(&rel_str, &content));
        docs += 1;
    }
    eprintln!(
        "included {docs} help doc(s) → {} searchable chunk(s); skipped {skipped} meta/changelog doc(s)",
        files.len(),
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
    fn chunk_markdown_splits_large_docs_by_heading() {
        // Small doc → one whole chunk.
        let small = chunk_markdown("x.md", "# Title\nshort body");
        assert_eq!(small.len(), 1);
        assert_eq!(small[0].path, "x.md");

        // A large doc with `## ` sections → one chunk per section, each carrying
        // the doc title + section label and the section's own content.
        let big = format!(
            "# Keys\n\n## Movement\n{pad}\n\n## Selection, clipboard\nCtrl+C copies.\n{pad}",
            pad = "filler line\n".repeat(200)
        );
        let chunks = chunk_markdown("KEYBINDING.md", &big);
        assert!(chunks.len() >= 2, "expected section chunks, got {}", chunks.len());
        // The clipboard section is its own retrievable chunk (not truncated away).
        let clip = chunks
            .iter()
            .find(|c| c.path.contains("Selection, clipboard"))
            .expect("clipboard section chunk");
        assert!(clip.content.contains("Ctrl+C copies."));
        assert!(clip.content.starts_with("# Keys — Selection, clipboard"));
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
