# generator_demo.rs

**File:** `examples/generator_demo.rs`

Demonstrates the `Generator`: producing synthetic telemetry, log, mixed, and template-driven JSON documents for testing and development.

## What it demonstrates

| Method | Description |
|---|---|
| `Generator::telemetry(n, duration)` | Generate `n` numeric telemetry records spanning `duration` |
| `Generator::log_entries(n, duration)` | Generate `n` structured log records (syslog, HTTP, traceback) |
| `Generator::mixed(n, duration, ratio)` | Generate a blend of telemetry and log records |
| `Generator::templated(n, duration, template)` | Generate records from a JSON template with `$placeholder` fields |

## Sections in the demo

1. **Telemetry** — 20 telemetry records over a 1-hour window; print key, value, unit
2. **Log entries** — 10 log records; show format variety (syslog/HTTP/traceback)
3. **Mixed (50/50)** — 30 records with ratio 0.5; count telemetry vs. log
4. **Mixed (80% telemetry)** — ratio 0.8 produces roughly 24 telemetry and 6 log
5. **Templated (IoT)** — custom template with `$float`, `$choice`, `$ip`, `$uuid` placeholders
6. **Templated (HTTP)** — HTTP access log template with `$choice` for method/status
7. **Templated (application)** — application event template with `$word`, `$int`, `$bool`
8. **`$placeholder` reference** — `"$float(0.0,1.0)"` in nested objects

## Placeholder syntax

| Placeholder | Description |
|---|---|
| `$int(min,max)` | Random integer in `[min, max]` |
| `$float(min,max)` | Random float in `[min, max]` |
| `$choice(a,b,c)` | Random selection from the listed options |
| `$bool` | Random `true` or `false` |
| `$uuid` | Random UUID v4 string |
| `$ip` | Random IPv4 address string |
| `$word` | Random word from a built-in vocabulary |
| `$name` | Random "First Last" name |

## Example output

```
telemetry[0]: key=cpu.usage value=73.4 unit=percent
log[0]:       key=syslog message="kernel: ..." host=web-01
mixed: 15 telemetry, 15 logs
iot[0]: {"sensor":"S-4a2","temp":21.3,"status":"ok","ip":"10.0.3.7"}
```
