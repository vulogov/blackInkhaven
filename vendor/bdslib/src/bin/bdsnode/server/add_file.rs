use crossbeam::channel::{Receiver, Sender, bounded};
use serde_json::Value as JsonValue;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Configuration for the file-ingestion thread.
///
/// | Key                | Type    | Default | Description |
/// |--------------------|---------|---------|-------------|
/// | `file_batch_size`  | integer | 100     | Records per batch before flushing to the shard store. |
/// | `file_timeout_ms`  | integer | 5000    | Milliseconds of channel inactivity before a partial batch is flushed. |
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
            .get("file_batch_size")
            .and_then(|v| v.as_f64())
            .map(|n| n as usize)
            .unwrap_or(100)
            .max(1);

        let timeout_ms = obj
            .get("file_timeout_ms")
            .and_then(|v| v.as_f64())
            .map(|n| n as u64)
            .unwrap_or(5000)
            .max(1);

        Ok(Some(Config {
            batch_size,
            timeout_ms,
        }))
    }
}

/// Handle returned by [`start`].
///
/// Call [`Handle::stop`] on server shutdown to drain remaining file paths from
/// the `"ingest_file"` channel and join the thread before calling `sync_db`.
pub struct Handle {
    shutdown_tx: Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Handle {
    /// Signal the thread to drain the `"ingest_file"` channel and exit, then
    /// block until it finishes.
    pub fn stop(mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(t) = self.thread.take() {
            if let Err(e) = t.join() {
                log::error!("[add_file] thread panicked on shutdown: {e:?}");
            }
        }
    }
}

/// Spawn the file-ingestion thread and return a [`Handle`] for graceful shutdown.
///
/// The thread drains the `"ingest_file"` crossbeam channel, which carries
/// file paths as JSON strings.  For each path it reads the file line-by-line
/// using [`bdslib::common::logparser::ingest_file`], parsing each line as a
/// JSON telemetry document (same format as [`ShardsManager::add`]).  Parsed
/// documents are accumulated into batches and flushed via
/// [`ShardsManager::add_batch`] when either `batch_size` records are queued
/// or `timeout_ms` milliseconds pass with no new file.
///
/// `current_file` is updated to `Some(path)` while a file is being processed
/// and reset to `None` when processing completes.  Pass
/// `crate::status::get().current_file.clone()` from the main thread.
pub fn start(cfg: Config, current_file: Arc<Mutex<Option<String>>>) -> Handle {
    let (shutdown_tx, shutdown_rx) = bounded(1);
    let timeout = Duration::from_millis(cfg.timeout_ms);
    let thread = std::thread::Builder::new()
        .name("bds-add-file".to_string())
        .spawn(move || run(cfg.batch_size, timeout, shutdown_rx, current_file))
        .expect("failed to spawn bds-add-file thread");
    Handle {
        shutdown_tx,
        thread: Some(thread),
    }
}

fn run(
    batch_size:   usize,
    timeout:      Duration,
    shutdown_rx:  Receiver<()>,
    current_file: Arc<Mutex<Option<String>>>,
) {
    log::debug!(
        "[add_file] started (batch_size={batch_size}, timeout={}ms)",
        timeout.as_millis()
    );

    let ingest_rx = match bdslib::pipe::receiver("ingest_file") {
        Ok(r) => r,
        Err(e) => {
            log::error!("[add_file] cannot access ingest_file channel: {e}");
            return;
        }
    };

    let mut batch: Vec<JsonValue> = Vec::with_capacity(batch_size);
    let mut total_records: u64 = 0;
    let run_start = Instant::now();
    let mut batch_start = Instant::now();

    loop {
        crossbeam::select! {
            recv(ingest_rx) -> msg => {
                match msg {
                    Ok(path_val) => {
                        if let Some(path) = path_val.as_str() {
                            process_file(
                                path,
                                batch_size,
                                &mut batch,
                                &mut total_records,
                                &mut batch_start,
                                &current_file,
                            );
                        } else {
                            log::warn!("[add_file] received non-string value on ingest_file channel, skipping");
                        }
                    }
                    Err(_) => break,
                }
            }
            recv(shutdown_rx) -> _ => {
                // Drain every file path still in the channel before exiting.
                while let Ok(path_val) = ingest_rx.try_recv() {
                    if let Some(path) = path_val.as_str() {
                        process_file(
                            path,
                            batch_size,
                            &mut batch,
                            &mut total_records,
                            &mut batch_start,
                            &current_file,
                        );
                    }
                }
                if !batch.is_empty() {
                    total_records += flush(&mut batch) as u64;
                }
                let elapsed = run_start.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    log::debug!(
                        "[add_file] shutdown complete — {total_records} records in {elapsed:.1}s ({:.1} avg records/s)",
                        total_records as f64 / elapsed
                    );
                } else {
                    log::debug!("[add_file] shutdown complete — {total_records} records");
                }
                break;
            }
            default(timeout) => {
                if !batch.is_empty() {
                    total_records += flush(&mut batch) as u64;
                    batch_start = Instant::now();
                }
            }
        }
    }
}

/// Read `path` line-by-line, parse each non-empty line as a JSON telemetry
/// document, and append to `batch`.  Flushes the batch whenever it reaches
/// `batch_size`.  Sets `current_file` to `Some(path)` for the duration of
/// processing and resets it to `None` before returning.
fn process_file(
    path:         &str,
    batch_size:   usize,
    batch:        &mut Vec<JsonValue>,
    total_records: &mut u64,
    batch_start:  &mut Instant,
    current_file: &Arc<Mutex<Option<String>>>,
) {
    if let Ok(mut g) = current_file.lock() {
        *g = Some(path.to_string());
    }
    log::debug!("[add_file] ingesting {path:?}");

    let mut file_docs: Vec<JsonValue> = Vec::new();

    let parse_json = |line: &str| -> bdslib::common::error::Result<JsonValue> {
        let doc: JsonValue = serde_json::from_str(line)
            .map_err(|e| bdslib::common::error::err_msg(format!("JSON parse error: {e}")))?;
        bdslib::common::logparser::validate_telemetry(&doc)?;
        Ok(doc)
    };

    if let Err(e) =
        bdslib::common::logparser::ingest_file(parse_json, |doc| file_docs.push(doc), path)
    {
        log::warn!("[add_file] error reading {path:?}: {e}");
        if let Ok(mut g) = current_file.lock() { *g = None; }
        return;
    }

    log::debug!(
        "[add_file] parsed {} records from {path:?}",
        file_docs.len()
    );

    for doc in file_docs {
        batch.push(doc);
        if batch.len() >= batch_size {
            let n = flush(batch) as u64;
            *total_records += n;
            let batch_secs = batch_start.elapsed().as_secs_f64();
            *batch_start = Instant::now();
            if batch_secs > 0.0 {
                log::debug!(
                    "[add_file] throughput: {:.1} records/s ({total_records} total)",
                    n as f64 / batch_secs
                );
            }
        }
    }

    if let Ok(mut g) = current_file.lock() { *g = None; }
}

fn flush(batch: &mut Vec<JsonValue>) -> usize {
    let docs = std::mem::take(batch);
    let n = docs.len();
    match bdslib::get_db().and_then(|db| db.add_batch(docs)) {
        Ok(ids) => {
            log::debug!("[add_file] flushed {n} records ({} stored)", ids.len());
            n
        }
        Err(e) => {
            log::error!("[add_file] add_batch error: {e}");
            0
        }
    }
}
