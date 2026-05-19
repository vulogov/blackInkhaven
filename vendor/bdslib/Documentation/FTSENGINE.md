# FTSEngine API

`FTSEngine` provides a simplified full-text search interface over [Tantivy](https://github.com/quickwit-oss/tantivy). Every document is assigned a UUIDv7 at insertion time, which serves as the stable identifier for retrieval and deletion. All write operations are immediately consistent: each one commits to the index and reloads the reader before returning.

All methods return `bdslib::common::error::Result<T>` — an alias for `Result<T, easy_error::Error>` defined in the shared [`common::error`](COMMON.md) module.

## Construction

```rust
FTSEngine::new(path: &str) -> Result<FTSEngine>
```

Creates or opens a Tantivy index at `path`.

| `path` value | Behaviour |
|---|---|
| `":memory:"` | RAM-only index; data is lost when the engine is dropped |
| Any other string | Treated as a filesystem directory path; created if it does not exist |

The index schema has two fields: `id` (exact-match, stored) and `body` (full-text tokenised, stored). The writer is allocated a 50 MB heap.

Returns `Err` if the directory cannot be created, the index cannot be opened, or internal Tantivy initialisation fails.

```rust
// In-memory — useful for tests and ephemeral workloads
let engine = FTSEngine::new(":memory:")?;

// File-backed — survives process restarts
let engine = FTSEngine::new("/var/lib/myapp/search-index")?;
```

`FTSEngine` is not `Clone`. Wrap in `Arc` to share across threads.

---

## Methods

### `add_document`

```rust
fn add_document(&self, text: &str) -> Result<Uuid>
```

Indexes `text` and returns its assigned UUIDv7. The document is committed and the reader is reloaded before returning, so the new document is immediately visible to `search`.

UUIDv7 identifiers are time-ordered: a document added later always has a greater UUID than one added earlier, making them suitable for use as sortable primary keys.

```rust
let id = engine.add_document("the quick brown fox jumps over the lazy dog")?;
println!("indexed as {id}");
```

---

### `drop_document`

```rust
fn drop_document(&self, id: Uuid) -> Result<()>
```

Removes the document with the given UUIDv7 from the index. Succeeds silently if the UUID does not exist — callers do not need to check for presence before calling.

The deletion is committed and the reader is reloaded before returning, so the document immediately disappears from subsequent `search` calls.

```rust
engine.drop_document(id)?;
// subsequent searches will not return `id`
```

---

### `search`

```rust
fn search(&self, query: &str, limit: usize) -> Result<Vec<Uuid>>
```

Parses `query` using Tantivy's query language and returns up to `limit` UUIDv7s ordered by descending relevance score. Returns an empty `Vec` when there are no matches.

Returns `Err` if `query` cannot be parsed (e.g. an unclosed phrase literal).

#### Query syntax

| Example | Matches |
|---|---|
| `fox` | Documents containing the word "fox" |
| `fox jumps` | Documents containing either "fox" or "jumps" (union) |
| `fox AND jumps` | Documents containing both "fox" and "jumps" |
| `fox OR wolf` | Documents containing "fox" or "wolf" |
| `"quick brown"` | Documents containing the exact phrase |
| `fox -wolf` | Documents with "fox" but not "wolf" |

```rust
let ids = engine.search("quick brown", 20)?;
for id in ids {
    println!("{id}");
}
```

---

### `sync`

```rust
fn sync(&self) -> Result<()>
```

Flushes any pending writer state to the index directory and reloads the reader. For file-backed indexes this ensures all previously committed segments are durably written. For in-memory indexes the call is safe and consistent but has no persistence effect.

Mirrors the `StorageEngine::sync` / DuckDB `CHECKPOINT` pattern. Call after a batch of writes when you need an explicit flush point.

```rust
engine.add_document("batch item one")?;
engine.add_document("batch item two")?;
engine.sync()?;
```

---

## Consistency model

Every write method (`add_document`, `drop_document`) issues a Tantivy `commit` followed by a reader `reload` before returning. This means:

- A document added by `add_document` is visible to `search` on the very next call.
- A document removed by `drop_document` is invisible to `search` on the very next call.
- Concurrent reads via `search` are non-blocking and do not interact with the writer mutex.

The writer is held behind a `parking_lot::Mutex`, so concurrent writes are serialised. Concurrent reads are fully parallel.

---

## Thread safety

`FTSEngine` is `Send + Sync`. Wrap in `Arc` to share across threads:

```rust
let engine = Arc::new(FTSEngine::new(":memory:")?);

let e = engine.clone();
std::thread::spawn(move || {
    let id = e.add_document("from another thread").unwrap();
    println!("indexed {id}");
});
```

---

## UUIDv7 properties

All document identifiers are [UUID version 7](https://www.rfc-editor.org/rfc/rfc9562#section-5.7) — 128-bit, time-ordered, and globally unique.

- **Sortable**: later insertions always produce a greater UUID, so sorting by ID is equivalent to sorting by insertion time.
- **Unique**: two calls to `add_document` with identical text still produce distinct IDs.
- **Opaque**: the UUID carries no information about document content.

```rust
let id1 = engine.add_document("first")?;
let id2 = engine.add_document("second")?;
assert!(id2 > id1); // time-ordered
```

---

## Error handling

All methods return `bdslib::common::error::Result<T>`. Use `?` to propagate or match on the error for diagnostics:

```rust
match engine.search("\"unclosed phrase", 10) {
    Ok(ids) => { /* ... */ }
    Err(e) => eprintln!("search error: {e}"),
}
```

See [`common::error`](COMMON.md) for the shared error type.
