//! Background tokio task that runs `bdslib::Scheduler::run` periodically.
//!
//! Each tick is delegated to a blocking thread (the scheduler's calls into
//! `ShardsManager` are synchronous DuckDB queries) so the tokio runtime
//! workers stay free for RPC traffic.
//!
//! Configured via `scheduler_interval_secs` in `bds.hjson`. Set to `0` to
//! disable the background task entirely.

use tokio::sync::oneshot;
use tokio::time::Duration;

/// Configuration for the cron-driven script scheduler.
///
/// | hjson key                  | default | description                                                |
/// |----------------------------|---------|------------------------------------------------------------|
/// | `scheduler_interval_secs`  | 60      | Tick cadence. `0` disables the scheduler entirely.         |
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
            .get("scheduler_interval_secs")
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
                log::error!("[scheduler] task panicked on shutdown: {e:?}");
            }
        }
    }
}

/// Spawn the periodic scheduler. When `interval_secs == 0` the task is
/// not spawned and a no-op handle is returned.
pub fn start(cfg: Config) -> Handle {
    if cfg.interval_secs == 0 {
        log::info!("[scheduler] disabled (scheduler_interval_secs=0)");
        return Handle::disabled();
    }

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task = tokio::spawn(run(cfg.interval_secs, shutdown_rx));
    log::info!(
        "[scheduler] started — running every {}s",
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
                log::debug!("[scheduler] shutdown signal received — stopping");
                break;
            }
            _ = tokio::time::sleep(interval) => {
                // Run on a blocking thread because Scheduler::run() does
                // synchronous DuckDB lookups via ShardsManager.
                let join = tokio::task::spawn_blocking(|| {
                    let db = match bdslib::get_db() {
                        Ok(db) => db,
                        Err(e) => {
                            log::warn!("[scheduler] get_db() failed: {e}");
                            return;
                        }
                    };
                    let s = bdslib::Scheduler::new(db.clone());
                    match s.run() {
                        Ok(0)  => log::debug!("[scheduler] tick: no scripts due"),
                        Ok(n)  => log::info!("[scheduler] tick: dispatched {n} script(s)"),
                        Err(e) => log::warn!("[scheduler] tick failed: {e}"),
                    }
                }).await;
                if let Err(e) = join {
                    log::error!("[scheduler] tick task panicked: {e:?}");
                }
            }
        }
    }
}
