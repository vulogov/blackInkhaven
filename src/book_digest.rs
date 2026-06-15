//! 1.3.1 SUBMISSION-1 P3 — the book digest.
//!
//! A novel doesn't fit a prompt, so the submission-package generators
//! (query letter / synopsis / comps / logline) work against a compact
//! **digest** instead of the raw manuscript: the deterministic skeleton
//! (title / author / length / chapter titles + the Characters and Threads
//! books) plus an AI one-line summary per chapter.  Cached in
//! `.inkhaven/digest-<slug>.json` and invalidated by a content hash so it
//! only rebuilds when the manuscript materially changes.
//!
//! This module is the model + cache + prompt rendering (deterministic,
//! tested); the AI summary pass + assembly live in `cli::submission`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterSummary {
    pub title: String,
    /// One-line AI summary (empty if not yet generated).
    pub summary: String,
}

/// The compact whole-book context the generators consume.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookDigest {
    pub book_slug: String,
    pub title: String,
    pub author: String,
    pub word_count: usize,
    pub chapters: Vec<ChapterSummary>,
    /// Character names (+ optional one-line) from the Characters book.
    #[serde(default)]
    pub characters: Vec<String>,
    /// Thread names from the Threads book.
    #[serde(default)]
    pub threads: Vec<String>,
    /// Hash of the structural signature; a mismatch means rebuild.
    pub content_hash: u64,
}

impl BookDigest {
    pub fn sidecar_path(project_root: &Path, slug: &str) -> PathBuf {
        project_root
            .join(".inkhaven")
            .join(format!("digest-{slug}.json"))
    }

    pub fn load(project_root: &Path, slug: &str) -> Option<Self> {
        let path = Self::sidecar_path(project_root, slug);
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn save(&self, project_root: &Path) -> Result<(), String> {
        let dir = project_root.join(".inkhaven");
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("digest: mkdir {}: {e}", dir.display()))?;
        let path = Self::sidecar_path(project_root, &self.book_slug);
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("digest: serialize: {e}"))?;
        crate::io_atomic::write(&path, raw.as_bytes())
            .map_err(|e| format!("digest: write {}: {e}", path.display()))
    }

    /// A cheap structural fingerprint: chapter titles + word count +
    /// character / thread names.  When the manuscript's shape changes the
    /// hash changes and the cached digest is considered stale — material
    /// enough without hashing every byte of prose.
    pub fn compute_hash(
        title: &str,
        word_count: usize,
        chapter_titles: &[String],
        characters: &[String],
        threads: &[String],
    ) -> u64 {
        let mut h = DefaultHasher::new();
        title.hash(&mut h);
        word_count.hash(&mut h);
        chapter_titles.hash(&mut h);
        characters.hash(&mut h);
        threads.hash(&mut h);
        h.finish()
    }

    /// True when this cached digest still matches the live signature.
    pub fn matches(
        &self,
        title: &str,
        word_count: usize,
        chapter_titles: &[String],
        characters: &[String],
        threads: &[String],
    ) -> bool {
        self.content_hash
            == Self::compute_hash(title, word_count, chapter_titles, characters, threads)
    }

    /// Render the digest as a compact text block for a generator prompt.
    pub fn as_context(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("TITLE: {}\n", self.title));
        if !self.author.is_empty() {
            s.push_str(&format!("AUTHOR: {}\n", self.author));
        }
        s.push_str(&format!(
            "LENGTH: ~{} words across {} chapter(s)\n",
            crate::manuscript::round_word_count(self.word_count),
            self.chapters.len(),
        ));
        if !self.characters.is_empty() {
            s.push_str(&format!("CHARACTERS: {}\n", self.characters.join("; ")));
        }
        if !self.threads.is_empty() {
            s.push_str(&format!("THREADS: {}\n", self.threads.join("; ")));
        }
        s.push_str("\nCHAPTER SUMMARIES:\n");
        for (i, c) in self.chapters.iter().enumerate() {
            if c.summary.trim().is_empty() {
                s.push_str(&format!("{}. {}\n", i + 1, c.title));
            } else {
                s.push_str(&format!("{}. {} — {}\n", i + 1, c.title, c.summary.trim()));
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> BookDigest {
        let chapters = vec![
            ChapterSummary { title: "The Wharf".into(), summary: "Mara arrives.".into() },
            ChapterSummary { title: "The Letter".into(), summary: "A secret surfaces.".into() },
        ];
        let titles: Vec<String> = chapters.iter().map(|c| c.title.clone()).collect();
        let characters = vec!["Mara — the lighthouse keeper".to_string()];
        let threads = vec!["the inheritance".to_string()];
        let hash = BookDigest::compute_hash("The Harbor Code", 80_000, &titles, &characters, &threads);
        BookDigest {
            book_slug: "the-harbor-code".into(),
            title: "The Harbor Code".into(),
            author: "Jane Writer".into(),
            word_count: 80_000,
            chapters,
            characters,
            threads,
            content_hash: hash,
        }
    }

    #[test]
    fn hash_tracks_structural_changes() {
        let d = sample();
        let titles: Vec<String> = d.chapters.iter().map(|c| c.title.clone()).collect();
        assert!(d.matches("The Harbor Code", 80_000, &titles, &d.characters, &d.threads));
        // a new chapter title → stale
        let mut t2 = titles.clone();
        t2.push("The Reckoning".into());
        assert!(!d.matches("The Harbor Code", 80_000, &t2, &d.characters, &d.threads));
        // a different word count → stale
        assert!(!d.matches("The Harbor Code", 95_000, &titles, &d.characters, &d.threads));
    }

    #[test]
    fn context_render_is_compact_and_ordered() {
        let ctx = sample().as_context();
        assert!(ctx.contains("TITLE: The Harbor Code"));
        assert!(ctx.contains("LENGTH: ~80000 words across 2 chapter(s)"));
        assert!(ctx.contains("CHARACTERS: Mara — the lighthouse keeper"));
        assert!(ctx.contains("THREADS: the inheritance"));
        // chapter summaries numbered in order
        let one = ctx.find("1. The Wharf — Mara arrives.").unwrap();
        let two = ctx.find("2. The Letter — A secret surfaces.").unwrap();
        assert!(one < two, "chapters in order");
    }

    #[test]
    fn round_trips_through_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(BookDigest::load(root, "the-harbor-code").is_none(), "absent → None");
        sample().save(root).unwrap();
        let back = BookDigest::load(root, "the-harbor-code").unwrap();
        assert_eq!(back.title, "The Harbor Code");
        assert_eq!(back.chapters.len(), 2);
        assert_eq!(back.content_hash, sample().content_hash);
    }
}
