use crossbeam::channel::{bounded, Receiver, Sender};
use std::time::Duration;

/// Configuration for the batch-ingestion thread.
///
/// | Key               | Type    | Default | Description |
/// |-------------------|---------|---------|-------------|
/// | `pipe_batch_size` | integer | 500     | Records per batch before flushing to the shard store. |
/// | `pipe_timeout_ms` | integer | 500     | Milliseconds of channel inactivity before a partial batch is flushed. |
///
/// The defaults trade interactive latency (~500ms worst case for a
/// trickle of records) for throughput (large batches amortise the
/// Tantivy-commit / DuckDB-transaction / ONNX-batch costs).  Lower
/// `pipe_timeout_ms` for a more interactive feel; raise
/// `pipe_batch_size` if your workload is consistently dense.
pub struct Config {
    pub batch_size: usize,
    pub timeout_ms: u64,
}

impl Config {
    /// Parse settings from the hjson config file.
    ///
    /// Returns `Ok(None)` only when no config path is available.
    pub fn from_config(config_path: Option<&str>) -> anyhow::Result<Option<Self>> {
        let path = match config_path {
            Some(p) => p.to_string(),
            None => match std::env::var("BDS_CONFIG") {
                Ok(p) => p,
                Err(_) => return Ok(None),
            },
        };

        let raw = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("cannot read config {path:?}: {e}"))?;
        let val: serde_hjson::Value = serde_hjson::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("hjson parse error in {path:?}: {e}"))?;
        let obj = val
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("config must be a JSON object"))?;

        let batch_size = obj
            .get("pipe_batch_size")
            .and_then(|v| v.as_f64())
            .map(|n| n as usize)
            .unwrap_or(500)
            .max(1);

        let timeout_ms = obj
            .get("pipe_timeout_ms")
            .and_then(|v| v.as_f64())
            .map(|n| n as u64)
            .unwrap_or(500)
            .max(1);

        Ok(Some(Config { batch_size, timeout_ms }))
    }
}

/// Handle returned by [`start`].
///
/// Call [`Handle::stop`] on server shutdown to drain remaining records from
/// the `"ingest"` channel and join the thread before calling `sync_db`.
pub struct Handle {
    shutdown_tx: Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Handle {
    /// Signal the thread to drain the `"ingest"` channel and exit, then block
    /// until it finishes.
    pub fn stop(mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(t) = self.thread.take() {
            if let Err(e) = t.join() {
                log::error!("[add] thread panicked on shutdown: {e:?}");
            }
        }
    }
}

/// Spawn the batch-ingestion thread and return a [`Handle`] for graceful shutdown.
///
/// The thread drains the `"ingest"` crossbeam channel, accumulates records into
/// batches, and calls [`ShardsManager::add_batch`] when either `batch_size`
/// records are queued or `timeout_ms` milliseconds pass with no new records.
///
/// On [`Handle::stop`]: any records still in the channel are flushed before
/// the thread exits, ensuring no data is lost on server shutdown.
pub fn start(cfg: Config) -> Handle {
    let (shutdown_tx, shutdown_rx) = bounded(1);
    let timeout = Duration::from_millis(cfg.timeout_ms);
    let thread = std::thread::Builder::new()
        .name("bds-add".to_string())
        .spawn(move || run(cfg.batch_size, timeout, shutdown_rx))
        .expect("failed to spawn bds-add thread");
    Handle { shutdown_tx, thread: Some(thread) }
}

fn run(batch_size: usize, timeout: Duration, shutdown_rx: Receiver<()>) {
    log::debug!(
        "[add] started (batch_size={batch_size}, timeout={}ms)",
        timeout.as_millis()
    );

    let ingest_rx = match bdslib::pipe::receiver("ingest") {
        Ok(r) => r,
        Err(e) => {
            log::error!("[add] cannot access ingest channel: {e}");
            return;
        }
    };

    let mut batch: Vec<serde_json::Value> = Vec::with_capacity(batch_size);
    let mut total_records: u64 = 0;
    let run_start = std::time::Instant::now();
    let mut batch_start = std::time::Instant::now();

    loop {
        crossbeam::select! {
            recv(ingest_rx) -> msg => {
                match msg {
                    Ok(doc) => {
                        batch.push(doc);
                        if batch.len() >= batch_size {
                            let n = flush(&mut batch) as u64;
                            total_records += n;
                            let batch_secs = batch_start.elapsed().as_secs_f64();
                            batch_start = std::time::Instant::now();
                            if batch_secs > 0.0 {
                                log::debug!(
                                    "[add] throughput: {:.1} records/s ({total_records} total)",
                                    n as f64 / batch_secs
                                );
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            recv(shutdown_rx) -> _ => {
                // Drain every record still sitting in the channel before exiting.
                while let Ok(doc) = ingest_rx.try_recv() {
                    batch.push(doc);
                    if batch.len() >= batch_size {
                        total_records += flush(&mut batch) as u64;
                    }
                }
                if !batch.is_empty() {
                    total_records += flush(&mut batch) as u64;
                }
                let elapsed = run_start.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    log::debug!(
                        "[add] shutdown complete — {total_records} records in {elapsed:.1}s ({:.1} avg records/s)",
                        total_records as f64 / elapsed
                    );
                } else {
                    log::debug!("[add] shutdown complete — {total_records} records");
                }
                break;
            }
            default(timeout) => {
                if !batch.is_empty() {
                    total_records += flush(&mut batch) as u64;
                    batch_start = std::time::Instant::now();
                }
            }
        }
    }
}

fn flush(batch: &mut Vec<serde_json::Value>) -> usize {
    let docs = std::mem::take(batch);
    let n = docs.len();
    match bdslib::get_db().and_then(|db| db.add_batch(docs)) {
        Ok(ids) => {
            log::debug!("[add] flushed {n} records ({} stored)", ids.len());
            n
        }
        Err(e) => {
            log::error!("[add] add_batch error: {e}");
            0
        }
    }
}
