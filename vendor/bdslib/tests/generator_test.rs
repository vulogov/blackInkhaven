use bdslib::common::generator::Generator;
use bdslib::common::logparser::validate_telemetry;
use bdslib::common::time::now_secs;

// ── helpers ───────────────────────────────────────────────────────────────────

fn assert_valid(doc: &serde_json::Value) {
    validate_telemetry(doc).unwrap_or_else(|e| panic!("invalid telemetry: {e}\ndoc: {doc}"));
}

// ── Generator::telemetry ──────────────────────────────────────────────────────

#[test]
fn test_telemetry_count() {
    assert_eq!(Generator::new().telemetry("1h", 50).len(), 50);
}

#[test]
fn test_telemetry_zero_returns_empty() {
    assert!(Generator::new().telemetry("1h", 0).is_empty());
}

#[test]
fn test_telemetry_all_pass_validate() {
    for doc in Generator::new().telemetry("1h", 100) {
        assert_valid(&doc);
    }
}

#[test]
fn test_telemetry_timestamp_within_duration_window() {
    let before = now_secs().saturating_sub(3600);
    let docs = Generator::new().telemetry("1h", 50);
    let after = now_secs() + 1;
    for doc in &docs {
        let ts = doc["timestamp"].as_u64().unwrap();
        assert!(ts >= before && ts <= after, "ts {ts} outside [{before}, {after}]");
    }
}

#[test]
fn test_telemetry_30m_window() {
    let before = now_secs().saturating_sub(1800);
    let docs = Generator::new().telemetry("30m", 30);
    let after = now_secs() + 1;
    for doc in &docs {
        let ts = doc["timestamp"].as_u64().unwrap();
        assert!(ts >= before && ts <= after);
    }
}

#[test]
fn test_telemetry_key_is_dotted_metric() {
    for doc in Generator::new().telemetry("1h", 50) {
        let key = doc["key"].as_str().unwrap();
        assert!(key.contains('.'), "expected dotted metric key, got {key:?}");
    }
}

#[test]
fn test_telemetry_data_has_required_fields() {
    for doc in Generator::new().telemetry("1h", 20) {
        let data = &doc["data"];
        assert!(data["value"].is_number(),  "missing 'value' in {data}");
        assert!(data["unit"].is_string(),   "missing 'unit' in {data}");
        assert!(data["host"].is_string(),   "missing 'host' in {data}");
        assert!(data["region"].is_string(), "missing 'region' in {data}");
        assert!(data["env"].is_string(),    "missing 'env' in {data}");
    }
}

#[test]
fn test_telemetry_value_non_negative() {
    for doc in Generator::new().telemetry("1h", 100) {
        let v = doc["data"]["value"].as_f64().unwrap();
        assert!(v >= 0.0, "negative value {v}");
    }
}

#[test]
fn test_telemetry_key_variety() {
    let docs = Generator::new().telemetry("1h", 200);
    let keys: std::collections::HashSet<_> =
        docs.iter().map(|d| d["key"].as_str().unwrap()).collect();
    assert!(keys.len() > 5, "expected multiple metric types, got {}", keys.len());
}

#[test]
fn test_telemetry_with_time_range_override() {
    let start = 1_700_000_000u64;
    let end   = 1_700_003_600u64;
    let g = Generator::with_time_range(start, end);
    // duration arg is ignored when with_time_range is set
    for doc in g.telemetry("1h", 30) {
        let ts = doc["timestamp"].as_u64().unwrap();
        assert!(ts >= start && ts <= end, "ts {ts} outside explicit range");
    }
}

// ── Generator::log_entries ────────────────────────────────────────────────────

#[test]
fn test_log_entries_count() {
    assert_eq!(Generator::new().log_entries("1h", 75).len(), 75);
}

#[test]
fn test_log_entries_zero_returns_empty() {
    assert!(Generator::new().log_entries("1h", 0).is_empty());
}

#[test]
fn test_log_entries_all_pass_validate() {
    for doc in Generator::new().log_entries("1h", 100) {
        assert_valid(&doc);
    }
}

#[test]
fn test_log_entries_timestamp_within_duration_window() {
    let before = now_secs().saturating_sub(1800);
    let docs = Generator::new().log_entries("30m", 50);
    let after = now_secs() + 1;
    for doc in &docs {
        let ts = doc["timestamp"].as_u64().unwrap();
        assert!(ts >= before && ts <= after, "ts {ts} outside [before, after]");
    }
}

#[test]
fn test_log_entries_multiple_formats_present() {
    let docs = Generator::new().log_entries("1h", 200);
    assert!(docs.iter().any(|d| d["data"].get("message").is_some()),    "no syslog");
    assert!(docs.iter().any(|d| d["data"].get("method").is_some()),     "no HTTP");
    assert!(docs.iter().any(|d| d["data"].get("exception_type").is_some()), "no tracebacks");
}

#[test]
fn test_log_entries_syslog_structure() {
    let docs = Generator::new().log_entries("1h", 200);
    let syslog: Vec<_> = docs.iter().filter(|d| d["data"].get("pid").is_some()).collect();
    assert!(!syslog.is_empty());
    for doc in syslog {
        assert!(doc["data"]["message"].is_string());
        assert!(doc["data"]["host"].is_string());
        assert!(doc["data"]["raw"].is_string());
    }
}

#[test]
fn test_log_entries_http_structure() {
    let docs = Generator::new().log_entries("1h", 200);
    let http: Vec<_> = docs.iter().filter(|d| d["data"].get("method").is_some()).collect();
    assert!(!http.is_empty());
    for doc in http {
        let data = &doc["data"];
        assert!(data["client"].is_string());
        assert!(data["path"].is_string());
        assert!(data["status"].is_string());
        assert!(data["bytes"].is_string());
        let key = doc["key"].as_str().unwrap();
        assert!(key.contains(' '), "HTTP key {key:?} should be 'METHOD /path'");
    }
}

#[test]
fn test_log_entries_nginx_has_server_field() {
    let docs = Generator::new().log_entries("1h", 300);
    let nginx: Vec<_> = docs.iter()
        .filter(|d| d["data"].get("server").and_then(|s| s.as_str()) == Some("nginx"))
        .collect();
    assert!(!nginx.is_empty(), "no nginx entries in 300-document sample");
}

#[test]
fn test_log_entries_traceback_structure() {
    let docs = Generator::new().log_entries("1h", 200);
    let tb: Vec<_> = docs.iter().filter(|d| d["data"].get("exception_type").is_some()).collect();
    assert!(!tb.is_empty());
    for doc in tb {
        let data = &doc["data"];
        assert!(data["exception_type"].is_string());
        assert!(data["exception_message"].is_string());
        assert!(data["frames"].is_array());
        assert!(!data["frames"].as_array().unwrap().is_empty());
        assert!(data["raw"].as_str().unwrap().starts_with("Traceback (most recent call last):"));
    }
}

#[test]
fn test_log_entries_traceback_frames_have_required_fields() {
    let docs = Generator::new().log_entries("1h", 200);
    for doc in docs.iter().filter(|d| d["data"].get("frames").is_some()) {
        for frame in doc["data"]["frames"].as_array().unwrap() {
            assert!(frame["file"].is_string());
            assert!(frame["line"].is_number());
            assert!(frame["function"].is_string());
        }
    }
}

// ── Generator::mixed ──────────────────────────────────────────────────────────

#[test]
fn test_mixed_count() {
    assert_eq!(Generator::new().mixed("1h", 100, 0.5).len(), 100);
}

#[test]
fn test_mixed_zero_returns_empty() {
    assert!(Generator::new().mixed("1h", 0, 0.5).is_empty());
}

#[test]
fn test_mixed_all_pass_validate() {
    for doc in Generator::new().mixed("1h", 100, 0.5) {
        assert_valid(&doc);
    }
}

#[test]
fn test_mixed_timestamp_within_window() {
    let before = now_secs().saturating_sub(7200);
    let docs = Generator::new().mixed("2h", 50, 0.5);
    let after = now_secs() + 1;
    for doc in &docs {
        let ts = doc["timestamp"].as_u64().unwrap();
        assert!(ts >= before && ts <= after);
    }
}

#[test]
fn test_mixed_ratio_1_is_pure_telemetry() {
    let docs = Generator::new().mixed("1h", 50, 1.0);
    for doc in &docs {
        assert!(doc["key"].as_str().unwrap().contains('.'));
        assert!(doc["data"]["value"].is_number());
    }
}

#[test]
fn test_mixed_ratio_0_is_pure_logs() {
    let docs = Generator::new().mixed("1h", 50, 0.0);
    for doc in &docs {
        let is_telemetry = doc["data"]["value"].is_number()
            && doc["key"].as_str().unwrap().contains('.');
        assert!(!is_telemetry, "ratio=0.0 produced a telemetry doc");
    }
}

#[test]
fn test_mixed_ratio_clamped_above_1() {
    let docs = Generator::new().mixed("1h", 30, 2.5);
    for doc in &docs {
        assert!(doc["data"]["value"].is_number());
    }
}

#[test]
fn test_mixed_ratio_clamped_below_0() {
    let docs = Generator::new().mixed("1h", 30, -1.0);
    for doc in &docs {
        let is_telemetry = doc["data"]["value"].is_number()
            && doc["key"].as_str().unwrap().contains('.');
        assert!(!is_telemetry);
    }
}

#[test]
fn test_mixed_balanced_produces_both_types() {
    let docs = Generator::new().mixed("1h", 200, 0.5);
    let telemetry_count = docs.iter()
        .filter(|d| d["data"]["value"].is_number() && d["key"].as_str().unwrap().contains('.'))
        .count();
    let log_count = docs.len() - telemetry_count;
    assert!(telemetry_count >= 30, "only {telemetry_count} telemetry docs in balanced mix");
    assert!(log_count >= 30,       "only {log_count} log docs in balanced mix");
}

// ── Generator::templated ──────────────────────────────────────────────────────

const BASIC_TEMPLATE: &str = r#"{
    "timestamp": "$timestamp",
    "key": "sensor",
    "data": { "value": "$float(0.0,100.0)", "host": "$word" }
}"#;

#[test]
fn test_templated_count() {
    assert_eq!(Generator::new().templated("1h", BASIC_TEMPLATE, 10).len(), 10);
}

#[test]
fn test_templated_zero_returns_empty() {
    assert!(Generator::new().templated("1h", BASIC_TEMPLATE, 0).is_empty());
}

#[test]
fn test_templated_timestamp_is_number_within_window() {
    let before = now_secs().saturating_sub(3600);
    let docs = Generator::new().templated("1h", BASIC_TEMPLATE, 20);
    let after = now_secs() + 1;
    for doc in &docs {
        let ts = doc["timestamp"].as_u64()
            .unwrap_or_else(|| panic!("timestamp not a u64: {:?}", doc["timestamp"]));
        assert!(ts >= before && ts <= after, "ts {ts} outside window");
    }
}

#[test]
fn test_templated_passes_validate_with_required_fields() {
    let template = r#"{"timestamp":"$timestamp","key":"metric","data":{"v":"$float(0.0,100.0)"}}"#;
    for doc in Generator::new().templated("1h", template, 20) {
        assert_valid(&doc);
    }
}

#[test]
fn test_templated_int_in_range() {
    let template = r#"{"timestamp":"$timestamp","key":"k","data":{"n":"$int(1,10)"}}"#;
    for doc in Generator::new().templated("1h", template, 50) {
        let v = doc["data"]["n"].as_i64().unwrap();
        assert!(v >= 1 && v <= 10, "int {v} outside [1, 10]");
    }
}

#[test]
fn test_templated_float_in_range() {
    let template = r#"{"timestamp":"$timestamp","key":"k","data":{"r":"$float(0.0,1.0)"}}"#;
    for doc in Generator::new().templated("1h", template, 50) {
        let v = doc["data"]["r"].as_f64().unwrap();
        assert!(v >= 0.0 && v <= 1.0, "float {v} outside [0, 1]");
    }
}

#[test]
fn test_templated_choice_restricted_to_options() {
    let template = r#"{"timestamp":"$timestamp","key":"$choice(cpu,mem,disk)","data":{"v":1}}"#;
    let valid = ["cpu", "mem", "disk"];
    for doc in Generator::new().templated("1h", template, 50) {
        let key = doc["key"].as_str().unwrap();
        assert!(valid.contains(&key), "unexpected key: {key}");
    }
}

#[test]
fn test_templated_bool_produces_booleans() {
    let template = r#"{"timestamp":"$timestamp","key":"k","data":{"flag":"$bool"}}"#;
    let docs = Generator::new().templated("1h", template, 50);
    let all_bool = docs.iter().all(|d| d["data"]["flag"].is_boolean());
    assert!(all_bool, "not all flags are booleans");
    // With 50 docs both true and false should appear with overwhelming probability
    let trues  = docs.iter().filter(|d| d["data"]["flag"] == true).count();
    let falses = docs.iter().filter(|d| d["data"]["flag"] == false).count();
    assert!(trues  > 0, "never got true");
    assert!(falses > 0, "never got false");
}

#[test]
fn test_templated_uuid_format() {
    let template = r#"{"timestamp":"$timestamp","key":"k","data":{"id":"$uuid"}}"#;
    for doc in Generator::new().templated("1h", template, 10) {
        let id = doc["data"]["id"].as_str().unwrap();
        assert_eq!(id.len(), 36, "UUID wrong length: {id}");
        assert_eq!(id.chars().filter(|&c| c == '-').count(), 4, "UUID wrong dash count: {id}");
    }
}

#[test]
fn test_templated_ip_is_dotted_quad() {
    let template = r#"{"timestamp":"$timestamp","key":"k","data":{"client":"$ip"}}"#;
    for doc in Generator::new().templated("1h", template, 10) {
        let ip = doc["data"]["client"].as_str().unwrap();
        assert_eq!(ip.split('.').count(), 4, "bad IPv4: {ip}");
        for octet in ip.split('.') {
            octet.parse::<u8>().unwrap_or_else(|_| panic!("non-numeric octet in {ip}"));
        }
    }
}

#[test]
fn test_templated_word_is_non_empty_string() {
    let template = r#"{"timestamp":"$timestamp","key":"$word","data":{"v":1}}"#;
    for doc in Generator::new().templated("1h", template, 20) {
        let key = doc["key"].as_str().unwrap();
        assert!(!key.is_empty());
        assert!(!key.contains('$'));
    }
}

#[test]
fn test_templated_name_is_two_words() {
    let template = r#"{"timestamp":"$timestamp","key":"k","data":{"user":"$name"}}"#;
    for doc in Generator::new().templated("1h", template, 10) {
        let name = doc["data"]["user"].as_str().unwrap();
        assert_eq!(name.split_whitespace().count(), 2, "name should be 'First Last': {name:?}");
    }
}

#[test]
fn test_templated_nested_object_processed() {
    let template = r#"{
        "timestamp": "$timestamp",
        "key": "sensor",
        "data": {
            "temp": "$float(15.0,35.0)",
            "location": {
                "city": "$choice(NYC,LA,CHI)",
                "lat":  "$float(-90.0,90.0)"
            }
        }
    }"#;
    for doc in Generator::new().templated("1h", template, 10) {
        assert!(doc["data"]["temp"].as_f64().is_some());
        let city = doc["data"]["location"]["city"].as_str().unwrap();
        assert!(["NYC", "LA", "CHI"].contains(&city), "unexpected city: {city}");
        let lat = doc["data"]["location"]["lat"].as_f64().unwrap();
        assert!(lat >= -90.0 && lat <= 90.0, "lat {lat} out of range");
    }
}

#[test]
fn test_templated_array_elements_processed() {
    let template = r#"{
        "timestamp": "$timestamp",
        "key": "batch",
        "data": {
            "readings": ["$float(0.0,100.0)", "$float(0.0,100.0)", "$float(0.0,100.0)"]
        }
    }"#;
    for doc in Generator::new().templated("1h", template, 5) {
        let arr = doc["data"]["readings"].as_array().unwrap();
        assert_eq!(arr.len(), 3);
        for r in arr {
            let v = r.as_f64().unwrap();
            assert!(v >= 0.0 && v <= 100.0);
        }
    }
}

#[test]
fn test_templated_static_values_pass_through() {
    let template = r#"{"timestamp":"$timestamp","key":"static_key","data":{"n":42,"s":"fixed"}}"#;
    for doc in Generator::new().templated("1h", template, 5) {
        assert_eq!(doc["key"].as_str().unwrap(), "static_key");
        assert_eq!(doc["data"]["n"].as_i64().unwrap(), 42);
        assert_eq!(doc["data"]["s"].as_str().unwrap(), "fixed");
    }
}

#[test]
fn test_templated_unknown_placeholder_passes_through() {
    let template = r#"{"timestamp":"$timestamp","key":"k","data":{"x":"$unknown_thing"}}"#;
    for doc in Generator::new().templated("1h", template, 5) {
        // Unknown placeholders are returned unchanged
        assert_eq!(doc["data"]["x"].as_str().unwrap(), "$unknown_thing");
    }
}

#[test]
fn test_templated_with_time_range_override() {
    let start = 1_700_000_000u64;
    let end   = 1_700_003_600u64;
    let g = Generator::with_time_range(start, end);
    let template = r#"{"timestamp":"$timestamp","key":"k","data":{"v":1}}"#;
    for doc in g.templated("1h", template, 20) {
        let ts = doc["timestamp"].as_u64().unwrap();
        assert!(ts >= start && ts <= end, "ts {ts} outside explicit range");
    }
}

#[test]
fn test_templated_duration_2h_uses_wider_window() {
    let before = now_secs().saturating_sub(7200);
    let docs = Generator::new().templated("2h", BASIC_TEMPLATE, 30);
    let after = now_secs() + 1;
    for doc in &docs {
        let ts = doc["timestamp"].as_u64().unwrap();
        assert!(ts >= before && ts <= after);
    }
}

// ── Generator configuration ───────────────────────────────────────────────────

#[test]
fn test_default_and_new_both_produce_docs() {
    assert_eq!(Generator::new().telemetry("1h", 5).len(), 5);
    assert_eq!(Generator::default().telemetry("1h", 5).len(), 5);
}

#[test]
fn test_with_time_range_pins_timestamps() {
    let start = 1_700_000_000u64;
    let end   = 1_700_003_600u64;
    let g = Generator::with_time_range(start, end);
    for doc in g.log_entries("1h", 20) {
        let ts = doc["timestamp"].as_u64().unwrap();
        assert!(ts >= start && ts <= end);
    }
}

#[test]
fn test_degenerate_range_all_same_timestamp() {
    let ts = 1_700_000_000u64;
    let g = Generator::with_time_range(ts, ts);
    for doc in g.telemetry("1h", 20) {
        assert_eq!(doc["timestamp"].as_u64().unwrap(), ts);
    }
}

#[test]
fn test_generator_is_clone() {
    let a = Generator::new();
    let b = a.clone();
    assert_eq!(a.telemetry("1h", 5).len(), b.telemetry("1h", 5).len());
}

#[test]
fn test_invalid_duration_falls_back_to_1h() {
    // "garbage" is not a valid humantime duration; should fall back to 1h silently
    let before = now_secs().saturating_sub(3_600);
    let docs = Generator::new().telemetry("garbage", 10);
    let after = now_secs() + 1;
    assert_eq!(docs.len(), 10);
    for doc in &docs {
        let ts = doc["timestamp"].as_u64().unwrap();
        assert!(ts >= before && ts <= after);
    }
}
