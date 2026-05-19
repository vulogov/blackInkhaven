# generator_test.rs

**File:** `tests/generator_test.rs`  
**Module:** `bdslib::Generator` — synthetic document generation

Tests all four generation modes: `telemetry`, `log_entries`, `mixed`, and `templated`.

## `Generator::telemetry`

| Test | Description |
|---|---|
| `test_telemetry_count` | Generates the requested number of documents |
| `test_telemetry_zero_returns_empty` | Count 0 returns an empty vector |
| `test_telemetry_all_pass_validate` | All generated documents pass `validate_telemetry` |
| `test_telemetry_timestamp_within_duration_window` | Timestamps fall within the duration window |
| `test_telemetry_30m_window` | 30-minute duration window is respected |
| `test_telemetry_key_is_dotted_metric` | Keys have the form `"category.metric"` |
| `test_telemetry_data_has_required_fields` | Each doc contains `value`, `unit`, `host`, `region`, `env` |
| `test_telemetry_value_non_negative` | All generated values are ≥ 0 |
| `test_telemetry_key_variety` | At least 5 distinct metric keys across 100 documents |
| `test_telemetry_with_time_range_override` | Custom `with_time_range` is respected |

## `Generator::log_entries`

| Test | Description |
|---|---|
| `test_log_entries_count` | Generates the requested count |
| `test_log_entries_zero_returns_empty` | Count 0 returns empty |
| `test_log_entries_all_pass_validate` | All logs pass `validate_telemetry` |
| `test_log_entries_timestamp_within_duration_window` | Timestamps are in range |
| `test_log_entries_multiple_formats_present` | Generates syslog, HTTP, and traceback formats |
| `test_log_entries_syslog_structure` | Syslog entries have `message`, `host`, `pid`, `raw` |
| `test_log_entries_http_structure` | HTTP entries have `method`, `path`, `status`, `bytes`, `client` |
| `test_log_entries_nginx_has_server_field` | Nginx entries include `server="nginx"` |
| `test_log_entries_traceback_structure` | Tracebacks have `exception_type`, `exception_message`, `frames` |
| `test_log_entries_traceback_frames_have_required_fields` | Frame objects have `file`, `line`, `function` |

## `Generator::mixed`

| Test | Description |
|---|---|
| `test_mixed_count` | Total count matches request |
| `test_mixed_zero_returns_empty` | Count 0 returns empty |
| `test_mixed_all_pass_validate` | All docs pass validation |
| `test_mixed_timestamp_within_window` | Timestamps are in range |
| `test_mixed_ratio_1_is_pure_telemetry` | Ratio 1.0 → only telemetry |
| `test_mixed_ratio_0_is_pure_logs` | Ratio 0.0 → only logs |
| `test_mixed_ratio_clamped_above_1` | Ratio > 1.0 treated as 1.0 |
| `test_mixed_ratio_clamped_below_0` | Ratio < 0.0 treated as 0.0 |
| `test_mixed_balanced_produces_both_types` | Ratio 0.5 → substantial numbers of both types |

## `Generator::templated`

| Test | Description |
|---|---|
| `test_templated_count` | Generates the requested count |
| `test_templated_zero_returns_empty` | Count 0 returns empty |
| `test_templated_timestamp_is_number_within_window` | Timestamps fall in the window |
| `test_templated_passes_validate_with_required_fields` | Generated docs pass validation |
| `test_templated_int_in_range` | `$int(1,10)` produces integers in `[1, 10]` |
| `test_templated_float_in_range` | `$float(0.0,1.0)` produces floats in `[0, 1]` |
| `test_templated_choice_restricted_to_options` | `$choice(a,b,c)` picks from the list only |
| `test_templated_bool_produces_booleans` | `$bool` produces both `true` and `false` |
| `test_templated_uuid_format` | `$uuid` produces 36-character UUID with 4 dashes |
| `test_templated_ip_is_dotted_quad` | `$ip` produces valid IPv4 format |
| `test_templated_word_is_non_empty_string` | `$word` is non-empty and contains no `$` |
| `test_templated_name_is_two_words` | `$name` is "First Last" format |
| `test_templated_nested_object_processed` | Placeholders work inside nested objects |
| `test_templated_array_elements_processed` | Placeholders work on array elements |
| `test_templated_static_values_pass_through` | Non-templated values are unchanged |
| `test_templated_unknown_placeholder_passes_through` | Unknown `$unknown_thing` is left as-is |
| `test_templated_with_time_range_override` | Custom time range works |
| `test_templated_duration_2h_uses_wider_window` | 2-hour duration spans twice the window of 1-hour |

## Generator configuration

| Test | Description |
|---|---|
| `test_default_and_new_both_produce_docs` | Both `Generator::new()` and `::default()` work |
| `test_with_time_range_pins_timestamps` | `with_time_range(start, end)` constrains timestamps |
| `test_degenerate_range_all_same_timestamp` | `start == end` → all docs share that timestamp |
| `test_generator_is_clone` | Generators can be cloned |
| `test_invalid_duration_falls_back_to_1h` | Invalid duration strings default to 1 hour |
