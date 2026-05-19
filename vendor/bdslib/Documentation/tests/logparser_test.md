# logparser_test.rs

**File:** `tests/logparser_test.rs`  
**Module:** `bdslib::common::logparser` — log parsing and ingestion

Tests log line validation, grok parsing, syslog/CLF/Apache/nginx parsing, Python traceback parsing, and file ingestion.

## `validate_telemetry`

| Test | Description |
|---|---|
| `test_validate_ok` | Valid doc with `timestamp`, `key`, `data` passes |
| `test_validate_missing_timestamp` | Missing timestamp fails |
| `test_validate_string_timestamp_rejected` | Non-numeric timestamp fails |
| `test_validate_negative_timestamp_rejected` | Negative timestamp fails |
| `test_validate_missing_key` | Missing key fails |
| `test_validate_empty_key_rejected` | Empty key string fails |
| `test_validate_non_string_key_rejected` | Non-string key fails |
| `test_validate_missing_data` | Missing `data` field fails |
| `test_validate_null_data_rejected` | `null` data fails |
| `test_validate_data_object_ok` | Object `data` is valid |

## `GrokParser`

| Test | Description |
|---|---|
| `test_grokparser_new_valid` | Constructor works with valid pattern |
| `test_grokparser_new_unknown_pattern_fails` | Unknown pattern name fails |
| `test_grokparser_with_custom_patterns` | Custom patterns can be registered |
| `test_grokparser_no_match_returns_none` | Non-matching text returns `None` |

## `parse_log_line`

| Test | Description |
|---|---|
| `test_parse_log_line_with_named_key_and_timestamp` | Extracts `key` and `timestamp` from grok captures |
| `test_parse_log_line_falls_back_to_now_for_timestamp` | Missing timestamp capture uses `now()` |
| `test_parse_log_line_falls_back_to_log_key` | Missing key capture defaults to `"log"` |
| `test_parse_log_line_no_match_returns_err` | Non-matching input returns `Err` |
| `test_parse_log_line_uses_program_as_key_fallback` | `"program"` field can serve as key |

## `parse_syslog`

| Test | Description |
|---|---|
| `test_parse_syslog_basic` | Parses standard syslog line with `program[pid]` |
| `test_parse_syslog_single_digit_day` | Handles single-digit days with leading space |
| `test_parse_syslog_no_pid` | Works without `[pid]` |
| `test_parse_syslog_bad_line_returns_err` | Invalid format returns `Err` |
| `test_parse_syslog_result_passes_validate` | All parsed syslog passes `validate_telemetry` |

## `parse_ncsa_clf` (Common Log Format)

| Test | Description |
|---|---|
| `test_parse_ncsa_clf_basic` | Parses Apache/nginx CLF lines |
| `test_parse_ncsa_clf_post_request` | Different HTTP methods work |
| `test_parse_ncsa_clf_missing_bytes_dash` | `"-"` bytes field is filtered out |
| `test_parse_ncsa_clf_bad_line_returns_err` | Invalid format returns `Err` |
| `test_parse_ncsa_clf_result_passes_validate` | Parsed CLF passes validation |

## `parse_apache`

| Test | Description |
|---|---|
| `test_parse_apache_basic` | Parses Apache combined log (CLF + referrer + user-agent) |
| `test_parse_apache_no_referrer` | `"-"` referrer is filtered out |
| `test_parse_apache_result_passes_validate` | Parsed Apache log passes validation |

## `parse_nginx`

| Test | Description |
|---|---|
| `test_parse_nginx_basic` | Parses nginx combined log |
| `test_parse_nginx_with_referer_and_agent` | Extracts referrer and user-agent |
| `test_parse_nginx_result_passes_validate` | Parsed nginx log passes validation |

## Timestamp accuracy

| Test | Description |
|---|---|
| `test_apache_ts_accuracy` | `"10/Oct/2000:13:55:36 -0700"` converts to correct Unix timestamp |
| `test_syslog_ts_current_year` | Syslog timestamps (no year) are assumed to be in the current year |

## Custom grok

| Test | Description |
|---|---|
| `test_parse_log_line_custom_grok_produces_valid_telemetry` | Custom grok patterns produce valid documents end-to-end |

## `common::time::now_secs`

| Test | Description |
|---|---|
| `test_now_secs_is_recent` | Returns a timestamp within the current second |
| `test_now_secs_greater_than_2024` | Timestamp is past 2024 |

## `ingest_file`

| Test | Description |
|---|---|
| `test_ingest_file_syslog_all_lines_parsed` | All valid lines in a file are parsed |
| `test_ingest_file_skips_blank_lines` | Empty/whitespace-only lines are silently skipped |
| `test_ingest_file_skips_unparseable_lines` | Malformed lines are skipped without error |
| `test_ingest_file_missing_file_returns_err` | Nonexistent file returns `Err` |
| `test_ingest_file_apache_log` | Apache format log files are parsed |
| `test_ingest_file_empty_file_ok` | Empty files don't error |
| `test_ingest_file_with_custom_parser` | Custom parsers can be supplied |

## Python traceback parsing

| Test | Description |
|---|---|
| `test_is_python_traceback_start_exact_match` | Recognizes `"Traceback (most recent call last):"` |
| `test_is_python_traceback_start_with_surrounding_whitespace` | Handles leading/trailing whitespace |
| `test_is_python_traceback_start_non_matching` | Rejects non-traceback lines |
| `test_parse_python_traceback_basic` | Extracts exception type, message, frames |
| `test_parse_python_traceback_frames_parsed` | Frames have `file`, `line`, `function`, `source` |
| `test_parse_python_traceback_bare_exception_no_message` | Exceptions without `: message` have empty message |
| `test_parse_python_traceback_result_passes_validate` | Parsed traceback passes validation |
| `test_parse_python_traceback_invalid_header_returns_err` | Missing "Traceback" header returns `Err` |
| `test_parse_python_traceback_no_exception_line_returns_err` | Missing exception line returns `Err` |
| `test_parse_python_traceback_colon_in_message` | `": "` in message is parsed correctly (first `:` splits) |
| `test_parse_python_traceback_namespaced_exception` | `"pkg.module.Exception"` format works |
| `test_parse_python_traceback_chained_exception` | Multiple Traceback blocks use the last one |

## `ingest_multiline_file`

| Test | Description |
|---|---|
| `test_ingest_multiline_file_two_tracebacks` | Two tracebacks in one file are separated correctly |
| `test_ingest_multiline_file_skips_non_traceback_blocks` | Non-traceback content is dropped |
| `test_ingest_multiline_file_empty_file_ok` | Empty files don't error |
| `test_ingest_multiline_file_missing_file_returns_err` | Nonexistent file returns `Err` |
| `test_ingest_multiline_file_all_docs_pass_validate` | All parsed tracebacks pass validation |
