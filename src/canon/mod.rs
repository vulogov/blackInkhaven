//! CANON-LEDGER-1 (CL-P0) — the story's development-history ledger, backed by
//! the `smysl` crate (github.com/vulogov/smysl, format `smysl/1.0`).
//!
//! This phase is the **storage substrate only**: an owned wrapper over a
//! `smysl::Store`, persisted per project at `<project>/canon.cbor`. It mirrors
//! the ergonomics of the vector index ([`crate::storage::vector::VectorEngine`]):
//! lazy open on first use, a dirty flag, atomic writes via [`crate::io_atomic`],
//! and an off-thread background flush so a paragraph save never blocks the
//! render thread.
//!
//! The ledger is a **derived** artifact — re-derivable by re-harvest from the
//! manuscript — so a crash mid-flush keeps the last good `canon.cbor` and loses
//! at most the pending append; no user prose is at risk. A corrupt/unreadable
//! file degrades to an empty store (with a warning) rather than failing the open.
//!
//! Later phases build on this substrate: the narrative unit model + host-node
//! bridge (CL-P1), on-save harvest (CL-P2), and the impact / why / diff queries
//! (CL-P3). CL-P0 provides only open / append / count / flush.
//!
//! The wrapper is exercised by this module's tests but not yet wired into the
//! binary (its consumers land in CL-P1+), so the surface is `allow(dead_code)`
//! until then — the annotation is removed when the store/CLI first calls it.
#![allow(dead_code)]

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use smysl::{from_cbor_seq, to_cbor_seq, Record, Store};

/// After this many consecutive background-flush failures, give up the pass
/// (leaving `dirty` set for the next trigger) rather than spinning — the same
/// no-spin guarantee the vector index makes.
const MAX_SYNC_RETRIES: u32 = 5;

/// The background-flush retry policy after one `drain_dirty` attempt. Pure, so
/// the no-spin behaviour is unit-testable without a failing store: returns the
/// new consecutive-failure count, an optional backoff sleep, and whether to give
/// up this pass. Mirrors [`crate::storage::vector`]'s `sync_retry_step`.
fn sync_retry_step(succeeded: bool, failures: u32) -> (u32, Option<std::time::Duration>, bool) {
    if succeeded {
        return (0, None, false);
    }
    let f = failures + 1;
    if f >= MAX_SYNC_RETRIES {
        (f, None, true)
    } else {
        (f, Some(std::time::Duration::from_millis(100 * f as u64)), false)
    }
}

/// Thread-safe handle to a project's canon ledger. The `smysl::Store` is opened
/// lazily on the first operation and held in memory; `append` marks the ledger
/// dirty, and `sync` / `sync_in_background` flush it to `canon.cbor`.
///
/// `Clone` is cheap (shared `Arc`s) so the handle can be moved into background
/// tasks the way `VectorEngine` is.
#[derive(Clone)]
pub struct CanonLedger {
    path: String,
    store: Arc<Mutex<Option<Store>>>,
    dirty: Arc<AtomicBool>,
    /// True while a background flush thread is running, so a burst of appends
    /// spawns at most one such thread (it coalesces later writes).
    sync_in_flight: Arc<AtomicBool>,
}

impl CanonLedger {
    /// Construct a handle for the ledger at `path` (`<project>/canon.cbor`). The
    /// store is not opened until the first operation, so opening a project that
    /// never touches canon costs nothing.
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            store: Arc::new(Mutex::new(None)),
            dirty: Arc::new(AtomicBool::new(false)),
            sync_in_flight: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Append records to the ledger and mark it dirty. smysl content-addresses
    /// units, so re-deriving the same decision is idempotent at the graph level.
    pub fn append(&self, records: &[Record]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        let dirty = self.dirty.clone();
        self.with_store(|s| {
            s.append(records)
                .map_err(|e| anyhow!("canon: append failed: {e}"))?;
            dirty.store(true, Ordering::Release);
            Ok(())
        })
    }

    /// Total record count in the ledger.
    pub fn count(&self) -> Result<usize> {
        self.with_store(|s| Ok(s.len()))
    }

    /// Flush to disk, but only when there are unpersisted writes. The clean-path
    /// fast return skips the lock entirely.
    pub fn sync(&self) -> Result<()> {
        if !self.dirty.load(Ordering::Acquire) {
            return Ok(());
        }
        Self::drain_dirty(&self.store, &self.dirty, &self.path)
    }

    /// Flush **off the calling thread** (single-flight). The write is atomic
    /// (temp + rename via `io_atomic`) and the ledger is derived, so backgrounding
    /// it keeps a routine paragraph save from freezing the render thread while the
    /// store serializes. Clean (`!dirty`) states don't spawn; the quit-path
    /// `sync()` stays synchronous, so the ledger always converges.
    pub fn sync_in_background(&self) {
        if !self.dirty.load(Ordering::Acquire) {
            return;
        }
        if self.sync_in_flight.swap(true, Ordering::AcqRel) {
            return;
        }
        let store = self.store.clone();
        let dirty = self.dirty.clone();
        let in_flight = self.sync_in_flight.clone();
        let path = self.path.clone();
        std::thread::spawn(move || {
            // Isolate a panic in the flush from the process-global crash hook so it
            // can't tear down a live terminal, and reset the single-flight flag on
            // panic so a later append can spawn a fresh flush.
            let panic_reset = in_flight.clone();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::crash::suppress_panic_report(|| {
                    Self::run_sync_loop(&store, &dirty, &in_flight, &path);
                })
            }));
            if outcome.is_err() {
                panic_reset.store(false, Ordering::Release);
                tracing::error!(
                    target: "inkhaven::canon",
                    "background canon flush panicked (isolated + flag reset)"
                );
            }
        });
    }

    /// The retry/backoff flush loop, extracted so the spawn site can wrap it in
    /// panic isolation. Owns nothing; takes shared references.
    fn run_sync_loop(
        store: &Mutex<Option<Store>>,
        dirty: &AtomicBool,
        in_flight: &AtomicBool,
        path: &str,
    ) {
        let mut failures: u32 = 0;
        loop {
            let (next_failures, backoff, give_up) = match Self::drain_dirty(store, dirty, path) {
                Ok(()) => sync_retry_step(true, failures),
                Err(e) => {
                    tracing::warn!(
                        target: "inkhaven::canon",
                        "background canon flush failed (attempt {}): {e}",
                        failures + 1,
                    );
                    sync_retry_step(false, failures)
                }
            };
            failures = next_failures;
            if let Some(delay) = backoff {
                std::thread::sleep(delay);
            }
            // Release the flag, then re-check: an append that set `dirty` during
            // our write is flushed on this same thread. Release BEFORE any give-up
            // so the next append can spawn a fresh flush once an I/O fault clears.
            in_flight.store(false, Ordering::Release);
            if give_up {
                break;
            }
            if !dirty.load(Ordering::Acquire) {
                break;
            }
            if in_flight.swap(true, Ordering::AcqRel) {
                break;
            }
        }
    }

    /// The shared flush body for [`Self::sync`] and [`Self::sync_in_background`]:
    /// under the store lock, re-check dirty (a racing flush may have drained it),
    /// then serialize and write atomically, clearing the flag on success.
    fn drain_dirty(store: &Mutex<Option<Store>>, dirty: &AtomicBool, path: &str) -> Result<()> {
        let guard = store.lock();
        if !dirty.load(Ordering::Acquire) {
            return Ok(());
        }
        let Some(s) = guard.as_ref() else {
            // Shouldn't happen — an append lazily opens the store before it can
            // flip dirty — but stay defensive.
            dirty.store(false, Ordering::Release);
            return Ok(());
        };
        match Self::save_store(s, path) {
            Ok(()) => {
                dirty.store(false, Ordering::Release);
                Ok(())
            }
            Err(e) => Err(anyhow!("failed to flush canon ledger: {e}")),
        }
    }

    fn with_store<R, F: FnOnce(&mut Store) -> Result<R>>(&self, f: F) -> Result<R> {
        let mut guard = self.store.lock();
        if guard.is_none() {
            *guard = Some(
                Self::load_store(&self.path)
                    .map_err(|e| anyhow!("failed to open canon ledger at {:?}: {e}", self.path))?,
            );
        }
        let store = guard.as_mut().expect("set immediately above when None");
        f(store)
    }

    /// Load the ledger from `canon.cbor`, or an empty store when the file is
    /// absent. A corrupt/unreadable file degrades to empty (the ledger is
    /// re-derivable) with a warning, rather than failing the open.
    fn load_store(path: &str) -> Result<Store> {
        let p = Path::new(path);
        if !p.is_file() {
            return Ok(Store::new());
        }
        let bytes = std::fs::read(p).map_err(|e| anyhow!("read {p:?}: {e}"))?;
        match from_cbor_seq(&bytes) {
            Ok((records, _)) => Ok(Store::from_records(records)),
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::canon",
                    "canon ledger at {p:?} unreadable ({e}); starting empty — a re-harvest will repopulate it"
                );
                Ok(Store::new())
            }
        }
    }

    /// Serialize the store's records to CBOR and write `canon.cbor` atomically.
    fn save_store(store: &Store, path: &str) -> Result<()> {
        let records: Vec<Record> = store.iter().cloned().collect();
        let bytes = to_cbor_seq(&records);
        crate::io_atomic::write(Path::new(path), &bytes)
            .map_err(|e| anyhow!("write canon ledger {path:?}: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smysl::{KernelType, Status, UnitCore, UnitCoreBuilder};

    /// A minimal, shape-valid unit record. `Speculative` needs neither grounds
    /// nor source, so it builds from schema + gist alone — enough to exercise the
    /// store wrapper without the CL-P1 narrative model.
    fn sample_unit(gist: &str) -> Record {
        let core: UnitCore = UnitCoreBuilder::new(KernelType::Claim, gist, Status::Speculative)
            .build()
            .expect("a speculative claim with a gist is shape-valid");
        Record::Unit(core)
    }

    #[test]
    fn append_flush_reopen_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canon.cbor");
        let path_s = path.to_str().unwrap().to_string();

        {
            let led = CanonLedger::new(&path_s);
            assert_eq!(led.count().unwrap(), 0, "a fresh ledger is empty");
            led.append(&[sample_unit("the harbour freezes over each winter")])
                .unwrap();
            led.append(&[sample_unit("the lighthouse keeper counts the ships")])
                .unwrap();
            assert_eq!(led.count().unwrap(), 2);
            led.sync().unwrap();
            assert!(path.is_file(), "sync wrote canon.cbor");
        }

        // Reopen — records persisted and load (no rebuild).
        {
            let led = CanonLedger::new(&path_s);
            assert_eq!(led.count().unwrap(), 2, "reopened ledger preserves its records");
        }
    }

    #[test]
    fn absent_file_opens_empty_and_clean_sync_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        // No file yet → empty store, and a clean sync writes nothing.
        assert_eq!(led.count().unwrap(), 0);
        led.sync().unwrap();
        assert!(!Path::new(&path_s).exists(), "a clean ledger writes no file");
    }

    #[test]
    fn retry_backs_off_then_gives_up_without_spinning() {
        // success resets and never backs off
        let (f, backoff, give_up) = sync_retry_step(true, 4);
        assert_eq!(f, 0);
        assert!(backoff.is_none() && !give_up);
        // failures back off linearly, then give up at the ceiling
        let mut failures = 0u32;
        let mut gave_up = false;
        for step in 1..=MAX_SYNC_RETRIES {
            let (nf, backoff, give_up) = sync_retry_step(false, failures);
            failures = nf;
            assert_eq!(nf, step);
            if step < MAX_SYNC_RETRIES {
                assert_eq!(backoff.unwrap().as_millis() as u64, 100 * step as u64);
                assert!(!give_up);
            } else {
                assert!(backoff.is_none() && give_up);
                gave_up = true;
            }
        }
        assert!(gave_up, "must give up at the retry ceiling");
    }
}
