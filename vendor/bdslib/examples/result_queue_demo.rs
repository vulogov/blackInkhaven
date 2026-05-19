/// result_queue_demo — Per-id FIFO queues of `rust_dynamic` values.
///
/// Walks the public API of `bdslib::vm::result_queue::ResultQueue`:
///
///   1. push / pop / len / FIFO ordering
///   2. JSON-typed values round-trip via `Value::json`
///   3. Multiple queues kept separately by id
///   4. TTL sweep evicts aged queues and leaves fresh ones untouched
///
/// Run with:
///
///     cargo run --example result_queue_demo

use bdslib::vm::result_queue::ResultQueue;
use rust_dynamic::value::Value;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

fn print_section(title: &str) {
    println!("\n=== {title} ===");
}

fn main() {
    // ── Section 1: Basic FIFO ────────────────────────────────────────────────

    print_section("Section 1 — Basic FIFO");

    let q = ResultQueue::new();
    let id = Uuid::now_v7();
    println!("queue id = {id}");

    for i in 1..=4i64 {
        q.push(id, Value::from_int(i * 10));
    }
    println!("len after 4 pushes = {}", q.len(id));
    println!("creation timestamp = {:?}", q.created_at(id));

    while let Some(v) = q.pop(id) {
        println!("  popped → {}", v.cast_int().unwrap());
    }
    println!("len after drain = {} (queue still tracked: {})",
        q.len(id), q.created_at(id).is_some());

    // ── Section 2: JSON-typed values ─────────────────────────────────────────

    print_section("Section 2 — JSON-typed values");

    let id = Uuid::now_v7();
    let payload = serde_json::json!({
        "kind":   "alert",
        "metric": "cpu.usage",
        "value":  87.5,
        "tags":   ["prod", "web"]
    });
    q.push(id, Value::json(payload.clone()));
    let popped = q.pop(id).expect("value");
    let round_tripped = popped.cast_json().expect("json");
    println!("pushed:        {payload}");
    println!("round-tripped: {round_tripped}");
    assert_eq!(payload, round_tripped, "JSON payload must round-trip exactly");

    // ── Section 3: Multiple queues ───────────────────────────────────────────

    print_section("Section 3 — Multiple queues");

    let alpha = Uuid::now_v7();
    let beta  = Uuid::now_v7();
    q.push(alpha, Value::from_string("alpha-1"));
    q.push(beta,  Value::from_string("beta-1"));
    q.push(alpha, Value::from_string("alpha-2"));

    println!("n_queues = {}", q.n_queues());
    println!("len(alpha) = {}, len(beta) = {}", q.len(alpha), q.len(beta));

    let ids = q.ids();
    println!("registered ids ({}):", ids.len());
    for id in &ids {
        println!("  {id} → len={}", q.len(*id));
    }

    // ── Section 4: TTL sweep ─────────────────────────────────────────────────

    print_section("Section 4 — TTL sweep");

    let stale = Uuid::now_v7();
    q.push(stale, Value::from_int(99));
    println!("pushed into stale queue ({stale}); waiting 2.2s …");
    thread::sleep(Duration::from_millis(2200));

    let fresh = Uuid::now_v7();
    q.push(fresh, Value::from_int(100));
    println!("pushed into fresh queue ({fresh})");

    let evicted = q.sweep_expired(1);
    println!("sweep(ttl=1s) evicted {evicted} queue(s)");
    println!("stale queue still present? {} (expect false)", q.created_at(stale).is_some());
    println!("fresh queue still present? {} (expect true)",  q.created_at(fresh).is_some());

    // sweep with ttl=0 is a no-op
    let evicted = q.sweep_expired(0);
    println!("sweep(ttl=0) evicted {evicted} queue(s) (expect 0)");

    println!("\nDone.");
}
