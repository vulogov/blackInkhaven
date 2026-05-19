# v2/eval.queued

Submit a BUND script to the process-wide `BundWorkerPool` for asynchronous
execution and return the result-queue id immediately.

The worker pool creates an ephemeral BUND VM for every job, executes the
script, and pushes every value left on the VM's workbench into the global
[`v2/results.*`](v2_results_push.md) queue under the returned `id`.  The
caller can retrieve results at any point afterwards using
[`v2/results.pull`](v2_results_pull.md).

The number of workers is controlled by the `n_workers` key in `bds.hjson`
(default: 4).

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | no | `""` | UUIDv7 session identifier. Accepted for symmetry; not consulted internally. |
| `script` | string | yes | — | BUND source code to compile and execute asynchronously. |

## Response

```json
{
  "id": "0192a3b4-c5d6-7e8f-9012-34567890abcd"
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | UUIDv7 of the result queue where workbench values will appear once execution finishes. |

## Example

```bash
# Submit a script and capture the queue id
ID=$(curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/eval.queued",
    "params": { "script": "6 7 * ." },
    "id": 1
  }' | jq -r '.result.id')

echo "Queued as $ID"

# Poll for the result
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d "{
    \"jsonrpc\": \"2.0\",
    \"method\": \"v2/results.pull\",
    \"params\": { \"id\": \"$ID\" },
    \"id\": 2
  }" | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": { "id": "...", "value": 42, "empty": true },
  "id": 2
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32002` | Worker pool not initialised (BundWorkerPool::start was not called) or channel send failed |

## Notes

- **Fire-and-forget.** The call returns as soon as the job is enqueued; it does
  not wait for the script to finish executing.
- **Fresh VM per job.** Each job runs in an independent BUND VM instance.
  State (heap, defined words) does not carry over between jobs.
- **All workbench items captured.** Every value pushed to the workbench with
  `.` is stored in the result queue.  The caller pops them one at a time via
  `v2/results.pull`.
- **Result queue TTL.** Result queues are subject to the same TTL sweeper as
  all other `v2/results.*` queues (controlled by `results_ttl_secs` and
  `results_sweep_secs` in `bds.hjson`, defaults 600 s and 30 s).
- **cf. `v2/eval`.** Use `v2/eval` for synchronous evaluation in a persistent
  named context; use `v2/eval.queued` for fire-and-forget execution in an
  ephemeral VM, typically when the script's runtime is long or the result is
  consumed by a different client.
