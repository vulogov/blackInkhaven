# storage_engine_demo.rs

**File:** `examples/storage_engine_demo.rs`

Demonstrates `StorageEngine`: the low-level DuckDB-backed SQL engine with R2D2 connection pooling.

## What it demonstrates

`StorageEngine` is the foundational layer beneath all higher-level bdslib components. It exposes a simple SQL interface with a `rust_dynamic` type bridge for working with DuckDB query results in a type-safe, polymorphic way.

## Sections

| Section | Description |
|---|---|
| 1. Construction | `StorageEngine::new(":memory:", SCHEMA, 4)` — in-memory DB, 4-connection pool |
| 2. INSERT | `execute(sql)` to insert rows |
| 3. `select_all` | Collect all rows as `Vec<Vec<Value>>` |
| 4. `select_foreach` | Stream rows via callback — avoids allocating the full result set |
| 5. Aggregate | COUNT, AVG, MAX via `select_all` |
| 6. UPDATE | `execute(sql)` for DML |
| 7. `sync` | DuckDB CHECKPOINT — flush WAL to disk |

## Key API

| Method | Signature | Description |
|---|---|---|
| `new` | `(path, init_sql, pool_size) -> EngineResult<StorageEngine>` | Open/create the database |
| `execute` | `(sql) -> EngineResult<()>` | Run a DML statement |
| `select_all` | `(sql) -> EngineResult<Vec<Vec<Value>>>` | Collect all result rows |
| `select_foreach` | `(sql, callback) -> EngineResult<()>` | Stream rows via callback |
| `sync` | `() -> EngineResult<()>` | Checkpoint the WAL |

## Type bridge

DuckDB column types map to `rust_dynamic::value::Value`:

| DuckDB type | Value variant |
|---|---|
| BOOLEAN | `Value::Bool` |
| INTEGER | `Value::Int` |
| BIGINT | `Value::BigInt` |
| FLOAT | `Value::Float` |
| DOUBLE | `Value::Double` |
| TEXT / VARCHAR | `Value::Text` |
| BLOB | `Value::Blob` |
| NULL | `Value::NoData` |

## Example

```rust
let engine = StorageEngine::new(":memory:", "CREATE TABLE t (id INTEGER, name TEXT)", 4)?;
engine.execute("INSERT INTO t VALUES (1, 'hello')")?;
let rows = engine.select_all("SELECT id, name FROM t")?;
// rows[0][0].cast_int() == 1
// rows[0][1].cast_string() == "hello"
```
