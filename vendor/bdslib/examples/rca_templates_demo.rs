/// rca_templates_demo — Template-level Root Cause Analysis on drain3 observations.
///
/// Injects template events directly into a temporary ShardsManager using
/// `tpl_add`, then runs `RcaTemplatesResult::analyze` and `analyze_failure`
/// to demonstrate cluster detection and causal ranking.
///
/// Dataset layout:
///
///   Auth cluster  (3 incidents × 2 templates per incident, buckets 0/2/4)
///     "user <*> logged in from <*>"
///     "session opened for user <*> by service <*>"
///
///   Disk/crash cluster  (3 incidents × 3 templates, buckets 1/3/5)
///     "disk <*> usage <*>% warning threshold reached"   (120 s before crash)
///     "disk <*> write error ENOSPC"                     ( 60 s before crash)
///     "service <*> crashed with exit code <*>"          (failure — 0 s)
///
/// The two clusters occupy non-overlapping 300-second buckets so Jaccard
/// similarity between auth templates and disk templates is 0 — they form two
/// fully separate clusters.  Within the disk cluster, `disk.warn` and
/// `disk.error` consistently precede `service.crash`, yielding a causal
/// ranking with positive lead times.
use bdslib::RcaTemplatesConfig;
use bdslib::RcaTemplatesResult;
use bdslib::embedding::Model;
use bdslib::shardsmanager::ShardsManager;
use bdslib::EmbeddingEngine;
use serde_json::json;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

// ── template body constants ───────────────────────────────────────────────────

const AUTH_LOGIN:   &str = "user <*> logged in from <*>";
const AUTH_SESSION: &str = "session opened for user <*> by service <*>";
const DISK_WARN:    &str = "disk <*> usage <*>% warning threshold reached";
const DISK_ERROR:   &str = "disk <*> write error ENOSPC";
const SVC_CRASH:    &str = "service <*> crashed with exit code <*>";

// ── helpers ───────────────────────────────────────────────────────────────────

static EMBEDDING: OnceLock<EmbeddingEngine> = OnceLock::new();

fn embedding() -> &'static EmbeddingEngine {
    EMBEDDING.get_or_init(|| {
        EmbeddingEngine::new(Model::AllMiniLML6V2, None).expect("embedding engine")
    })
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn hr(secs: f64) -> String {
    let s = secs.abs() as u64;
    let sign = if secs < 0.0 { "-" } else { "" };
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let ss = s % 60;
    if h > 0 { format!("{sign}{h}h {m:02}m {ss:02}s") }
    else if m > 0 { format!("{sign}{m}m {ss:02}s") }
    else { format!("{sign}{ss}s") }
}

fn hdr(label: &str) {
    println!();
    println!("  ┌─────────────────────────────────────────────────────────┐");
    println!("  │  {label:<55}│");
    println!("  └─────────────────────────────────────────────────────────┘");
}

fn store_tpl(mgr: &ShardsManager, body: &str, ts: u64) {
    mgr.tpl_add(
        json!({ "name": body, "timestamp": ts, "type": "tpl" }),
        body.as_bytes(),
    )
    .unwrap_or_else(|e| panic!("tpl_add failed for {body:?}: {e}"));
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    let now = now_secs();

    // ── 1. Temporary ShardsManager ────────────────────────────────────────────

    let dir = TempDir::new().expect("tempdir");
    let dbpath = dir.path().join("db");
    let cfg_path = dir.path().join("bds.hjson");

    let hjson = format!(
        r#"{{
  dbpath: "{}"
  shard_duration: "4h"
  pool_size: 4
  similarity_threshold: 0.85
}}"#,
        dbpath.display()
    );
    std::fs::write(&cfg_path, &hjson).expect("write config");

    let mgr = ShardsManager::with_embedding(
        cfg_path.to_str().unwrap(),
        embedding().clone(),
    )
    .expect("ShardsManager");

    println!();
    println!("  ╔═════════════════════════════════════════════════════════╗");
    println!("  ║   RCA TEMPLATES DEMO  —  Template Co-Occurrence RCA     ║");
    println!("  ╚═════════════════════════════════════════════════════════╝");

    // ── 2. Inject template events ─────────────────────────────────────────────
    //
    // Auth cluster: 3 incidents in buckets 0, 2, 4 (at t-3600, t-3000, t-2400).
    // Each incident fires AUTH_LOGIN and AUTH_SESSION within the same 300 s bucket.
    //
    // Disk/crash cluster: 3 incidents in buckets 1, 3, 5 (at t-3300, t-2700, t-2100).
    // Within each incident a causal chain plays out:
    //   DISK_WARN at t+0  (120 s before crash)
    //   DISK_ERROR at t+60 (60 s before crash)
    //   SVC_CRASH  at t+120 (the failure)

    hdr("INJECTING TEMPLATE EVENTS");

    // Auth incidents — buckets anchored at even multiples of 600 s
    for i in 0..3usize {
        let base = now.saturating_sub(3600 - i as u64 * 600);
        store_tpl(&mgr, AUTH_LOGIN,   base);
        store_tpl(&mgr, AUTH_SESSION, base + 30);
    }
    println!("  Auth cluster   — {} events injected", 3 * 2);

    // Disk/crash incidents — buckets anchored at odd multiples of 600 s
    for i in 0..3usize {
        let base = now.saturating_sub(3300 - i as u64 * 600);
        store_tpl(&mgr, DISK_WARN,  base);
        store_tpl(&mgr, DISK_ERROR, base + 60);
        store_tpl(&mgr, SVC_CRASH,  base + 120);
    }
    println!("  Disk/crash cluster — {} events injected (causal chain per incident)", 3 * 3);

    let total_events = 3 * 2 + 3 * 3;
    println!();
    println!("  Total template events stored : {total_events}");
    println!("  Template bodies              : 5  ({AUTH_LOGIN:?}, ...)");
    println!("  Analysis bucket width        : 300 s");

    // ── 3. Run RCA — cluster all templates ───────────────────────────────────

    hdr("RCA  —  CO-OCCURRENCE CLUSTERING  (all templates, 2 h window)");

    let cfg = RcaTemplatesConfig {
        bucket_secs:       300,
        min_support:       2,
        jaccard_threshold: 0.5,
        max_keys:          100,
    };

    let full = RcaTemplatesResult::analyze(&mgr, "2h", &cfg).expect("analyze");

    println!(
        "  Template events analysed : {}   Distinct bodies : {}   Clusters : {}",
        full.n_events, full.n_keys, full.clusters.len()
    );
    println!();

    if full.clusters.is_empty() {
        println!("  (no clusters — not enough co-occurring templates in the window)");
    } else {
        println!("  {:<3}  {:<12}  {:<8}  Members", "ID", "Cohesion", "Support");
        println!("  {}", "-".repeat(72));
        for c in &full.clusters {
            let members = c.members.iter()
                .map(|m| format!("{m:?}"))
                .collect::<Vec<_>>()
                .join(",  ");
            println!(
                "  #{:<2}  {:<12.3}  {:<8}  {}",
                c.id, c.cohesion, format!("{} buckets", c.support), members
            );
        }
    }

    // Verify expected clustering
    let has_auth_cluster = full.clusters.iter().any(|c| {
        c.members.iter().any(|m| m == AUTH_LOGIN)
            && c.members.iter().any(|m| m == AUTH_SESSION)
    });
    let has_disk_cluster = full.clusters.iter().any(|c| {
        c.members.iter().any(|m| m == DISK_WARN)
            && c.members.iter().any(|m| m == SVC_CRASH)
    });

    println!();
    if has_auth_cluster { println!("  Auth cluster detected") }
    else { println!("  Auth cluster NOT found (unexpected)") }
    if has_disk_cluster { println!("  Disk/crash cluster detected") }
    else { println!("  Disk/crash cluster NOT found (unexpected)") }

    // ── 4. Run RCA — causal ranking for service crash ─────────────────────────

    hdr(&format!("RCA  —  PROBABLE CAUSES  for {SVC_CRASH:?}"));

    let causal = RcaTemplatesResult::analyze_failure(
        &mgr, SVC_CRASH, "2h", &cfg,
    )
    .expect("analyze_failure");

    println!(
        "  Candidates ranked by avg lead time (positive = fires before the failure):"
    );
    println!();

    if causal.probable_causes.is_empty() {
        println!("  (no causal candidates — failure body not observed in window)");
    } else {
        println!(
            "  {:<4}  {:<45}  {:<10}  {:<8}  {}",
            "Rank", "Template body", "Avg lead", "Jaccard", "Co-occ"
        );
        println!("  {}", "-".repeat(80));
        for (i, c) in causal.probable_causes.iter().enumerate() {
            let body_trunc: String = c.body.chars().take(42).collect();
            let body_display = if c.body.len() > 42 {
                format!("{body_trunc}...")
            } else {
                body_trunc
            };
            let assessment = if c.avg_lead_secs > 90.0 {
                "LIKELY ROOT CAUSE"
            } else if c.avg_lead_secs > 30.0 {
                "Contributing factor"
            } else {
                "Possible precursor"
            };
            println!(
                "  #{:<3}  {:<45}  {:<10}  {:<8.3}  {} × ({})",
                i + 1,
                body_display,
                hr(c.avg_lead_secs),
                c.jaccard,
                c.co_occurrence_count,
                assessment,
            );
        }
    }

    // ── 5. Summary ────────────────────────────────────────────────────────────

    hdr("SUMMARY");

    println!("  Template RCA ran over {} events from {} distinct template bodies.", full.n_events, full.n_keys);
    println!("  {} co-occurrence clusters found (jaccard_threshold=0.5).", full.clusters.len());

    if let Some(top) = causal.probable_causes.first() {
        println!();
        println!("  Most probable precursor of {SVC_CRASH:?}:");
        println!("    Template   : {:?}", top.body);
        println!("    Avg lead   : {:.0} s  (fires this far before the crash)", top.avg_lead_secs);
        println!("    Jaccard    : {:.3}", top.jaccard);
        println!("    Co-occ     : {} shared buckets", top.co_occurrence_count);
    }

    println!();
    println!("  Done.");
    println!();
}
