/// Tests for `bdslib::globals` — init_db / get_db / sync_db.
///
/// All assertions run sequentially inside a single test function because
/// `DB` is a process-wide `OnceLock`: once it is successfully initialised it
/// cannot be reset.  The ordering is:
///
///   error paths (no config, bad path, env var fallback)
///   → successful init
///   → get_db / sync_db after init
///   → duplicate-init rejection
use bdslib::{get_db, init_db, sync_db};

// ── hjson config helpers ──────────────────────────────────────────────────────

fn write_config(dir: &tempfile::TempDir) -> String {
    let db_path = dir.path().join("db");
    std::fs::create_dir_all(&db_path).unwrap();
    let config_path = dir.path().join("bds.hjson");
    std::fs::write(
        &config_path,
        format!(
            "{{\n  dbpath: \"{}\"\n  shard_duration: \"1h\"\n  pool_size: 2\n}}\n",
            db_path.display()
        ),
    )
    .unwrap();
    config_path.to_str().unwrap().to_string()
}

// ── test ──────────────────────────────────────────────────────────────────────

#[test]
fn test_globals_lifecycle() {
    // ── 1. get_db before init returns a descriptive error ────────────────────
    let err = get_db().err().unwrap().to_string();
    assert!(
        err.contains("not initialized"),
        "expected 'not initialized' in error, got: {err}"
    );

    // ── 2. sync_db before init is a no-op ────────────────────────────────────
    sync_db().expect("sync_db before init should succeed (no-op)");

    // ── 3. init_db(None) without BDS_CONFIG → error ──────────────────────────
    unsafe { std::env::remove_var("BDS_CONFIG") };
    let err = init_db(None).err().unwrap().to_string();
    assert!(
        err.contains("BDS_CONFIG"),
        "expected 'BDS_CONFIG' mention in error, got: {err}"
    );

    // ── 4. init_db(None) with BDS_CONFIG pointing to a missing file → error ──
    unsafe { std::env::set_var("BDS_CONFIG", "/nonexistent/__bds_test_config.hjson") };
    let err = init_db(None).err().unwrap().to_string();
    // Error originates from ShardsManager (cannot read config), not env-var resolution.
    assert!(
        !err.contains("BDS_CONFIG"),
        "error should be about missing file, not env var; got: {err}"
    );
    unsafe { std::env::remove_var("BDS_CONFIG") };

    // ── 5. init_db(Some) with a non-existent path → error ────────────────────
    let err = init_db(Some("/nonexistent/__bds_test_config.hjson"))
        .err()
        .unwrap()
        .to_string();
    assert!(
        err.contains("cannot read config") || err.contains("No such file"),
        "expected file-not-found error, got: {err}"
    );

    // ── 6. init_db(Some) with malformed hjson → error ────────────────────────
    let bad_dir = tempfile::TempDir::new().unwrap();
    let bad_cfg = bad_dir.path().join("bad.hjson");
    std::fs::write(&bad_cfg, "{ not valid hjson ??? }").unwrap();
    let err = init_db(Some(bad_cfg.to_str().unwrap()))
        .err()
        .unwrap()
        .to_string();
    assert!(
        err.contains("hjson") || err.contains("parse") || err.contains("invalid config"),
        "expected parse error, got: {err}"
    );

    // ── 7. init_db(Some) with a valid config → Ok ────────────────────────────
    let dir = tempfile::TempDir::new().unwrap();
    let config_path = write_config(&dir);
    init_db(Some(&config_path)).expect("init_db with valid config should succeed");

    // ── 8. get_db after init returns the instance ─────────────────────────────
    let db = get_db().expect("get_db should succeed after init_db");
    // Basic smoke-check: the ShardsCache is accessible.
    let _ = db.cache();

    // ── 9. sync_db after init flushes without error ───────────────────────────
    sync_db().expect("sync_db after init should succeed");

    // ── 10. init_db a second time → error "already initialized" ──────────────
    let err = init_db(Some(&config_path)).err().unwrap().to_string();
    assert!(
        err.contains("already initialized"),
        "expected 'already initialized' error on second init, got: {err}"
    );

    // ── 11. get_db still works after the failed second init ───────────────────
    get_db().expect("get_db should still work after rejected second init");

    // ── 12. init_db(None) after init with BDS_CONFIG set → still "already initialized"
    unsafe { std::env::set_var("BDS_CONFIG", &config_path) };
    let err = init_db(None).err().unwrap().to_string();
    assert!(
        err.contains("already initialized"),
        "expected 'already initialized', got: {err}"
    );
    unsafe { std::env::remove_var("BDS_CONFIG") };
}
