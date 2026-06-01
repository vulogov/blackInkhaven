use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{Error, Result};

pub const CONFIG_FILE: &str = "inkhaven.hjson";
pub const METADATA_DB: &str = "metadata.db";
/// bdslib stores HNSW indexes here. We expose the path so `inkhaven init`
/// can print it, but we never create or write to it ourselves — bdslib does.
pub const VECSTORE_DIR: &str = "vectors";
pub const BOOKS_DIR: &str = "books";
pub const PROMPTS_FILE_DEFAULT: &str = "prompts.hjson";

#[derive(Debug, Clone)]
pub struct ProjectLayout {
    pub root: PathBuf,
}

impl ProjectLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join(CONFIG_FILE)
    }

    pub fn metadata_db_path(&self) -> PathBuf {
        self.root.join(METADATA_DB)
    }

    pub fn vecstore_path(&self) -> PathBuf {
        self.root.join(VECSTORE_DIR)
    }

    pub fn books_path(&self) -> PathBuf {
        self.root.join(BOOKS_DIR)
    }

    /// 1.2.15+ Phase S.6 (H4) — resolve
    /// `cfg.prompts_file` to an absolute path while
    /// preventing `..` traversal in relative paths.
    /// Absolute paths are honoured as documented
    /// (the project owner may want to point at a
    /// shared prompts library) — when opening an
    /// untrusted project, the S.6.H1 project-trust
    /// prompt is the gate, not path safety.
    ///
    /// On rejection of a `..`-escaping relative
    /// path, falls back to the default
    /// `prompts.hjson` under the project root + logs
    /// a warning.  We never crash the editor over a
    /// bad config field — the user gets a status
    /// message + a safe default.
    pub fn prompts_path(&self, cfg: &Config) -> PathBuf {
        match crate::path_safety::resolve_within_or_absolute(&self.root, &cfg.prompts_file) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::security",
                    "prompts_file `{}` rejected ({e}); falling back to default `prompts.hjson` under project root",
                    cfg.prompts_file.display(),
                );
                self.root.join("prompts.hjson")
            }
        }
    }

    /// bdslib's DocumentStorage root. We use the project root directly so the
    /// `metadata.db` and `vecstore/` files end up adjacent to `books/`, matching
    /// the spec.
    pub fn store_root(&self) -> &Path {
        &self.root
    }

    pub fn is_initialized(&self) -> bool {
        self.config_path().is_file()
    }

    pub fn require_initialized(&self) -> Result<()> {
        if self.is_initialized() {
            Ok(())
        } else {
            Err(Error::ProjectNotFound(self.root.clone()))
        }
    }

    pub fn create_layout(&self) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.books_path())?;
        // bdslib creates `metadata.db`, `blobs.db`, `frequency.db`, and the
        // `vectors/` HNSW directory on first store open — we don't touch them.
        Ok(())
    }
}
