//! Fixed-size, TTL-based in-memory cache mapping `(id, timestamp)` → [`JsonValue`].
//!
//! ## Design
//!
//! - **Key**: `(String, u64)` — document ID paired with a Unix-second timestamp,
//!   matching the natural lookup key used by [`DocumentStorage`] and [`ShardsManager`].
//! - **TTL**: Every entry carries an `expires_at` instant.  Expired entries are
//!   lazily removed on every [`get`] call and eagerly swept by a background thread
//!   at the configured cleanup interval.
//! - **Capacity**: When the cache is full and a new (distinct) key is inserted,
//!   a randomly chosen existing entry is evicted to make room (after a lazy expiry
//!   sweep that may already free space).
//! - **Thread safety**: All state lives behind `Arc<Mutex<_>>`; [`JsonCache`]
//!   is `Clone`, `Send`, and `Sync`.  Every clone shares the same underlying
//!   store.
//! - **Background thread**: A detached `std::thread` wakes up every 100 ms,
//!   checks whether any [`JsonCache`] clones are still alive (by inspecting the
//!   `Arc` strong-count), and runs the expiry sweep once per `cleanup_interval`.
//!   It exits automatically when all clones are dropped — no explicit shutdown
//!   call is required.
//!
//! [`DocumentStorage`]: crate::documentstorage::DocumentStorage
//! [`ShardsManager`]: crate::shardsmanager::ShardsManager
//! [`get`]: JsonCache::get

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ── internal state ────────────────────────────────────────────────────────────

struct CacheEntry {
    value:      JsonValue,
    expires_at: Instant,
}

struct Inner {
    entries:  HashMap<(String, u64), CacheEntry>,
    capacity: usize,
    ttl:      Duration,
}

impl Inner {
    fn evict_expired(&mut self) {
        let now = Instant::now();
        self.entries.retain(|_, e| e.expires_at > now);
    }

    /// Remove a uniformly random existing entry.  No-op when the cache is empty.
    fn evict_random(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        // HashMap iteration order is non-deterministic, so selecting the
        // nth element gives an effectively random pick without allocating.
        let idx = fastrand::usize(0..self.entries.len());
        if let Some(key) = self.entries.keys().nth(idx).cloned() {
            self.entries.remove(&key);
        }
    }
}

// ── public type ───────────────────────────────────────────────────────────────

/// Fixed-size, TTL-based in-memory cache mapping `(id, timestamp)` → [`JsonValue`].
///
/// All clones share the same underlying storage; any mutation through one clone
/// is immediately visible through all others.
///
/// # Background thread
///
/// [`new`] / [`with_cleanup_interval`] spawn a single background thread per
/// cache instance.  The thread performs periodic expired-entry sweeps and exits
/// automatically once every [`JsonCache`] clone has been dropped.  No explicit
/// shutdown is required.
///
/// # Example
///
/// ```
/// use bdslib::common::cache_json::JsonCache;
/// use std::time::Duration;
///
/// let cache = JsonCache::new(100, Duration::from_secs(60));
/// cache.insert("doc-abc", 1_700_000_000, serde_json::json!({"status": "ok"}));
/// assert!(cache.get("doc-abc", 1_700_000_000).is_some());
/// ```
///
/// [`new`]: JsonCache::new
/// [`with_cleanup_interval`]: JsonCache::with_cleanup_interval
#[derive(Clone)]
pub struct JsonCache {
    inner: Arc<Mutex<Inner>>,
}

impl JsonCache {
    /// Create a cache with `capacity` slots, a per-entry `ttl`, and a default
    /// background-cleanup interval of 60 seconds.
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self::with_cleanup_interval(capacity, ttl, Duration::from_secs(60))
    }

    /// Create a cache with an explicit background-cleanup `interval`.
    ///
    /// The interval controls how often the background thread sweeps for expired
    /// entries.  Shorter values reduce memory overshoot at the cost of more
    /// lock acquisitions.
    pub fn with_cleanup_interval(capacity: usize, ttl: Duration, interval: Duration) -> Self {
        let inner = Arc::new(Mutex::new(Inner {
            entries: HashMap::new(),
            capacity,
            ttl,
        }));

        // Spawn a background thread that performs periodic expiry sweeps.
        // It holds a clone of `inner`; when the strong-count drops to 1 (only
        // the thread's own clone remains), no live JsonCache instances exist and
        // the thread exits.
        let inner_bg = Arc::clone(&inner);
        std::thread::Builder::new()
            .name("jsoncache-cleanup".to_owned())
            .spawn(move || {
                let tick = Duration::from_millis(100);
                let mut elapsed = Duration::ZERO;
                loop {
                    std::thread::sleep(tick);
                    // All JsonCache clones have been dropped — time to exit.
                    if Arc::strong_count(&inner_bg) <= 1 {
                        break;
                    }
                    elapsed += tick;
                    if elapsed >= interval {
                        elapsed = Duration::ZERO;
                        if let Ok(mut g) = inner_bg.lock() {
                            g.evict_expired();
                        }
                    }
                }
            })
            .expect("failed to spawn jsoncache-cleanup thread");

        Self { inner }
    }

    // ── writes ────────────────────────────────────────────────────────────────

    /// Insert `value` under the key `(id, timestamp)`.
    ///
    /// Updating an existing key resets its TTL without evicting anything.
    ///
    /// When the cache is at capacity and a new key is inserted the method first
    /// attempts a lazy expiry sweep; if the cache is still full afterwards, one
    /// randomly chosen entry is evicted to make room.
    pub fn insert(&self, id: impl Into<String>, timestamp: u64, value: JsonValue) {
        let key = (id.into(), timestamp);
        let Ok(mut g) = self.inner.lock() else { return };

        if !g.entries.contains_key(&key) && g.entries.len() >= g.capacity {
            g.evict_expired();
            if g.entries.len() >= g.capacity {
                g.evict_random();
            }
        }

        let expires_at = Instant::now() + g.ttl;
        g.entries.insert(key, CacheEntry { value, expires_at });
    }

    /// Remove the entry at `(id, timestamp)` and return its value.
    ///
    /// Returns `None` if the key is absent (including if it has already expired).
    pub fn remove(&self, id: &str, timestamp: u64) -> Option<JsonValue> {
        let key = (id.to_owned(), timestamp);
        self.inner.lock().ok()?.entries.remove(&key).map(|e| e.value)
    }

    /// Remove all entries from the cache.
    pub fn clear(&self) {
        if let Ok(mut g) = self.inner.lock() {
            g.entries.clear();
        }
    }

    /// Immediately remove all entries whose TTL has elapsed.
    ///
    /// The background thread calls this automatically at the configured
    /// interval.  Call it manually for on-demand cleanup.
    pub fn evict_expired(&self) {
        if let Ok(mut g) = self.inner.lock() {
            g.evict_expired();
        }
    }

    // ── reads ─────────────────────────────────────────────────────────────────

    /// Return a clone of the value stored under `(id, timestamp)`.
    ///
    /// Returns `None` when the key is absent or the entry's TTL has elapsed.
    /// Expired entries are lazily removed during this call.
    pub fn get(&self, id: &str, timestamp: u64) -> Option<JsonValue> {
        let key = (id.to_owned(), timestamp);
        let mut g = self.inner.lock().ok()?;
        let now = Instant::now();
        match g.entries.get(&key) {
            None => None,
            Some(e) if e.expires_at <= now => {
                g.entries.remove(&key);
                None
            }
            Some(e) => Some(e.value.clone()),
        }
    }

    /// Return the number of entries currently held (including stale entries not
    /// yet swept by the background thread).
    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.entries.len()).unwrap_or(0)
    }

    /// Return `true` if there are no entries in the cache.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return the maximum number of entries the cache will hold before evicting.
    pub fn capacity(&self) -> usize {
        self.inner.lock().map(|g| g.capacity).unwrap_or(0)
    }

    /// Return the per-entry time-to-live.
    pub fn ttl(&self) -> Duration {
        self.inner.lock().map(|g| g.ttl).unwrap_or(Duration::ZERO)
    }

    /// Return the number of entries whose TTL has elapsed but have not yet been
    /// swept.  Useful for monitoring cache health.
    pub fn expired_count(&self) -> usize {
        let Ok(g) = self.inner.lock() else { return 0 };
        let now = Instant::now();
        g.entries.values().filter(|e| e.expires_at <= now).count()
    }

    /// Return a clone of any live entry whose id component matches, ignoring
    /// the timestamp component of the key.
    ///
    /// Useful when the caller knows the id but not the timestamp (e.g. after a
    /// search that returns UUIDs without fetching the full record).  Expired
    /// entries matching `id` are lazily removed during this call.
    /// Returns `None` if no live entry exists for `id`.
    pub fn get_by_id(&self, id: &str) -> Option<JsonValue> {
        let mut g = self.inner.lock().ok()?;
        let now = Instant::now();
        // Collect expired keys for this id to clean up lazily.
        let expired: Vec<(String, u64)> = g.entries.iter()
            .filter(|((eid, _), e)| eid == id && e.expires_at <= now)
            .map(|(k, _)| k.clone())
            .collect();
        for k in expired {
            g.entries.remove(&k);
        }
        // Return the first live entry (a given id should have at most one
        // live entry in normal ShardsManager usage since UUIDs are unique).
        g.entries.iter()
            .find(|((eid, _), e)| eid == id && e.expires_at > now)
            .map(|(_, e)| e.value.clone())
    }

    /// Remove all entries whose id component matches, regardless of timestamp.
    ///
    /// Use this when only the id is known at eviction time (e.g. `delete_by_id`).
    pub fn remove_by_id(&self, id: &str) {
        let Ok(mut g) = self.inner.lock() else { return };
        g.entries.retain(|(eid, _), _| eid != id);
    }
}
