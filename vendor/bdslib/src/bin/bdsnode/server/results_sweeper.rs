//! Background sweeper for the per-id [`ResultQueue`] singleton.
//!
//! Once per `sweep_interval_secs`, scans every queue currently registered in
//! [`bdslib::vm::RESULTS`] and drops queues whose creation timestamp is older
//! than `ttl_secs` from now.  Configured via `results_ttl_secs` and
//! `results_sweep_secs` in `bds.hjson`.

use tokio::sync::oneshot;
use tokio::time::Duration;

/// Configuration for the result-queue sweeper.
///
/// | hjson key             | default | description                                          |
/// |-----------------------|---------|------------------------------------------------------|
/// | `results_ttl_secs`    | 600     | Age (Unix-seconds) above which a queue is evicted.   |
/// | `results_sweep_secs`  | 30      | Interval between sweeps. Ignored when `ttl_secs==0`. |
pub struct Config {
    pub ttl_secs:            u64,
    pub sweep_interval_secs: u64,
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

        let ttl_secs = obj
            .get("results_ttl_secs")
            .and_then(|v| v.as_f64())
            .map(|n| n as u64)
            .unwrap_or(600);
        let sweep_interval_secs = obj
            .get("results_sweep_secs")
            .and_then(|v| v.as_f64())
            .map(|n| n as u64)
            .unwrap_or(30)
            .max(1);

        Ok(Self { ttl_secs, sweep_interval_secs })
    }

    fn default() -> Self {
        Self { ttl_secs: 600, sweep_interval_secs: 30 }
    }
}

/// Handle returned by [`start`].  Drop or call [`Handle::stop`] to terminate.
pub struct Handle {
    shutdown_tx: oneshot::Sender<()>,
    task:        tokio::task::JoinHandle<()>,
}

impl Handle {
    pub async fn stop(self) {
        let _ = self.shutdown_tx.send(());
        if let Err(e) = self.task.await {
            log::error!("[results_sweeper] task panicked on shutdown: {e:?}");
        }
    }
}

/// Spawn the result-queue TTL sweeper.  No-op when `ttl_secs == 0`
/// (background task still spawns but `sweep_expired` short-circuits).
pub fn start(cfg: Config) -> Handle {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task = tokio::spawn(run(cfg.ttl_secs, cfg.sweep_interval_secs, shutdown_rx));
    Handle { shutdown_tx, task }
}

async fn run(ttl_secs: u64, sweep_interval_secs: u64, mut shutdown_rx: oneshot::Receiver<()>) {
    log::debug!(
        "[results_sweeper] started (ttl={}s interval={}s)",
        ttl_secs, sweep_interval_secs
    );
    let interval = Duration::from_secs(sweep_interval_secs);
    let queues = bdslib::vm::results();

    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown_rx => {
                log::debug!("[results_sweeper] shutdown signal received — stopping");
                break;
            }
            _ = tokio::time::sleep(interval) => {
                let evicted = queues.sweep_expired(ttl_secs);
                if evicted > 0 {
                    log::debug!("[results_sweeper] evicted {evicted} expired queue(s)");
                }
            }
        }
    }
}
