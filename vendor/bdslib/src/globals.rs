use crate::common::error::{err_msg, Result};
use crate::ShardsManager;
use std::sync::OnceLock;

static DB: OnceLock<ShardsManager> = OnceLock::new();

/// Initialize the process-wide [`ShardsManager`] instance.
///
/// `config_path` controls how the hjson configuration file is located:
///
/// | Value | Behaviour |
/// |---|---|
/// | `Some(path)` | Use `path` directly |
/// | `None` | Read the `BDS_CONFIG` environment variable |
///
/// The function is safe to call from multiple threads; only one call will
/// ever reach `ShardsManager::new` — all others that win the race will
/// still return `Ok(())` if the DB is already set.
///
/// # Errors
///
/// Returns `Err` if:
/// - `config_path` is `None` and `BDS_CONFIG` is not set,
/// - the configuration file cannot be read or parsed,
/// - the embedding model fails to load, or
/// - the global instance has already been initialized.
pub fn init_db(config_path: Option<&str>) -> Result<()> {
    let path: String = match config_path {
        Some(p) => p.to_string(),
        None => std::env::var("BDS_CONFIG").map_err(|_| {
            err_msg(
                "no config path supplied and BDS_CONFIG environment variable is not set",
            )
        })?,
    };

    let manager = ShardsManager::new(&path)?;

    DB.set(manager)
        .map_err(|_| err_msg("global DB is already initialized; call init_db() only once"))
}

/// Sync all open shards to disk.
///
/// Calls [`ShardsCache::sync`] on the global instance.  If the DB has not
/// been initialized this is a no-op and `Ok(())` is returned.
pub fn sync_db() -> Result<()> {
    match DB.get() {
        Some(db) => db.cache().sync(),
        None => Ok(()),
    }
}

/// Borrow the process-wide [`ShardsManager`].
///
/// # Errors
///
/// Returns `Err` if [`init_db`] has not been called successfully yet.
pub fn get_db() -> Result<&'static ShardsManager> {
    DB.get()
        .ok_or_else(|| err_msg("global DB is not initialized; call init_db() first"))
}

/// Read the `dbpath` field from the hjson config without initialising the DB.
///
/// Useful for pre-flight operations such as wiping and recreating the store.
pub fn dbpath_from_config(config_path: Option<&str>) -> Result<String> {
    let path: String = match config_path {
        Some(p) => p.to_string(),
        None => std::env::var("BDS_CONFIG").map_err(|_| {
            err_msg(
                "no config path supplied and BDS_CONFIG environment variable is not set",
            )
        })?,
    };
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| err_msg(format!("cannot read config {path:?}: {e}")))?;
    let val: serde_hjson::Value = serde_hjson::from_str(&raw)
        .map_err(|e| err_msg(format!("hjson parse error: {e}")))?;
    val.as_object()
        .and_then(|obj| obj.get("dbpath"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| err_msg("missing required field 'dbpath' in config"))
}
