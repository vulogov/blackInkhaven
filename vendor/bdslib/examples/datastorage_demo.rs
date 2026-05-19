use bdslib::common::error::{err_msg, Result};
use bdslib::datastorage::{BlobStorage, JsonStorage, JsonStorageConfig};
use serde_json::json;
use tempfile::TempDir;

fn main() -> Result<()> {
    // ── BlobStorage ───────────────────────────────────────────────────────────

    println!("=== BlobStorage ===\n");

    let blob_dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let blob_path = blob_dir.path().join("blobs.db");
    let blobs = BlobStorage::new(blob_path.to_str().unwrap(), 4)?;

    // add
    let payload1 = b"Hello, bdslib! This is binary payload #1.";
    let id1 = blobs.add_blob(payload1)?;
    println!("add_blob  -> {id1}  ({} bytes)", payload1.len());

    let payload2: Vec<u8> = (0u8..=255).collect();
    let id2 = blobs.add_blob(&payload2)?;
    println!("add_blob  -> {id2}  (256-byte binary ramp)");

    // get
    let got1 = blobs.get_blob(id1)?.expect("should exist");
    println!("\nget_blob({id1})");
    println!("  content : {:?}", String::from_utf8_lossy(&got1));

    let got2 = blobs.get_blob(id2)?.expect("should exist");
    println!("get_blob({id2})");
    println!("  bytes 0..4 : {:?}", &got2[..4]);
    println!("  bytes match: {}", got2 == payload2);

    // get non-existent
    let missing = uuid::Uuid::now_v7();
    println!("\nget_blob({missing})  ->  {:?}", blobs.get_blob(missing)?);

    // update
    blobs.update_blob(id1, b"Payload #1 has been updated.")?;
    let updated = blobs.get_blob(id1)?.expect("should still exist");
    println!("\nupdate_blob({id1})");
    println!("  new content: {:?}", String::from_utf8_lossy(&updated));

    // drop
    blobs.drop_blob(id2)?;
    println!("\ndrop_blob({id2})");
    println!("  exists after drop: {}", blobs.get_blob(id2)?.is_some());

    // clone shares the store
    println!("\n-- clone shares underlying store --");
    let clone = blobs.clone();
    let id3 = blobs.add_blob(b"written via original")?;
    let via_clone = clone.get_blob(id3)?.expect("clone must see same data");
    println!("  original wrote {id3}");
    println!("  clone read   : {:?}", String::from_utf8_lossy(&via_clone));

    println!();

    // ── JsonStorage — no key field (default key) ──────────────────────────────

    println!("=== JsonStorage — default key ===\n");

    let json_dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let json_path = json_dir.path().join("json.db");
    let json_store = JsonStorage::new(
        json_path.to_str().unwrap(),
        4,
        JsonStorageConfig::default(), // key_field: None, default_key: "default"
    )?;

    let doc_a = json!({ "message": "first document", "value": 1 });
    let id_a = json_store.add_json(doc_a.clone())?;
    println!("add_json (doc_a) -> {id_a}");

    // No key_field → all docs share "default" key → second add_json is an upsert
    let doc_b = json!({ "message": "second document (overwrites first)", "value": 2 });
    let id_b = json_store.add_json(doc_b)?;
    println!("add_json (doc_b) -> {id_b}  (same UUID — upserted)");
    assert_eq!(id_a, id_b, "same key must return the same UUID");

    let got = json_store.get_json(id_a)?.expect("should exist");
    println!("get_json  -> {got}");

    println!();

    // ── JsonStorage — key field extraction ────────────────────────────────────

    println!("=== JsonStorage — key_field = \"id\" ===\n");

    let keyed_dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let keyed_path = keyed_dir.path().join("keyed.db");
    let keyed = JsonStorage::new(
        keyed_path.to_str().unwrap(),
        4,
        JsonStorageConfig {
            key_field: Some("id".to_string()),
            default_key: "anonymous".to_string(),
        },
    )?;

    // Insert three distinct documents
    let users = [
        json!({ "id": "user-1", "name": "Alice", "role": "admin" }),
        json!({ "id": "user-2", "name": "Bob",   "role": "viewer" }),
        json!({ "id": "user-3", "name": "Carol",  "role": "editor" }),
    ];

    let mut ids = Vec::new();
    for u in &users {
        let id = keyed.add_json(u.clone())?;
        ids.push(id);
        println!("add_json  id={} -> {}", u["id"], id);
    }
    println!();

    // Upsert by key: same "id" field → updates in place, returns original UUID
    let updated_user1 = json!({ "id": "user-1", "name": "Alice", "role": "superadmin" });
    let upsert_id = keyed.add_json(updated_user1)?;
    println!("add_json  id=user-1 (role changed)  -> {upsert_id}  (same UUID: {})", upsert_id == ids[0]);

    let got = keyed.get_json(ids[0])?.expect("user-1 should exist");
    println!("get_json(user-1) -> {got}");
    println!();

    // Explicit update
    keyed.update_json(ids[1], json!({ "id": "user-2", "name": "Bob", "role": "admin" }))?;
    let got2 = keyed.get_json(ids[1])?.expect("user-2 should exist");
    println!("update_json(user-2)  -> {got2}");
    println!();

    // Document missing the key field → falls back to default_key
    let anon = json!({ "source": "import", "data": "no id field here" });
    let anon_id = keyed.add_json(anon)?;
    println!("add_json (no 'id' field) -> {anon_id}  (key = 'anonymous')");

    // Second anonymous doc upserts the first
    let anon2 = json!({ "source": "import", "data": "also no id field" });
    let anon_id2 = keyed.add_json(anon2)?;
    println!("add_json (no 'id' field) -> {anon_id2}  (same UUID: {})", anon_id == anon_id2);
    println!();

    // ── JsonStorage — nested key path ─────────────────────────────────────────

    println!("=== JsonStorage — key_field = \"meta.source\" ===\n");

    let nested_dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let nested_path = nested_dir.path().join("nested.db");
    let nested = JsonStorage::new(
        nested_path.to_str().unwrap(),
        4,
        JsonStorageConfig {
            key_field: Some("meta.source".to_string()),
            default_key: "unknown".to_string(),
        },
    )?;

    let events = [
        json!({ "meta": { "source": "sensor-A" }, "temp": 21.5, "unit": "C" }),
        json!({ "meta": { "source": "sensor-B" }, "temp": 19.0, "unit": "C" }),
    ];

    for e in &events {
        let id = nested.add_json(e.clone())?;
        println!("add_json  source={} -> {id}", e["meta"]["source"]);
    }

    // Update sensor-A reading
    let updated = json!({ "meta": { "source": "sensor-A" }, "temp": 22.3, "unit": "C" });
    let upd_id = nested.add_json(updated)?;
    let reading = nested.get_json(upd_id)?.expect("sensor-A should exist");
    println!("upsert sensor-A  temp -> {}", reading["temp"]);
    println!();

    // ── documents with special characters ────────────────────────────────────

    println!("=== Documents with special characters ===\n");

    let special_dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let special_path = special_dir.path().join("special.db");
    let special = JsonStorage::new(
        special_path.to_str().unwrap(),
        4,
        JsonStorageConfig {
            key_field: Some("name".to_string()),
            default_key: "default".to_string(),
        },
    )?;

    let tricky = json!({ "name": "O'Brien", "quote": "it's a trap", "data": [1, 2, 3] });
    let t_id = special.add_json(tricky.clone())?;
    let t_got = special.get_json(t_id)?.expect("should exist");
    println!("add_json  name=O'Brien -> {t_id}");
    println!("get_json  -> {t_got}");
    println!("  roundtrip ok: {}", t_got == tricky);

    // drop
    special.drop_json(t_id)?;
    println!("drop_json({t_id})  exists: {}", special.get_json(t_id)?.is_some());

    Ok(())
}
