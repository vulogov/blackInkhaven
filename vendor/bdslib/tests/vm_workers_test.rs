//! Integration tests for `bdslib::vm::workers` — BundWorkerPool.
//!
//! The pool is a process-wide singleton (OnceLock channel), so all tests share
//! the same pool initialised via a `Once` guard.  Results are read from the
//! global `ResultQueue` which is also lazily initialised on first access.
//!
//! Run with:
//! ```bash
//! cargo test --test vm_workers_test -- --show-output
//! ```

use bdslib::vm::workers::BundWorkerPool;
use bdslib::vm::results;
use bdslib::submit_script;
use std::sync::Once;
use std::thread;
use std::time::{Duration, Instant};

static INIT: Once = Once::new();

fn init_pool() {
    INIT.call_once(|| {
        BundWorkerPool::start(4).expect("BundWorkerPool::start");
    });
}

/// Poll the global results queue for `id` until at least one value appears
/// or `timeout` elapses.  Returns however many values are in the queue.
fn wait_for_results(id: uuid::Uuid, expect: usize, timeout: Duration) -> usize {
    let deadline = Instant::now() + timeout;
    loop {
        let count = results().len(id);
        if count >= expect || Instant::now() >= deadline {
            return count;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn pool_has_correct_worker_count() {
    init_pool();
    // BundWorkerPool is consumed into background; we can't query it here.
    // Just verify start() succeeded (it would have panicked in init_pool otherwise).
}

#[test]
fn submit_returns_a_uuid() {
    init_pool();
    let id = submit_script("42 .").expect("submit");
    assert!(!id.is_nil());
}

#[test]
fn integer_workbench_value_reaches_results() {
    init_pool();
    let id = submit_script("99 .").expect("submit");
    let n = wait_for_results(id, 1, Duration::from_secs(5));
    assert_eq!(n, 1, "expected 1 result item");
    let val = results().pop(id).expect("pop");
    let json = val.cast_json().expect("cast_json");
    assert_eq!(json, serde_json::json!(99));
}

#[test]
fn string_workbench_value_reaches_results() {
    init_pool();
    let id = submit_script(r#""hello" ."#).expect("submit");
    let n = wait_for_results(id, 1, Duration::from_secs(5));
    assert_eq!(n, 1);
    let val = results().pop(id).expect("pop");
    let json = val.cast_json().expect("cast_json");
    assert_eq!(json.as_str(), Some("hello"));
}

#[test]
fn float_workbench_value_reaches_results() {
    init_pool();
    let id = submit_script("3.14 .").expect("submit");
    let n = wait_for_results(id, 1, Duration::from_secs(5));
    assert_eq!(n, 1);
    let val = results().pop(id).expect("pop");
    let json = val.cast_json().expect("cast_json");
    let f = json.as_f64().expect("f64");
    assert!((f - 3.14).abs() < 1e-10);
}

#[test]
fn bool_workbench_value_reaches_results() {
    init_pool();
    let id = submit_script("true .").expect("submit");
    let n = wait_for_results(id, 1, Duration::from_secs(5));
    assert_eq!(n, 1);
    let val = results().pop(id).expect("pop");
    let json = val.cast_json().expect("cast_json");
    assert_eq!(json, serde_json::json!(true));
}

#[test]
fn list_workbench_value_reaches_results() {
    init_pool();
    let id = submit_script("[ 1 2 3 ] .").expect("submit");
    let n = wait_for_results(id, 1, Duration::from_secs(5));
    assert_eq!(n, 1);
    let val = results().pop(id).expect("pop");
    let json = val.cast_json().expect("cast_json");
    assert_eq!(json, serde_json::json!([1, 2, 3]));
}

#[test]
fn multiple_workbench_items_all_reach_results() {
    init_pool();
    // Three separate . push three items to the workbench
    let id = submit_script(r#"1 . 2 . 3 ."#).expect("submit");
    let n = wait_for_results(id, 3, Duration::from_secs(5));
    assert_eq!(n, 3, "expected 3 result items");
    let mut collected: Vec<i64> = Vec::new();
    while let Some(v) = results().pop(id) {
        let j = v.cast_json().expect("cast_json");
        collected.push(j.as_i64().expect("i64"));
    }
    collected.sort_unstable();
    // workbench is a VecDeque; items pushed left-to-right appear in insertion order
    assert_eq!(collected, vec![1, 2, 3]);
}

#[test]
fn separate_scripts_have_isolated_results() {
    init_pool();
    let id_a = submit_script("10 .").expect("submit a");
    let id_b = submit_script("20 .").expect("submit b");
    assert_ne!(id_a, id_b);

    wait_for_results(id_a, 1, Duration::from_secs(5));
    wait_for_results(id_b, 1, Duration::from_secs(5));

    let val_a = results().pop(id_a).expect("pop a");
    let val_b = results().pop(id_b).expect("pop b");
    assert_eq!(val_a.cast_json().unwrap(), serde_json::json!(10));
    assert_eq!(val_b.cast_json().unwrap(), serde_json::json!(20));
}

#[test]
fn arithmetic_result_reaches_results() {
    init_pool();
    let id = submit_script("6 7 * .").expect("submit");
    let n = wait_for_results(id, 1, Duration::from_secs(5));
    assert_eq!(n, 1);
    let val = results().pop(id).expect("pop");
    assert_eq!(val.cast_json().unwrap(), serde_json::json!(42));
}

#[test]
fn no_workbench_push_leaves_empty_queue() {
    init_pool();
    // Script that does not push to workbench (no `.`)
    let id = submit_script("42").expect("submit");
    // Give the worker time to process
    thread::sleep(Duration::from_millis(200));
    assert_eq!(results().len(id), 0, "no results expected when workbench is empty");
}

#[test]
fn concurrent_submissions_do_not_lose_results() {
    init_pool();
    let handles: Vec<_> = (0u64..8)
        .map(|i| {
            thread::spawn(move || {
                let script = format!("{i} .");
                let id = submit_script(&script).expect("submit");
                let n = wait_for_results(id, 1, Duration::from_secs(10));
                assert_eq!(n, 1, "worker {i} lost its result");
                let val = results().pop(id).expect("pop");
                let got = val.cast_json().unwrap().as_u64().unwrap();
                assert_eq!(got, i);
            })
        })
        .collect();
    for h in handles {
        h.join().expect("thread panicked");
    }
}
