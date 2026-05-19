# globals_test.rs

**File:** `tests/globals_test.rs`  
**Module:** `bdslib` — process-wide `ShardsManager` singleton

Tests `init_db`, `get_db`, and `sync_db` — the global database initialization lifecycle.

## Test function

### `test_globals_lifecycle`

A single comprehensive test covering the entire initialization state machine in sequence:

| Step | Operation | Expected result |
|---|---|---|
| 1 | `get_db()` before `init_db` | `Err("not initialized")` |
| 2 | `sync_db()` before `init_db` | `Ok(())` — no-op |
| 3 | `init_db(None)` without `BDS_CONFIG` env var | `Err` (missing config) |
| 4 | `init_db(None)` with `BDS_CONFIG` pointing to missing file | `Err` (file not found) |
| 5 | `init_db(Some(path))` with nonexistent path | `Err` (file not found) |
| 6 | `init_db(Some(path))` with malformed hjson | `Err` (parse error) |
| 7 | `init_db(Some(path))` with valid config | `Ok(())` |
| 8 | `get_db()` after init | `Ok(ShardsManager)` |
| 9 | `sync_db()` after init | `Ok(())` |
| 10 | `init_db()` second time | `Err("already initialized")` |
| 11 | `get_db()` after failed second init | Still `Ok` — original instance unchanged |
| 12 | `init_db(None)` with `BDS_CONFIG` set, after init | `Err("already initialized")` |

## Key properties verified

- **Pre-init safety** — `get_db` fails gracefully; `sync_db` is a no-op
- **Config resolution order** — explicit path > `BDS_CONFIG` env var
- **Error messages** — missing file, malformed hjson, and missing required fields all return distinct errors
- **Double-init guard** — the `OnceLock` prevents re-initialization from any code path
- **Stability after errors** — a failed second `init_db` does not corrupt the existing instance

## Notes

The test uses a single `#[test]` function rather than many small ones because `OnceLock` is per-process and cannot be reset between tests. Parallel test execution would cause races between initialization attempts.
