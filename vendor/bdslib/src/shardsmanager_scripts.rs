//! BUND script storage on top of `ShardsManager`.
//!
//! Scripts are stored in a dedicated [`DocumentStorage`] reachable via
//! `ShardsManager::scripts` (analogous to the existing `docstore` and
//! `signals` stores). Every script has:
//!
//! - **Metadata** (JSON object) that **must** contain at minimum:
//!   - `name`     — human-readable script name
//!   - `schedule` — crontab-style execution schedule string
//! - **Body** — the BUND script source code (UTF-8 text)
//!
//! The exposed `ShardsManager` methods are:
//!
//! | Method | Backing call |
//! |---|---|
//! | [`script_add`](ShardsManager::script_add) | `scripts.add_document` |
//! | [`scripts`](ShardsManager::scripts) | `scripts.meta.list_all` |
//! | [`script`](ShardsManager::script) | `scripts.get_content` |
//! | [`update_script`](ShardsManager::update_script) | `scripts.update_metadata` + `scripts.update_content` |
//! | [`script_delete`](ShardsManager::script_delete) | `scripts.delete_document` |

use crate::common::error::{err_msg, Result};
use crate::documentstorage::DocumentStorage;
use crate::shardsmanager::ShardsManager;
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Verify that script metadata has both `name` and `schedule` non-empty
/// strings. Returns the same metadata back on success for convenience.
fn verify_script_metadata(metadata: &JsonValue) -> Result<()> {
    let obj = metadata
        .as_object()
        .ok_or_else(|| err_msg("script metadata must be a JSON object"))?;

    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err_msg("script metadata must contain 'name' (string)"))?;
    if name.trim().is_empty() {
        return Err(err_msg("script metadata 'name' must not be empty"));
    }

    let schedule = obj
        .get("schedule")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err_msg("script metadata must contain 'schedule' (crontab string)"))?;
    if schedule.trim().is_empty() {
        return Err(err_msg("script metadata 'schedule' must not be empty"));
    }

    Ok(())
}

impl ShardsManager {
    /// Borrow the underlying script [`DocumentStorage`].
    pub fn scripts_store(&self) -> &DocumentStorage {
        &self.scripts
    }

    /// Store a new BUND script.
    ///
    /// `metadata` must be a JSON object with non-empty `name` and `schedule`
    /// fields; any additional fields are preserved verbatim. `script` is the
    /// raw BUND source code stored as a UTF-8 byte blob. Returns the assigned
    /// UUIDv7.
    pub fn script_add(&self, metadata: JsonValue, script: &str) -> Result<Uuid> {
        verify_script_metadata(&metadata)?;
        self.scripts.add_document(metadata, script.as_bytes())
    }

    /// List every stored script as `(uuid, schedule)` pairs.
    ///
    /// Records whose metadata is missing `schedule` (should not happen in
    /// normal operation since [`script_add`](Self::script_add) enforces it)
    /// are silently filtered out.
    ///
    /// Note: this returns the schedule string only. To also retrieve the
    /// human name, use [`scripts_with_metadata`](Self::scripts_with_metadata).
    pub fn scripts(&self) -> Result<Vec<(Uuid, String)>> {
        let mut out: Vec<(Uuid, String)> = Vec::new();
        for (id, meta) in self.scripts.list_metadata()? {
            if let Some(s) = meta.get("schedule").and_then(|v| v.as_str()) {
                out.push((id, s.to_owned()));
            }
        }
        Ok(out)
    }

    /// List every stored script as `(uuid, full_metadata)` pairs.
    ///
    /// Convenience helper that exposes the full metadata document — useful
    /// for UIs that need to display `name` alongside `schedule`.
    pub fn scripts_with_metadata(&self) -> Result<Vec<(Uuid, JsonValue)>> {
        self.scripts.list_metadata()
    }

    /// Return the BUND source code stored under `id`, or `None` if there is
    /// no script with that UUID.
    ///
    /// Bytes are decoded with `String::from_utf8_lossy` — invalid sequences
    /// are replaced with the Unicode replacement character.
    pub fn script(&self, id: Uuid) -> Result<Option<String>> {
        match self.scripts.get_content(id)? {
            Some(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            None => Ok(None),
        }
    }

    /// Return the metadata stored for the script `id`, or `None` if absent.
    pub fn script_metadata(&self, id: Uuid) -> Result<Option<JsonValue>> {
        self.scripts.get_metadata(id)
    }

    /// Replace both metadata and body of an existing script.
    ///
    /// `metadata` must again contain non-empty `name` and `schedule` fields.
    /// Both the metadata record and the content blob are updated; the vector
    /// index is left untouched (scripts are addressed by UUID, not searched
    /// semantically).
    ///
    /// Returns `Ok(())` even if `id` does not exist (matches the underlying
    /// `update_metadata` / `update_content` no-op semantics).
    pub fn update_script(
        &self,
        id: Uuid,
        metadata: JsonValue,
        script: &str,
    ) -> Result<()> {
        verify_script_metadata(&metadata)?;
        self.scripts.update_metadata(id, metadata)?;
        self.scripts.update_content(id, script.as_bytes())?;
        Ok(())
    }

    /// Remove a script from all sub-stores (metadata, blob, vector, frequency).
    ///
    /// Idempotent — succeeds even if `id` does not exist.
    pub fn script_delete(&self, id: Uuid) -> Result<()> {
        self.scripts.delete_document(id)
    }
}
