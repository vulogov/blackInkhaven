//! Demonstrates template storage embedded in a [`Shard`].
//!
//! Templates live inside the shard's `tplstorage` [`DocumentStorage`] at
//! `{shard_path}/tplstorage`.  They are time-partitioned the same way
//! telemetry records are, but indexed and searched independently through the
//! `tpl_*` API surface.
//!
//! Run with:
//! ```
//! cargo run --example shard_tplstorage_demo
//! ```

use bdslib::common::error::{err_msg, Result};
use bdslib::embedding::Model;
use bdslib::shard::Shard;
use bdslib::EmbeddingEngine;
use serde_json::{json, Value};
use tempfile::TempDir;

// ── template catalogue ────────────────────────────────────────────────────────

struct Template {
    name:        &'static str,
    tags:        &'static [&'static str],
    description: &'static str,
    body:        &'static str,
}

const TEMPLATES: &[Template] = &[
    Template {
        name:        "cpu-high-runbook",
        tags:        &["ops", "cpu", "performance"],
        description: "Runbook for high CPU utilisation alerts",
        body:
            "## CPU High Utilisation\n\
             1. Run `top -bn1 | head -20` to identify the top processes.\n\
             2. Check for runaway jobs: `ps aux --sort=-%cpu | head -10`.\n\
             3. If a specific PID is at fault, gather a flame graph.\n\
             4. Verify cron jobs and batch tasks are not scheduled simultaneously.\n\
             5. Escalate to the on-call team if load average > 8 for more than 10 min.",
    },
    Template {
        name:        "memory-pressure-runbook",
        tags:        &["ops", "memory", "oom"],
        description: "Runbook for memory pressure and OOM events",
        body:
            "## Memory Pressure\n\
             1. Run `free -h` and `vmstat 1 5` to assess current state.\n\
             2. Check for OOM kills: `dmesg | grep -i 'out of memory'`.\n\
             3. Identify memory hogs: `ps aux --sort=-%mem | head -10`.\n\
             4. Review JVM heap settings if Java services are implicated.\n\
             5. If swap is exhausted, consider a controlled restart of heavy services.",
    },
    Template {
        name:        "network-unreachable-runbook",
        tags:        &["ops", "network", "connectivity"],
        description: "Runbook for network connectivity failures",
        body:
            "## Network Unreachable\n\
             1. Ping the default gateway: `ping -c 4 $(ip route | awk '/default/ {print $3}')`.\n\
             2. Check interface state: `ip link show` and `ethtool <nic>`.\n\
             3. Trace the route: `traceroute <destination>`.\n\
             4. Verify DNS: `dig @8.8.8.8 <hostname>`.\n\
             5. Review firewall rules: `iptables -L -n` or `nft list ruleset`.",
    },
    Template {
        name:        "database-slow-query-runbook",
        tags:        &["ops", "database", "performance", "sql"],
        description: "Runbook for database slow query and connection pool exhaustion",
        body:
            "## Database Slow Query\n\
             1. Identify slow queries: check `pg_stat_activity` or `SHOW FULL PROCESSLIST`.\n\
             2. Run EXPLAIN ANALYZE on the offending query.\n\
             3. Check index usage: look for sequential scans on large tables.\n\
             4. Inspect connection pool utilisation in the application metrics.\n\
             5. If replication lag is elevated, check replica I/O and network.",
    },
    Template {
        name:        "pagerduty-alert-notification",
        tags:        &["alerting", "pagerduty", "notification"],
        description: "PagerDuty-style alert notification template",
        body:
            "**[{{ severity | upper }}] {{ service }} — {{ summary }}**\n\n\
             - **Host:** {{ host }}\n\
             - **Environment:** {{ env }}\n\
             - **Triggered:** {{ timestamp }}\n\
             - **Runbook:** {{ runbook_url }}\n\n\
             > {{ details }}\n\n\
             Ack this alert in PagerDuty or escalate to the secondary on-call.",
    },
    Template {
        name:        "slack-incident-message",
        tags:        &["alerting", "slack", "incident"],
        description: "Slack channel message for incident declaration",
        body:
            ":rotating_light: *INCIDENT DECLARED* :rotating_light:\n\n\
             *Service:* {{ service }}\n\
             *Severity:* {{ severity }}\n\
             *Incident Commander:* {{ ic }}\n\
             *Bridge:* {{ bridge_url }}\n\
             *Status page:* {{ status_url }}\n\n\
             Please join the bridge and stand by for updates.",
    },
    Template {
        name:        "incident-postmortem",
        tags:        &["process", "postmortem", "documentation"],
        description: "Blameless postmortem template for incident review",
        body:
            "# Postmortem: {{ title }}\n\n\
             **Date:** {{ date }}  \n\
             **Duration:** {{ duration }}  \n\
             **Severity:** {{ severity }}  \n\
             **Author(s):** {{ authors }}\n\n\
             ## Summary\n{{ summary }}\n\n\
             ## Timeline\n| Time | Event |\n|------|-------|\n{{ timeline }}\n\n\
             ## Root Cause\n{{ root_cause }}\n\n\
             ## Contributing Factors\n{{ contributing_factors }}\n\n\
             ## Action Items\n| Owner | Action | Due |\n|-------|--------|-----|\n{{ action_items }}\n\n\
             ## Lessons Learned\n{{ lessons_learned }}",
    },
    Template {
        name:        "k8s-pod-crashloop-runbook",
        tags:        &["ops", "kubernetes", "k8s", "crashloop"],
        description: "Runbook for Kubernetes pod CrashLoopBackOff",
        body:
            "## Pod CrashLoopBackOff\n\
             1. Describe the pod: `kubectl describe pod <pod> -n <namespace>`.\n\
             2. Read the previous container logs: `kubectl logs <pod> -p -n <namespace>`.\n\
             3. Check resource limits: look for OOMKilled in the state section.\n\
             4. Verify the image exists and the tag is correct.\n\
             5. Inspect ConfigMaps/Secrets mounted by the pod.\n\
             6. Try a manual restart: `kubectl rollout restart deployment/<name>`.",
    },
    Template {
        name:        "slo-breach-notification",
        tags:        &["slo", "alerting", "reliability"],
        description: "SLO error budget burn rate alert notification",
        body:
            "**SLO Breach Alert**\n\n\
             Service **{{ service }}** is burning error budget at **{{ burn_rate }}×** the \
             expected rate.\n\n\
             - **SLO target:** {{ slo_target }}%\n\
             - **Current error rate:** {{ current_error_rate }}%\n\
             - **Budget remaining:** {{ budget_remaining }}%\n\
             - **Window:** {{ window }}\n\n\
             At this rate, the monthly budget will be exhausted in **{{ exhaustion_eta }}**.\n\
             Immediate investigation required.",
    },
    Template {
        name:        "auto-remediation-restart",
        tags:        &["automation", "remediation", "ops"],
        description: "Automated service restart script template",
        body:
            "#!/bin/bash\n\
             # Auto-remediation: restart {{ service }} on {{ host }}\n\
             set -euo pipefail\n\n\
             SERVICE={{ service }}\n\
             MAX_WAIT=30\n\n\
             echo \"[$(date -u +%FT%TZ)] Restarting $SERVICE\"\n\
             systemctl restart \"$SERVICE\"\n\n\
             for i in $(seq 1 $MAX_WAIT); do\n\
             \tsystemctl is-active --quiet \"$SERVICE\" && { echo \"[OK] $SERVICE is up\"; exit 0; }\n\
             \tsleep 1\n\
             done\n\n\
             echo \"[FAIL] $SERVICE did not recover in $MAX_WAIT seconds\" >&2\n\
             exit 1",
    },
];

// ── helpers ───────────────────────────────────────────────────────────────────

fn show_result(r: &Value, idx: usize) {
    let id    = r["id"].as_str().unwrap_or("?");
    let name  = r["metadata"]["name"].as_str().unwrap_or("?");
    let score = r["score"].as_f64().unwrap_or(0.0);
    let body  = r["document"].as_str().unwrap_or("");
    let preview: String = body.chars().take(72).collect();
    let ellipsis = if body.len() > 72 { "…" } else { "" };
    println!(
        "  [{idx}] score={score:.4}  name=\"{name}\"\n      id={id}\n      body: \"{preview}{ellipsis}\""
    );
}

fn divider(title: &str) {
    println!(
        "\n{}\n {}\n{}",
        "═".repeat(64), title, "═".repeat(64)
    );
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    println!("Loading embedding model (AllMiniLML6V2)…");
    let embedding = EmbeddingEngine::new(Model::AllMiniLML6V2, None)
        .map_err(|e| err_msg(format!("embedding init failed: {e}")))?;
    println!("Model ready.\n");

    let dir   = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let shard = Shard::new(dir.path().to_str().unwrap(), 4, embedding)?;

    println!("Shard created at: {}", dir.path().display());
    println!("  └─ obs.db         (telemetry observability)");
    println!("  └─ fts/           (telemetry full-text index)");
    println!("  └─ vec/           (telemetry vector index)");
    println!("  └─ tplstorage/    (template DocumentStorage)");
    println!("       └─ metadata.db");
    println!("       └─ blobs.db");
    println!("       └─ vectors/");

    // ── Section 1: store all templates ───────────────────────────────────────

    divider("Section 1: Storing templates");
    println!();

    const BASE_TS: u64 = 1_748_000_000;
    let mut ids = Vec::new();

    for (i, tpl) in TEMPLATES.iter().enumerate() {
        let ts = BASE_TS + i as u64 * 60;
        let metadata = json!({
            "name":        tpl.name,
            "tags":        tpl.tags,
            "description": tpl.description,
            "type":        "template",
            "timestamp":   ts,
            "created_at":  ts,
        });
        let id = shard.tpl_add(metadata, tpl.body.as_bytes())?;
        ids.push(id);
        println!("  stored  id={}  name=\"{}\"", id, tpl.name);
    }

    println!("\n  {} templates stored.", ids.len());

    // ── Section 2: list all templates ────────────────────────────────────────

    divider("Section 2: Listing all templates (tpl_list)");
    println!();

    let all = shard.tpl_list()?;
    println!("  Total stored: {}", all.len());
    println!();
    for (id, meta) in &all {
        let name  = meta["name"].as_str().unwrap_or("?");
        let tags  = meta["tags"].as_array()
            .map(|a| a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_default();
        println!("  {id}  name=\"{name}\"  tags=[{tags}]");
    }

    // ── Section 3: retrieve a specific template by ID ────────────────────────

    divider("Section 3: Point retrieval by ID (tpl_get_metadata + tpl_get_body)");
    println!();

    let target_id = ids[6]; // incident-postmortem
    let meta = shard.tpl_get_metadata(target_id)?.expect("template must exist");
    let body_bytes = shard.tpl_get_body(target_id)?.unwrap_or_default();
    let body = String::from_utf8_lossy(&body_bytes);

    println!("  id:          {target_id}");
    println!("  name:        {}", meta["name"].as_str().unwrap_or("?"));
    println!("  description: {}", meta["description"].as_str().unwrap_or("?"));
    println!("  tags:        {:?}", meta["tags"]);
    println!("  body preview:");
    for line in body.lines().take(5) {
        println!("    {line}");
    }
    if body.lines().count() > 5 {
        println!("    … ({} more lines)", body.lines().count() - 5);
    }

    // ── Section 4: semantic text search ──────────────────────────────────────

    divider("Section 4: Semantic text search (tpl_search_text)");

    let queries: &[(&str, usize)] = &[
        ("CPU load runaway process performance",                  3),
        ("memory out of memory OOM killer",                      3),
        ("kubernetes pod crash loop restart",                     3),
        ("database slow query connection pool",                   3),
        ("incident notification Slack PagerDuty alert",           3),
        ("SLO error budget burn rate reliability",                 3),
        ("automated bash script service remediation restart",     3),
    ];

    for (query, limit) in queries {
        println!("\n  query=\"{query}\"  limit={limit}");
        let results = shard.tpl_search_text(query, *limit)?;
        if results.is_empty() {
            println!("    (no results)");
        } else {
            for (i, r) in results.iter().enumerate() {
                show_result(r, i + 1);
            }
        }
    }

    // ── Section 5: JSON query search ─────────────────────────────────────────

    divider("Section 5: JSON query search (tpl_search_json)");
    println!();

    let json_queries: &[(&str, Value)] = &[
        ("ops + network runbook",
            json!({ "tags": ["ops", "network"], "description": "network connectivity failure runbook" })),
        ("postmortem documentation",
            json!({ "name": "postmortem", "description": "incident review documentation" })),
        ("automation script",
            json!({ "tags": ["automation"], "description": "automated bash remediation script" })),
    ];

    for (label, query) in json_queries {
        let results = shard.tpl_search_json(query, 3)?;
        println!("  query=\"{label}\"  ({} results)", results.len());
        for (i, r) in results.iter().enumerate() {
            show_result(r, i + 1);
        }
        println!();
    }

    // ── Section 6: update a template ─────────────────────────────────────────

    divider("Section 6: Update (tpl_update_metadata + tpl_update_body)");
    println!();

    let update_id = ids[0]; // cpu-high-runbook
    let original_meta = shard.tpl_get_metadata(update_id)?.unwrap();
    println!("  Before update:");
    println!("    name:  {}", original_meta["name"].as_str().unwrap_or("?"));
    println!("    tags:  {:?}", original_meta["tags"]);

    let mut new_meta = original_meta.clone();
    new_meta.as_object_mut().unwrap().insert(
        "name".to_owned(), json!("cpu-high-runbook-v2"),
    );
    new_meta.as_object_mut().unwrap().insert(
        "tags".to_owned(), json!(["ops", "cpu", "performance", "v2"]),
    );
    shard.tpl_update_metadata(update_id, new_meta)?;

    shard.tpl_update_body(
        update_id,
        b"## CPU High Utilisation (v2 - includes cgroup inspection)\n\
          1. Check cgroup CPU quotas: `cat /sys/fs/cgroup/cpu/cpu.cfs_quota_us`.\n\
          2. Run `top -bn1 | head -20` to identify top processes.\n\
          3. Verify kernel scheduler settings: `sysctl kernel.sched_*`.\n\
          4. If throttled, consider raising the cgroup quota or moving the workload.",
    )?;

    let updated_meta  = shard.tpl_get_metadata(update_id)?.unwrap();
    let updated_bytes = shard.tpl_get_body(update_id)?.unwrap();
    let updated_body  = String::from_utf8_lossy(&updated_bytes);
    println!("\n  After update:");
    println!("    name:  {}", updated_meta["name"].as_str().unwrap_or("?"));
    println!("    tags:  {:?}", updated_meta["tags"]);
    println!("    body:  {}…", &updated_body.chars().take(72).collect::<String>());

    // Updated template should surface for relevant queries.
    let search_after = shard.tpl_search_text("cgroup cpu quota throttling", 3)?;
    println!("\n  Search 'cgroup cpu quota throttling' after update:");
    for (i, r) in search_after.iter().enumerate() {
        show_result(r, i + 1);
    }

    // ── Section 7: delete a template ─────────────────────────────────────────

    divider("Section 7: Delete (tpl_delete)");
    println!();

    let del_id = ids[5]; // slack-incident-message
    let del_meta = shard.tpl_get_metadata(del_id)?.unwrap();
    println!("  Deleting  id={}  name=\"{}\"", del_id, del_meta["name"].as_str().unwrap_or("?"));

    let count_before = shard.tpl_list()?.len();
    shard.tpl_delete(del_id)?;
    let count_after = shard.tpl_list()?.len();

    println!("  Templates before: {count_before}  →  after: {count_after}");
    println!("  get_metadata after delete: {:?}", shard.tpl_get_metadata(del_id)?);

    let search_deleted = shard.tpl_search_text("slack incident message", 5)?;
    let found = search_deleted.iter().any(|r| r["id"].as_str().unwrap_or("") == del_id.to_string());
    println!("  Deleted template still in search results: {found}");

    // ── Section 8: telemetry + templates coexist ─────────────────────────────

    divider("Section 8: Telemetry and templates coexist in the same shard");
    println!();

    let tel_records = [
        json!({ "timestamp": BASE_TS + 1000, "key": "cpu.usage",   "data": 91 }),
        json!({ "timestamp": BASE_TS + 1030, "key": "cpu.usage",   "data": 94 }),
        json!({ "timestamp": BASE_TS + 1060, "key": "mem.pressure", "data": "critical" }),
        json!({ "timestamp": BASE_TS + 1090, "key": "net.error",    "data": "connection refused" }),
    ];

    let mut tel_ids = Vec::new();
    for rec in &tel_records {
        let id = shard.add(rec.clone())?;
        tel_ids.push(id);
        println!("  telemetry: id={}  key={}  data={}", id, rec["key"], rec["data"]);
    }

    println!();
    println!("  Templates in tplstorage:     {}", shard.tpl_list()?.len());
    println!("  Primaries in observability:  {}", shard.observability().list_primaries()?.len());

    // FTS index contains only telemetry fingerprints.
    let tpl_in_fts = shard.search_fts("runbook", 5)?;
    println!("\n  FTS search 'runbook' hits (should be 0, runbooks are in tplstorage): {}",
        tpl_in_fts.len());

    // Template search only returns templates — telemetry IDs must never appear.
    let tel_ids: std::collections::HashSet<String> =
        tel_ids.iter().map(|id| id.to_string()).collect();
    let tel_in_tpl = shard.tpl_search_text("cpu usage 91", 5)?;
    let tpl_has_tel = tel_in_tpl.iter()
        .any(|r| tel_ids.contains(r["id"].as_str().unwrap_or("")));
    println!("  Template search 'cpu usage 91': {} results (may include CPU runbook)",
        tel_in_tpl.len());
    println!("  Any result is a telemetry ID (must be false): {tpl_has_tel}");

    // ── Section 9: reindex ────────────────────────────────────────────────────

    divider("Section 9: Rebuild vector index (tpl_reindex)");
    println!();

    let indexed = shard.tpl_reindex()?;
    println!("  tpl_reindex: {indexed} templates re-embedded and indexed");

    // Search should still work after rebuild.
    let after_reindex = shard.tpl_search_text("kubernetes pod crash loop", 3)?;
    println!("\n  Search 'kubernetes pod crash loop' after reindex ({} results):", after_reindex.len());
    for (i, r) in after_reindex.iter().enumerate() {
        show_result(r, i + 1);
    }

    // ── Section 10: sync ─────────────────────────────────────────────────────

    divider("Section 10: Flush to disk (sync)");
    println!();

    shard.sync()?;
    println!("  shard.sync() completed — telemetry + tplstorage flushed.");

    println!("\nDone.");
    Ok(())
}
