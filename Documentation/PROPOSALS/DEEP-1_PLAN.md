# DEEP-1 — The TUI deep-refresh (background jobs) — 1.3.12

_The deferred 1.3.11 item, done properly. The deep AI refresh (facts check /
facts scan / drift / continuity) is a multi-minute batch of blocking AI
calls. 1.3.11 deferred a TUI trigger because the obvious implementations all
break: a subprocess can't open the DB the TUI holds (DuckDB is single-writer),
and running inline freezes the editor. This cut builds the right substrate — a
**background-job harness** that runs the batch off the main thread against a
**shared** Store — and wires the deep refresh onto a chord._

## The two facts that decide the design

1. **Single-writer lock.** `Store::open` takes an exclusive DuckDB lock — its
   own error says "Another inkhaven process may be using the project." So the
   background work must **not** open a second Store (subprocess or in-process).
2. **`Store` is `Clone` + pool-backed.** It's `Arc<ProjectLayout>` +
   `DocumentStorage` over an r2d2 pool (4 connections for metadata). Cloning it
   shares the pool; a background thread checks out its own pooled connection
   (r2d2 is thread-safe), with connections to spare. So: **clone the TUI's
   Store into the thread — no reopen, no lock, no deadlock.**

The scans currently open their own Store, so the work is to let them accept an
injected one.

Zero new dependencies (`std::thread` + `std::sync::mpsc`; r2d2 already in).

---

## P0 — The background-job harness (reusable substrate)

A general mechanism on `App`, not specific to the deep refresh (the 1.4
Whole-Book AI Editor will reuse it for queries):

- `App.bg_job: Option<BgJob>` where `BgJob { rx: mpsc::Receiver<BgMsg>, label:
  String }` and `enum BgMsg { Progress(String), Done(Result<String, String>) }`.
- `start_bg_job(label, work: impl FnOnce(Sender<BgMsg>) + Send + 'static)` —
  refuses if a job is already running; spawns a thread, stores the receiver.
- The main loop drains `rx` each tick (alongside `pump_inference`):
  `Progress` → status line; `Done` → run the registered completion handler,
  clear `bg_job`. A dropped sender (panicked thread) is treated as a failed
  `Done`.
- A status chip (`⟳ <label>…`) while a job runs.

**Deliverable:** spawn/await background work with progress, one job at a time,
no UI freeze. (`Store` proven `Send` by the thread spawn compiling.)

---

## P1 — Store-injected scan entry points

Give each deep-refresh scan a variant that runs against an **already-open**
Store, with progress routed through a callback (not `eprintln`, which would
corrupt the TUI display):

- `facts_scan::check_with` / `scan_with`, `drift::scan_with`,
  `continuity::extract_with`, each `(store, hierarchy, cfg, layout, provider,
  progress: &dyn Fn(&str))`. The existing CLI `check`/`scan`/`extract` become
  thin wrappers: open the Store, then delegate with an `eprintln` progress
  closure — **no behaviour change** to the CLI.
- `deep_refresh_shared(store, h, cfg, layout, provider, progress)` orchestrates
  the four in sequence, emitting `progress("drift scan…")` between them and
  tolerating any single scan's failure (logs to the callback, continues).
- **Multilingual (hard requirement):** every scan's prompt envelope must carry
  the project `cfg.language` — `deep_refresh_shared` threads it through so the
  deep AI runs in the manuscript's language (Russian / French / German /
  Spanish), never an English fallback. The injected scans already accept
  `cfg`; the orchestrator must not drop the language on the way.

**Deliverable:** the deep refresh runs from a single shared-Store call with no
reopen; the CLI paths are unchanged. Scope matches `world --deep` (facts check
· facts scan · drift · continuity).

---

## P2 — The chord + completion

- A view chord — **`Ctrl+V Shift+F`** (reFresh; verified free in `view_sub`) —
  → `start_deep_refresh()`: pre-flight (a provider is configured; no job
  already running), then `start_bg_job` with a closure that **moves a clone**
  of the Store + layout + cfg and runs `deep_refresh_shared`, forwarding
  progress to the channel.
- Completion handler: reload the world/editorial sidecars; if the story bible
  or Editorial Pass cockpit is open, rebuild its rows from the fresh sidecars;
  status `deep refresh done — World: N issue(s)`.

**Deliverable:** one keystroke kicks off the whole world-layer AI refresh; the
editor stays fully responsive; the open modal updates itself when it lands.

---

## P3 — Progress, guardrails, lifecycle

- **Live progress** — each scan reports as it runs (`⟳ deep refresh: drift 3/4`).
- **Guardrails** — provider-missing → a clear status, no thread; double-press
  → "a refresh is already running"; the job survives modal open/close.
- **Quit mid-job** — abandon cleanly (the thread is detached; sidecars it
  finished are already written atomically; an unfinished one is simply stale,
  not corrupt).
- **No stderr leakage** — the `*_with` scans emit only through the callback.

**Deliverable:** robust, legible background refresh that never freezes or
corrupts the UI.

---

## P4 — Docs + 1.3.12 release cut

- **Tutorial 72** (or a refresh of 71/68) — the `Ctrl+V Shift+F` background
  refresh: what it runs, that it's non-blocking, the open-modal auto-update.
- **KEYBINDING.md** + quick-help — the new chord.
- RELEASE_NOTES/1.3.12.md + index row; top README; version bump; signed tag
  `v1.3.12`; `cargo publish`; merge to main; open the next cycle.

---

## Out of scope (carryovers)

- **The Whole-Book AI Editor** — the 1.4 headline; it will reuse the P0
  harness for retrieve-then-reason queries.
- Cancelling an in-flight job (vs abandon-on-quit); a multi-job queue.
- Tension-scan in the TUI refresh (structural, separate from the world layer).
- PDF N-up / booklet presets; CMYK-JPEG grayscale; ePub inline images; sixth
  language.

## Phase order

P0 (harness) and P1 (injection) are independent; P2 needs both; P3 polishes
P2. Sequence: **P0 → P1 → P2 → P3 → P4**.
