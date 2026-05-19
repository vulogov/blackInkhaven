//! Ephemeral BUND worker pool backed by the `EPHEMERAL_PIPE` channel.
//!
//! Functionally identical to [`crate::vm::workers`] but uses separate type
//! names (`WorkerPool` / `Worker`) and a separate static channel
//! (`EPHEMERAL_PIPE`), so two independent pools can coexist in one process.
//!
//! Jobs arrive as `{"id": "<uuidv7>", "code": "<bund script>"}` JSON messages.
//! Each `Worker` thread creates a fresh `Bund` VM for every job, executes the
//! script, and drains the workbench into [`crate::vm::RESULTS`].
//!
//! ## Lifecycle
//!
//! ```no_run
//! use bdslib::vm::ephemeral::WorkerPool;
//!
//! WorkerPool::start(4).expect("pool start");
//! // enqueue via EPHEMERAL_PIPE or wrap in your own submit helper
//! ```

use bundcore::bundcore::Bund;
use crossbeam::channel::{self, Receiver, Sender};
use easy_error::{Error, err_msg};
use rust_dynamic::value::Value;
use serde_json::Value as JsonValue;
use std::sync::OnceLock;
use std::thread;
use uuid::Uuid;

use crate::vm::helpers::eval::{bund_compile_and_eval, dynamic_to_json};
use crate::vm::vm::init_stdlib;

// ── static pipe ───────────────────────────────────────────────────────────────

/// Sender side of the ephemeral worker-pool input channel.
///
/// Populated by [`WorkerPool::start`].
pub static EPHEMERAL_PIPE: OnceLock<Sender<JsonValue>> = OnceLock::new();

// ── types ─────────────────────────────────────────────────────────────────────

/// A single background worker thread belonging to a [`WorkerPool`].
pub struct Worker {
    _handle: thread::JoinHandle<()>,
}

/// A pool of [`Worker`] threads sharing one ephemeral input channel.
pub struct WorkerPool {
    workers: Vec<Worker>,
}

// ── implementation ────────────────────────────────────────────────────────────

impl WorkerPool {
    /// Spawn `n_workers` threads and publish the channel sender into
    /// [`EPHEMERAL_PIPE`].  Returns `Err` if called a second time.
    pub fn start(n_workers: usize) -> Result<WorkerPool, Error> {
        let (tx, rx) = channel::unbounded::<JsonValue>();
        EPHEMERAL_PIPE
            .set(tx)
            .map_err(|_| err_msg("WorkerPool already initialised"))?;

        let workers = (0..n_workers)
            .map(|i| {
                let rx = rx.clone();
                let handle = thread::Builder::new()
                    .name(format!("bund-ephemeral-{i}"))
                    .spawn(move || worker_loop(rx))
                    .expect("bund-ephemeral thread spawn");
                Worker { _handle: handle }
            })
            .collect();

        Ok(WorkerPool { workers })
    }

    /// Number of worker threads in this pool.
    pub fn n_workers(&self) -> usize {
        self.workers.len()
    }
}

fn worker_loop(rx: Receiver<JsonValue>) {
    while let Ok(msg) = rx.recv() {
        let Some(id_str) = msg.get("id").and_then(|v| v.as_str()) else {
            log::warn!("[bund-ephemeral] message missing 'id' field; skipping");
            continue;
        };
        let id = match Uuid::try_parse(id_str) {
            Ok(u) => u,
            Err(e) => {
                log::warn!("[bund-ephemeral] invalid uuid {id_str:?}: {e}");
                continue;
            }
        };
        let Some(code) = msg.get("code").and_then(|v| v.as_str()) else {
            log::warn!("[bund-ephemeral] message missing 'code' field for id={id}");
            continue;
        };
        let code = code.to_string();

        let mut bund = Bund::new();
        if let Err(e) = init_stdlib(&mut bund) {
            log::error!("[bund-ephemeral] stdlib init failed for id={id}: {e}");
            continue;
        }

        match bund_compile_and_eval(&mut bund.vm, code) {
            Err(e) => log::error!("[bund-ephemeral] eval error for id={id}: {e}"),
            Ok(_) => {
                let results = crate::vm::results();
                while let Some(raw) = bund.vm.stack.pull_from_workbench() {
                    results.push(id, Value::json(dynamic_to_json(raw)));
                }
            }
        }
    }
}

// ── public helper ─────────────────────────────────────────────────────────────

/// Generate a UUIDv7, enqueue `{"id": ..., "code": script}` in the ephemeral
/// pool, and return the id.
///
/// Returns `Err` if [`WorkerPool::start`] has not been called.
pub fn submit_ephemeral(script: &str) -> Result<Uuid, Error> {
    let tx = EPHEMERAL_PIPE
        .get()
        .ok_or_else(|| err_msg("WorkerPool not initialised; call WorkerPool::start() first"))?;
    let id = Uuid::now_v7();
    let msg = serde_json::json!({ "id": id.to_string(), "code": script });
    tx.send(msg).map_err(|e| err_msg(e.to_string()))?;
    Ok(id)
}
