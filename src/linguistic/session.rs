//! L-P0 — session persistence for the Linguistic companion.
//!
//! A session is the chat transcript, stored as plain serde JSON at
//! `.inkhaven/linguistic-sessions/<slug>.json` (the established `.inkhaven/`
//! sidecar pattern, mirroring the Research companion's threads). `--session
//! <name>` opens (or creates) a named one; without it, `default`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::project::ProjectLayout;

/// The `.inkhaven/linguistic-sessions/` directory for a project.
fn sessions_dir(layout: &ProjectLayout) -> PathBuf {
    layout.root.join(".inkhaven").join("linguistic-sessions")
}

/// Slugify a display name into a stable filename stem.
pub(crate) fn session_slug(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "default".to_string() } else { s }
}

/// One persisted chat exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionTurn {
    pub prompt: String,
    pub response: String,
    /// The language sub-book (title) that grounded the answer, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub timestamp: String,
}

/// A named chat session: its transcript, persisted across launches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Session {
    pub name: String,
    pub slug: String,
    pub created: String,
    #[serde(default)]
    pub turns: Vec<SessionTurn>,
}

impl Session {
    fn path(layout: &ProjectLayout, slug: &str) -> PathBuf {
        sessions_dir(layout).join(format!("{slug}.json"))
    }

    /// Load a session by slug, or `None` if it doesn't exist / won't parse.
    fn load(layout: &ProjectLayout, slug: &str) -> Option<Session> {
        let raw = std::fs::read_to_string(Session::path(layout, slug)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Open the named session, creating (and persisting) an empty one if absent.
    pub(crate) fn open_or_create(
        layout: &ProjectLayout,
        display_name: &str,
        now: String,
    ) -> Result<Session> {
        let slug = session_slug(display_name);
        if let Some(s) = Session::load(layout, &slug) {
            return Ok(s);
        }
        let s = Session {
            name: display_name.trim().to_string(),
            slug,
            created: now,
            turns: Vec::new(),
        };
        s.save(layout)?;
        Ok(s)
    }

    /// Append a completed turn and persist. Errors are surfaced to the caller
    /// (which shows them on the status line) rather than lost.
    pub(crate) fn record(&mut self, turn: SessionTurn, layout: &ProjectLayout) -> Result<()> {
        self.turns.push(turn);
        self.save(layout)
    }

    fn save(&self, layout: &ProjectLayout) -> Result<()> {
        let dir = sessions_dir(layout);
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let json = serde_json::to_string_pretty(self).context("serialise session")?;
        crate::io_atomic::write(&Session::path(layout, &self.slug), json.as_bytes())
            .with_context(|| "write linguistic session")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_stable_and_filesystem_safe() {
        assert_eq!(session_slug("Quenya notes"), "quenya-notes");
        assert_eq!(session_slug("  ??  "), "default");
        assert_eq!(session_slug(""), "default");
        assert_eq!(session_slug("A/B:C"), "a-b-c");
    }

    #[test]
    fn open_or_create_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let layout = ProjectLayout::new(dir.path());
        std::fs::create_dir_all(dir.path().join(".inkhaven")).unwrap();

        let mut s = Session::open_or_create(&layout, "Work", "2026-07-16T00:00:00Z".into()).unwrap();
        s.record(
            SessionTurn {
                prompt: "how many vowels?".into(),
                response: "five".into(),
                scope: Some("Quenya".into()),
                timestamp: "2026-07-16T00:00:01Z".into(),
            },
            &layout,
        )
        .unwrap();

        // Re-open by the same name → the turn is there.
        let reloaded = Session::open_or_create(&layout, "Work", "later".into()).unwrap();
        assert_eq!(reloaded.turns.len(), 1);
        assert_eq!(reloaded.turns[0].prompt, "how many vowels?");
        assert_eq!(reloaded.turns[0].scope.as_deref(), Some("Quenya"));
        // `created` is preserved from the first open, not overwritten.
        assert_eq!(reloaded.created, "2026-07-16T00:00:00Z");
    }
}
