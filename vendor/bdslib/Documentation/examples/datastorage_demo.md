# datastorage_demo.rs

**File:** `examples/datastorage_demo.rs`

Demonstrates `BlobStorage` and `JsonStorage`: storing, retrieving, updating, and dropping arbitrary data blobs and JSON documents with optional key-based deduplication.

## What it demonstrates

### BlobStorage

| Operation | Description |
|---|---|
| `add_blob(bytes)` | Store raw bytes; returns a UUIDv7 |
| `get_blob(id)` | Retrieve bytes by UUID |
| `update_blob(id, bytes)` | Replace the stored bytes for an ID |
| `drop_blob(id)` | Delete the entry |
| Clone semantics | Cloned `BlobStorage` instances share the same underlying store |

### JsonStorage

| Operation | Description |
|---|---|
| `add_json(doc)` | Store a JSON document; returns a UUIDv7 |
| `get_json(id)` | Retrieve a document by UUID |
| `update_json(id, doc)` | Replace the document for an ID |
| `drop_json(id)` | Delete the entry |

### JsonStorage key deduplication modes

| Mode | Description |
|---|---|
| Default key | All documents share one slot — subsequent adds are upserts |
| `key_field` | Key is extracted from a named field in the document |
| Nested key path | Key extracted from a nested path like `"meta.id"` |
| Numeric/bool keys | Non-string key values are coerced to strings for keying |

## Key concepts

**UUIDv7 ordering** — all returned IDs are time-ordered, enabling chronological sorting of entries without a separate timestamp column.

**Upsert semantics** — when a document matches an existing key (via `key_field`), `add_json` returns the *original* UUID and replaces the stored document. The UUID is stable across updates.

**Nested key paths** — `key_field = "meta.id"` extracts from `{"meta": {"id": "x"}}`, enabling deduplication against deeply nested identifiers.

## Example flow

```rust
let store = BlobStorage::new(":memory:")?;
let id = store.add_blob(b"hello")?;
let data = store.get_blob(&id)?;     // Some(b"hello")
store.update_blob(&id, b"world")?;
store.drop_blob(&id)?;
```
