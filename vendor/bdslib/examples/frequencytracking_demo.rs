/// frequencytracking_demo — Record and query event-frequency data.
///
/// Sections:
///   1. Setup       — open an in-memory FrequencyTracking store
///   2. Ingest      — record observations at controlled timestamps
///   3. by_id       — retrieve every timestamp for a specific ID
///   4. by_timestamp — which IDs were active at an exact second
///   5. time_range  — IDs that fired in a historical window
///   6. recent      — IDs observed in the last N minutes (live data)
///   7. Sync        — flush WAL to disk
use bdslib::common::time::now_secs;
use bdslib::FrequencyTracking;
use tempfile::TempDir;

fn main() {
    // ── Section 1: Setup ─────────────────────────────────────────────────────

    println!("=== Section 1: Setup ===");

    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("frequency.db");
    let ft = FrequencyTracking::new(db_path.to_str().unwrap(), 4).unwrap();
    println!("Opened frequency-tracking store at {:?}", db_path);

    // ── Section 2: Ingest ────────────────────────────────────────────────────

    println!("\n=== Section 2: Ingest ===");

    // Simulate a 10-minute window of activity with explicit timestamps.
    // T=0 is 10 minutes ago; each step is 60 s.
    let base = now_secs().saturating_sub(600); // 10 min ago
    let t = |minutes: u64| base + minutes * 60;

    // api.login fires frequently throughout the window.
    let login_events = [
        (0, "api.login"), (1, "api.login"), (2, "api.login"),
        (3, "api.login"), (5, "api.login"), (7, "api.login"),
        (8, "api.login"), (9, "api.login"),
    ];
    for (min, id) in login_events {
        ft.add_with_timestamp(t(min), id).unwrap();
    }

    // api.search fires less often.
    for min in [1u64, 4, 6, 9] {
        ft.add_with_timestamp(t(min), "api.search").unwrap();
    }

    // api.export fires rarely.
    ft.add_with_timestamp(t(3), "api.export").unwrap();
    ft.add_with_timestamp(t(8), "api.export").unwrap();

    // alert.cpu fires in a burst mid-window.
    for min in [4u64, 5, 5, 6] {   // t=5 fires twice (burst)
        ft.add_with_timestamp(t(min), "alert.cpu").unwrap();
    }

    // alert.disk fires once.
    ft.add_with_timestamp(t(7), "alert.disk").unwrap();

    // drain.cluster.0 and drain.cluster.1 — template-cluster IDs being tracked.
    for min in [0u64, 2, 4, 6, 8] {
        ft.add_with_timestamp(t(min), "drain.cluster.0").unwrap();
    }
    for min in [1u64, 3, 5, 7, 9] {
        ft.add_with_timestamp(t(min), "drain.cluster.1").unwrap();
    }

    // A few live events (current time) for the recent() demo.
    ft.add("api.login").unwrap();
    ft.add("api.search").unwrap();
    ft.add("drain.cluster.0").unwrap();

    println!("Recorded synthetic events over a 10-minute window + 3 live events");

    // ── Section 3: by_id ─────────────────────────────────────────────────────

    println!("\n=== Section 3: by_id ===");

    let ids_to_inspect = [
        "api.login",
        "api.search",
        "api.export",
        "alert.cpu",
        "drain.cluster.0",
    ];

    for id in ids_to_inspect {
        let timestamps = ft.by_id(id).unwrap();
        println!(
            "  {:20}  {} occurrences  {:?}",
            id,
            timestamps.len(),
            timestamps
                .iter()
                .map(|ts| format!("T+{}m", ts.saturating_sub(base) / 60))
                .collect::<Vec<_>>()
        );
    }

    // ── Section 4: by_timestamp ───────────────────────────────────────────────

    println!("\n=== Section 4: by_timestamp ===");

    let probe_minutes = [0u64, 1, 3, 5, 8];
    for min in probe_minutes {
        let ts = t(min);
        let ids = ft.by_timestamp(ts).unwrap();
        println!("  T+{min}min  →  {:?}", ids);
    }

    // ── Section 5: time_range ────────────────────────────────────────────────

    println!("\n=== Section 5: time_range ===");

    let windows = [
        ("first 3 minutes",  t(0),  t(2)),
        ("middle 4 minutes", t(3),  t(6)),
        ("last 3 minutes",   t(7),  t(9)),
        ("alert burst",      t(4),  t(6)),
    ];

    for (label, start, end) in windows {
        let ids = ft.time_range(start, end).unwrap();
        println!("  [{label}]  {:?}", ids);
    }

    // ── Section 6: recent ────────────────────────────────────────────────────

    println!("\n=== Section 6: recent ===");

    let durations = ["30s", "5min", "15min", "1h"];
    for dur in durations {
        let ids = ft.recent(dur).unwrap();
        println!("  recent({dur:<6})  →  {} IDs: {:?}", ids.len(), ids);
    }

    // ── Section 7: Sync ──────────────────────────────────────────────────────

    println!("\n=== Section 7: Sync ===");

    ft.sync().unwrap();
    println!("CHECKPOINT written to {:?}", db_path);

    println!("\nDone.");
}
