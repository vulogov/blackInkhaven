/// cache_json_demo — Demonstrate JsonCache: a fixed-size, TTL-based in-memory
/// cache mapping `(id, timestamp)` → `JsonValue`.
///
/// Sections:
///   1. Setup           — create a cache and inspect initial state
///   2. Basic I/O       — insert, get, remove
///   3. TTL expiry      — lazy and eager expiry paths
///   4. Capacity        — random eviction when the cache is full
///   5. Clone / sharing — multiple handles to the same store
///   6. Concurrent use  — parallel writers and readers
///   7. Background sweep — watch the cleanup thread work
use bdslib::common::cache_json::JsonCache;
use serde_json::json;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    // ── Section 1: Setup ─────────────────────────────────────────────────────

    println!("=== Section 1: Setup ===");

    let cache = JsonCache::new(8, Duration::from_secs(30));
    println!(
        "Created JsonCache  capacity={} ttl={:?}",
        cache.capacity(),
        cache.ttl()
    );
    println!("len={} is_empty={}", cache.len(), cache.is_empty());

    // ── Section 2: Basic I/O ─────────────────────────────────────────────────

    println!("\n=== Section 2: Basic I/O ===");

    let base_ts: u64 = 1_700_000_000;

    // Insert several (id, timestamp) → value pairs.
    let docs = vec![
        ("shard-a:uuid-1", base_ts,     json!({"host": "web-1", "level": "INFO",  "msg": "request ok"})),
        ("shard-a:uuid-2", base_ts + 1, json!({"host": "web-2", "level": "WARN",  "msg": "high latency"})),
        ("shard-b:uuid-3", base_ts + 2, json!({"host": "db-1",  "level": "ERROR", "msg": "connection refused"})),
        ("shard-b:uuid-4", base_ts + 3, json!({"host": "db-2",  "level": "INFO",  "msg": "query ok"})),
    ];
    for (id, ts, val) in &docs {
        cache.insert(*id, *ts, val.clone());
        println!("  insert ({id:20}, ts={ts})");
    }
    println!("len after inserts: {}", cache.len());

    // Lookups
    println!("\nGet hits:");
    for (id, ts, _) in &docs {
        if let Some(v) = cache.get(id, *ts) {
            println!("  ({id}, ts={ts})  →  {v}");
        }
    }

    println!("\nGet misses:");
    println!("  wrong id     → {:?}", cache.get("no-such-id", base_ts));
    println!("  wrong ts     → {:?}", cache.get("shard-a:uuid-1", base_ts + 999));

    // Update an existing key (resets its TTL, no eviction).
    cache.insert("shard-a:uuid-1", base_ts, json!({"updated": true}));
    println!(
        "\nAfter updating shard-a:uuid-1: {:?}",
        cache.get("shard-a:uuid-1", base_ts)
    );
    println!("len unchanged: {}", cache.len());

    // Remove
    let removed = cache.remove("shard-b:uuid-3", base_ts + 2);
    println!("\nRemoved shard-b:uuid-3: {:?}", removed);
    println!("len after remove: {}", cache.len());

    // ── Section 3: TTL expiry ────────────────────────────────────────────────

    println!("\n=== Section 3: TTL expiry ===");

    let short = JsonCache::with_cleanup_interval(
        100,
        Duration::from_millis(150), // entries live 150 ms
        Duration::from_secs(60),    // background sweeps every 60 s
    );

    short.insert("ephemeral", base_ts, json!({"ttl": "150ms"}));
    println!("Inserted ephemeral entry  len={}", short.len());

    println!("Immediate get: {:?}", short.get("ephemeral", base_ts));

    thread::sleep(Duration::from_millis(200));

    // Lazy eviction: get detects the expired entry and removes it.
    println!("Get after 200 ms (lazy evict): {:?}", short.get("ephemeral", base_ts));
    println!("len after lazy evict: {}", short.len());

    // Eager eviction via evict_expired().
    short.insert("a", base_ts,     json!(1));
    short.insert("b", base_ts + 1, json!(2));
    thread::sleep(Duration::from_millis(200));
    println!(
        "\nBefore eager evict  len={}  expired={}",
        short.len(),
        short.expired_count()
    );
    short.evict_expired();
    println!(
        "After  eager evict  len={}  expired={}",
        short.len(),
        short.expired_count()
    );

    // ── Section 4: Capacity ──────────────────────────────────────────────────

    println!("\n=== Section 4: Capacity ===");

    let small = JsonCache::new(3, Duration::from_secs(3600));
    small.insert("slot-1", base_ts,     json!(1));
    small.insert("slot-2", base_ts + 1, json!(2));
    small.insert("slot-3", base_ts + 2, json!(3));
    println!("Filled 3-slot cache  len={}", small.len());

    for extra in 0..4u64 {
        small.insert(format!("extra-{extra}"), base_ts + 10 + extra, json!(extra));
        println!(
            "  inserted extra-{extra}  len={}  (random eviction occurred)",
            small.len()
        );
    }

    println!("Final len (must be ≤ 3): {}", small.len());

    // ── Section 5: Clone / sharing ───────────────────────────────────────────

    println!("\n=== Section 5: Clone / sharing ===");

    let original = JsonCache::new(50, Duration::from_secs(3600));
    let handle_a = original.clone();
    let handle_b = original.clone();

    original.insert("shared-key", base_ts, json!({"origin": "original"}));
    println!(
        "handle_a sees: {:?}",
        handle_a.get("shared-key", base_ts)
    );

    handle_b.insert("from-b", base_ts + 1, json!({"origin": "handle_b"}));
    println!(
        "original sees from-b: {:?}",
        original.get("from-b", base_ts + 1)
    );

    handle_a.clear();
    println!(
        "After clear via handle_a  original.len={}",
        original.len()
    );

    // ── Section 6: Concurrent use ────────────────────────────────────────────

    println!("\n=== Section 6: Concurrent use ===");

    let concurrent = Arc::new(JsonCache::new(200, Duration::from_secs(3600)));
    let mut handles = vec![];

    // 8 writer threads × 25 keys each = 200 total (exactly at capacity).
    for t in 0u64..8 {
        let c = Arc::clone(&concurrent);
        handles.push(thread::spawn(move || {
            for i in 0u64..25 {
                let id = format!("t{t}-doc-{i}");
                c.insert(id, base_ts + i, json!({"thread": t, "seq": i}));
            }
        }));
    }

    // 4 reader threads
    for r in 0u64..4 {
        let c = Arc::clone(&concurrent);
        handles.push(thread::spawn(move || {
            for i in 0u64..25 {
                let _ = c.get(&format!("t{r}-doc-{i}"), base_ts + i);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
    println!("Concurrent ops complete  len={}", concurrent.len());

    // ── Section 7: Background sweep ──────────────────────────────────────────

    println!("\n=== Section 7: Background sweep ===");

    let bg = JsonCache::with_cleanup_interval(
        100,
        Duration::from_millis(100), // entries expire after 100 ms
        Duration::from_millis(150), // background sweeps every 150 ms
    );

    for i in 0u64..10 {
        bg.insert(format!("bg-{i}"), base_ts + i, json!(i));
    }
    println!("Inserted 10 entries  len={}", bg.len());

    // Let entries expire and the background thread sweep them.
    thread::sleep(Duration::from_millis(400));

    // Background thread should have already cleared these.
    println!(
        "After 400 ms  len={}  expired={}",
        bg.len(),
        bg.expired_count()
    );
    for i in 0u64..3 {
        println!(
            "  bg-{i}: {:?}",
            bg.get(&format!("bg-{i}"), base_ts + i)
        );
    }

    println!("\nDone.");
}
