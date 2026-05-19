//! `WorkerPool` (ephemeral) demo — same as `workers_demo` but uses the
//! independent `EPHEMERAL_PIPE` channel and shows per-job VM isolation.
//!
//! ```bash
//! cargo run --example ephemeral_demo
//! ```

use bdslib::vm::ephemeral::{WorkerPool, submit_ephemeral};
use bdslib::vm::results;
use std::thread;
use std::time::{Duration, Instant};

fn poll(id: uuid::Uuid, expect: usize, timeout: Duration) -> Vec<serde_json::Value> {
    let deadline = Instant::now() + timeout;
    loop {
        if results().len(id) >= expect || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let mut out = Vec::new();
    while let Some(v) = results().pop(id) {
        out.push(v.cast_json().unwrap_or(serde_json::Value::Null));
    }
    out
}

fn main() {
    env_logger::init();

    let pool = WorkerPool::start(4).expect("pool start");
    println!("Started WorkerPool with {} workers", pool.n_workers());

    // --- Example 1: basic arithmetic ---
    let id = submit_ephemeral("2 3 + .").expect("submit");
    println!("Submitted '2 3 + .'  id={id}");
    let res = poll(id, 1, Duration::from_secs(5));
    println!("  results: {res:?}");   // [5]

    // --- Example 2: VM isolation — define a word in one job ---
    let id1 = submit_ephemeral(":square { dup * } register  9 square .").expect("submit 1");
    // Second job runs in a fresh VM — 'square' is unknown here, which is fine.
    let id2 = submit_ephemeral("7 7 * .").expect("submit 2");
    println!("Submitted VM-isolation pair");
    let r1 = poll(id1, 1, Duration::from_secs(5));
    let r2 = poll(id2, 1, Duration::from_secs(5));
    println!("  job1 (9²): {r1:?}");    // [81]
    println!("  job2 (7×7): {r2:?}");   // [49]

    // --- Example 3: string result ---
    let id = submit_ephemeral(r#""ephemeral pool" ."#).expect("submit");
    let res = poll(id, 1, Duration::from_secs(5));
    println!("String result: {res:?}");

    // --- Example 4: list result ---
    let id = submit_ephemeral("[ 1 2 3 4 5 ] .").expect("submit");
    let res = poll(id, 1, Duration::from_secs(5));
    println!("List result: {res:?}");

    // --- Example 5: concurrent jobs ---
    println!("Concurrent batch (8 jobs) …");
    let ids: Vec<_> = (1u64..=8)
        .map(|i| {
            let id = submit_ephemeral(&format!("{i} .")).expect("submit");
            (i, id)
        })
        .collect();
    for (i, id) in ids {
        let res = poll(id, 1, Duration::from_secs(5));
        let val = res.first().and_then(|v| v.as_u64()).unwrap_or(u64::MAX);
        println!("  job {i}: got {val}");
    }

    println!("Done.");
}
