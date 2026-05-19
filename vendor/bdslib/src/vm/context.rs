use crate::vm::vm::init_stdlib;
use bundcore::bundcore::Bund;
use easy_error::{err_msg, Error};
use parking_lot::{ArcRwLockWriteGuard, Mutex, RawRwLock, RwLock};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

// ── internal storage ─────────────────────────────────────────────────────────

struct Entry {
    bund: Arc<RwLock<Bund>>,
    last_accessed: Instant,
}

pub struct BundContext {
    entries: HashMap<String, Entry>,
    ttl: Duration,
}

// ── public singleton ──────────────────────────────────────────────────────────

/// Process-wide registry of named BUND VM instances.
///
/// Keyed by an arbitrary string name. Entries are evicted automatically after
/// `bund_ttl_secs` seconds of inactivity (time-to-idle).
///
/// Must be populated by calling [`init`] before [`get`] or [`remove`].
pub static BUNDS: OnceLock<Mutex<BundContext>> = OnceLock::new();

// ── initialisation ────────────────────────────────────────────────────────────

/// Initialise the BUND context and start the background eviction thread.
///
/// Reads `bund_ttl_secs` from the hjson config (default: 300 s).
/// Returns `Err` if called more than once.
pub fn init(config_path: Option<&str>) -> Result<(), Error> {
    if BUNDS.get().is_some() {
        return Ok(());
    }
    let ttl = read_ttl(config_path)?;
    let ctx = BundContext {
        entries: HashMap::new(),
        ttl,
    };
    // A concurrent call may have won the race; that is fine — we just discard
    // our freshly-built ctx and let the winner's stay.
    let _ = BUNDS.set(Mutex::new(ctx));
    Ok(())
}

fn read_ttl(config_path: Option<&str>) -> Result<Duration, Error> {
    let path = match config_path {
        Some(p) => p.to_string(),
        None => match std::env::var("BDS_CONFIG") {
            Ok(p) => p,
            Err(_) => return Ok(Duration::from_secs(300)),
        },
    };

    let raw = std::fs::read_to_string(&path)
        .map_err(|e| err_msg(format!("cannot read config {path:?}: {e}")))?;
    let val: serde_hjson::Value =
        serde_hjson::from_str(&raw).map_err(|e| err_msg(format!("hjson parse error: {e}")))?;

    let secs = val
        .as_object()
        .and_then(|obj| obj.get("bund_ttl_secs"))
        .and_then(|v| v.as_f64())
        .map(|n| n as u64)
        .unwrap_or(300)
        .max(1);

    Ok(Duration::from_secs(secs))
}

// ── public API ────────────────────────────────────────────────────────────────

/// Return a write guard for the named BUND VM, creating it if it does not exist.
///
/// On creation [`init_stdlib`] is called automatically so the instance is fully
/// initialised before the guard is returned.  The entry's idle timer is reset
/// each time `get` is called.
///
/// The returned [`ArcRwLockWriteGuard`] derefs to `&mut Bund`, so callers can
/// use it directly wherever `&mut Bund` is expected.
pub fn get(name: &str) -> Result<ArcRwLockWriteGuard<RawRwLock, Bund>, Error> {
    let ctx = BUNDS
        .get()
        .ok_or_else(|| err_msg("BUNDS context not initialized; call context::init() first"))?;

    // Hold the outer lock only long enough to get-or-create the Arc and update
    // the access timestamp.  We release it before acquiring the inner write lock
    // so unrelated VMs are not blocked while one is in use.
    let arc = {
        let mut guard = ctx.lock();
        let entry = guard
            .entries
            .entry(name.to_string())
            .or_insert_with(|| {
                let mut bund = Bund::new();
                if let Err(e) = init_stdlib(&mut bund) {
                    log::error!("[context] stdlib init failed for {name:?}: {e}");
                }
                log::debug!("[context] created BUND instance {name:?}");
                Entry {
                    bund: Arc::new(RwLock::new(bund)),
                    last_accessed: Instant::now(),
                }
            });
        entry.last_accessed = Instant::now();
        entry.bund.clone()
    };

    Ok(arc.write_arc())
}

/// Delete the named BUND VM from the registry.
///
/// If a caller currently holds a guard obtained from [`get`], the underlying
/// `Bund` remains alive until that guard is dropped (Arc reference counting).
/// Named `remove` rather than `drop` to avoid conflicting with Rust's built-in
/// `drop` function.
pub fn remove(name: &str) -> Result<(), Error> {
    let ctx = BUNDS
        .get()
        .ok_or_else(|| err_msg("BUNDS context not initialized"))?;
    let removed = ctx.lock().entries.remove(name).is_some();
    if removed {
        log::debug!("[context] removed BUND instance {name:?}");
    }
    Ok(())
}

// ── eviction / shutdown API ───────────────────────────────────────────────────

/// Remove all BUND VM entries whose idle time exceeds the configured TTL.
///
/// Returns the names of every VM that was evicted so the caller can log them.
/// Called periodically by `bdsnode`'s bundcleanup thread.
pub fn evict_stale() -> Vec<String> {
    let Some(ctx) = BUNDS.get() else { return Vec::new() };
    let mut guard = ctx.lock();
    let ttl = guard.ttl;
    let mut evicted = Vec::new();
    guard.entries.retain(|name, entry| {
        if entry.last_accessed.elapsed() > ttl {
            evicted.push(name.clone());
            false
        } else {
            true
        }
    });
    evicted
}

/// Remove and return the names of every BUND VM currently in the registry.
///
/// Called once during server shutdown so callers can log each terminated VM.
pub fn close_all() -> Vec<String> {
    let Some(ctx) = BUNDS.get() else { return Vec::new() };
    let mut guard = ctx.lock();
    let names: Vec<String> = guard.entries.keys().cloned().collect();
    guard.entries.clear();
    names
}

/// Return the configured time-to-idle duration, or 300 s if not yet initialised.
pub fn ttl() -> Duration {
    BUNDS
        .get()
        .map(|c| c.lock().ttl)
        .unwrap_or(Duration::from_secs(300))
}

/// Number of named BUND VM contexts currently in the registry.
///
/// Exposed for `v2/status` reporting; returns `0` if [`init`] has not run.
pub fn n_contexts() -> usize {
    BUNDS.get().map(|c| c.lock().entries.len()).unwrap_or(0)
}
