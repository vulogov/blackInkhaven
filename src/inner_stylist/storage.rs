//! The per-project Inner Stylist store (`<project>/inner_stylist.db`). The
//! findings are book-wide and deterministic (the fast track recomputes them from
//! the manuscript any time), so this store holds only the author's
//! **suppressions** — the decision to stop being told about a particular
//! complaint (by its stable `finding.key`). Built on the in-tree `StorageEngine`,
//! like the other Inner-family stores; its own DB file (one per reader).

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use duckdb::types::Value as DuckValue;

use crate::storage::engine::StorageEngine;
use crate::world::proposals::now_secs;

const INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS stylist_suppressions (
        finding_key   TEXT   NOT NULL PRIMARY KEY,
        suppressed_at BIGINT NOT NULL
    );
";

#[derive(Clone)]
pub(crate) struct InnerStylistStore {
    engine: Arc<StorageEngine>,
}

impl InnerStylistStore {
    /// `<project>/inner_stylist.db`, beside the other Inner-family stores.
    pub(crate) fn open_for_project(project_root: &Path) -> Result<Self> {
        let path = project_root.join("inner_stylist.db");
        Ok(Self { engine: Arc::new(StorageEngine::new_versioned(&path, INIT_SQL, 2, 1)?) })
    }

    /// Silence a finding by its stable key.
    pub(crate) fn suppress(&self, key: &str) -> Result<()> {
        let (k, now) = (key.to_string(), now_secs());
        self.engine.execute_with(
            "INSERT OR REPLACE INTO stylist_suppressions (finding_key, suppressed_at) VALUES (?, ?)",
            &[&k, &now],
        )?;
        Ok(())
    }

    /// Un-silence a finding.
    pub(crate) fn unsuppress(&self, key: &str) -> Result<()> {
        let k = key.to_string();
        self.engine
            .execute_with("DELETE FROM stylist_suppressions WHERE finding_key = ?", &[&k])?;
        Ok(())
    }

    /// Every suppressed key.
    pub(crate) fn all_suppressions(&self) -> Result<Vec<String>> {
        let rows = self
            .engine
            .select_all("SELECT finding_key FROM stylist_suppressions ORDER BY finding_key")?;
        Ok(rows
            .iter()
            .filter_map(|r| match r.first() {
                Some(DuckValue::Text(s)) => Some(s.clone()),
                _ => None,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppressions_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let st = InnerStylistStore::open_for_project(dir.path()).unwrap();
        assert!(st.all_suppressions().unwrap().is_empty());
        st.suppress("distinct:joren|mara").unwrap();
        st.suppress("tense:5:1").unwrap();
        let mut got = st.all_suppressions().unwrap();
        got.sort();
        assert_eq!(got, vec!["distinct:joren|mara".to_string(), "tense:5:1".to_string()]);
        st.unsuppress("tense:5:1").unwrap();
        assert_eq!(st.all_suppressions().unwrap(), vec!["distinct:joren|mara".to_string()]);
    }
}
