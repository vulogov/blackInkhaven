# StorageEngine API

`StorageEngine` is the primary public type in **bdslib**. It provides a thread-safe SQL interface over DuckDB, backed by a connection pool, with all query results returned as `rust_dynamic::value::Value`.

All methods return `bdslib::common::error::Result<T>` — an alias for `Result<T, easy_error::Error>` defined in the shared [`common::error`](COMMON.md) module.

## Construction

```rust
StorageEngine::new(path: P, init_sql: &'static str, pool_size: u32) -> Result<StorageEngine>
where P: AsRef<Path>
```

Opens or creates a DuckDB database at `path` and immediately executes `init_sql` to initialize the schema. Pass `":memory:"` as `path` for an in-memory database.

`init_sql` is run once via `execute_batch`, so it may contain multiple semicolon-separated statements (e.g., `CREATE TABLE` followed by seed `INSERT`s).

`pool_size` sets the maximum number of concurrent DuckDB connections in the pool. A value of `4` suits most single-process workloads; raise it toward the number of CPU cores for highly concurrent read workloads.

Returns `Err` if the connection pool cannot be built or if `init_sql` fails (syntax error, constraint violation, etc.).

```rust
// File-backed, 8-connection pool
let engine = StorageEngine::new("data/store.db", "CREATE TABLE events (id INTEGER, msg TEXT);", 8)?;

// In-memory, minimal pool
let engine = StorageEngine::new(":memory:", "CREATE TABLE kv (k TEXT, v TEXT);", 4)?;
```

`StorageEngine` is not `Clone`; wrap it in `Arc` to share it across threads.

---

## Methods

### `select_all`

```rust
fn select_all(&self, sql: &str) -> Result<Vec<Vec<Value>>>
```

Executes `sql` and collects every row into a `Vec`. Each row is a `Vec<Value>` whose elements correspond to the columns in projection order.

Returns `Err` on preparation failure, execution failure, or row conversion failure.

Use this when the result set fits comfortably in memory. For large or unbounded result sets, prefer `select_foreach`.

```rust
let rows = engine.select_all("SELECT id, name FROM users WHERE active = true")?;
for row in rows {
    let id   = row[0].cast_int()?;
    let name = row[1].cast_string()?;
}
```

---

### `select_foreach`

```rust
fn select_foreach<F>(&self, sql: &str, f: F) -> Result<()>
where F: FnMut(Vec<Value>) -> Result<()>
```

Executes `sql` and calls `f` once per row in streaming fashion, without accumulating the full result set. Iteration stops immediately if `f` returns `Err`, and that error is propagated as the return value.

```rust
engine.select_foreach("SELECT payload FROM events ORDER BY ts", |row| {
    process(row[0].cast_bin().unwrap());
    Ok(())
})?;
```

---

### `execute`

```rust
fn execute(&self, sql: &str) -> Result<()>
```

Executes a DML statement (`INSERT`, `UPDATE`, `DELETE`) or any other SQL that does not return rows. The number of affected rows is not returned.

```rust
engine.execute("INSERT INTO events (id, msg) VALUES (1, 'hello')")?;
engine.execute("DELETE FROM events WHERE id < 100")?;
```

---

### `sync`

```rust
fn sync(&self) -> Result<()>
```

Issues a DuckDB `CHECKPOINT`, flushing the write-ahead log to the main database file. Only meaningful for file-backed databases; safe to call on in-memory databases (no-op effect).

Call this after a batch of writes when durability matters and you cannot rely on DuckDB's background checkpointing.

```rust
engine.execute("INSERT INTO ...")?;
engine.sync()?;
```

---

## Type mapping

Query results are returned as `rust_dynamic::value::Value`. The mapping from DuckDB column types is:

| DuckDB type | `Value` type name | Accessor |
|---|---|---|
| `BOOLEAN` | `Bool` | `.cast_bool()` |
| `INTEGER` | `Integer` | `.cast_int()` |
| `BIGINT` | `Integer` | `.cast_int()` |
| `FLOAT` | `Float` | `.cast_float()` |
| `DOUBLE` | `Float` | `.cast_float()` |
| `TEXT` / `VARCHAR` | `String` | `.cast_string()` |
| `BLOB` | `Binary` | `.cast_bin()` |
| anything else / `NULL` | `NODATA` | `.type_name() == "NODATA"` |

Types not listed above (e.g., `TIMESTAMP`, `DATE`, `LIST`, `STRUCT`) fall through to `NODATA`. Check `.type_name()` to distinguish a typed value from a null or unrecognised one.

```rust
let rows = engine.select_all("SELECT flag, count, label, data, score FROM t")?;
let row = &rows[0];
let flag:  bool    = row[0].cast_bool()?;
let count: i64     = row[1].cast_int()?;
let label: String  = row[2].cast_string()?;
let data:  Vec<u8> = row[3].cast_bin()?;
let score: f64     = row[4].cast_float()?;
```

---

## Thread safety

`StorageEngine` is `Send + Sync` via the underlying `r2d2` pool. Wrap in `Arc` to share across threads:

```rust
let engine = Arc::new(StorageEngine::new(":memory:", INIT_SQL, 8)?);

let e = engine.clone();
std::thread::spawn(move || {
    e.execute("INSERT INTO t VALUES (1)").unwrap();
});
```

DuckDB serialises concurrent writes internally; concurrent reads scale across pool connections up to the configured `pool_size`.

---

## Error handling

All methods return `bdslib::common::error::Result<T>`. Errors carry a human-readable context message and the underlying cause:

```rust
match engine.select_all("SELECT * FROM nonexistent") {
    Ok(rows) => { /* ... */ }
    Err(e) => eprintln!("query failed: {e}"),
}
```

Use `?` to propagate errors in functions that return a compatible `Result`. See [`common::error`](COMMON.md) for the shared error type.
