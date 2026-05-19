use crate::common::error::{err_msg, Result};
use crate::common::timerange::align_to_duration;
use crate::observability::ObservabilityStorageConfig;
use crate::shard::Shard;
use crate::shardsinfo::ShardInfoEngine;
use crate::EmbeddingEngine;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct CacheInner {
    map: HashMap<(SystemTime, SystemTime), Shard>,
    /// Keys in most-recently-used-first order.
    lru: VecDeque<(SystemTime, SystemTime)>,
}

/// In-memory cache of open [`Shard`] instances, keyed by their `[start, end)` time interval.
///
/// `ShardsCache` owns a [`ShardInfoEngine`] catalog that records every shard's
/// filesystem path and time interval on disk. The in-memory cache is a fast
/// lookup layer on top of that catalog.
///
/// ## `shard()` lookup order
///
/// 1. In-memory cache — O(1) lookup by aligned `(start, end)` key; returns immediately on hit.
/// 2. [`ShardInfoEngine`] catalog — if the cache misses, the catalog is queried for a
///    shard covering the given timestamp. On a catalog hit the shard is opened from
///    its stored path and inserted into the cache.
/// 3. Auto-create — if neither the cache nor the catalog covers the timestamp, a new
///    shard directory is provisioned at `{root_path}/{start_ts}_{end_ts}`, registered
///    in the catalog, opened, and inserted into the cache.
///
/// Time intervals are aligned to `shard_duration` boundaries relative to the Unix
/// epoch, so all shards of the same duration are non-overlapping and contiguous.
///
/// `ShardsCache` is `Clone`; all clones share the same underlying cache, catalog,
/// and connection pool.
#[derive(Clone)]
pub struct ShardsCache {
    root_path: String,
    shard_duration: Duration,
    pool_size: u32,
    embedding: EmbeddingEngine,
    obs_config: ObservabilityStorageConfig,
    info: ShardInfoEngine,
    cache: Arc<Mutex<CacheInner>>,
    /// Maximum number of shards kept open simultaneously. When exceeded, the
    /// least-recently-used shard is synced and evicted to reclaim file descriptors.
    max_open_shards: usize,
}

impl ShardsCache {
    /// Open or create a shard cache rooted at `root_path` with default
    /// [`ObservabilityStorageConfig`] (similarity threshold `0.85`).
    ///
    /// `shard_duration` is a human-readable duration string parsed by
    /// [`humantime`](https://docs.rs/humantime), e.g. `"1h"`, `"30min"`, `"1day"`.
    ///
    /// The catalog database is stored at `{root_path}/shards_info.db`.
    /// The root directory is created automatically if it does not exist.
    pub fn new(
        root_path: &str,
        shard_duration: &str,
        pool_size: u32,
        embedding: EmbeddingEngine,
    ) -> Result<Self> {
        Self::with_config(
            root_path,
            shard_duration,
            pool_size,
            embedding,
            ObservabilityStorageConfig::default(),
            16,
        )
    }

    /// Open or create a shard cache with a custom [`ObservabilityStorageConfig`].
    ///
    /// `shard_duration` uses the same human-readable format as [`new`](Self::new).
    /// `max_open_shards` caps the number of shards held open at once; the LRU shard
    /// is synced and evicted when the limit is reached.
    pub fn with_config(
        root_path: &str,
        shard_duration: &str,
        pool_size: u32,
        embedding: EmbeddingEngine,
        obs_config: ObservabilityStorageConfig,
        max_open_shards: usize,
    ) -> Result<Self> {
        let duration = humantime::parse_duration(shard_duration).map_err(|e| {
            err_msg(format!(
                "invalid shard_duration '{shard_duration}': {e}"
            ))
        })?;
        if duration.is_zero() {
            return Err(err_msg("shard_duration must be non-zero"));
        }
        std::fs::create_dir_all(root_path)
            .map_err(|e| err_msg(format!("cannot create shard cache root '{root_path}': {e}")))?;
        let info_path = format!("{root_path}/shards_info.db");
        let info = ShardInfoEngine::new(&info_path, pool_size)?;
        Ok(Self {
            root_path: root_path.to_string(),
            shard_duration: duration,
            pool_size,
            embedding,
            obs_config,
            info,
            cache: Arc::new(Mutex::new(CacheInner {
                map: HashMap::new(),
                lru: VecDeque::new(),
            })),
            max_open_shards: max_open_shards.max(1),
        })
    }

    // ── primary API ───────────────────────────────────────────────────────────

    /// Return the [`Shard`] whose interval `[start, end)` covers `timestamp`.
    ///
    /// Lookup order: in-memory cache → catalog → auto-create. See struct-level
    /// documentation for details.
    ///
    /// The returned `Shard` is a cheap clone that shares all underlying resources
    /// with the cached instance.
    pub fn shard(&self, timestamp: SystemTime) -> Result<Shard> {
        let (start, end) = align_to_duration(timestamp, self.shard_duration)?;
        let key = (start, end);

        let mut state = self.cache.lock();

        // 1. In-memory cache hit — promote to MRU position.
        if let Some(shard) = state.map.get(&key) {
            let shard = shard.clone();
            state.lru.retain(|k| k != &key);
            state.lru.push_front(key);
            return Ok(shard);
        }

        // 2. Catalog lookup.
        let infos = self.info.shards_at(timestamp)?;
        let (insert_key, shard) = if let Some(info) = infos.into_iter().next() {
            let shard = Shard::with_config(
                &info.path,
                self.pool_size,
                self.embedding.clone(),
                self.obs_config.clone(),
            )?;
            ((info.start_time, info.end_time), shard)
        } else {
            // 3. Auto-create.
            let start_secs = start
                .duration_since(UNIX_EPOCH)
                .map_err(|e| err_msg(format!("shard start predates epoch: {e}")))?
                .as_secs();
            let end_secs = end
                .duration_since(UNIX_EPOCH)
                .map_err(|e| err_msg(format!("shard end predates epoch: {e}")))?
                .as_secs();
            let path = format!("{}/{start_secs}_{end_secs}", self.root_path);
            let shard = Shard::with_config(
                &path,
                self.pool_size,
                self.embedding.clone(),
                self.obs_config.clone(),
            )?;
            self.info.add_shard(&path, start, end)?;
            (key, shard)
        };

        state.map.insert(insert_key, shard.clone());
        state.lru.push_front(insert_key);

        // Evict LRU shards until we're within the open-shard limit.
        while state.map.len() > self.max_open_shards {
            if let Some(oldest) = state.lru.pop_back() {
                if let Some(evicted) = state.map.remove(&oldest) {
                    let _ = evicted.sync();
                }
            } else {
                break;
            }
        }

        Ok(shard)
    }

    /// Return one [`Shard`] per aligned interval that overlaps `[start_ts, end_ts)`.
    ///
    /// Intervals are enumerated by stepping in `shard_duration` increments starting
    /// from the aligned floor of `start_ts`. Each step calls [`shard`](Self::shard),
    /// so shards are auto-created when not already present.
    ///
    /// Returns an empty `Vec` when `start_ts >= end_ts`.
    pub fn shards_span(
        &self,
        start_ts: SystemTime,
        end_ts: SystemTime,
    ) -> Result<Vec<Shard>> {
        if start_ts >= end_ts {
            return Ok(vec![]);
        }
        let (mut cursor, _) = align_to_duration(start_ts, self.shard_duration)?;
        let mut shards = Vec::new();
        while cursor < end_ts {
            shards.push(self.shard(cursor)?);
            cursor += self.shard_duration;
        }
        Ok(shards)
    }

    /// Return one [`Shard`] per aligned interval that overlaps the window
    /// `[now, now + duration)`.
    ///
    /// `duration` uses the same human-readable format as the constructor
    /// (e.g. `"1h"`, `"30min"`, `"2days"`).
    ///
    /// This is a convenience wrapper around [`shards_span`](Self::shards_span).
    pub fn current(&self, duration: &str) -> Result<Vec<Shard>> {
        let dur = humantime::parse_duration(duration)
            .map_err(|e| err_msg(format!("invalid duration '{duration}': {e}")))?;
        let now = SystemTime::now();
        self.shards_span(now, now + dur)
    }

    /// Flush all cached shards to disk.
    ///
    /// All shards are attempted; the first error encountered is returned after
    /// the remaining shards have been synced.
    ///
    /// The cache lock is held only for the initial snapshot — not across the
    /// DuckDB CHECKPOINT calls — so concurrent shard lookups are not blocked
    /// during the flush.
    pub fn sync(&self) -> Result<()> {
        let shards: Vec<Shard> = self.cache.lock().map.values().cloned().collect();
        let mut first_err: Option<String> = None;
        for shard in &shards {
            if let Err(e) = shard.sync() {
                first_err.get_or_insert_with(|| e.to_string());
            }
        }
        match first_err {
            None => Ok(()),
            Some(msg) => Err(err_msg(msg)),
        }
    }

    /// Flush all cached shards to disk and evict them from the in-memory cache.
    ///
    /// After `close` the cache is empty. The catalog and on-disk shard data are
    /// unaffected; a subsequent [`shard`](Self::shard) call will reopen from disk.
    ///
    /// Note: underlying engine resources (IndexWriter lock, connection pool) are
    /// released only when all clones of the evicted [`Shard`]s are dropped by
    /// callers.
    pub fn close(&self) -> Result<()> {
        let mut state = self.cache.lock();
        let mut first_err: Option<String> = None;
        for shard in state.map.values() {
            if let Err(e) = shard.sync() {
                first_err.get_or_insert_with(|| e.to_string());
            }
        }
        state.map.clear();
        state.lru.clear();
        match first_err {
            None => Ok(()),
            Some(msg) => Err(err_msg(msg)),
        }
    }

    // ── accessors ─────────────────────────────────────────────────────────────

    /// Borrow the underlying [`ShardInfoEngine`] catalog.
    pub fn info(&self) -> &ShardInfoEngine {
        &self.info
    }

    /// Borrow the shared [`EmbeddingEngine`].
    ///
    /// Used by callers that want to embed a query once and pass the resulting
    /// vector to multiple per-shard searches.
    pub fn embedding(&self) -> &EmbeddingEngine {
        &self.embedding
    }

    /// Return the configured shard width.
    pub fn shard_duration(&self) -> Duration {
        self.shard_duration
    }

    /// Return the number of shards currently in the in-memory cache.
    pub fn cached_count(&self) -> usize {
        self.cache.lock().map.len()
    }
}
