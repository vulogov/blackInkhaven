//! `BundWorkerPool` demo — submit BUND scripts, collect results via RESULTS queue.
//!
//! ```bash
//! cargo run --example workers_demo
//! ```

use bdslib::submit_script;
use bdslib::vm::results;
use bdslib::vm::workers::BundWorkerPool;
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

    // Start a pool of 4 workers.
    let pool = BundWorkerPool::start(4).expect("pool start");
    println!("Started BundWorkerPool with {} workers", pool.n_workers());

    // --- Example 1: arithmetic ---
    let id = submit_script("6 7 * .").expect("submit");
    println!("Submitted '6 7 * .'  id={id}");
    let res = poll(id, 1, Duration::from_secs(5));
    println!("  results: {res:?}"); // [42]

    // --- Example 2: string on workbench ---
    let id = submit_script(r#""hello from BUND" ."#).expect("submit");
    println!("Submitted string push  id={id}");
    let res = poll(id, 1, Duration::from_secs(5));
    println!("  results: {res:?}");

    // --- Example 3: multiple workbench items ---
    let id = submit_script(r#"1 . 2 . 3 ."#).expect("submit");
    println!("Submitted '1 . 2 . 3 .'  id={id}");
    let res = poll(id, 3, Duration::from_secs(5));
    println!("  results: {res:?}"); // [1, 2, 3]

    // --- Example 4: list on workbench ---
    let id = submit_script("[ 10 20 30 ] .").expect("submit");
    println!("Submitted list push  id={id}");
    let res = poll(id, 1, Duration::from_secs(5));
    println!("  results: {res:?}");

    // --- Example 5: named function + recursion ---
    let id = submit_script(":is_true { true  { true } if } register is_true .").expect("submit");
    println!("Submitted is_true  id={id}");
    let res = poll(id, 1, Duration::from_secs(10));
    println!("  results: {res:?}"); // [21]

    // --- Example 6: concurrent batch ---
    println!("Submitting 8 scripts concurrently …");
    let ids: Vec<_> = (0u64..8)
        .map(|i| {
            let id = submit_script(&format!("{i} {i} * .", i = i)).expect("submit");
            (i, id)
        })
        .collect();
    for (i, id) in ids {
        let res = poll(id, 1, Duration::from_secs(5));
        let val = res.first().and_then(|v| v.as_u64()).unwrap_or(u64::MAX);
        println!("  {i}² = {val}");
    }

    println!("Done.");
}
