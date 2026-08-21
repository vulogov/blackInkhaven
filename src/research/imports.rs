//! RESRCH-2 (R2-B) — document import. Ingests a local Markdown / plain-text /
//! PDF file as a **research source**: read → chunk → (the app) embed each chunk
//! into the shared vector store tagged `kind: research_source`, so retrieval can
//! ground answers on it and `/fact` can cite it (provenance `origin=document`).
//!
//! This module is the pure half — reading + chunking + the sources sidecar
//! (`.inkhaven/research-sources.json`). Embedding (`add_document`) and removal
//! (`delete_document`) live in the app, which owns the store. PDF text needs the
//! one new runtime crate (`pdf-extract`); MD/text needs none.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::project::ProjectLayout;

/// Metadata `kind` tagging an embedded research-source chunk in the doc store.
pub(super) const SOURCE_KIND: &str = "research_source";

/// One imported source's record in the sidecar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ImportedSource {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub doc_ids: Vec<String>,
    #[serde(default)]
    pub thread: String,
    pub imported_at: String,
    #[serde(default)]
    pub chunks: usize,
}

/// The imported-sources sidecar: source name (slug) → record.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct Imports {
    #[serde(default)]
    pub sources: BTreeMap<String, ImportedSource>,
}

impl Imports {
    fn path(layout: &ProjectLayout) -> std::path::PathBuf {
        layout.root.join(".inkhaven").join("research-sources.json")
    }

    pub(super) fn load(layout: &ProjectLayout) -> Imports {
        let path = Imports::path(layout);
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Imports::default();
        };
        match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                // Don't silently reset: the next save would overwrite the corrupt
                // file and orphan every imported source's vector-store chunks. Back
                // it up and start fresh.
                let backup = path.with_file_name("research-sources.json.corrupt");
                let _ = std::fs::rename(&path, &backup);
                tracing::warn!(
                    target: "inkhaven::research",
                    "research-sources.json unreadable ({e}); backed up to {}",
                    backup.display()
                );
                Imports::default()
            }
        }
    }

    pub(super) fn save(&self, layout: &ProjectLayout) -> Result<()> {
        let dir = layout.root.join(".inkhaven");
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let json = serde_json::to_string_pretty(self).context("serialise imports")?;
        crate::io_atomic::write(&Imports::path(layout), json.as_bytes())
            .context("write research-sources.json")?;
        Ok(())
    }
}

/// The display name (slug) for a source path — the file stem (no extension).
pub(super) fn source_name(path: &Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("source");
    let s = slug::slugify(stem);
    if s.is_empty() { "source".to_string() } else { s }
}

/// Read a source file to plain text, dispatching on extension. Markdown / text
/// are read directly (no crate); PDF goes through `pdf-extract`.
pub(super) fn read_source(path: &Path) -> Result<String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "md" | "markdown" | "txt" | "text" | "" => {
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
        }
        "pdf" => {
            // Reject an oversized file up front — a crafted PDF can also drive
            // `pdf-extract` into a huge allocation / unbounded loop.
            const MAX_PDF_BYTES: u64 = 100 * 1024 * 1024; // 100 MiB
            if let Ok(meta) = std::fs::metadata(path) {
                if meta.len() > MAX_PDF_BYTES {
                    return Err(anyhow!(
                        "PDF too large to import ({} MiB > {} MiB cap): {}",
                        meta.len() / (1024 * 1024),
                        MAX_PDF_BYTES / (1024 * 1024),
                        path.display()
                    ));
                }
            }
            // `pdf-extract` byte-slices and can `panic!`/`unwrap`/index out of
            // bounds on malformed PDFs (corrupt xref, broken font encodings),
            // and has no internal time/allocation bound — a pathological file
            // can hang or OOM, which `catch_unwind` cannot catch (abort / loop).
            // Run it on a worker thread with a deadline: a panic is isolated (as
            // the RTF importer does), and a runaway is abandoned so the
            // single-threaded research TUI stays responsive.
            const PDF_EXTRACT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
            let owned = path.to_path_buf();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let guarded = crate::crash::suppress_panic_report(|| {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        pdf_extract::extract_text(&owned)
                    }))
                });
                let _ = tx.send(guarded);
            });
            match rx.recv_timeout(PDF_EXTRACT_TIMEOUT) {
                Ok(Ok(Ok(text))) => Ok(text),
                Ok(Ok(Err(e))) => {
                    Err(anyhow!("PDF text extraction failed for {}: {e}", path.display()))
                }
                Ok(Err(_)) => Err(anyhow!(
                    "PDF text extraction failed for {} (malformed or unsupported PDF)",
                    path.display()
                )),
                Err(_) => Err(anyhow!(
                    "PDF text extraction timed out for {} (> {}s — malformed or pathological PDF)",
                    path.display(),
                    PDF_EXTRACT_TIMEOUT.as_secs()
                )),
            }
        }
        other => Err(anyhow!("unsupported source format: .{other} (md / txt / pdf)")),
    }
}

/// Greedily pack blank-line-separated paragraphs into chunks of at most
/// `max_chars`, hard-splitting any single oversize paragraph. Empty input → no
/// chunks.
pub(super) fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let max = max_chars.max(200);
    let mut chunks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for para in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        if para.chars().count() > max {
            // Flush, then hard-split the long paragraph.
            if !cur.is_empty() {
                chunks.push(std::mem::take(&mut cur));
            }
            let chars: Vec<char> = para.chars().collect();
            for piece in chars.chunks(max) {
                chunks.push(piece.iter().collect());
            }
            continue;
        }
        let need = if cur.is_empty() { para.chars().count() } else { cur.chars().count() + 2 + para.chars().count() };
        if need > max && !cur.is_empty() {
            chunks.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push_str("\n\n");
        }
        cur.push_str(para);
    }
    if !cur.trim().is_empty() {
        chunks.push(cur);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_packs_and_splits() {
        // Three short paragraphs pack into one chunk under a generous budget.
        let text = "Alpha para.\n\nBeta para.\n\nGamma para.";
        let c = chunk_text(text, 1000);
        assert_eq!(c.len(), 1);
        assert!(c[0].contains("Alpha") && c[0].contains("Gamma"));

        // Paragraphs whose combined size exceeds the (≥200) budget split. Three
        // ~150-char paragraphs at budget 200 → at least 2 chunks.
        let para = "w".repeat(150);
        let big_text = format!("{para}\n\n{para}\n\n{para}");
        let c2 = chunk_text(&big_text, 200);
        assert!(c2.len() >= 2, "got {} chunks", c2.len());

        // An oversize single paragraph is hard-split.
        let big = "x".repeat(900);
        let c3 = chunk_text(&big, 200);
        assert!(c3.len() >= 4);

        // Empty input → nothing.
        assert!(chunk_text("   \n\n  ", 500).is_empty());
    }

    #[test]
    fn names_and_unsupported() {
        // The slug drops the extension.
        assert_eq!(source_name(Path::new("/a/Roman Aqueducts.md")), "roman-aqueducts");
        assert!(read_source(Path::new("/x/notes.docx")).is_err());
    }
}
