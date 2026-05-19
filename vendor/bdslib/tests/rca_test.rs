/// Tests for `bdslib::analysis::rca`.
///
/// All assertions run in a single function because `OnceLock<ShardsManager>`
/// cannot be reset between test runs in the same process.
///
/// The config uses `similarity_threshold: 2.0` so every ingested record is
/// stored as a primary regardless of embedding similarity — making primary
/// counts fully predictable without relying on fastembed behaviour.
///
/// Timestamp arithmetic is bucket-aligned: all incident-start timestamps are
/// exact multiples of `bucket_secs` so bucket assignment is deterministic
/// regardless of when `t0` falls within a 300-second window.
///
/// Ordering:
///   error paths  (before init_db)
///   → init_db
///   → empty window
///   → telemetry filtering
///   → two-cluster detection  {sshd, auditd} ↔ {nginx, postgres}
///   → causal ranking  disk_full (lead≈90s) > oom_killer (lead≈30s)
///   → RcaConfig overrides  (tight threshold, wide buckets)
///   → edge case: unknown failure key
///   → result metadata invariants

use bdslib::{get_db, init_db, EventCluster, RcaConfig, RcaResult};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

// ── helpers ───────────────────────────────────────────────────────────────────

fn write_config(dir: &tempfile::TempDir) -> String {
    let db_path = dir.path().join("db");
    std::fs::create_dir_all(&db_path).unwrap();
    let cfg = dir.path().join("bds.hjson");
    // similarity_threshold: 2.0 ensures every record is a primary (cosine
    // similarity is always ≤ 1.0, so the threshold can never be reached).
    std::fs::write(
        &cfg,
        format!(
            "{{\n  dbpath: \"{}\"\n  shard_duration: \"1h\"\n  pool_size: 2\n  similarity_threshold: 2.0\n}}\n",
            db_path.display()
        ),
    )
    .unwrap();
    cfg.to_str().unwrap().to_string()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn ingest(docs: Vec<serde_json::Value>) {
    get_db().unwrap().add_batch(docs).unwrap();
}

// ── test ──────────────────────────────────────────────────────────────────────

#[test]
fn test_rca_lifecycle() {
    let t0  = now();
    const B: u64 = 300; // bucket width in seconds
    // Align to the current bucket boundary so all offsets land exactly at
    // bucket starts, making co-occurrence assignment deterministic.
    let b = (t0 / B) * B;

    // ── 1. before init_db → "not initialized" error ───────────────────────────
    let err = RcaResult::analyze("1h", &RcaConfig::default())
        .err().unwrap().to_string();
    assert!(
        err.contains("not initialized"),
        "expected not-initialized error before init_db; got: {err}"
    );
    let err = RcaResult::analyze_failure("kernel", "1h", &RcaConfig::default())
        .err().unwrap().to_string();
    assert!(
        err.contains("not initialized"),
        "expected not-initialized error before init_db; got: {err}"
    );

    // ── 2. init_db ────────────────────────────────────────────────────────────
    let dir = tempfile::TempDir::new().unwrap();
    init_db(Some(&write_config(&dir))).expect("init_db should succeed");

    // ── 3. empty window → all-zero result ────────────────────────────────────
    let empty = RcaResult::analyze("1h", &RcaConfig::default())
        .expect("empty window should not error");
    assert_eq!(empty.n_events, 0, "no events ingested yet");
    assert_eq!(empty.n_keys,   0);
    assert!(empty.clusters.is_empty(),       "no clusters expected in empty DB");
    assert!(empty.probable_causes.is_empty());
    assert_eq!(empty.failure_key, None);

    // ── 4. telemetry filtering ────────────────────────────────────────────────
    // A bare-number data field and the structured {value: N, unit: …} shape
    // are both telemetry; neither should appear in any cluster.
    let tele_ts = b.saturating_sub(100);
    ingest(vec![
        json!({ "timestamp": tele_ts,     "key": "cpu.usage", "data": 72.5 }),
        json!({ "timestamp": tele_ts + 1, "key": "cpu.usage", "data": 73.0 }),
        json!({ "timestamp": tele_ts + 2, "key": "cpu.usage", "data": 74.1 }),
        json!({ "timestamp": tele_ts + 3, "key": "mem.used",  "data": { "value": 4096.0, "unit": "MB" } }),
        json!({ "timestamp": tele_ts + 4, "key": "mem.used",  "data": { "value": 4200.0, "unit": "MB" } }),
        json!({ "timestamp": tele_ts + 5, "key": "mem.used",  "data": { "value": 4300.0, "unit": "MB" } }),
    ]);

    let after_tele = RcaResult::analyze("2h", &RcaConfig { min_support: 2, ..Default::default() })
        .expect("after telemetry ingest should not error");
    assert_eq!(after_tele.n_events, 0, "all ingested records were telemetry — n_events must be 0");
    let all_tele_cluster_keys: Vec<&str> = after_tele.clusters
        .iter().flat_map(|c| c.members.iter().map(String::as_str)).collect();
    assert!(!all_tele_cluster_keys.contains(&"cpu.usage"), "cpu.usage is telemetry, must not cluster");
    assert!(!all_tele_cluster_keys.contains(&"mem.used"),  "mem.used is telemetry, must not cluster");

    // ── 5. cluster detection: two isolated co-occurrence groups ───────────────
    //
    // Cluster A  { sshd, auditd }:   buckets b−22B, b−20B, b−18B, b−16B
    // Cluster B  { nginx, postgres }: buckets b−21B, b−19B, b−17B, b−15B
    //
    // Interleaved, never sharing a bucket → Jaccard(A×B) = 0.
    // Both members of each cluster share all 4 of their buckets → Jaccard = 1.
    //
    // Distinct messages in each incident prevent exact-match dedup from
    // collapsing the 4 occurrences of a key into a single primary.

    let sshd_msgs = [
        "accepted publickey for alice from 10.0.0.100 port 43210 ssh2",
        "failed password for root from 192.168.2.50 port 11111 ssh2",
        "invalid user carol from 172.16.0.1 port 55555 ssh2",
        "connection closed by authenticating user deploy 10.0.0.200 port 22222",
    ];
    let auditd_msgs = [
        "syscall read auid 1000 pid 2345 exe /bin/bash success yes",
        "syscall execve auid 0 pid 9876 comm bash exit 0 hostname server01",
        "file open permission denied path /etc/shadow uid 500 pid 1111",
        "user login session started uid 1001 tty pts/0 addr 10.0.0.5",
    ];
    let nginx_msgs = [
        "upstream timed out 60s connecting to 127.0.0.1:8080 while reading headers",
        "worker process 1234 exited with code 1 signal 0 none",
        "no live upstreams while connecting to upstream backend pool app",
        "limiting requests zone one rate 100r/s excess 5.500 requests",
    ];
    let postgres_msgs = [
        "fatal sorry too many clients already max_connections 100 reached",
        "error deadlock detected process 4567 transaction abort rollback",
        "checkpoint complete wrote 512 buffers 0.0 elapsed 2.301 s",
        "autovacuum launcher started 8 workers max_autovacuum_workers",
    ];

    let mut cluster_docs: Vec<serde_json::Value> = Vec::new();
    for i in 0usize..4 {
        let a_ts  = b - 22 * B + (i as u64) * 2 * B;   // b-22B, b-20B, b-18B, b-16B
        let bb_ts = b - 21 * B + (i as u64) * 2 * B;   // b-21B, b-19B, b-17B, b-15B
        cluster_docs.extend([
            json!({ "timestamp": a_ts,      "key": "sshd",     "data": { "message": sshd_msgs[i] } }),
            json!({ "timestamp": a_ts  + 5, "key": "auditd",   "data": { "message": auditd_msgs[i] } }),
            json!({ "timestamp": bb_ts,     "key": "nginx",    "data": { "message": nginx_msgs[i] } }),
            json!({ "timestamp": bb_ts + 5, "key": "postgres", "data": { "message": postgres_msgs[i] } }),
        ]);
    }
    ingest(cluster_docs);

    let cfg_detect = RcaConfig {
        bucket_secs: B,
        min_support: 2,
        jaccard_threshold: 0.5,
        max_keys: 200,
    };
    let clustered = RcaResult::analyze("2h", &cfg_detect)
        .expect("cluster detection query should succeed");

    // Telemetry must remain absent from all clusters.
    let member_set: std::collections::HashSet<&str> = clustered.clusters
        .iter().flat_map(|c| c.members.iter().map(String::as_str)).collect();
    assert!(!member_set.contains("cpu.usage"), "telemetry key cpu.usage must not cluster");
    assert!(!member_set.contains("mem.used"),  "telemetry key mem.used must not cluster");

    // Exactly two multi-member clusters.
    let multi: Vec<&EventCluster> = clustered.clusters.iter().filter(|c| c.members.len() >= 2).collect();
    assert_eq!(multi.len(), 2, "expected exactly 2 two-member clusters; got: {multi:?}");

    // The two clusters are precisely {sshd, auditd} and {nginx, postgres}.
    let cluster_sets: Vec<std::collections::HashSet<&str>> = multi.iter()
        .map(|c| c.members.iter().map(String::as_str).collect())
        .collect();
    let want_a: std::collections::HashSet<&str> = ["sshd", "auditd"].into();
    let want_b: std::collections::HashSet<&str> = ["nginx", "postgres"].into();
    assert!(cluster_sets.contains(&want_a), "cluster {{sshd, auditd}} not found; got {cluster_sets:?}");
    assert!(cluster_sets.contains(&want_b), "cluster {{nginx, postgres}} not found; got {cluster_sets:?}");

    // Cohesion = 1.0 (always co-occur with identical bucket sets).
    for c in &multi {
        assert!(
            (c.cohesion - 1.0).abs() < 1e-9,
            "cluster {:?} cohesion should be 1.0, got {}",
            c.members, c.cohesion
        );
    }
    // Each key appeared in 4 distinct buckets → support ≥ 4.
    for c in &multi {
        assert!(c.support >= 4, "support should be ≥ 4; got {} for {:?}", c.support, c.members);
    }

    // ── 6. causal ranking ─────────────────────────────────────────────────────
    //
    // Five incidents, each in a separate 300s bucket (600s apart).
    // Within each bucket:
    //   disk_full  at offset   0 → avg_lead relative to app_crash ≈  90 s
    //   oom_killer at offset  60 → avg_lead relative to app_crash ≈  30 s
    //   app_crash  at offset  90 → the failure
    //
    // Expected ranking: disk_full first (larger positive lead = earlier precursor).

    let df_msgs = [
        "filesystem /dev/sda1 is 100 percent full inode table exhausted",
        "no space left on device /var/log write syscall failed eagain",
        "disk usage critical /data 99 percent used quota barrier reached",
        "write error io failure /dev/sda1 bad sector reallocated event",
        "volume storage pool exhausted cannot allocate new 4k blocks",
    ];
    let oom_msgs = [
        "out of memory kill process 3456 total vm 8gb anon 4gb rss",
        "oom killer invoked memory allocation failed 4096 bytes arena",
        "oom killer invoked process nginx pid 1234 oom score 800 killed",
        "task httpd pid 5678 oom score 950 memory cgroup limit exceeded",
        "vm overcommit strict mode malloc returned null ptr 16384 bytes",
    ];
    let crash_msgs = [
        "application segmentation fault signal 11 core dumped pid 4321",
        "process exited unexpectedly sigsegv received watchdog timeout",
        "service systemd watchdog restarting crashed worker unit restart",
        "fatal exception unhandled panic stack overflow abort called",
        "health check endpoint /health not responding after 30s timeout",
    ];

    let mut causal_docs: Vec<serde_json::Value> = Vec::new();
    for i in 0usize..5 {
        // Causal incidents at b-12B, b-11B, b-10B, b-9B, b-8B (600s apart = 2 buckets).
        // All well separated from cluster events (which ended at b-15B).
        let start = b - 12 * B + (i as u64) * 2 * B;
        causal_docs.extend([
            json!({ "timestamp": start,      "key": "disk_full",  "data": { "message": df_msgs[i] } }),
            json!({ "timestamp": start + 60, "key": "oom_killer", "data": { "message": oom_msgs[i] } }),
            json!({ "timestamp": start + 90, "key": "app_crash",  "data": { "message": crash_msgs[i] } }),
        ]);
    }
    ingest(causal_docs);

    let rca = RcaResult::analyze_failure("app_crash", "2h", &RcaConfig {
        bucket_secs: B,
        min_support: 2,
        jaccard_threshold: 0.2,
        max_keys: 200,
    })
    .expect("causal analysis should succeed");

    assert_eq!(rca.failure_key.as_deref(), Some("app_crash"));

    let cause_keys: Vec<&str> = rca.probable_causes.iter().map(|c| c.key.as_str()).collect();
    assert!(cause_keys.contains(&"disk_full"),  "disk_full must be a cause; got {cause_keys:?}");
    assert!(cause_keys.contains(&"oom_killer"), "oom_killer must be a cause; got {cause_keys:?}");

    // Both precursors have positive avg_lead_secs and non-zero Jaccard.
    for c in &rca.probable_causes {
        assert!(
            c.avg_lead_secs > 0.0,
            "{} avg_lead_secs should be positive (precedes failure); got {}",
            c.key, c.avg_lead_secs
        );
        assert!(c.jaccard > 0.0, "{} jaccard should be > 0; got {}", c.key, c.jaccard);
    }

    // disk_full has the larger lead → it must rank first.
    assert_eq!(
        rca.probable_causes[0].key, "disk_full",
        "disk_full (lead≈90s) should rank above oom_killer (lead≈30s); order: {cause_keys:?}"
    );

    // Verify approximate lead times.
    let df  = rca.probable_causes.iter().find(|c| c.key == "disk_full").unwrap();
    let oom = rca.probable_causes.iter().find(|c| c.key == "oom_killer").unwrap();
    assert!(
        (df.avg_lead_secs - 90.0).abs() < 1.0,
        "disk_full avg_lead should be ≈90s; got {}", df.avg_lead_secs
    );
    assert!(
        (oom.avg_lead_secs - 30.0).abs() < 1.0,
        "oom_killer avg_lead should be ≈30s; got {}", oom.avg_lead_secs
    );

    // Each candidate co-occurred in all 5 incidents.
    assert_eq!(df.co_occurrence_count,  5, "disk_full co_occurrence_count should be 5; got {}",  df.co_occurrence_count);
    assert_eq!(oom.co_occurrence_count, 5, "oom_killer co_occurrence_count should be 5; got {}", oom.co_occurrence_count);

    // ── 7. tight threshold (1.0): only perfectly co-occurring keys merge ───────
    let tight = RcaResult::analyze("2h", &RcaConfig {
        bucket_secs: B,
        min_support: 2,
        jaccard_threshold: 1.0,
        max_keys: 200,
    })
    .expect("tight-threshold query should succeed");

    // {sshd, auditd}, {nginx, postgres}, {disk_full, oom_killer, app_crash}
    // all have Jaccard = 1.0 within their groups, so they still cluster.
    let tight_multi: Vec<&EventCluster> = tight.clusters.iter().filter(|c| c.members.len() >= 2).collect();
    assert!(
        tight_multi.len() >= 2,
        "at least 2 perfectly-cohesive clusters should survive threshold=1.0; got: {tight_multi:?}"
    );

    // ── 8. wide buckets: all events in one bucket → one mega-cluster ──────────
    let wide = RcaResult::analyze("2h", &RcaConfig {
        bucket_secs: 86_400, // 24-hour bucket encloses the whole 2h window
        min_support: 2,
        jaccard_threshold: 0.01,
        max_keys: 200,
    })
    .expect("wide-bucket query should succeed");

    let max_members = wide.clusters.iter().map(|c| c.members.len()).max().unwrap_or(0);
    assert!(
        max_members >= 4,
        "wide bucket should produce a cluster of ≥4 members; sizes: {:?}",
        wide.clusters.iter().map(|c| c.members.len()).collect::<Vec<_>>()
    );

    // ── 9. unknown failure key → empty probable_causes ────────────────────────
    let unknown = RcaResult::analyze_failure("no.such.event", "2h", &RcaConfig::default())
        .expect("unknown failure key should not error");
    assert!(
        unknown.probable_causes.is_empty(),
        "unknown failure key must produce empty probable_causes"
    );

    // ── 10. metadata invariants ───────────────────────────────────────────────
    let meta = RcaResult::analyze("2h", &RcaConfig {
        bucket_secs: B,
        min_support: 2,
        jaccard_threshold: 0.5,
        max_keys: 200,
    })
    .expect("metadata invariants query should succeed");

    assert!(meta.start <= meta.end, "start must be ≤ end; start={} end={}", meta.start, meta.end);
    assert_eq!(meta.failure_key, None, "analyze() must not set failure_key");

    // Every key appears in exactly one cluster.
    let total_members: usize = meta.clusters.iter().map(|c| c.members.len()).sum();
    assert_eq!(
        total_members, meta.n_keys,
        "sum of cluster members ({total_members}) must equal n_keys ({})",
        meta.n_keys
    );

    // Cluster ids are sequential from 0.
    for (want_id, c) in meta.clusters.iter().enumerate() {
        assert_eq!(c.id, want_id, "cluster ids must be sequential; expected {want_id}, got {}", c.id);
    }

    // Clusters are sorted by cohesion descending.
    for pair in meta.clusters.windows(2) {
        assert!(
            pair[0].cohesion >= pair[1].cohesion,
            "clusters must be sorted by cohesion desc; {} < {} (ids {} {})",
            pair[0].cohesion, pair[1].cohesion, pair[0].id, pair[1].id
        );
    }

    println!(
        "RCA lifecycle test complete — {} events, {} keys, {} clusters",
        meta.n_events, meta.n_keys, meta.clusters.len()
    );
}
