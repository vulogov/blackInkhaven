use tokio::sync::oneshot;
use tokio::time::Duration;

/// Configuration for the BUND-VM cleanup task.
///
/// | hjson key                  | default | description                           |
/// |----------------------------|---------|---------------------------------------|
/// | `vm_cleanup_interval_secs` | 60      | How often to scan for stale BUND VMs. |
pub struct Config {
    pub scan_interval_secs: u64,
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

        let scan_interval_secs = obj
            .get("vm_cleanup_interval_secs")
            .and_then(|v| v.as_f64())
            .map(|n| n as u64)
            .unwrap_or(60)
            .max(1);

        Ok(Self { scan_interval_secs })
    }

    fn default() -> Self {
        Self { scan_interval_secs: 60 }
    }
}

/// Handle returned by [`start`].
///
/// Call [`Handle::stop`] during server shutdown to cancel the task and wait for
/// it to finish before calling [`vm_close`].
pub struct Handle {
    shutdown_tx: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl Handle {
    /// Send the shutdown signal and await task completion.
    pub async fn stop(self) {
        let _ = self.shutdown_tx.send(());
        if let Err(e) = self.task.await {
            log::error!("[bundcleanup] task panicked on shutdown: {e:?}");
        }
    }
}

/// Spawn the BUND-VM cleanup task and return a [`Handle`] for graceful shutdown.
///
/// The task sleeps `scan_interval_secs` seconds between scans, evicts every
/// BUND VM whose idle time exceeds the TTL configured via `bund_ttl_secs`,
/// and logs each removal individually. It exits immediately when the shutdown
/// signal arrives, without waiting for the next scan interval to elapse.
pub fn start(cfg: Config) -> Handle {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task = tokio::spawn(run(cfg.scan_interval_secs, shutdown_rx));
    Handle { shutdown_tx, task }
}

async fn run(scan_interval_secs: u64, mut shutdown_rx: oneshot::Receiver<()>) {
    log::debug!("[bundcleanup] started (interval={}s)", scan_interval_secs);
    let interval = Duration::from_secs(scan_interval_secs);

    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown_rx => {
                log::debug!("[bundcleanup] shutdown signal received — stopping");
                break;
            }
            _ = tokio::time::sleep(interval) => {
                for name in bdslib::context::evict_stale() {
                    log::debug!("[bundcleanup] evicted stale BUND VM {name:?}");
                }
            }
        }
    }
}

/// Terminate every BUND VM currently in the registry.
///
/// Intended to be called once during server shutdown, after [`Handle::stop`],
/// to ensure no VM instances outlive the process. Each terminated VM is logged
/// individually at `debug` level.
pub fn vm_close() {
    let names = bdslib::context::close_all();
    for name in &names {
        log::debug!("[bundcleanup] vm_close: terminated BUND VM {name:?}");
    }
    if !names.is_empty() {
        log::debug!("[bundcleanup] vm_close: {} BUND VM(s) shut down", names.len());
    }
}
