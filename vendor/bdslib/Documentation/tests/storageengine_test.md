# storageengine_test.rs

**File:** `tests/storageengine_test.rs`  
**Module:** `bdslib::StorageEngine` — DuckDB-backed SQL engine with R2D2 connection pool

Tests the core SQL engine: lifecycle, type coverage, sync, and concurrent access.

## Lifecycle

| Test | Description |
|---|---|
| `test_full_lifecycle` | Complete create / insert / select workflow |
| `test_select_all_multi_column` | Multi-column select and per-column type casting |
| `test_select_all_empty` | Empty result set is handled (returns `[]`) |
| `test_select_foreach_empty` | Callback is not called when there are no rows |
| `test_init_invalid_sql` | Invalid `init_sql` returns `Err` at construction time |
| `test_select_foreach_callback_error_stops_iteration` | Callback `Err` stops iteration after the first call |

## Type coverage

| Test | Description |
|---|---|
| `test_type_boolean` | `BOOLEAN` values cast to `Value::Bool` |
| `test_type_integer` | `INTEGER` values cast to `Value::Int` |
| `test_type_bigint` | `BIGINT` values cast to `Value::BigInt` |
| `test_type_float` | `FLOAT` values cast to `Value::Float` (±1e-6 tolerance) |
| `test_type_double` | `DOUBLE` values cast to `Value::Double` (±1e-9 tolerance) |
| `test_type_text` | `TEXT` / `VARCHAR` values cast to `Value::Text` |
| `test_type_blob` | `BLOB` values cast to `Value::Blob` |
| `test_type_null_maps_to_nodata` | `NULL` values map to `Value::NoData` |

## Sync

| Test | Description |
|---|---|
| `test_sync_does_not_error` | `sync()` after inserts succeeds without error |

## Concurrency

| Test | Description |
|---|---|
| `test_concurrent_access` | 100 parallel threads (20% writes, 80% reads) all succeed |

## Type bridge reference

| DuckDB type | `rust_dynamic::value::Value` variant | Cast method |
|---|---|---|
| `BOOLEAN` | `Bool` | `cast_bool()` |
| `INTEGER` | `Int` | `cast_int()` |
| `BIGINT` | `BigInt` | `cast_bigint()` |
| `FLOAT` | `Float` | `cast_float()` |
| `DOUBLE` | `Double` | `cast_double()` |
| `TEXT` / `VARCHAR` | `Text` | `cast_string()` |
| `BLOB` | `Blob` | `cast_binary()` |
| `NULL` | `NoData` | — |

## Notes

- Each test creates its own `:memory:` DuckDB instance — tests are fully isolated and can run in parallel
- The `test_concurrent_access` test uses Rayon for data parallelism and validates that the 16-connection R2D2 pool handles 100 simultaneous threads without deadlock or data corruption
