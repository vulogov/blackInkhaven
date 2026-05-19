//! Per-id FIFO queues of [`rust_dynamic::value::Value`].
//!
//! Each queue is keyed by a UUIDv7, carries a creation timestamp (Unix
//! seconds), and stores values in insertion order.  Designed for short-lived
//! result-passing between a producer (e.g. an async task posting results
//! into bdsnode) and a consumer (a client polling `v2/results.pull`).
//!
//! All state lives behind a single `Mutex` — the queue grain is small enough
//! that contention is negligible for the intended workload (a handful of
//! producers, a handful of consumers).
//!
//! Expiration is opportunistic: callers must invoke
//! [`ResultQueue::sweep_expired`] periodically (bdsnode does this from a
//! background tokio task driven by the `results_ttl_secs` config field).

use parking_lot::Mutex;
use rust_dynamic::value::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug)]
struct Entry {
    /// Unix seconds when this queue was first created via [`ResultQueue::push`].
    created_at: u64,
    /// FIFO of values waiting to be pulled.
    queue: VecDeque<Value>,
}

/// Hashtable of per-id FIFO queues with creation timestamps.
///
/// Cheap to clone — all clones share the same underlying state.
#[derive(Clone, Default)]
pub struct ResultQueue {
    inner: Arc<Mutex<HashMap<Uuid, Entry>>>,
}

impl ResultQueue {
    /// Create an empty `ResultQueue`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push `value` onto the back of the FIFO for `id`.
    ///
    /// Auto-creates the queue (and stamps it with the current Unix time)
    /// when no queue exists for `id`.
    pub fn push(&self, id: Uuid, value: Value) {
        let now = now_unix_secs();
        let mut map = self.inner.lock();
        let entry = map.entry(id).or_insert_with(|| Entry {
            created_at: now,
            queue: VecDeque::new(),
        });
        entry.queue.push_back(value);
    }

    /// Length of the queue for `id`, or `0` when no queue exists.
    pub fn len(&self, id: Uuid) -> usize {
        self.inner
            .lock()
            .get(&id)
            .map(|e| e.queue.len())
            .unwrap_or(0)
    }

    /// Pop the front value from the queue for `id`.
    ///
    /// Returns `None` when the queue is missing or empty.  An empty queue
    /// is **not** removed — its creation timestamp remains, so subsequent
    /// pushes append to the same TTL window.
    pub fn pop(&self, id: Uuid) -> Option<Value> {
        let mut map = self.inner.lock();
        map.get_mut(&id).and_then(|e| e.queue.pop_front())
    }

    /// Number of distinct queues currently tracked, regardless of length.
    pub fn n_queues(&self) -> usize {
        self.inner.lock().len()
    }

    /// Snapshot of every queue id currently tracked, in arbitrary order.
    pub fn ids(&self) -> Vec<Uuid> {
        self.inner.lock().keys().copied().collect()
    }

    /// Drop every queue whose creation timestamp is older than `ttl_secs`
    /// from now.  Returns the number of queues evicted.
    ///
    /// A `ttl_secs` of `0` is a no-op (would otherwise evict everything,
    /// which is rarely useful).
    pub fn sweep_expired(&self, ttl_secs: u64) -> usize {
        if ttl_secs == 0 {
            return 0;
        }
        let now = now_unix_secs();
        let cutoff = now.saturating_sub(ttl_secs);
        let mut map = self.inner.lock();
        let before = map.len();
        map.retain(|_id, entry| entry.created_at > cutoff);
        before - map.len()
    }

    /// Return the creation timestamp (Unix seconds) for `id`, if known.
    /// Useful for diagnostics and tests.
    pub fn created_at(&self, id: Uuid) -> Option<u64> {
        self.inner.lock().get(&id).map(|e| e.created_at)
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
