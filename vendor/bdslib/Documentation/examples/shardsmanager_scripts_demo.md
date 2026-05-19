# shardsmanager_scripts_demo.rs

**File:** `examples/shardsmanager_scripts_demo.rs`

Demonstrates `ShardsManager::script_*` — the BUND script registry: create, list, fetch, update, delete, with metadata validation.

## What it demonstrates

| Function | Description |
|---|---|
| `ShardsManager::script_add(metadata, script)` | Store a new script; metadata must contain non-empty `name` and `schedule` |
| `ShardsManager::scripts()` | List `(uuid, schedule)` pairs for every stored script |
| `ShardsManager::scripts_with_metadata()` | Same listing but with the full metadata document for UI use |
| `ShardsManager::script(id)` | Fetch the BUND source body of a single script |
| `ShardsManager::script_metadata(id)` | Fetch only the metadata document |
| `ShardsManager::update_script(id, metadata, script)` | Replace metadata and body in place |
| `ShardsManager::script_delete(id)` | Remove a script from all sub-stores |

## Sections

| # | Topic | Behaviour shown |
|---|---|---|
| 1 | `script_add` | Adds three scripts: `hello` (with extra metadata), `daily_report`, `cleanup` |
| 2 | `scripts()` and `scripts_with_metadata()` | List ID/schedule pairs and full metadata |
| 3 | `script(id)` | Fetch a stored body; show `None` for an unknown UUID |
| 4 | `update_script` | Replace metadata and body; verify new schedule and an extra `version` field |
| 5 | `script_delete` | Remove `cleanup`; verify the listing shrinks |
| 6 | Validation errors | Reject missing `schedule`, missing `name`, and non-object metadata |

## Run

```bash
cargo run --example shardsmanager_scripts_demo
```

## Storage layout

Scripts live in their own `DocumentStorage` rooted at `{dbpath}/scripts`, alongside the existing `docstore` (`{dbpath}/docstore`) and `signals` (`{dbpath}/signals`) stores. The registry is independent of the time-partitioned shard cache — scripts are addressed by UUID, not by timestamp.
