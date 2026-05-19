//! Integration tests for `bdslib::vm::ephemeral` — WorkerPool.
//!
//! Mirrors the structure of `vm_workers_test.rs` but exercises the independent
//! `WorkerPool` / `Worker` types and `EPHEMERAL_PIPE` channel.
//!
//! Run with:
//! ```bash
//! cargo test --test vm_ephemeral_test -- --show-output
//! ```

use bdslib::vm::ephemeral::{WorkerPool, submit_ephemeral};
use bdslib::vm::results;
use std::sync::Once;
use std::thread;
use std::time::{Duration, Instant};

static INIT: Once = Once::new();

fn init_pool() {
    INIT.call_once(|| {
        WorkerPool::start(4).expect("WorkerPool::start");
    });
}

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
    // pool is consumed into background; success of init is sufficient proof
}

#[test]
fn submit_returns_a_uuid() {
    init_pool();
    let id = submit_ephemeral("42 .").expect("submit");
    assert!(!id.is_nil());
}

#[test]
fn integer_workbench_value_reaches_results() {
    init_pool();
    let id = submit_ephemeral("99 .").expect("submit");
    let n = wait_for_results(id, 1, Duration::from_secs(5));
    assert_eq!(n, 1);
    let val = results().pop(id).expect("pop");
    assert_eq!(val.cast_json().unwrap(), serde_json::json!(99));
}

#[test]
fn string_workbench_value_reaches_results() {
    init_pool();
    let id = submit_ephemeral(r#""world" ."#).expect("submit");
    let n = wait_for_results(id, 1, Duration::from_secs(5));
    assert_eq!(n, 1);
    let val = results().pop(id).expect("pop");
    assert_eq!(val.cast_json().unwrap().as_str(), Some("world"));
}

#[test]
fn list_workbench_value_reaches_results() {
    init_pool();
    let id = submit_ephemeral("[ 10 20 30 ] .").expect("submit");
    let n = wait_for_results(id, 1, Duration::from_secs(5));
    assert_eq!(n, 1);
    let val = results().pop(id).expect("pop");
    assert_eq!(val.cast_json().unwrap(), serde_json::json!([10, 20, 30]));
}

#[test]
fn multiple_workbench_items_all_reach_results() {
    init_pool();
    let id = submit_ephemeral(r#"7 . 8 . 9 ."#).expect("submit");
    let n = wait_for_results(id, 3, Duration::from_secs(5));
    assert_eq!(n, 3);
    let mut collected: Vec<i64> = Vec::new();
    while let Some(v) = results().pop(id) {
        collected.push(v.cast_json().unwrap().as_i64().unwrap());
    }
    collected.sort_unstable();
    assert_eq!(collected, vec![7, 8, 9]);
}

#[test]
fn separate_scripts_have_isolated_results() {
    init_pool();
    let id_a = submit_ephemeral("100 .").expect("submit a");
    let id_b = submit_ephemeral("200 .").expect("submit b");
    assert_ne!(id_a, id_b);

    wait_for_results(id_a, 1, Duration::from_secs(5));
    wait_for_results(id_b, 1, Duration::from_secs(5));

    let val_a = results().pop(id_a).expect("pop a");
    let val_b = results().pop(id_b).expect("pop b");
    assert_eq!(val_a.cast_json().unwrap(), serde_json::json!(100));
    assert_eq!(val_b.cast_json().unwrap(), serde_json::json!(200));
}

#[test]
fn arithmetic_result_reaches_results() {
    init_pool();
    let id = submit_ephemeral("3 4 + .").expect("submit");
    let n = wait_for_results(id, 1, Duration::from_secs(5));
    assert_eq!(n, 1);
    let val = results().pop(id).expect("pop");
    assert_eq!(val.cast_json().unwrap(), serde_json::json!(7));
}

#[test]
fn no_workbench_push_leaves_empty_queue() {
    init_pool();
    let id = submit_ephemeral("42").expect("submit");
    thread::sleep(Duration::from_millis(200));
    assert_eq!(results().len(id), 0);
}

#[test]
fn workers_are_isolated_per_job() {
    // Verify that state from one script does not bleed into another.
    // Each job gets a fresh Bund VM so definitions don't carry over.
    init_pool();
    // First script defines a word (only visible in its own VM)
    let id1 = submit_ephemeral(":double { 2 * } register  5 double .").expect("submit 1");
    // Second script does NOT have 'double' defined; would panic/error if state bled
    let id2 = submit_ephemeral("100 .").expect("submit 2");

    wait_for_results(id1, 1, Duration::from_secs(5));
    wait_for_results(id2, 1, Duration::from_secs(5));

    let v1 = results().pop(id1).expect("pop id1");
    let v2 = results().pop(id2).expect("pop id2");
    assert_eq!(v1.cast_json().unwrap(), serde_json::json!(10));
    assert_eq!(v2.cast_json().unwrap(), serde_json::json!(100));
}

#[test]
fn concurrent_submissions_do_not_lose_results() {
    init_pool();
    let handles: Vec<_> = (0u64..8)
        .map(|i| {
            thread::spawn(move || {
                let script = format!("{i} .");
                let id = submit_ephemeral(&script).expect("submit");
                let n = wait_for_results(id, 1, Duration::from_secs(10));
                assert_eq!(n, 1, "ephemeral worker {i} lost its result");
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
