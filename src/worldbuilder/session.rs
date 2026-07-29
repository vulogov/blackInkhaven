//! WBLD-1 (WB-P0) — session persistence for the worldbuilder.
//!
//! A session is the record of one worldbuilding sitting: an ordered list of
//! turns (commands + their world/map deltas + plausibility arc + inserted
//! world-facts), plus the pane sizing the author last used. Stored as plain
//! serde JSON at `.inkhaven/worldbuilder-sessions/<slug>.json` — the established
//! `.inkhaven/` sidecar pattern, mirroring the Research threads and the
//! Linguistic sessions. `--session <name>` opens (or creates) a named one;
//! without it, `default`. Later phases fill in the turn fields; WB-P0 only needs
//! the round-trip.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::project::ProjectLayout;

/// The `.inkhaven/worldbuilder-sessions/` directory for a project.
fn sessions_dir(layout: &ProjectLayout) -> PathBuf {
    layout.root.join(".inkhaven").join("worldbuilder-sessions")
}

/// Slugify a display name into a stable, filesystem-safe filename stem.
pub(crate) fn session_slug(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    // Collapse runs of '-' and trim the ends.
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c == '-' {
            if !prev_dash {
                out.push('-');
            }
            prev_dash = true;
        } else {
            out.push(c);
            prev_dash = false;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() { "default".to_string() } else { out }
}

/// One recorded worldbuilder turn. Most fields are filled by later phases; WB-P0
/// persists the shape so no migration is needed as they land.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SessionTurn {
    pub seq: u64,
    pub at: String,
    /// The command / question the author entered.
    pub user: String,
    /// A one-line summary of the worldbuilder's response.
    #[serde(default)]
    pub assistant_summary: String,
    /// The `world.hjson` fragment this turn added (WB-P4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hjson_delta: Option<String>,
    /// The MapSpec fragment this turn added (WB-P6); `None` for non-geographic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapspec_delta: Option<String>,
    /// Plausibility score before / after this turn (WB-P3).
    #[serde(default)]
    pub plausibility_before: Option<u8>,
    #[serde(default)]
    pub plausibility_after: Option<u8>,
    /// Map render PNG paths produced this turn (WB-P6).
    #[serde(default)]
    pub map_renders: Vec<String>,
    /// Facts-book node UUIDs created by `/wfact` this turn (WB-P7).
    #[serde(default)]
    pub facts_inserted: Vec<String>,
}

/// A named worldbuilder session, persisted across launches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorldbuilderSession {
    pub name: String,
    pub slug: String,
    pub created: String,
    /// The world being built (from `world.hjson`; empty until named).
    #[serde(default)]
    pub world_name: String,
    #[serde(default)]
    pub turns: Vec<SessionTurn>,
    /// Persisted pane sizing (WB-P1 resize gestures). `left_split` = Facts/World
    /// vertical ratio; `split_ratio` = left-column/right-pane width ratio. Both
    /// clamped 2–8 on load.
    #[serde(default = "default_left_split")]
    pub left_split: u8,
    #[serde(default = "default_split_ratio")]
    pub split_ratio: u8,
}

fn default_left_split() -> u8 {
    5
}
fn default_split_ratio() -> u8 {
    4
}

impl WorldbuilderSession {
    fn path(layout: &ProjectLayout, slug: &str) -> PathBuf {
        sessions_dir(layout).join(format!("{slug}.json"))
    }

    fn load(layout: &ProjectLayout, slug: &str) -> Option<WorldbuilderSession> {
        let raw = std::fs::read_to_string(WorldbuilderSession::path(layout, slug)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Open the named session, creating (and persisting) an empty one if absent.
    pub(crate) fn open_or_create(
        layout: &ProjectLayout,
        display_name: &str,
        now: String,
    ) -> Result<WorldbuilderSession> {
        let slug = session_slug(display_name);
        if let Some(mut s) = WorldbuilderSession::load(layout, &slug) {
            s.left_split = s.left_split.clamp(2, 8);
            s.split_ratio = s.split_ratio.clamp(2, 8);
            return Ok(s);
        }
        let s = WorldbuilderSession {
            name: display_name.trim().to_string(),
            slug,
            created: now,
            world_name: String::new(),
            turns: Vec::new(),
            left_split: default_left_split(),
            split_ratio: default_split_ratio(),
        };
        s.save(layout)?;
        Ok(s)
    }

    /// List all session display names for a project (slugs, sorted). Used by
    /// `/sessions` (WB-P10).
    #[allow(dead_code)]
    pub(crate) fn list(layout: &ProjectLayout) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(sessions_dir(layout)) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("json") {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        out.push(stem.to_string());
                    }
                }
            }
        }
        out.sort();
        out
    }

    /// Persist the session atomically. Errors are surfaced to the caller.
    pub(crate) fn save(&self, layout: &ProjectLayout) -> Result<()> {
        let dir = sessions_dir(layout);
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let json = serde_json::to_string_pretty(self).context("serialise worldbuilder session")?;
        crate::io_atomic::write(&WorldbuilderSession::path(layout, &self.slug), json.as_bytes())
            .context("write worldbuilder session")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_stable_and_filesystem_safe() {
        assert_eq!(session_slug("Initial Build!"), "initial-build");
        assert_eq!(session_slug("  Aldoria — v2  "), "aldoria-v2");
        assert_eq!(session_slug(""), "default");
        assert_eq!(session_slug("///"), "default");
    }

    #[test]
    fn session_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let layout = ProjectLayout::new(dir.path());
        let mut s = WorldbuilderSession::open_or_create(&layout, "session-01", "now".into()).unwrap();
        s.world_name = "Aldoria".into();
        s.left_split = 6;
        s.split_ratio = 3;
        s.turns.push(SessionTurn {
            seq: 1,
            at: "t".into(),
            user: "/star K".into(),
            assistant_summary: "Set star to K-dwarf".into(),
            plausibility_before: Some(100),
            plausibility_after: Some(100),
            ..Default::default()
        });
        s.save(&layout).unwrap();

        let reopened = WorldbuilderSession::open_or_create(&layout, "session-01", "ignored".into()).unwrap();
        assert_eq!(reopened.world_name, "Aldoria");
        assert_eq!(reopened.left_split, 6);
        assert_eq!(reopened.split_ratio, 3);
        assert_eq!(reopened.turns.len(), 1);
        assert_eq!(reopened.turns[0].user, "/star K");
        assert_eq!(reopened.created, "now"); // not overwritten on reopen
        assert!(WorldbuilderSession::list(&layout).contains(&"session-01".to_string()));
    }

    #[test]
    fn out_of_range_sizing_is_clamped_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let layout = ProjectLayout::new(dir.path());
        let mut s = WorldbuilderSession::open_or_create(&layout, "s", "now".into()).unwrap();
        s.left_split = 99;
        s.split_ratio = 0;
        s.save(&layout).unwrap();
        let reopened = WorldbuilderSession::open_or_create(&layout, "s", "x".into()).unwrap();
        assert_eq!(reopened.left_split, 8);
        assert_eq!(reopened.split_ratio, 2);
    }
}
