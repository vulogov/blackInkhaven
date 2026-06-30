//! RESRCH-1 (R-P2) — named research threads (RFC §18). Each thread is a JSON
//! file at `.inkhaven/research-threads/<slug>.json`: query turns, fact / note
//! insertions, RAG mode, and pinned nodes — a persistent, resumable session.
//!
//! Storage is plain serde JSON (the established `.inkhaven/` sidecar pattern,
//! like `submissions.json` / `continuity.json`); no DuckDB. Timestamps are
//! RFC3339 strings.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::project::ProjectLayout;

/// The `.inkhaven/research-threads/` directory for a project.
pub(crate) fn threads_dir(layout: &ProjectLayout) -> PathBuf {
    layout.root.join(".inkhaven").join("research-threads")
}

/// The on-disk slug for a thread display name (filesystem-safe, stable).
pub(crate) fn thread_slug(name: &str) -> String {
    let s = slug::slugify(name);
    if s.is_empty() { "default".to_string() } else { s }
}

/// RAG retrieval mode (RFC §8). Serialised with the RFC's variant names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum RagMode {
    #[default]
    FactsPlusFull,
    FactsOnly,
    FullOnly,
}

impl RagMode {
    /// F10 cycle: Facts+Full → Facts only → Full only → Facts+Full.
    pub(crate) fn next(self) -> RagMode {
        match self {
            RagMode::FactsPlusFull => RagMode::FactsOnly,
            RagMode::FactsOnly => RagMode::FullOnly,
            RagMode::FullOnly => RagMode::FactsPlusFull,
        }
    }

    /// The status-bar label.
    pub(crate) fn label(self) -> &'static str {
        match self {
            RagMode::FactsPlusFull => "Facts+Full",
            RagMode::FactsOnly => "Facts only",
            RagMode::FullOnly => "Full only",
        }
    }
}

/// One turn in a thread. `kind` discriminates the shape (a plain query vs a
/// fact / note insertion). Optional fields stay absent in JSON when unused.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ResearchTurn {
    pub id: String,
    pub kind: TurnKind,
    pub timestamp: String,

    // query turns
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,

    // fact_insertion / note_insertion turns
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_book: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TurnKind {
    Query,
    FactInsertion,
    NoteInsertion,
}

impl ResearchTurn {
    /// A completed query/response turn.
    pub(crate) fn query(id: String, prompt: String, response: String, cost: f64, now: String) -> ResearchTurn {
        ResearchTurn {
            id,
            kind: TurnKind::Query,
            timestamp: now,
            prompt: Some(prompt),
            response: Some(response),
            cost: Some(cost),
            command: None,
            extracted_title: None,
            extracted_text: None,
            insertion_path: None,
            target_book: None,
        }
    }

    /// A confirmed fact / note insertion turn.
    pub(crate) fn insertion(
        id: String,
        kind: TurnKind,
        command: String,
        title: String,
        text: String,
        path: String,
        target_book: String,
        now: String,
    ) -> ResearchTurn {
        ResearchTurn {
            id,
            kind,
            timestamp: now,
            prompt: None,
            response: None,
            cost: None,
            command: Some(command),
            extracted_title: Some(title),
            extracted_text: Some(text),
            insertion_path: Some(path),
            target_book: Some(target_book),
        }
    }
}

/// A persistent research session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ResearchThread {
    pub name: String,
    pub display_name: String,
    pub created_at: String,
    pub last_active: String,
    #[serde(default)]
    pub rag_mode: RagMode,
    #[serde(default)]
    pub pinned_nodes: Vec<String>,
    #[serde(default)]
    pub turns: Vec<ResearchTurn>,
}

impl ResearchThread {
    /// A fresh thread with the given display name.
    pub(crate) fn new(display_name: &str, now: String) -> ResearchThread {
        let name = thread_slug(display_name);
        ResearchThread {
            name,
            display_name: display_name.to_string(),
            created_at: now.clone(),
            last_active: now,
            rag_mode: RagMode::default(),
            pinned_nodes: Vec::new(),
            turns: Vec::new(),
        }
    }

    /// The thread's JSON path under a project.
    pub(crate) fn path(layout: &ProjectLayout, slug: &str) -> PathBuf {
        threads_dir(layout).join(format!("{slug}.json"))
    }

    /// Total cost across all turns.
    pub(crate) fn total_cost(&self) -> f64 {
        self.turns.iter().filter_map(|t| t.cost).sum()
    }

    /// Load a thread by slug, or `None` if the file is absent / unreadable.
    pub(crate) fn load(layout: &ProjectLayout, slug: &str) -> Option<ResearchThread> {
        let path = ResearchThread::path(layout, slug);
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Open the named thread, creating it (and the threads dir) if absent.
    pub(crate) fn open_or_create(layout: &ProjectLayout, display_name: &str, now: String) -> Result<ResearchThread> {
        let slug = thread_slug(display_name);
        if let Some(t) = ResearchThread::load(layout, &slug) {
            return Ok(t);
        }
        let t = ResearchThread::new(display_name, now);
        t.save(layout)?;
        Ok(t)
    }

    /// Persist the thread (atomic-ish: write then rename would be ideal; the
    /// established sidecar writers use a direct write, matched here).
    pub(crate) fn save(&self, layout: &ProjectLayout) -> Result<()> {
        let dir = threads_dir(layout);
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let path = ResearchThread::path(layout, &self.name);
        let json = serde_json::to_string_pretty(self).context("serialise thread")?;
        crate::io_atomic::write(&path, json.as_bytes())
            .with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    /// Append a turn and bump `last_active`, then save.
    pub(crate) fn push_turn(&mut self, turn: ResearchTurn, layout: &ProjectLayout) -> Result<()> {
        self.last_active = turn.timestamp.clone();
        self.turns.push(turn);
        self.save(layout)
    }
}

/// Metadata for the thread picker / `--list-threads` (no turn bodies).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ThreadSummary {
    pub name: String,
    pub display_name: String,
    pub last_active: String,
    pub turns: usize,
    pub cost: f64,
}

/// Every thread in a project, newest-active first.
pub(crate) fn list_threads(layout: &ProjectLayout) -> Vec<ThreadSummary> {
    let dir = threads_dir(layout);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<ThreadSummary> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(slug) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        if let Some(t) = ResearchThread::load(layout, slug) {
            out.push(ThreadSummary {
                name: t.name.clone(),
                display_name: t.display_name.clone(),
                last_active: t.last_active.clone(),
                turns: t.turns.len(),
                cost: t.total_cost(),
            });
        }
    }
    out.sort_by(|a, b| b.last_active.cmp(&a.last_active));
    out
}

/// Delete a thread file. Returns whether a file was removed.
pub(crate) fn delete_thread(layout: &ProjectLayout, slug: &str) -> Result<bool> {
    let path = ResearchThread::path(layout, slug);
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_layout(tag: &str) -> ProjectLayout {
        // Unique per test (the suite runs tests in parallel within one process,
        // so a pid-only dir would race).
        let dir = std::env::temp_dir().join(format!("resrch-thread-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        ProjectLayout::new(dir)
    }

    #[test]
    fn slug_is_filesystem_safe() {
        assert_eq!(thread_slug("Ancient Rome!"), "ancient-rome");
        assert_eq!(thread_slug(""), "default");
    }

    #[test]
    fn rag_mode_cycles() {
        assert_eq!(RagMode::FactsPlusFull.next(), RagMode::FactsOnly);
        assert_eq!(RagMode::FactsOnly.next(), RagMode::FullOnly);
        assert_eq!(RagMode::FullOnly.next(), RagMode::FactsPlusFull);
    }

    #[test]
    fn create_load_save_roundtrip() {
        let layout = tmp_layout("roundtrip");
        let now = "2026-07-15T14:32:00Z".to_string();
        let mut t = ResearchThread::open_or_create(&layout, "Rome", now.clone()).unwrap();
        assert_eq!(t.name, "rome");
        assert_eq!(t.turns.len(), 0);

        t.push_turn(
            ResearchTurn::query("id1".into(), "q?".into(), "a.".into(), 0.012, now.clone()),
            &layout,
        )
        .unwrap();

        let reloaded = ResearchThread::load(&layout, "rome").unwrap();
        assert_eq!(reloaded.turns.len(), 1);
        assert_eq!(reloaded.turns[0].kind, TurnKind::Query);
        assert!((reloaded.total_cost() - 0.012).abs() < 1e-9);

        // open_or_create returns the existing one, not a fresh thread.
        let again = ResearchThread::open_or_create(&layout, "Rome", now).unwrap();
        assert_eq!(again.turns.len(), 1);
    }

    #[test]
    fn insertion_turn_roundtrips() {
        let layout = tmp_layout("insertion");
        let now = "2026-07-15T14:35:00Z".to_string();
        let mut t = ResearchThread::open_or_create(&layout, "rome", now.clone()).unwrap();
        t.push_turn(
            ResearchTurn::insertion(
                "i1".into(),
                TurnKind::FactInsertion,
                "/fact \"capacity\"".into(),
                "Aqua Claudia Capacity".into(),
                "~190,000 m³/day.".into(),
                "facts/rome/aqua-claudia".into(),
                "Facts".into(),
                now,
            ),
            &layout,
        )
        .unwrap();
        let reloaded = ResearchThread::load(&layout, "rome").unwrap();
        assert_eq!(reloaded.turns.len(), 1);
        let turn = &reloaded.turns[0];
        assert_eq!(turn.kind, TurnKind::FactInsertion);
        assert_eq!(turn.extracted_title.as_deref(), Some("Aqua Claudia Capacity"));
        assert_eq!(turn.target_book.as_deref(), Some("Facts"));
        assert_eq!(turn.insertion_path.as_deref(), Some("facts/rome/aqua-claudia"));
    }

    #[test]
    fn list_and_delete() {
        let layout = tmp_layout("listdel");
        let now = "2026-07-15T14:32:00Z".to_string();
        ResearchThread::open_or_create(&layout, "rome", now.clone()).unwrap();
        ResearchThread::open_or_create(&layout, "medieval", now).unwrap();
        let listed = list_threads(&layout);
        assert_eq!(listed.len(), 2);
        assert!(delete_thread(&layout, "rome").unwrap());
        assert!(!delete_thread(&layout, "rome").unwrap());
        assert_eq!(list_threads(&layout).len(), 1);
    }
}
