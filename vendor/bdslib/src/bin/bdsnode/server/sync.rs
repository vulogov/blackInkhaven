//! Background tokio task that periodically calls [`bdslib::sync_db`].
//!
//! `sync_db()` walks every open shard and runs DuckDB `CHECKPOINT` + Tantivy
//! `commit()` + VecStore flush + `tplstorage` HNSW save on each. Without a
//! periodic schedule this only happens at LRU shard eviction or process
//! shutdown, which means an unclean exit (`kill -9`, OOM, hardware fault)
//! can lose every write since the last eviction — typically hours of data
//! on the active shard.
//!
//! The tick runs on a `tokio::task::spawn_blocking` because the underlying
//! `cache().sync()` is synchronous and may take seconds on large WALs.
//!
//! Configured via `sync_interval_secs` in `bds.hjson`. Set to `0` to
//! disable the background task entirely (sync still runs on shard eviction
//! and at shutdown).

use tokio::sync::oneshot;
use tokio::time::Duration;

/// Configuration for the periodic global-sync background task.
///
/// | hjson key             | default | description                                                          |
/// |-----------------------|---------|----------------------------------------------------------------------|
/// | `sync_interval_secs`  | 60      | Tick cadence in seconds. `0` disables the background task entirely.  |
pub struct Config {
    pub interval_secs: u64,
}

impl Config {
    pub fn from_config(config_path: Option<&str>) -> anyhow::Result<Self> {
        let path = match config_path {
            Some(p) => p.to_string(),
            None => match std::env::var("BDS_CONFIG") {
                Ok(p) => p,
                Err(_) => return Ok(Self::default()),
            },
        };

        let raw = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("cannot read config {path:?}: {e}"))?;
        let val: serde_hjson::Value = serde_hjson::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("hjson parse error in {path:?}: {e}"))?;
        let obj = val
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("config must be a JSON object"))?;

        let interval_secs = obj
            .get("sync_interval_secs")
            .and_then(|v| v.as_f64())
            .map(|n| n as u64)
            .unwrap_or(60);

        Ok(Self { interval_secs })
    }

    fn default() -> Self {
        Self { interval_secs: 60 }
    }
}

/// Handle returned by [`start`]. Drop or call [`Handle::stop`] to terminate.
pub struct Handle {
    shutdown_tx: Option<oneshot::Sender<()>>,
    task:        Option<tokio::task::JoinHandle<()>>,
}

impl Handle {
    /// Disabled handle returned when `interval_secs == 0`.
    fn disabled() -> Self {
        Self { shutdown_tx: None, task: None }
    }

    pub async fn stop(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            if let Err(e) = task.await {
                log::error!("[sync] task panicked on shutdown: {e:?}");
            }
        }
    }
}

/// Spawn the periodic sync task. When `interval_secs == 0` the task is
/// not spawned and a no-op handle is returned (the existing
/// per-shard-eviction and shutdown sync paths still apply).
pub fn start(cfg: Config) -> Handle {
    if cfg.interval_secs == 0 {
        log::info!("[sync] disabled (sync_interval_secs=0)");
        return Handle::disabled();
    }

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task = tokio::spawn(run(cfg.interval_secs, shutdown_rx));
    log::info!(
        "[sync] started — global sync_db every {}s",
        cfg.interval_secs
    );
    Handle { shutdown_tx: Some(shutdown_tx), task: Some(task) }
}

async fn run(interval_secs: u64, mut shutdown_rx: oneshot::Receiver<()>) {
    let interval = Duration::from_secs(interval_secs);
    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown_rx => {
                log::debug!("[sync] shutdown signal received — stopping");
                break;
            }
            _ = tokio::time::sleep(interval) => {
                // Run on a blocking thread because sync_db() touches DuckDB,
                // Tantivy, and HNSW indexes synchronously and may take
                // seconds on large WALs.
                let join = tokio::task::spawn_blocking(|| {
                    let started = std::time::Instant::now();
                    if let Err(e) = bdslib::sync_db() {
                        log::warn!("[sync] tick failed: {e}");
                    } else {
                        log::debug!(
                            "[sync] tick completed in {:?}",
                            started.elapsed()
                        );
                    }
                }).await;
                if let Err(e) = join {
                    log::error!("[sync] tick task panicked: {e:?}");
                }
            }
        }
    }
}
