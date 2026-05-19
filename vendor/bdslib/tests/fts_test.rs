use bdslib::fts::FTSEngine;
use std::sync::Arc;
use tempfile::TempDir;

// --- construction ---

#[test]
fn test_new_memory() {
    FTSEngine::new(":memory:").expect("in-memory engine should construct");
}

#[test]
fn test_new_file_backed() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("idx").to_string_lossy().into_owned();
    FTSEngine::new(&path).expect("file-backed engine should construct");
}

// --- add_document ---

#[test]
fn test_add_returns_uuidv7() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let id = engine.add_document("the quick brown fox").unwrap();
    assert_eq!(id.get_version_num(), 7, "assigned ID must be UUIDv7");
}

#[test]
fn test_add_same_text_produces_distinct_ids() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let id1 = engine.add_document("duplicate text").unwrap();
    let id2 = engine.add_document("duplicate text").unwrap();
    assert_ne!(id1, id2);
}

#[test]
fn test_uuidv7_ids_are_monotonically_increasing() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let id1 = engine.add_document("first document").unwrap();
    let id2 = engine.add_document("second document").unwrap();
    assert!(id2 > id1, "later UUIDv7 must sort after the earlier one");
}

// --- search ---

#[test]
fn test_search_empty_index_returns_empty() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let results = engine.search("hello", 10).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_search_finds_added_document() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let id = engine.add_document("the quick brown fox jumps").unwrap();
    let results = engine.search("fox", 10).unwrap();
    assert!(results.contains(&id));
}

#[test]
fn test_search_no_match_returns_empty() {
    let engine = FTSEngine::new(":memory:").unwrap();
    engine.add_document("the quick brown fox").unwrap();
    let results = engine.search("elephant", 10).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_search_is_selective() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let id_rust = engine.add_document("rust systems programming language").unwrap();
    let _id_python = engine.add_document("python scripting language").unwrap();

    let results = engine.search("rust", 10).unwrap();
    assert!(results.contains(&id_rust));
    assert_eq!(results.len(), 1, "only the rust document should match");
}

#[test]
fn test_search_returns_all_matching_documents() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let id1 = engine.add_document("database storage engine").unwrap();
    let id2 = engine.add_document("storage backend design").unwrap();
    let _id3 = engine.add_document("networking and protocols").unwrap();

    let results = engine.search("storage", 10).unwrap();
    assert!(results.contains(&id1));
    assert!(results.contains(&id2));
    assert_eq!(results.len(), 2);
}

#[test]
fn test_search_respects_limit() {
    let engine = FTSEngine::new(":memory:").unwrap();
    for i in 0..10 {
        engine.add_document(&format!("common word document {i}")).unwrap();
    }
    let results = engine.search("common", 3).unwrap();
    assert_eq!(results.len(), 3);
}

#[test]
fn test_search_returns_correct_ids_not_aliases() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let id = engine.add_document("unique xyzzy token").unwrap();
    let results = engine.search("xyzzy", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], id);
}

// --- drop_document ---

#[test]
fn test_drop_removes_document_from_search() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let id = engine.add_document("soon to be gone").unwrap();
    assert!(engine.search("gone", 10).unwrap().contains(&id));

    engine.drop_document(id).unwrap();
    assert!(!engine.search("gone", 10).unwrap().contains(&id));
}

#[test]
fn test_drop_nonexistent_uuid_is_silent() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let phantom = uuid::Uuid::now_v7();
    engine.drop_document(phantom).expect("dropping unknown UUID should not error");
}

#[test]
fn test_drop_only_removes_target_document() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let id_keep = engine.add_document("shared word alpha").unwrap();
    let id_drop = engine.add_document("shared word beta").unwrap();

    engine.drop_document(id_drop).unwrap();

    let results = engine.search("shared", 10).unwrap();
    assert!(results.contains(&id_keep), "surviving document must still appear");
    assert!(!results.contains(&id_drop), "dropped document must not appear");
}

// --- error handling ---

#[test]
fn test_invalid_query_returns_error() {
    let engine = FTSEngine::new(":memory:").unwrap();
    // Unmatched quote is a parse error in Tantivy
    let result = engine.search("\"unclosed phrase", 10);
    assert!(result.is_err());
}

// --- file-backed lifecycle ---

#[test]
fn test_file_backed_add_search_drop() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("idx").to_string_lossy().into_owned();
    let engine = FTSEngine::new(&path).unwrap();

    let id = engine.add_document("persistent tantivy index").unwrap();

    let results = engine.search("persistent", 10).unwrap();
    assert!(results.contains(&id));

    engine.drop_document(id).unwrap();
    assert!(engine.search("persistent", 10).unwrap().is_empty());
}

// --- sync ---

#[test]
fn test_sync_memory_does_not_error() {
    let engine = FTSEngine::new(":memory:").unwrap();
    engine.add_document("something to commit").unwrap();
    engine.sync().expect("sync on in-memory index should not error");
}

#[test]
fn test_sync_file_backed_does_not_error() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("idx").to_string_lossy().into_owned();
    let engine = FTSEngine::new(&path).unwrap();
    engine.add_document("persistent content").unwrap();
    engine.sync().expect("sync on file-backed index should not error");
}

#[test]
fn test_sync_data_readable_after_sync() {
    let engine = FTSEngine::new(":memory:").unwrap();
    let id = engine.add_document("post sync searchable").unwrap();
    engine.sync().unwrap();
    let results = engine.search("searchable", 10).unwrap();
    assert!(results.contains(&id));
}

#[test]
fn test_sync_on_empty_index_does_not_error() {
    let engine = FTSEngine::new(":memory:").unwrap();
    engine.sync().expect("sync on empty index should not error");
}

// --- concurrency ---

#[test]
fn test_concurrent_adds_produce_unique_ids() {
    use std::collections::HashSet;
    let engine = Arc::new(FTSEngine::new(":memory:").unwrap());
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let e = engine.clone();
            std::thread::spawn(move || {
                e.add_document(&format!("thread document {i}")).unwrap()
            })
        })
        .collect();

    let ids: HashSet<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(ids.len(), 8, "every concurrent add must yield a distinct UUIDv7");
}
