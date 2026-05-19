//! Tests for `bdslib::vm::result_queue::ResultQueue`.

use bdslib::vm::result_queue::ResultQueue;
use rust_dynamic::value::Value;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

// ── push / pop / len ──────────────────────────────────────────────────────────

#[test]
fn pop_on_unknown_id_returns_none() {
    let q = ResultQueue::new();
    assert!(q.pop(Uuid::now_v7()).is_none());
}

#[test]
fn len_on_unknown_id_returns_zero() {
    let q = ResultQueue::new();
    assert_eq!(q.len(Uuid::now_v7()), 0);
}

#[test]
fn push_then_pop_returns_same_value() {
    let q = ResultQueue::new();
    let id = Uuid::now_v7();
    q.push(id, Value::from_int(42));
    let popped = q.pop(id).expect("should have a value");
    assert_eq!(popped.cast_int().unwrap(), 42);
}

#[test]
fn push_creates_queue_and_stamps_timestamp() {
    let q = ResultQueue::new();
    let id = Uuid::now_v7();
    assert!(q.created_at(id).is_none(), "no queue before push");
    q.push(id, Value::from_int(1));
    let ts = q.created_at(id).expect("queue created on push");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(now >= ts && now - ts <= 2, "timestamp ~= now (got {ts}, now {now})");
}

#[test]
fn fifo_order_preserved_within_queue() {
    let q = ResultQueue::new();
    let id = Uuid::now_v7();
    for i in 1..=5i64 {
        q.push(id, Value::from_int(i));
    }
    assert_eq!(q.len(id), 5);
    for expected in 1..=5i64 {
        let v = q.pop(id).unwrap();
        assert_eq!(v.cast_int().unwrap(), expected, "FIFO violated");
    }
    assert_eq!(q.len(id), 0);
    assert!(q.pop(id).is_none(), "extra pop on drained queue is None");
}

#[test]
fn separate_ids_keep_separate_queues() {
    let q = ResultQueue::new();
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    q.push(a, Value::from_string("alpha"));
    q.push(b, Value::from_string("beta"));
    q.push(a, Value::from_string("alpha-2"));

    assert_eq!(q.len(a), 2);
    assert_eq!(q.len(b), 1);
    assert_eq!(q.n_queues(), 2);

    assert_eq!(q.pop(a).unwrap().cast_string().unwrap(), "alpha");
    assert_eq!(q.pop(b).unwrap().cast_string().unwrap(), "beta");
    assert_eq!(q.pop(a).unwrap().cast_string().unwrap(), "alpha-2");
}

#[test]
fn empty_queue_is_kept_until_swept() {
    // Draining a queue does not remove it — the creation timestamp persists
    // so subsequent pushes share the same TTL window.
    let q = ResultQueue::new();
    let id = Uuid::now_v7();
    q.push(id, Value::from_int(1));
    let _ = q.pop(id);
    assert_eq!(q.len(id), 0);
    assert_eq!(q.n_queues(), 1, "drained queue still tracked");
    assert!(q.created_at(id).is_some());
}

#[test]
fn n_queues_and_ids_track_population() {
    let q = ResultQueue::new();
    assert_eq!(q.n_queues(), 0);
    assert!(q.ids().is_empty());

    let ids: Vec<Uuid> = (0..3).map(|_| Uuid::now_v7()).collect();
    for id in &ids {
        q.push(*id, Value::from_int(1));
    }
    assert_eq!(q.n_queues(), 3);
    let mut listed = q.ids();
    listed.sort();
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(listed, expected);
}

// ── sweep_expired ─────────────────────────────────────────────────────────────

#[test]
fn sweep_with_zero_ttl_is_noop() {
    let q = ResultQueue::new();
    let id = Uuid::now_v7();
    q.push(id, Value::from_int(1));
    let evicted = q.sweep_expired(0);
    assert_eq!(evicted, 0);
    assert_eq!(q.n_queues(), 1, "ttl=0 must not evict");
}

#[test]
fn sweep_keeps_fresh_queues() {
    let q = ResultQueue::new();
    let id = Uuid::now_v7();
    q.push(id, Value::from_int(1));
    let evicted = q.sweep_expired(3600);
    assert_eq!(evicted, 0);
    assert_eq!(q.len(id), 1);
}

#[test]
fn sweep_evicts_aged_queues() {
    // Sleep ~2s so a 1-second TTL fires reliably.
    let q = ResultQueue::new();
    let id = Uuid::now_v7();
    q.push(id, Value::from_int(1));
    thread::sleep(Duration::from_millis(2200));
    let evicted = q.sweep_expired(1);
    assert_eq!(evicted, 1, "aged queue must be evicted");
    assert_eq!(q.n_queues(), 0);
    assert!(q.pop(id).is_none());
}

#[test]
fn sweep_only_evicts_expired_queues_not_fresh_ones() {
    let q = ResultQueue::new();
    let old = Uuid::now_v7();
    q.push(old, Value::from_int(1));
    thread::sleep(Duration::from_millis(2200));

    let new = Uuid::now_v7();
    q.push(new, Value::from_int(2));

    let evicted = q.sweep_expired(1);
    assert_eq!(evicted, 1);
    assert!(q.pop(old).is_none(), "old queue gone");
    assert_eq!(q.len(new), 1, "fresh queue retained");
}

// ── thread-safety smoke test ──────────────────────────────────────────────────

#[test]
fn concurrent_pushers_do_not_lose_values() {
    let q = ResultQueue::new();
    let id = Uuid::now_v7();
    let n_threads = 8;
    let n_per_thread = 100i64;

    let mut handles = Vec::new();
    for _ in 0..n_threads {
        let q = q.clone();
        handles.push(thread::spawn(move || {
            for i in 0..n_per_thread {
                q.push(id, Value::from_int(i));
            }
        }));
    }
    for h in handles { h.join().unwrap(); }

    assert_eq!(q.len(id), (n_threads * n_per_thread as usize));
    let mut drained = 0;
    while q.pop(id).is_some() { drained += 1; }
    assert_eq!(drained, n_threads * n_per_thread as usize);
}

// ── round-trip JSON values ────────────────────────────────────────────────────

#[test]
fn json_typed_value_round_trips() {
    let q = ResultQueue::new();
    let id = Uuid::now_v7();
    let payload = serde_json::json!({ "name": "ok", "count": 7, "items": [1, 2, 3] });
    q.push(id, Value::json(payload.clone()));
    let popped = q.pop(id).expect("should have a value");
    assert_eq!(popped.cast_json().unwrap(), payload);
}
