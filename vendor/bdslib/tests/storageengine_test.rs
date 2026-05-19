use bdslib::storageengine::StorageEngine;
use rayon::prelude::*;
use std::sync::Arc;
use tempfile::TempDir;

const INIT_SQL: &str = "
    CREATE TABLE test_data (
        id INTEGER PRIMARY KEY,
        name TEXT,
        score DOUBLE,
        payload BLOB
    );
    INSERT INTO test_data VALUES (1, 'Initial', 1.0, 'seed');
";

const TYPE_SCHEMA_SQL: &str = "
    CREATE TABLE types (
        b   BOOLEAN,
        i   INTEGER,
        bi  BIGINT,
        f   FLOAT,
        d   DOUBLE,
        t   TEXT,
        bl  BLOB,
        n   INTEGER
    );
    INSERT INTO types VALUES (true, 42, 9000000000, 1.5, 2.718281828, 'hello', X'DEADBEEF', NULL);
";

// --- lifecycle ---

#[test]
fn test_full_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let engine = StorageEngine::new(tmp.path().join("test.db"), INIT_SQL, 4).expect("engine init failed");

    engine
        .execute("INSERT INTO test_data VALUES (2, 'Second', 2.5, 'more')")
        .unwrap();

    let rows = engine
        .select_all("SELECT name FROM test_data WHERE id = 2")
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].len(), 1);
    assert_eq!(rows[0][0].cast_string().unwrap(), "Second");

    let mut count = 0usize;
    engine
        .select_foreach("SELECT * FROM test_data", |_row| {
            count += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn test_select_all_multi_column() {
    let engine = StorageEngine::new(":memory:", INIT_SQL, 4).unwrap();
    let rows = engine
        .select_all("SELECT id, name, score FROM test_data WHERE id = 1")
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].len(), 3);
    assert_eq!(rows[0][0].cast_int().unwrap(), 1);
    assert_eq!(rows[0][1].cast_string().unwrap(), "Initial");
    assert!((rows[0][2].cast_float().unwrap() - 1.0).abs() < 1e-10);
}

// --- empty results ---

#[test]
fn test_select_all_empty() {
    let engine = StorageEngine::new(":memory:", INIT_SQL, 4).unwrap();
    let rows = engine
        .select_all("SELECT * FROM test_data WHERE id = 999")
        .unwrap();
    assert!(rows.is_empty());
}

#[test]
fn test_select_foreach_empty() {
    let engine = StorageEngine::new(":memory:", INIT_SQL, 4).unwrap();
    let mut called = false;
    engine
        .select_foreach("SELECT * FROM test_data WHERE id = 999", |_row| {
            called = true;
            Ok(())
        })
        .unwrap();
    assert!(!called);
}

// --- error paths ---

#[test]
fn test_init_invalid_sql() {
    assert!(StorageEngine::new(":memory:", "THIS IS NOT VALID SQL;", 4).is_err());
}

#[test]
fn test_select_foreach_callback_error_stops_iteration() {
    let engine = StorageEngine::new(":memory:", INIT_SQL, 4).unwrap();
    engine
        .execute("INSERT INTO test_data VALUES (2, 'B', 2.0, NULL)")
        .unwrap();

    let mut calls = 0usize;
    let result = engine.select_foreach("SELECT * FROM test_data", |_row| {
        calls += 1;
        Err(easy_error::err_msg("stop"))
    });
    assert!(result.is_err());
    assert_eq!(calls, 1);
}

// --- sync ---

#[test]
fn test_sync_does_not_error() {
    let tmp = TempDir::new().unwrap();
    let engine = StorageEngine::new(tmp.path().join("test.db"), INIT_SQL, 4).unwrap();
    engine
        .execute("INSERT INTO test_data VALUES (2, 'B', 2.0, NULL)")
        .unwrap();
    engine.sync().unwrap();
    let rows = engine
        .select_all("SELECT count(*) FROM test_data")
        .unwrap();
    assert_eq!(rows[0][0].cast_int().unwrap(), 2);
}

// --- type coverage ---

#[test]
fn test_type_boolean() {
    let engine = StorageEngine::new(":memory:", TYPE_SCHEMA_SQL, 4).unwrap();
    let rows = engine.select_all("SELECT b FROM types").unwrap();
    assert_eq!(rows[0][0].cast_bool().unwrap(), true);
}

#[test]
fn test_type_integer() {
    let engine = StorageEngine::new(":memory:", TYPE_SCHEMA_SQL, 4).unwrap();
    let rows = engine.select_all("SELECT i FROM types").unwrap();
    assert_eq!(rows[0][0].cast_int().unwrap(), 42);
}

#[test]
fn test_type_bigint() {
    let engine = StorageEngine::new(":memory:", TYPE_SCHEMA_SQL, 4).unwrap();
    let rows = engine.select_all("SELECT bi FROM types").unwrap();
    assert_eq!(rows[0][0].cast_int().unwrap(), 9_000_000_000);
}

#[test]
fn test_type_float() {
    let engine = StorageEngine::new(":memory:", TYPE_SCHEMA_SQL, 4).unwrap();
    let rows = engine.select_all("SELECT f FROM types").unwrap();
    assert!((rows[0][0].cast_float().unwrap() - 1.5).abs() < 1e-6);
}

#[test]
fn test_type_double() {
    let engine = StorageEngine::new(":memory:", TYPE_SCHEMA_SQL, 4).unwrap();
    let rows = engine.select_all("SELECT d FROM types").unwrap();
    assert!((rows[0][0].cast_float().unwrap() - 2.718_281_828).abs() < 1e-9);
}

#[test]
fn test_type_text() {
    let engine = StorageEngine::new(":memory:", TYPE_SCHEMA_SQL, 4).unwrap();
    let rows = engine.select_all("SELECT t FROM types").unwrap();
    assert_eq!(rows[0][0].cast_string().unwrap(), "hello");
}

#[test]
fn test_type_blob() {
    let engine = StorageEngine::new(":memory:", TYPE_SCHEMA_SQL, 4).unwrap();
    let rows = engine.select_all("SELECT bl FROM types").unwrap();
    assert_eq!(rows[0][0].type_name(), "Binary");
    assert!(!rows[0][0].cast_bin().unwrap().is_empty());
}

#[test]
fn test_type_null_maps_to_nodata() {
    let engine = StorageEngine::new(":memory:", TYPE_SCHEMA_SQL, 4).unwrap();
    let rows = engine.select_all("SELECT n FROM types").unwrap();
    assert_eq!(rows[0][0].type_name(), "NODATA");
}

// --- concurrency ---

#[test]
fn test_concurrent_access() {
    let tmp = TempDir::new().unwrap();
    let engine =
        Arc::new(StorageEngine::new(tmp.path().join("test.db"), INIT_SQL, 4).expect("engine init failed"));

    (0..100).into_par_iter().for_each(|i| {
        let e = engine.clone();
        if i % 5 == 0 {
            let sql = format!(
                "INSERT INTO test_data (id, name) VALUES ({}, 'Thread-{}')",
                i + 10,
                i
            );
            e.execute(&sql).expect("concurrent write failed");
        } else {
            e.select_all("SELECT count(*) FROM test_data")
                .expect("concurrent read failed");
        }
    });

    let rows = engine
        .select_all("SELECT count(*) FROM test_data")
        .unwrap();
    assert_eq!(rows[0][0].cast_int().unwrap(), 21);
}
