/// RCA demo — Root Cause Analysis on a synthetic infrastructure incident log.
///
/// Ingests ~100 log events across four realistic clusters into a temporary
/// in-process database, then runs `RcaResult::analyze` and
/// `RcaResult::analyze_failure` to demonstrate cluster detection and causal
/// ranking.
///
/// Dataset layout (all times relative to current time, 300-second buckets):
///
///   Auth cluster      sshd, pam, auditd          10 incidents × 3 events
///   Web cluster       nginx, haproxy              10 incidents × 2 events
///   DB cluster        postgres, redis             10 incidents × 2 events
///   Failure cascade   disk_warn → disk_full
///                     → nfs_timeout → app_error
///                     → app_crash                  6 incidents × 5 events
///   Telemetry (noise) cpu.usage, mem.used         filtered out by RCA
///
/// Total events ingested: 110 event records + 14 telemetry records.
/// Events reaching the RCA engine after telemetry filtering: 110.

use bdslib::{get_db, init_db, RcaConfig, RcaResult};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Attribute, Cell, Color, Table};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

// ── dataset constants ─────────────────────────────────────────────────────────

const BUCKET: u64 = 300; // 5-minute co-occurrence window

// ── helpers ───────────────────────────────────────────────────────────────────

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn bucket_start(t0: u64) -> u64 {
    (t0 / BUCKET) * BUCKET
}

fn hr(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 { format!("{h}h {m:02}m {s:02}s") }
    else if m > 0 { format!("{m}m {s:02}s") }
    else { format!("{s}s") }
}

fn hdr(label: &str) {
    println!();
    println!("  ┌─────────────────────────────────────────────────────────┐");
    println!("  │  {label:<55}│");
    println!("  └─────────────────────────────────────────────────────────┘");
}

// ── dataset builders ──────────────────────────────────────────────────────────

fn auth_events(b: u64) -> Vec<serde_json::Value> {
    // 10 incidents, every 600 s (2 buckets).  sshd, pam, auditd within 30 s.
    let sshd_msgs = [
        "accepted publickey for alice from 10.0.1.50 port 43210 ssh2",
        "failed password for bob from 192.168.2.100 port 11111 ssh2",
        "accepted publickey for deploy from 10.0.0.1 port 52200 ssh2",
        "invalid user guest from 172.16.0.5 port 33333 ssh2",
        "session opened for user alice by sshd uid 0",
        "session closed for user bob pam login",
        "accepted publickey for carol from 10.0.1.20 port 61234 ssh2",
        "failed password for root from 203.0.113.5 port 22 ssh2",
        "disconnect from authenticating user admin 10.0.0.99 port 8732",
        "received disconnect from 192.168.1.1 port 54321 reason normal",
    ];
    let pam_msgs = [
        "pam unix sshd auth authentication failure uid 0 user alice",
        "pam unix sshd session opened for user bob by uid 0",
        "pam unix sudo auth authentication failure uid 1001 user deploy",
        "pam unix sshd session closed for user guest",
        "pam tally2 user alice attempts 0 allowed",
        "pam unix cron session opened for user root by uid 0",
        "pam unix sshd auth user carol authenticated successfully",
        "pam unix sshd auth failure maxretries exceeded root",
        "pam unix su session opened for user www-data by root uid 0",
        "pam unix sshd session closed for user admin",
    ];
    let auditd_msgs = [
        "type user_auth pid 2001 uid 0 auid 4294967295 msg login acct alice",
        "type syscall pid 3456 uid 1000 auid 1000 comm bash exe bin bash",
        "type user_login pid 2002 uid 0 auid 1001 msg op login id 1001",
        "type avc denied read comm httpd name shadow scontext unconfined",
        "type user_end pid 2003 uid 0 auid 1000 msg op login id 1000",
        "type cred_refr pid 1234 uid 0 auid 0 msg op login acct root",
        "type user_auth pid 2004 uid 0 auid 4294967295 msg login acct carol",
        "type syscall pid 9876 uid 0 auid 0 comm sudo exe usr bin sudo",
        "type user_acct pid 2005 uid 0 auid 1000 msg op PAM acct user",
        "type user_end pid 2006 uid 0 auid 1001 msg op login id 1001",
    ];
    let mut docs = Vec::new();
    for i in 0usize..10 {
        let t = b.saturating_sub(6000 + (i as u64) * 600);
        docs.extend([
            json!({ "timestamp": t,      "key": "sshd",   "data": { "message": sshd_msgs[i],   "host": "auth-01" } }),
            json!({ "timestamp": t + 10, "key": "pam",    "data": { "message": pam_msgs[i],    "host": "auth-01" } }),
            json!({ "timestamp": t + 20, "key": "auditd", "data": { "message": auditd_msgs[i], "host": "auth-01" } }),
        ]);
    }
    docs
}

fn web_events(b: u64) -> Vec<serde_json::Value> {
    // 10 incidents, every 600 s.  nginx and haproxy within 15 s.
    let nginx_msgs = [
        "upstream timed out 60s while reading response header from 127.0.0.1:8080",
        "worker process 1201 exited on signal 11 sigsegv core dumped",
        "no live upstreams while connecting to upstream backend app_pool",
        "client 10.0.5.10 closed connection while waiting for request",
        "recv failed broken pipe peer reset connection 10.0.5.20 port 61234",
        "limiting connections zone api addr 10.0.6.1 rate exceeded 50/s",
        "ssl handshake failed fd 12 SSL_do_handshake unknown protocol",
        "upstream sent invalid header while reading upstream 127.0.0.1:9090",
        "directory index of /var/www/ is forbidden client 10.0.5.30",
        "rewrite or internal redirection cycle while internally redirecting",
    ];
    let haproxy_msgs = [
        "backend app down no server available for backend app_pool",
        "health check failed for server app_pool/app01 status 502",
        "session 0x7f1234 backend app_pool timeout expired queue 1500ms",
        "proxy app_pool has no server available queue 1024ms reached",
        "server app_pool/app02 is DOWN reason layer4 timeout after 2001ms",
        "connection rate limit exceeded src 10.0.6.1 maxconnrate 1000",
        "proxy app_pool stopping 3 active sessions pending",
        "server app03 administratively down maintenance mode enabled",
        "TCP health check to app01:8080 failed connection refused errno 111",
        "load balancer frontend stats listener bound to 0.0.0.0:9000",
    ];
    let mut docs = Vec::new();
    for i in 0usize..10 {
        let t = b.saturating_sub(5800 + (i as u64) * 600);
        docs.extend([
            json!({ "timestamp": t,      "key": "nginx",   "data": { "message": nginx_msgs[i],   "host": "web-01" } }),
            json!({ "timestamp": t + 15, "key": "haproxy", "data": { "message": haproxy_msgs[i], "host": "lb-01"  } }),
        ]);
    }
    docs
}

fn db_events(b: u64) -> Vec<serde_json::Value> {
    // 10 incidents, every 600 s.  postgres and redis within 20 s.
    let pg_msgs = [
        "fatal sorry too many clients already max_connections 150 exhausted",
        "error deadlock detected process 5678 waits for lock relation 99",
        "log checkpoint starting shutdown immediate immediate mode",
        "error could not connect to the primary server connection refused",
        "fatal pg_hba.conf rejects connection host 10.0.2.50 user app",
        "warning out of shared memory cannot lock relation 12345",
        "log automatic vacuum of table public events elapsed 45.210 s",
        "error invalid input syntax for type integer null value",
        "log statement duration 15432 ms select star from events where",
        "fatal terminating connection due to administrator command",
    ];
    let redis_msgs = [
        "server connection from 10.0.2.1 rejected maxclients 10000 reached",
        "server MISCONF redis is configured to save rdb snapshots but",
        "server Warning 32 bytes of memory are being used for replication",
        "server Possible SECURITY ATTACK detected binding to 0.0.0.0",
        "server Loading DB in progress bgsave failed aof rewrite failed",
        "server out of memory trying to allocate bytes when freed 0 bytes",
        "server cluster NOAUTH Authentication required connection 172.16",
        "server rdb background saving failed signal 13 broken pipe write",
        "server Memory fragmentation ratio 2.34 consider restarting redis",
        "server replication master auth error wrong password provided",
    ];
    let mut docs = Vec::new();
    for i in 0usize..10 {
        let t = b.saturating_sub(5600 + (i as u64) * 600);
        docs.extend([
            json!({ "timestamp": t,      "key": "postgres", "data": { "message": pg_msgs[i],    "host": "db-01" } }),
            json!({ "timestamp": t + 20, "key": "redis",    "data": { "message": redis_msgs[i], "host": "db-01" } }),
        ]);
    }
    docs
}

fn failure_cascade(b: u64) -> Vec<serde_json::Value> {
    // 6 failure incidents spaced 1800 s apart.
    // Within each bucket (300 s), 5 events form a causal chain:
    //   disk_warn   at +0   (avg lead relative to app_crash: 180 s)
    //   disk_full   at +45  (avg lead: 135 s)
    //   nfs_timeout at +90  (avg lead:  90 s)
    //   app_error   at +150 (avg lead:  30 s)
    //   app_crash   at +180 (failure)
    let warn_msgs  = [
        "disk /dev/sda1 usage 85 percent warning threshold reached inode 70pct",
        "disk /dev/nvme0 usage 88 percent warning quota soft limit approaching",
        "disk /var/log usage 90 percent alert logrotate failed insufficient space",
        "disk /dev/sdb usage 87 percent warning smart reallocated sectors 5",
        "disk /data usage 86 percent warning backup partition nearly full",
        "disk /tmp usage 89 percent warning tmpfs capacity nearly exhausted",
    ];
    let full_msgs  = [
        "disk /dev/sda1 100 percent full write syscall returned ENOSPC",
        "disk /dev/nvme0 full inode table exhausted cannot create new file",
        "disk /var/log full logd dropped 1024 messages rotating failed",
        "disk /dev/sdb full smart failure imminent reallocated sectors 99",
        "disk /data full backup aborted destination volume has no space",
        "disk /tmp full process pid 9876 failed to create temp file",
    ];
    let nfs_msgs   = [
        "nfs server nfs-01 not responding still trying tcp port 2049",
        "nfs mount nfs-01:/exports/data timeout after 90s retrying",
        "nfs client nfs-01 timed out reading inode 4096 path exports",
        "nfs rpc program not registered portmap query nfs-01 failed",
        "nfs server nfs-01 not responding nohang flag set aborting",
        "nfs client request to nfs-01 timed out retrying with softerr",
    ];
    let err_msgs   = [
        "application error unhandled exception IOError no space left device",
        "application error write failed errno 28 ENOSPC filesystem full",
        "application error database connection refused upstream postgres down",
        "application error cache miss redis connection timeout pipeline",
        "application error queue overflow 50000 messages dropped backpressure",
        "application error runtime panic goroutine nil pointer dereference",
    ];
    let crash_msgs = [
        "application crashed segfault signal 11 core pid 4321 dumped",
        "application exited unexpectedly status 139 watchdog restart triggered",
        "service app crashed systemd unit entered failed state restarting",
        "process fatal unrecoverable error abort called stack trace follows",
        "health check /health returned 503 application not responding 30s",
        "application oom killed kernel invoked termination signal 9 sent",
    ];
    let mut docs = Vec::new();
    for i in 0usize..6 {
        let t = b.saturating_sub(9000).saturating_add((i as u64) * 1800);
        docs.extend([
            json!({ "timestamp": t,       "key": "disk_warn",    "data": { "message": warn_msgs[i],  "host": "app-01" } }),
            json!({ "timestamp": t +  45, "key": "disk_full",    "data": { "message": full_msgs[i],  "host": "app-01" } }),
            json!({ "timestamp": t +  90, "key": "nfs_timeout",  "data": { "message": nfs_msgs[i],   "host": "app-01" } }),
            json!({ "timestamp": t + 150, "key": "app_error",    "data": { "message": err_msgs[i],   "host": "app-01" } }),
            json!({ "timestamp": t + 180, "key": "app_crash",    "data": { "message": crash_msgs[i], "host": "app-01" } }),
        ]);
    }
    docs
}

fn telemetry_noise() -> Vec<serde_json::Value> {
    // Numeric records that RCA must silently discard.
    let base = now() - 600;
    let mut docs = Vec::new();
    for i in 0usize..7 {
        docs.push(json!({ "timestamp": base + i as u64, "key": "cpu.usage",  "data": 50.0 + i as f64 }));
    }
    for i in 0usize..7 {
        docs.push(json!({ "timestamp": base + 60 + i as u64, "key": "mem.used", "data": { "value": 4096.0 + i as f64 * 100.0, "unit": "MB" } }));
    }
    docs
}

// ── output helpers ────────────────────────────────────────────────────────────

fn print_cluster_table(result: &RcaResult) {
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec![
        Cell::new("Cluster").add_attribute(Attribute::Bold),
        Cell::new("Members").add_attribute(Attribute::Bold),
        Cell::new("Cohesion").add_attribute(Attribute::Bold),
        Cell::new("Support").add_attribute(Attribute::Bold),
    ]);
    for c in &result.clusters {
        let members = c.members.join(",  ");
        let cohesion_cell = if c.cohesion > 0.8 {
            Cell::new(format!("{:.3}", c.cohesion)).fg(Color::Green)
        } else if c.cohesion > 0.4 {
            Cell::new(format!("{:.3}", c.cohesion)).fg(Color::Yellow)
        } else {
            Cell::new(format!("{:.3}", c.cohesion))
        };
        table.add_row(vec![
            Cell::new(format!("#{}", c.id)),
            Cell::new(&members),
            cohesion_cell,
            Cell::new(format!("{} buckets", c.support)),
        ]);
    }
    println!("{table}");
}

fn print_causes_table(result: &RcaResult) {
    if result.probable_causes.is_empty() {
        println!("  (no causal candidates found)");
        return;
    }
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec![
        Cell::new("Rank").add_attribute(Attribute::Bold),
        Cell::new("Key").add_attribute(Attribute::Bold),
        Cell::new("Avg lead").add_attribute(Attribute::Bold),
        Cell::new("Jaccard").add_attribute(Attribute::Bold),
        Cell::new("Co-occ").add_attribute(Attribute::Bold),
        Cell::new("Assessment").add_attribute(Attribute::Bold),
    ]);
    for (i, c) in result.probable_causes.iter().enumerate() {
        let rank = format!("#{}", i + 1);
        let lead = format!("{:.1}s", c.avg_lead_secs);
        let jaccard = format!("{:.3}", c.jaccard);
        let co = c.co_occurrence_count.to_string();

        let assessment = if c.avg_lead_secs > 120.0 {
            Cell::new("LIKELY ROOT CAUSE").fg(Color::Red).add_attribute(Attribute::Bold)
        } else if c.avg_lead_secs > 60.0 {
            Cell::new("Contributing factor").fg(Color::Yellow)
        } else if c.avg_lead_secs > 0.0 {
            Cell::new("Possible precursor").fg(Color::Cyan)
        } else {
            Cell::new("Concurrent / consequence")
        };

        table.add_row(vec![
            Cell::new(rank),
            Cell::new(&c.key).add_attribute(Attribute::Bold),
            Cell::new(lead),
            Cell::new(jaccard),
            Cell::new(co),
            assessment,
        ]);
    }
    println!("{table}");
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    let t0 = now();
    let b  = bucket_start(t0);

    // ── 1. Temporary DB ───────────────────────────────────────────────────────
    let tmp = std::env::temp_dir().join(format!("bdsrca_{t0}"));
    let db_path = tmp.join("db");
    let cfg_path = tmp.join("bds.hjson");
    std::fs::create_dir_all(&db_path).expect("create temp db dir");
    std::fs::write(
        &cfg_path,
        format!(
            "{{\n  dbpath: \"{}\"\n  shard_duration: \"6h\"\n  pool_size: 4\n  similarity_threshold: 2.0\n}}\n",
            db_path.display()
        ),
    )
    .expect("write config");

    println!();
    println!("  ╔═════════════════════════════════════════════════════════╗");
    println!("  ║      RCA DEMO  —  Infrastructure Incident Analysis      ║");
    println!("  ╚═════════════════════════════════════════════════════════╝");

    init_db(Some(cfg_path.to_str().unwrap())).expect("init_db");
    let db = get_db().expect("get_db");

    // ── 2. Ingest synthetic log corpus ────────────────────────────────────────
    hdr("INGESTING DATASET");

    let auth  = auth_events(b);
    let web   = web_events(b);
    let dbe   = db_events(b);
    let fail  = failure_cascade(b);
    let noise = telemetry_noise();

    let n_auth  = auth.len();
    let n_web   = web.len();
    let n_db    = dbe.len();
    let n_fail  = fail.len();
    let n_noise = noise.len();

    println!("  Auth cluster   (sshd, pam, auditd)              {:>3} events", n_auth);
    db.add_batch(auth).expect("ingest auth");

    println!("  Web cluster    (nginx, haproxy)                  {:>3} events", n_web);
    db.add_batch(web).expect("ingest web");

    println!("  DB cluster     (postgres, redis)                 {:>3} events", n_db);
    db.add_batch(dbe).expect("ingest db");

    println!("  Failure cascade (disk_warn → ... → app_crash)   {:>3} events", n_fail);
    db.add_batch(fail).expect("ingest failure");

    println!("  Telemetry noise (cpu.usage, mem.used) — filtered {:>3} records", n_noise);
    db.add_batch(noise).expect("ingest noise");

    let total_ingested = n_auth + n_web + n_db + n_fail + n_noise;
    let total_events   = n_auth + n_web + n_db + n_fail;
    println!();
    println!("  Total ingested : {total_ingested} records");
    println!("  Event records  : {total_events}  (telemetry filtered before clustering)");

    // ── 3. Run RCA — cluster all events ───────────────────────────────────────
    hdr("RCA  —  CO-OCCURRENCE CLUSTERING  (all events, 3 h window)");

    let cfg = RcaConfig {
        bucket_secs: BUCKET,
        min_support: 2,
        jaccard_threshold: 0.5,
        max_keys: 200,
    };

    let full = RcaResult::analyze("3h", &cfg).expect("RcaResult::analyze");

    println!(
        "  Events analysed : {}   Distinct keys : {}   Clusters found : {}",
        full.n_events, full.n_keys, full.clusters.len()
    );
    println!(
        "  Window          : {} ago  →  {} ago",
        hr(t0 - full.start),
        hr(t0.saturating_sub(full.end))
    );
    println!();
    print_cluster_table(&full);

    // ── 4. Highlight the cascade cluster ─────────────────────────────────────
    if let Some(cascade) = full.clusters.iter().find(|c| c.members.contains(&"app_crash".to_string())) {
        println!();
        println!("  Failure-cascade cluster (id #{}):", cascade.id);
        println!("    Members  : {}", cascade.members.join(" → "));
        println!("    Cohesion : {:.3}  (all 5 keys always co-occur)", cascade.cohesion);
        println!("    Support  : {} incidents detected in 3 h window", cascade.support);
    }

    // ── 5. Run RCA — causal ranking for app_crash ─────────────────────────────
    hdr("RCA  —  PROBABLE CAUSES  for failure key \"app_crash\"");

    let causal = RcaResult::analyze_failure("app_crash", "3h", &cfg).expect("analyze_failure");

    println!("  Candidates ranked by average lead time (positive = precedes failure):");
    println!();
    print_causes_table(&causal);

    // ── 6. Narrative summary ──────────────────────────────────────────────────
    hdr("SUMMARY");

    if let Some(top) = causal.probable_causes.first() {
        println!("  Most probable root cause  :  {}", top.key);
        println!(
            "  Avg precedes app_crash by :  {:.0}s across {} co-occurring incidents",
            top.avg_lead_secs, top.co_occurrence_count
        );
        println!("  Jaccard co-occurrence     :  {:.3}", top.jaccard);
    }
    println!();
    println!("  Cluster interpretation:");
    for c in &full.clusters {
        let label = if c.members.contains(&"app_crash".to_string()) {
            "← failure-cascade cluster (investigate root cause)"
        } else if c.members.contains(&"sshd".to_string()) {
            "← authentication-activity cluster (normal)"
        } else if c.members.contains(&"nginx".to_string()) {
            "← web-tier cluster (normal)"
        } else if c.members.contains(&"postgres".to_string()) {
            "← data-tier cluster (normal)"
        } else {
            ""
        };
        println!(
            "    #{id}  [{members}]  cohesion={coh:.2}  {label}",
            id      = c.id,
            members = c.members.join(", "),
            coh     = c.cohesion,
        );
    }

    // ── 7. cleanup ─────────────────────────────────────────────────────────────
    println!();
    let _ = std::fs::remove_dir_all(&tmp);
    println!("  Demo complete.  Temp directory removed.");
    println!();
}
