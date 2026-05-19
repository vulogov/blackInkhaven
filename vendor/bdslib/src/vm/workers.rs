//! Persistent BUND worker pool backed by the `WORKERS_PIPE` channel.
//!
//! Jobs arrive as `{"id": "<uuidv7>", "code": "<bund script>"}` JSON messages.
//! Each worker thread competes for the next message on the shared channel
//! (natural least-busy dispatch), creates an ephemeral `Bund` VM, executes
//! the script, then drains every value left on the workbench into the global
//! [`crate::vm::RESULTS`] queue under the job's `id`.
//!
//! ## Lifecycle
//!
//! ```no_run
//! use bdslib::vm::workers::BundWorkerPool;
//!
//! BundWorkerPool::start(4).expect("pool start");
//! let id = bdslib::submit_script("42 .").expect("submit");
//! // poll bdslib::vm::results().pop(id) …
//! ```

use bundcore::bundcore::Bund;
use crossbeam::channel::{self, Receiver, Sender};
use easy_error::{Error, err_msg};
use parking_lot::Mutex;
use rust_dynamic::value::Value;
use serde_json::Value as JsonValue;
use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::vm::helpers::eval::{bund_compile_and_eval, dynamic_to_json};
use crate::vm::vm::init_stdlib;

// ── observability state (used by v2/status) ───────────────────────────────────

/// Number of recent submissions kept in the ring buffer for `v2/status`.
const RECENT_SUBMISSIONS_CAP: usize = 5;

/// Map of `worker_id → currently-executing job id`. Absent key means the
/// worker is idle. Updated by [`worker_loop`] on every job entry/exit.
static RUNNING: OnceLock<Mutex<HashMap<usize, Uuid>>> = OnceLock::new();

/// Bounded ring buffer of the most-recently submitted job ids and the
/// Unix-seconds timestamp at which they were enqueued.  Holds at most
/// [`RECENT_SUBMISSIONS_CAP`] entries; older entries are evicted FIFO.
static RECENT_SUBMISSIONS: OnceLock<Mutex<VecDeque<(Uuid, u64)>>> = OnceLock::new();

fn running_map() -> &'static Mutex<HashMap<usize, Uuid>> {
    RUNNING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn recent_buf() -> &'static Mutex<VecDeque<(Uuid, u64)>> {
    RECENT_SUBMISSIONS.get_or_init(|| Mutex::new(VecDeque::with_capacity(RECENT_SUBMISSIONS_CAP)))
}

/// Snapshot of every worker currently executing a job, sorted by worker id.
///
/// Used by `v2/status` to report which BUND scripts are running right now.
pub fn running_snapshot() -> Vec<(usize, Uuid)> {
    let map = running_map().lock();
    let mut v: Vec<(usize, Uuid)> = map.iter().map(|(k, v)| (*k, *v)).collect();
    v.sort_by_key(|(k, _)| *k);
    v
}

/// Most-recent-first list of `(job_id, submitted_at_unix_secs)` pairs.
///
/// Capped at [`RECENT_SUBMISSIONS_CAP`] entries.
pub fn recent_submissions() -> Vec<(Uuid, u64)> {
    recent_buf().lock().iter().rev().copied().collect()
}

fn record_submission(id: Uuid) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut buf = recent_buf().lock();
    if buf.len() == RECENT_SUBMISSIONS_CAP {
        buf.pop_front();
    }
    buf.push_back((id, now));
}

fn mark_running(worker_id: usize, job_id: Uuid) {
    running_map().lock().insert(worker_id, job_id);
}

fn mark_idle(worker_id: usize) {
    running_map().lock().remove(&worker_id);
}

/// RAII guard so `mark_idle` runs even if `bund_compile_and_eval` panics.
struct RunGuard(usize);
impl Drop for RunGuard {
    fn drop(&mut self) {
        mark_idle(self.0);
    }
}

// ── static pipe ───────────────────────────────────────────────────────────────

/// Sender side of the worker-pool input channel.
///
/// Populated by [`BundWorkerPool::start`].  External code should prefer the
/// [`submit_script`] helper, but direct access is available for embedding the
/// channel in other MPMC topologies.
pub static WORKERS_PIPE: OnceLock<Sender<JsonValue>> = OnceLock::new();

// ── types ─────────────────────────────────────────────────────────────────────

/// A single background worker thread that pulls jobs from the shared receiver.
pub struct BundWorker {
    _handle: thread::JoinHandle<()>,
}

/// A pool of [`BundWorker`] threads sharing one input channel.
///
/// Workers compete for jobs naturally — an idle worker picks up the next
/// pending job, giving least-busy dispatch semantics without explicit tracking.
pub struct BundWorkerPool {
    workers: Vec<BundWorker>,
}

// ── implementation ────────────────────────────────────────────────────────────

impl BundWorkerPool {
    /// Spawn `n_workers` threads and publish the channel sender into
    /// [`WORKERS_PIPE`].  Returns `Err` if called a second time.
    pub fn start(n_workers: usize) -> Result<BundWorkerPool, Error> {
        let (tx, rx) = channel::unbounded::<JsonValue>();
        WORKERS_PIPE
            .set(tx)
            .map_err(|_| err_msg("BundWorkerPool already initialised"))?;

        let workers = (0..n_workers)
            .map(|i| {
                let rx = rx.clone();
                let handle = thread::Builder::new()
                    .name(format!("bund-worker-{i}"))
                    .spawn(move || worker_loop(i, rx))
                    .expect("bund-worker thread spawn");
                BundWorker { _handle: handle }
            })
            .collect();

        Ok(BundWorkerPool { workers })
    }

    /// Number of worker threads in this pool.
    pub fn n_workers(&self) -> usize {
        self.workers.len()
    }
}

fn worker_loop(worker_id: usize, rx: Receiver<JsonValue>) {
    while let Ok(msg) = rx.recv() {
        let Some(id_str) = msg.get("id").and_then(|v| v.as_str()) else {
            log::warn!("[bund-worker] message missing 'id' field; skipping");
            continue;
        };
        let id = match Uuid::try_parse(id_str) {
            Ok(u) => u,
            Err(e) => {
                log::warn!("[bund-worker] invalid uuid {id_str:?}: {e}");
                continue;
            }
        };
        let Some(code) = msg.get("code").and_then(|v| v.as_str()) else {
            log::warn!("[bund-worker] message missing 'code' field for id={id}");
            continue;
        };
        let code = code.to_string();

        // Mark this worker as actively processing `id` until the iteration
        // completes — the RAII guard clears the slot even on early `continue`
        // or a panic inside the eval call.
        mark_running(worker_id, id);
        let _guard = RunGuard(worker_id);

        let mut bund = Bund::new();
        if let Err(e) = init_stdlib(&mut bund) {
            log::error!("[bund-worker] stdlib init failed for id={id}: {e}");
            continue;
        }

        match bund_compile_and_eval(&mut bund.vm, code) {
            Err(e) => log::error!("[bund-worker] eval error for id={id}: {e}"),
            Ok(_) => {
                let results = crate::vm::results();
                while let Some(raw) = bund.vm.stack.pull_from_workbench() {
                    results.push(id, Value::json(dynamic_to_json(raw)));
                }
            }
        }
    }
}

// ── public helpers ────────────────────────────────────────────────────────────

/// Enqueue `{"id": id, "code": script}` in the worker pool using a
/// caller-supplied UUID, and return the same id.
///
/// Useful for submissions where the queue id must match an external key —
/// e.g. the scheduler reuses the script's storage UUID so that all results
/// for a given scheduled script accumulate under one well-known queue id.
///
/// Poll [`crate::vm::results()`]`.pop(id)` to retrieve results once the
/// worker has finished executing.
///
/// Returns `Err` if [`BundWorkerPool::start`] has not been called.
pub fn submit_script_with_id(id: Uuid, script: &str) -> Result<Uuid, Error> {
    let tx = WORKERS_PIPE
        .get()
        .ok_or_else(|| err_msg("BundWorkerPool not initialised; call BundWorkerPool::start() first"))?;
    let msg = serde_json::json!({ "id": id.to_string(), "code": script });
    tx.send(msg).map_err(|e| err_msg(e.to_string()))?;
    record_submission(id);
    Ok(id)
}

/// Generate a fresh UUIDv7 and enqueue the script — convenience wrapper
/// around [`submit_script_with_id`] for callers that don't already have an id.
pub fn submit_script(script: &str) -> Result<Uuid, Error> {
    submit_script_with_id(Uuid::now_v7(), script)
}
