//! RESRCH-3 (R3-D) — folder-watch / sync. A designated research folder is
//! re-imported when its contents change: `inkhaven research --sync <folder>`
//! registers it (and imports it now), and every subsequent `inkhaven research`
//! launch re-imports any registered folder whose newest file is newer than the
//! last sync. A `.inkhaven/research-sync.json` manifest (the established sidecar
//! pattern); no file-watcher crate — mtime-gated re-import on launch.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::project::ProjectLayout;

/// The sync manifest: absolute folder path → last-sync unix timestamp (seconds).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SyncManifest {
    #[serde(default)]
    pub folders: BTreeMap<String, i64>,
}

impl SyncManifest {
    fn path(layout: &ProjectLayout) -> std::path::PathBuf {
        layout.root.join(".inkhaven").join("research-sync.json")
    }

    pub(crate) fn load(layout: &ProjectLayout) -> SyncManifest {
        match std::fs::read_to_string(SyncManifest::path(layout)) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => SyncManifest::default(),
        }
    }

    fn save(&self, layout: &ProjectLayout) -> Result<()> {
        let dir = layout.root.join(".inkhaven");
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let json = serde_json::to_string_pretty(self).context("serialise sync manifest")?;
        crate::io_atomic::write(&SyncManifest::path(layout), json.as_bytes())
            .context("write research-sync.json")?;
        Ok(())
    }

    /// Register a folder (canonicalized) and record `ts` as its last sync.
    pub(crate) fn register(layout: &ProjectLayout, folder: &str, ts: i64) -> Result<String> {
        let abs = std::fs::canonicalize(folder)
            .map(|c| c.to_string_lossy().into_owned())
            .unwrap_or_else(|_| folder.to_string());
        let mut m = SyncManifest::load(layout);
        m.folders.insert(abs.clone(), ts);
        m.save(layout)?;
        Ok(abs)
    }

    /// Update a folder's last-sync timestamp (no-op if not registered).
    pub(crate) fn mark_synced(layout: &ProjectLayout, abs: &str, ts: i64) {
        let mut m = SyncManifest::load(layout);
        if m.folders.contains_key(abs) {
            m.folders.insert(abs.to_string(), ts);
            let _ = m.save(layout);
        }
    }
}

/// The newest modification time (unix seconds) among the importable files under
/// `folder`, or `0` when the folder is empty / unreadable. Drives the "changed
/// since last sync?" check.
pub(crate) fn newest_mtime(folder: &Path) -> i64 {
    let mut newest = 0i64;
    for entry in walkdir::WalkDir::new(folder).into_iter().filter_map(|e| e.ok()) {
        let fp = entry.path();
        let ext = fp.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if !fp.is_file() || !matches!(ext.as_str(), "md" | "markdown" | "txt" | "text" | "pdf") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                    newest = newest.max(dur.as_secs() as i64);
                }
            }
        }
    }
    newest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_layout(tag: &str) -> ProjectLayout {
        let dir = std::env::temp_dir().join(format!("resrch-sync-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        ProjectLayout::new(dir)
    }

    #[test]
    fn register_and_reload_roundtrip() {
        let layout = tmp_layout("roundtrip");
        let folder = std::env::temp_dir().join(format!("resrch-sync-src-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let abs = SyncManifest::register(&layout, folder.to_str().unwrap(), 100).unwrap();
        let m = SyncManifest::load(&layout);
        assert_eq!(m.folders.get(&abs), Some(&100));
        SyncManifest::mark_synced(&layout, &abs, 200);
        assert_eq!(SyncManifest::load(&layout).folders.get(&abs), Some(&200));
    }

    #[test]
    fn newest_mtime_sees_supported_files() {
        let folder = std::env::temp_dir().join(format!("resrch-mtime-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("note.md"), b"hello").unwrap();
        assert!(newest_mtime(&folder) > 0);
        // An empty folder (no supported files) → 0.
        let empty = std::env::temp_dir().join(format!("resrch-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&empty);
        std::fs::create_dir_all(&empty).unwrap();
        assert_eq!(newest_mtime(&empty), 0);
    }
}
