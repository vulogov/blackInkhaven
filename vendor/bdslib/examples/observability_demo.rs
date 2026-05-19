use bdslib::common::error::{err_msg, Result};
use bdslib::observability::{ObservabilityStorage, ObservabilityStorageConfig};
use bdslib::EmbeddingEngine;
use bdslib::embedding::Model;
use serde_json::json;
use std::time::{Duration, UNIX_EPOCH};
use tempfile::TempDir;

// ── helpers ───────────────────────────────────────────────────────────────────

fn tel(key: &str, data: serde_json::Value, ts: u64) -> serde_json::Value {
    json!({ "timestamp": ts, "key": key, "data": data })
}

fn tel_meta(key: &str, data: serde_json::Value, ts: u64, host: &str) -> serde_json::Value {
    json!({ "timestamp": ts, "key": key, "data": data, "host": host, "env": "prod" })
}

fn systime_label(ts: u64) -> String {
    let t = UNIX_EPOCH + Duration::from_secs(ts);
    let secs = t.duration_since(UNIX_EPOCH).unwrap().as_secs();
    format!("T+{secs}s")
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    println!("Loading embedding model (AllMiniLML6V2)...");
    let embedding = EmbeddingEngine::new(Model::AllMiniLML6V2, None)
        .map_err(|e| err_msg(format!("embedding init failed: {e}")))?;
    println!("Model ready.\n");

    // ── Section 1: Deduplication with timestamp preservation ──────────────────

    println!("════════════════════════════════════════");
    println!(" Section 1: Deduplication");
    println!("════════════════════════════════════════\n");

    let dir1 = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let path1 = dir1.path().join("dedup.db");
    let store1 = ObservabilityStorage::new(path1.to_str().unwrap(), 4, embedding.clone())?;

    // Submit the same (key, data) four times at different timestamps.
    // The first submission is stored; the remaining three are duplicates.
    let base_ts: u64 = 1_700_000_000;
    let offsets = [0u64, 60, 120, 180]; // one reading every minute

    println!("Submitting 'cpu.usage' = 82 four times (every 60 s):");
    let mut stored_id = None;
    for &offset in &offsets {
        let ts = base_ts + offset;
        let (id, _, _) = store1.add(tel("cpu.usage", json!(82), ts))?;
        if stored_id.is_none() {
            stored_id = Some(id);
            println!("  [{}]  stored  -> {id}", systime_label(ts));
        } else {
            let is_same = id == stored_id.unwrap();
            println!(
                "  [{}]  dedup   -> {id}  (same UUID: {is_same})",
                systime_label(ts)
            );
        }
    }

    // The store should contain exactly one record.
    let range_start = UNIX_EPOCH + Duration::from_secs(base_ts);
    let range_end = UNIX_EPOCH + Duration::from_secs(base_ts + 300);
    let ids_in_range = store1.list_ids_by_time_range(range_start, range_end)?;
    println!("\nRecords in store for the 5-minute window: {}", ids_in_range.len());

    // Duplicate timestamps are preserved in the dedup log.
    let dup_times = store1.get_duplicate_timestamps("cpu.usage")?;
    println!("Duplicate timestamps recorded ({} entries):", dup_times.len());
    for dt in &dup_times {
        let secs = dt.duration_since(UNIX_EPOCH).unwrap().as_secs();
        println!("  {}", systime_label(secs));
    }

    // A different data value for the same key is a new record, not a duplicate.
    let (id_spike, _, _) = store1.add(tel("cpu.usage", json!(97), base_ts + 240))?;
    println!(
        "\nSubmitting 'cpu.usage' = 97 (different value) -> {id_spike}"
    );
    let ids_after = store1.list_ids_by_time_range(range_start, range_end)?;
    println!("Records in store now: {}", ids_after.len());

    // ── Section 2: Mixed data types — all become primaries ────────────────────

    println!("\n════════════════════════════════════════");
    println!(" Section 2: Mixed data types");
    println!("════════════════════════════════════════\n");

    // threshold > 1.0 so the classifier never marks anything secondary;
    // every record becomes its own primary regardless of similarity.
    let dir2 = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let path2 = dir2.path().join("mixed.db");
    let store2 = ObservabilityStorage::with_config(
        path2.to_str().unwrap(),
        4,
        embedding.clone(),
        ObservabilityStorageConfig { similarity_threshold: 1.1 },
    )?;

    let mixed_events: &[(&str, serde_json::Value, &str)] = &[
        ("cpu.usage",    json!(72),             "integer metric"),
        ("health.ok",    json!(true),           "boolean flag"),
        ("response.lat", json!(0.034),          "float measurement"),
        ("error.msg",    json!("disk full"),    "plain string"),
        ("db.stats",     json!({"queries": 1200, "slow": 3, "errors": 0}), "JSON object"),
        ("tags",         json!(["web", "api", "v2"]), "JSON array"),
    ];

    println!("Storing one record per data type (threshold=1.1 → all primaries):\n");
    for (key, data, label) in mixed_events {
        let (id, _, _) = store2.add(tel(key, data.clone(), base_ts))?;
        println!("  {label:<22}  key={key:<14}  data={data:<35}  -> {id}");
    }

    let primaries = store2.list_primaries()?;
    println!("\nTotal primaries: {}  (matches records submitted: {})", primaries.len(), mixed_events.len());

    // Verify each record is retrievable and its data survives the round-trip.
    println!("\nRound-trip check for JSON object:");
    let db_stats_id = store2
        .list_ids_by_time_range(
            UNIX_EPOCH + Duration::from_secs(base_ts),
            UNIX_EPOCH + Duration::from_secs(base_ts + 1),
        )?
        .into_iter()
        .find(|_| true) // just grab one to demo get_by_id
        .unwrap();
    let doc = store2.get_by_id(db_stats_id)?.unwrap();
    println!("  get_by_id({db_stats_id})");
    println!("  key       : {}", doc["key"]);
    println!("  data      : {}", doc["data"]);
    println!("  timestamp : {}", doc["timestamp"]);

    // ── Section 3: Metadata preserved alongside mandatory fields ──────────────

    println!("\n════════════════════════════════════════");
    println!(" Section 3: Metadata extraction");
    println!("════════════════════════════════════════\n");

    let dir3 = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let path3 = dir3.path().join("meta.db");
    let store3 = ObservabilityStorage::new(path3.to_str().unwrap(), 4, embedding.clone())?;

    let (id_meta, _, _) = store3.add(tel_meta("mem.rss", json!(1_073_741_824u64), base_ts, "worker-03"))?;
    let retrieved = store3.get_by_id(id_meta)?.unwrap();
    println!("Submitted: timestamp, key, data  +  host, env extra fields");
    println!("Retrieved document:");
    println!("  id        : {}", retrieved["id"]);
    println!("  timestamp : {}", retrieved["timestamp"]);
    println!("  key       : {}", retrieved["key"]);
    println!("  data      : {}", retrieved["data"]);
    println!("  host      : {}", retrieved["host"]);
    println!("  env       : {}", retrieved["env"]);

    // ── Section 4: Primary / secondary splitting ───────────────────────────────

    println!("\n════════════════════════════════════════");
    println!(" Section 4: Primary / secondary splitting");
    println!("════════════════════════════════════════\n");

    // Default threshold (0.85).  Semantically close sentences cluster under one
    // primary; clearly unrelated signals (numeric, boolean, JSON) each become
    // their own primary.
    let dir4 = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let path4 = dir4.path().join("classify.db");
    let store4 = ObservabilityStorage::new(path4.to_str().unwrap(), 4, embedding.clone())?;

    // First batch: semantically related error descriptions.  After the first one
    // is stored as a primary, subsequent ones that the model scores >= 0.85
    // cosine similarity will become secondaries linked to it.
    let error_texts: &[(&str, u64)] = &[
        ("connection refused by remote host", 1_000),
        ("remote host refused the connection", 2_000),
        ("host refused connection attempt",   3_000),
        ("peer rejected connection request",  4_000),
    ];

    println!("Batch A — semantically related error strings (key=\"net.error\"):\n");
    let mut error_ids = Vec::new();
    for (msg, offset) in error_texts {
        let (id, _, _) = store4.add(tel("net.error", json!(msg), base_ts + offset))?;
        error_ids.push((id, *msg));
        println!("  \"{msg}\"  -> {id}");
    }

    // Second batch: clearly distinct signal types — each should become its own primary.
    println!("\nBatch B — distinct signal types (different keys and data types):\n");
    let distinct: &[(&str, serde_json::Value, u64)] = &[
        ("cpu.temp",   json!(68),        5_000),
        ("disk.full",  json!(true),      6_000),
        ("req.latency",json!(0.045),     7_000),
    ];

    let mut distinct_ids = Vec::new();
    for (key, data, offset) in distinct {
        let (id, _, _) = store4.add(tel(key, data.clone(), base_ts + offset))?;
        distinct_ids.push(id);
        println!("  key={key:<14}  data={:<8}  -> {id}", data.to_string());
    }

    // Show how the classifier partitioned everything.
    let all_primaries = store4.list_primaries()?;
    println!("\nTotal primaries: {}", all_primaries.len());

    let mut total_secondaries = 0;
    for pid in &all_primaries {
        let secs = store4.list_secondaries(*pid)?;
        total_secondaries += secs.len();
        let rec = store4.get_by_id(*pid)?.unwrap();
        let data_preview = rec["data"].to_string();
        let preview = if data_preview.len() > 45 {
            format!("{}…", &data_preview[..45])
        } else {
            data_preview
        };
        if secs.is_empty() {
            println!("  PRIMARY  key={:<14}  data={preview}", rec["key"].as_str().unwrap_or("?"));
        } else {
            println!(
                "  PRIMARY  key={:<14}  data={preview}  ({} secondaries)",
                rec["key"].as_str().unwrap_or("?"),
                secs.len()
            );
            for sid in &secs {
                let srec = store4.get_by_id(*sid)?.unwrap();
                println!("    SECONDARY  data={}", srec["data"]);
            }
        }
    }
    println!("Total secondaries: {total_secondaries}");

    // ── Section 5: Time-range queries ────────────────────────────────────────

    println!("\n════════════════════════════════════════");
    println!(" Section 5: Time-range queries");
    println!("════════════════════════════════════════\n");

    // The store4 dataset spans base_ts + 1000 … base_ts + 7000.
    // Query the first half (batch A) and the second half (batch B) separately.
    let t_start = UNIX_EPOCH + Duration::from_secs(base_ts);
    let t_mid   = UNIX_EPOCH + Duration::from_secs(base_ts + 5_000);
    let t_end   = UNIX_EPOCH + Duration::from_secs(base_ts + 8_000);

    let batch_a_ids = store4.list_ids_by_time_range(t_start, t_mid)?;
    let batch_b_ids = store4.list_ids_by_time_range(t_mid, t_end)?;
    println!(
        "Records in [T, T+5000): {}  (batch A — error strings)",
        batch_a_ids.len()
    );
    println!(
        "Records in [T+5000, T+8000): {}  (batch B — distinct signals)",
        batch_b_ids.len()
    );

    let primaries_in_a = store4.list_primaries_in_range(t_start, t_mid)?;
    let primaries_in_b = store4.list_primaries_in_range(t_mid, t_end)?;
    println!(
        "\nPrimaries in batch A window: {}",
        primaries_in_a.len()
    );
    println!(
        "Primaries in batch B window: {}",
        primaries_in_b.len()
    );

    // ── Section 6: Delete and verify cleanup ─────────────────────────────────

    println!("\n════════════════════════════════════════");
    println!(" Section 6: Delete");
    println!("════════════════════════════════════════\n");

    let dir6 = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let path6 = dir6.path().join("delete.db");
    let store6 = ObservabilityStorage::new(path6.to_str().unwrap(), 4, embedding.clone())?;

    let (id_a, _, _) = store6.add(tel("x", json!("alpha"),   base_ts))?;
    let (_id_b, _, _) = store6.add(tel("x", json!("beta"),    base_ts + 1))?;
    let (id_c, _, _) = store6.add(tel("y", json!("gamma"),    base_ts + 2))?;

    println!("Stored 3 records under keys 'x' (2 records) and 'y' (1 record).");

    store6.delete_by_id(id_a)?;
    let x_after_id_del = store6.get_by_key("x")?;
    println!("After delete_by_id(alpha):  'x' records remaining = {}", x_after_id_del.len());

    store6.delete_by_key("x")?;
    let x_after_key_del = store6.get_by_key("x")?;
    println!("After delete_by_key('x'):   'x' records remaining = {}", x_after_key_del.len());

    let y_records = store6.get_by_key("y")?;
    println!("'y' records unaffected:     {}", y_records.len());
    let y_doc = store6.get_by_id(id_c)?.unwrap();
    println!("  get_by_id({id_c}) -> data={}", y_doc["data"]);

    println!("\nDone.");
    Ok(())
}
