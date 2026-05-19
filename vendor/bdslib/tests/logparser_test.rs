use bdslib::common::logparser::{
    ingest_file, ingest_multiline_file, is_python_traceback_start, parse_apache, parse_log_line,
    parse_ncsa_clf, parse_nginx, parse_python_traceback, parse_syslog, validate_telemetry,
    GrokParser,
};
use bdslib::common::time::now_secs;
use serde_json::json;

// ── validate_telemetry ────────────────────────────────────────────────────────

#[test]
fn test_validate_ok() {
    let doc = json!({ "timestamp": 1_700_000_000u64, "key": "cpu", "data": 42 });
    assert!(validate_telemetry(&doc).is_ok());
}

#[test]
fn test_validate_missing_timestamp() {
    let doc = json!({ "key": "cpu", "data": 42 });
    let err = validate_telemetry(&doc).unwrap_err().to_string();
    assert!(err.contains("timestamp"));
}

#[test]
fn test_validate_string_timestamp_rejected() {
    let doc = json!({ "timestamp": "not-a-number", "key": "cpu", "data": 42 });
    assert!(validate_telemetry(&doc).is_err());
}

#[test]
fn test_validate_negative_timestamp_rejected() {
    let doc = json!({ "timestamp": -1i64, "key": "cpu", "data": 42 });
    assert!(validate_telemetry(&doc).is_err());
}

#[test]
fn test_validate_missing_key() {
    let doc = json!({ "timestamp": 1_700_000_000u64, "data": 42 });
    let err = validate_telemetry(&doc).unwrap_err().to_string();
    assert!(err.contains("key"));
}

#[test]
fn test_validate_empty_key_rejected() {
    let doc = json!({ "timestamp": 1_700_000_000u64, "key": "", "data": 42 });
    assert!(validate_telemetry(&doc).is_err());
}

#[test]
fn test_validate_non_string_key_rejected() {
    let doc = json!({ "timestamp": 1_700_000_000u64, "key": 42, "data": "x" });
    assert!(validate_telemetry(&doc).is_err());
}

#[test]
fn test_validate_missing_data() {
    let doc = json!({ "timestamp": 1_700_000_000u64, "key": "cpu" });
    let err = validate_telemetry(&doc).unwrap_err().to_string();
    assert!(err.contains("data"));
}

#[test]
fn test_validate_null_data_rejected() {
    let doc = json!({ "timestamp": 1_700_000_000u64, "key": "cpu", "data": null });
    assert!(validate_telemetry(&doc).is_err());
}

#[test]
fn test_validate_data_object_ok() {
    let doc = json!({ "timestamp": 1_700_000_000u64, "key": "cpu", "data": {"val": 42} });
    assert!(validate_telemetry(&doc).is_ok());
}

// ── GrokParser ────────────────────────────────────────────────────────────────

#[test]
fn test_grokparser_new_valid() {
    let parser = GrokParser::new("%{WORD:word} %{NUMBER:num}").unwrap();
    let m = parser.pattern().match_against("hello 42").unwrap();
    assert_eq!(m.get("word"), Some("hello"));
    assert_eq!(m.get("num"), Some("42"));
}

#[test]
fn test_grokparser_new_unknown_pattern_fails() {
    assert!(GrokParser::new("%{DOES_NOT_EXIST_PATTERN}").is_err());
}

#[test]
fn test_grokparser_with_custom_patterns() {
    let parser = GrokParser::with_patterns(
        &[("MYWORD", r"[a-z]+")],
        "%{MYWORD:w}",
    )
    .unwrap();
    let m = parser.pattern().match_against("hello").unwrap();
    assert_eq!(m.get("w"), Some("hello"));
}

#[test]
fn test_grokparser_no_match_returns_none() {
    let parser = GrokParser::new("%{NUMBER:n}").unwrap();
    assert!(parser.pattern().match_against("not-a-number").is_none());
}

// ── parse_log_line ────────────────────────────────────────────────────────────

#[test]
fn test_parse_log_line_with_named_key_and_timestamp() {
    let parser = GrokParser::new("%{NUMBER:timestamp} %{WORD:key} %{GREEDYDATA:message}").unwrap();
    let doc = parse_log_line(&parser, "1700000000 cpu high usage detected").unwrap();

    assert_eq!(doc["timestamp"].as_u64().unwrap(), 1_700_000_000u64);
    assert_eq!(doc["key"].as_str().unwrap(), "cpu");
    assert!(doc["data"]["message"].as_str().unwrap().contains("high usage"));
}

#[test]
fn test_parse_log_line_falls_back_to_now_for_timestamp() {
    let parser = GrokParser::new("%{WORD:key} %{GREEDYDATA:message}").unwrap();
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let doc = parse_log_line(&parser, "cpu high usage").unwrap();
    let ts = doc["timestamp"].as_u64().unwrap();
    assert!(ts >= before);
}

#[test]
fn test_parse_log_line_falls_back_to_log_key() {
    let parser = GrokParser::new("%{GREEDYDATA:message}").unwrap();
    let doc = parse_log_line(&parser, "some log line with no key field").unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "log");
}

#[test]
fn test_parse_log_line_no_match_returns_err() {
    let parser = GrokParser::new("%{NUMBER:n} %{NUMBER:m}").unwrap();
    assert!(parse_log_line(&parser, "not numbers").is_err());
}

#[test]
fn test_parse_log_line_uses_program_as_key_fallback() {
    let parser = GrokParser::with_patterns(
        &[("PROG", r"[a-z]+")],
        "%{PROG:program}: %{GREEDYDATA:message}",
    )
    .unwrap();
    let doc = parse_log_line(&parser, "sshd: user login accepted").unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "sshd");
}

// ── parse_syslog ──────────────────────────────────────────────────────────────

const SYSLOG_LINES: &[&str] = &[
    "Nov 16 23:01:55 myhost sshd[1234]: Accepted publickey for alice from 10.0.0.1",
    "Jan  5 08:12:34 web-01 nginx[5678]: server started",
    "Apr 22 14:00:00 db-03 postgres: connection from 192.168.0.5",
];

#[test]
fn test_parse_syslog_basic() {
    let doc = parse_syslog(SYSLOG_LINES[0]).unwrap();
    assert!(doc["timestamp"].as_u64().unwrap() > 0);
    assert_eq!(doc["key"].as_str().unwrap(), "sshd");
    assert!(doc["data"]["message"]
        .as_str()
        .unwrap()
        .contains("Accepted publickey"));
    assert_eq!(doc["data"]["host"].as_str().unwrap(), "myhost");
    assert_eq!(doc["data"]["pid"].as_str().unwrap(), "1234");
}

#[test]
fn test_parse_syslog_single_digit_day() {
    // "Jan  5" — double space before single-digit day
    let doc = parse_syslog(SYSLOG_LINES[1]).unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "nginx");
    assert!(doc["timestamp"].as_u64().unwrap() > 0);
}

#[test]
fn test_parse_syslog_no_pid() {
    // "postgres:" with no [pid] field
    let doc = parse_syslog(SYSLOG_LINES[2]).unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "postgres");
    assert!(doc["data"].get("pid").is_none());
}

#[test]
fn test_parse_syslog_bad_line_returns_err() {
    assert!(parse_syslog("this is not a syslog line at all").is_err());
}

#[test]
fn test_parse_syslog_result_passes_validate() {
    for line in SYSLOG_LINES {
        let doc = parse_syslog(line).unwrap();
        validate_telemetry(&doc).unwrap();
    }
}

// ── parse_ncsa_clf ────────────────────────────────────────────────────────────

const CLF_LINES: &[&str] = &[
    r#"127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326"#,
    r#"10.0.0.5 - - [22/Apr/2026:08:30:00 +0000] "POST /api/submit HTTP/1.1" 201 512"#,
    r#"192.168.1.100 - admin [01/Jan/2025:00:00:01 +0000] "DELETE /resource/42 HTTP/1.1" 204 -"#,
];

#[test]
fn test_parse_ncsa_clf_basic() {
    let doc = parse_ncsa_clf(CLF_LINES[0]).unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "GET /apache_pb.gif");
    assert_eq!(doc["data"]["client"].as_str().unwrap(), "127.0.0.1");
    assert_eq!(doc["data"]["status"].as_str().unwrap(), "200");
    assert_eq!(doc["data"]["method"].as_str().unwrap(), "GET");
    // timestamp: 10/Oct/2000:13:55:36 -0700
    let ts = doc["timestamp"].as_u64().unwrap();
    assert!(ts > 0);
}

#[test]
fn test_parse_ncsa_clf_post_request() {
    let doc = parse_ncsa_clf(CLF_LINES[1]).unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "POST /api/submit");
    assert_eq!(doc["data"]["status"].as_str().unwrap(), "201");
}

#[test]
fn test_parse_ncsa_clf_missing_bytes_dash() {
    // bytes field is "-" (no bytes reported)
    let doc = parse_ncsa_clf(CLF_LINES[2]).unwrap();
    assert!(doc["data"].get("bytes").is_none()); // "-" is filtered out
    assert_eq!(doc["data"]["method"].as_str().unwrap(), "DELETE");
}

#[test]
fn test_parse_ncsa_clf_bad_line_returns_err() {
    assert!(parse_ncsa_clf("not an access log line").is_err());
}

#[test]
fn test_parse_ncsa_clf_result_passes_validate() {
    for line in CLF_LINES {
        let doc = parse_ncsa_clf(line).unwrap();
        validate_telemetry(&doc).unwrap();
    }
}

// ── parse_apache ──────────────────────────────────────────────────────────────

const APACHE_LINES: &[&str] = &[
    r#"127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326 "http://www.example.com/start.html" "Mozilla/4.08 [en] (Win98; I ;Nav)""#,
    r#"10.0.0.2 - - [22/Apr/2026:09:00:00 +0000] "POST /api/data HTTP/1.1" 201 128 "-" "curl/7.81.0""#,
];

#[test]
fn test_parse_apache_basic() {
    let doc = parse_apache(APACHE_LINES[0]).unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "GET /apache_pb.gif");
    assert_eq!(doc["data"]["client"].as_str().unwrap(), "127.0.0.1");
    assert_eq!(doc["data"]["status"].as_str().unwrap(), "200");
    let ua = doc["data"]["user_agent"].as_str().unwrap();
    assert!(ua.contains("Mozilla"));
    let referrer = doc["data"]["referrer"].as_str().unwrap();
    assert!(referrer.contains("example.com"));
}

#[test]
fn test_parse_apache_no_referrer() {
    let doc = parse_apache(APACHE_LINES[1]).unwrap();
    // referrer is "-" → filtered out
    assert!(doc["data"].get("referrer").is_none());
    assert_eq!(doc["data"]["user_agent"].as_str().unwrap(), "curl/7.81.0");
}

#[test]
fn test_parse_apache_result_passes_validate() {
    for line in APACHE_LINES {
        let doc = parse_apache(line).unwrap();
        validate_telemetry(&doc).unwrap();
    }
}

// ── parse_nginx ───────────────────────────────────────────────────────────────

const NGINX_LINES: &[&str] = &[
    r#"192.168.1.1 - - [22/Apr/2026:13:55:36 +0000] "GET /health HTTP/1.1" 200 0 "-" "kube-probe/1.27""#,
    r#"10.10.0.5 - alice [01/Jan/2025:00:00:01 +0000] "PUT /api/v2/resource HTTP/2.0" 204 - "https://app.example.com" "HTTPie/3.2.1""#,
];

#[test]
fn test_parse_nginx_basic() {
    let doc = parse_nginx(NGINX_LINES[0]).unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "GET /health");
    assert_eq!(doc["data"]["client"].as_str().unwrap(), "192.168.1.1");
    assert_eq!(doc["data"]["status"].as_str().unwrap(), "200");
    assert_eq!(doc["data"]["server"].as_str().unwrap(), "nginx");
}

#[test]
fn test_parse_nginx_with_referer_and_agent() {
    let doc = parse_nginx(NGINX_LINES[1]).unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "PUT /api/v2/resource");
    let referrer = doc["data"]["referrer"].as_str().unwrap();
    assert!(referrer.contains("app.example.com"));
}

#[test]
fn test_parse_nginx_result_passes_validate() {
    for line in NGINX_LINES {
        let doc = parse_nginx(line).unwrap();
        validate_telemetry(&doc).unwrap();
    }
}

// ── timestamp accuracy ────────────────────────────────────────────────────────

#[test]
fn test_apache_ts_accuracy() {
    // 10/Oct/2000:13:55:36 -0700  →  Unix 971211336 + 7*3600 = 971236536 (UTC)
    // 13:55:36 local -0700 = 20:55:36 UTC = 2000-10-10 20:55:36 UTC
    let line = r#"127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET / HTTP/1.0" 200 -"#;
    let doc = parse_ncsa_clf(line).unwrap();
    let ts = doc["timestamp"].as_u64().unwrap();
    // 2000-10-10 20:55:36 UTC
    assert_eq!(ts, 971211336);
}

#[test]
fn test_syslog_ts_current_year() {
    let line = "Jan  1 00:00:00 host prog: message";
    let doc = parse_syslog(line).unwrap();
    let ts = doc["timestamp"].as_u64().unwrap();
    let year_2000_ts: u64 = 946684800; // 2000-01-01 00:00:00 UTC
    // Must be more recent than year 2000
    assert!(ts > year_2000_ts, "timestamp {ts} should be recent (current year)");
}

// ── ingest_stdin (functional, not I/O-connected) ──────────────────────────────

#[test]
fn test_parse_log_line_custom_grok_produces_valid_telemetry() {
    // Simulate what ingest_stdin does: parse lines and validate each result.
    let parser = GrokParser::new(
        "%{NUMBER:timestamp} %{WORD:key} %{GREEDYDATA:message}",
    )
    .unwrap();

    let lines = [
        "1700000001 cpu usage=78",
        "1700000030 mem pressure=high",
        "1700000060 disk io_wait=12ms",
    ];

    let mut results = Vec::new();
    for line in &lines {
        let doc = parse_log_line(&parser, line).unwrap();
        validate_telemetry(&doc).unwrap();
        results.push(doc);
    }

    assert_eq!(results.len(), 3);
    assert_eq!(results[0]["key"].as_str().unwrap(), "cpu");
    assert_eq!(results[1]["key"].as_str().unwrap(), "mem");
    assert_eq!(results[2]["key"].as_str().unwrap(), "disk");
}

// ── common::time::now_secs ────────────────────────────────────────────────────

#[test]
fn test_now_secs_is_recent() {
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let ts = now_secs();
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(ts >= before, "now_secs() {ts} should be >= {before}");
    assert!(ts <= after, "now_secs() {ts} should be <= {after}");
}

#[test]
fn test_now_secs_greater_than_2024() {
    // 2024-01-01 00:00:00 UTC
    assert!(now_secs() > 1_704_067_200, "now_secs() should reflect a date after 2024");
}

// ── ingest_file ───────────────────────────────────────────────────────────────

fn write_temp_log(lines: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("test.log");
    std::fs::write(&path, lines.join("\n")).unwrap();
    (dir, path)
}

#[test]
fn test_ingest_file_syslog_all_lines_parsed() {
    let lines = [
        "Nov 16 23:01:55 host sshd[1]: Accepted key for alice",
        "Nov 16 23:02:10 host cron[2]: job started",
        "Nov 16 23:03:00 host kernel: OOM killer invoked",
    ];
    let (_dir, path) = write_temp_log(&lines);

    let mut collected = Vec::new();
    ingest_file(
        parse_syslog,
        |doc| collected.push(doc),
        path.to_str().unwrap(),
    )
    .unwrap();

    assert_eq!(collected.len(), 3);
    assert_eq!(collected[0]["key"].as_str().unwrap(), "sshd");
    assert_eq!(collected[1]["key"].as_str().unwrap(), "cron");
    assert_eq!(collected[2]["key"].as_str().unwrap(), "kernel");
}

#[test]
fn test_ingest_file_skips_blank_lines() {
    let content = "Nov 16 23:01:55 host sshd[1]: msg\n\n   \nNov 16 23:02:00 host cron[2]: msg2\n";
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("test.log");
    std::fs::write(&path, content).unwrap();

    let mut count = 0usize;
    ingest_file(parse_syslog, |_| count += 1, path.to_str().unwrap()).unwrap();
    assert_eq!(count, 2);
}

#[test]
fn test_ingest_file_skips_unparseable_lines() {
    let lines = [
        "Nov 16 23:01:55 host sshd[1]: valid syslog",
        "this line is not a syslog line at all",
        "Nov 16 23:02:00 host cron[2]: another valid line",
    ];
    let (_dir, path) = write_temp_log(&lines);

    let mut count = 0usize;
    ingest_file(parse_syslog, |_| count += 1, path.to_str().unwrap()).unwrap();
    // Unparseable line silently skipped; 2 valid lines processed.
    assert_eq!(count, 2);
}

#[test]
fn test_ingest_file_missing_file_returns_err() {
    let result = ingest_file(
        parse_syslog,
        |_| {},
        "/nonexistent/path/to/file.log",
    );
    assert!(result.is_err());
    let msg = result.err().unwrap().to_string();
    assert!(msg.contains("cannot open"));
}

#[test]
fn test_ingest_file_apache_log() {
    let lines = [
        r#"127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.0" 200 1024 "-" "curl/7.81.0""#,
        r#"10.0.0.1 - - [10/Oct/2000:14:00:00 +0000] "POST /api/submit HTTP/1.1" 201 512 "-" "HTTPie/3.2""#,
    ];
    let (_dir, path) = write_temp_log(&lines);

    let mut results = Vec::new();
    ingest_file(parse_apache, |doc| results.push(doc), path.to_str().unwrap()).unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["key"].as_str().unwrap(), "GET /index.html");
    assert_eq!(results[1]["key"].as_str().unwrap(), "POST /api/submit");
    for doc in &results {
        validate_telemetry(doc).unwrap();
    }
}

#[test]
fn test_ingest_file_empty_file_ok() {
    let (_dir, path) = write_temp_log(&[]);
    let mut count = 0usize;
    ingest_file(parse_syslog, |_| count += 1, path.to_str().unwrap()).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_ingest_file_with_custom_parser() {
    let lines = [
        "1700000001 cpu usage=78",
        "1700000030 mem pressure=high",
    ];
    let (_dir, path) = write_temp_log(&lines);

    let parser = GrokParser::new("%{NUMBER:timestamp} %{WORD:key} %{GREEDYDATA:message}")
        .unwrap();
    let parser = std::sync::Arc::new(parser);

    let mut results = Vec::new();
    ingest_file(
        |line| parse_log_line(&parser, line),
        |doc| results.push(doc),
        path.to_str().unwrap(),
    )
    .unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["timestamp"].as_u64().unwrap(), 1_700_000_001);
    assert_eq!(results[1]["key"].as_str().unwrap(), "mem");
}

// ── is_python_traceback_start ─────────────────────────────────────────────────

#[test]
fn test_is_python_traceback_start_exact_match() {
    assert!(is_python_traceback_start("Traceback (most recent call last):"));
}

#[test]
fn test_is_python_traceback_start_with_surrounding_whitespace() {
    assert!(is_python_traceback_start("  Traceback (most recent call last):  "));
}

#[test]
fn test_is_python_traceback_start_non_matching() {
    assert!(!is_python_traceback_start("not a traceback"));
    assert!(!is_python_traceback_start("  File \"app.py\", line 1, in f"));
    assert!(!is_python_traceback_start("ZeroDivisionError: division by zero"));
    assert!(!is_python_traceback_start(""));
}

// ── parse_python_traceback ────────────────────────────────────────────────────

const SIMPLE_TB: &str = "Traceback (most recent call last):\n  \
    File \"app.py\", line 10, in <module>\n    \
    result = divide(10, 0)\n  \
    File \"lib.py\", line 5, in divide\n    \
    return a / b\n\
    ZeroDivisionError: division by zero";

#[test]
fn test_parse_python_traceback_basic() {
    let doc = parse_python_traceback(SIMPLE_TB).unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "ZeroDivisionError");
    assert_eq!(doc["data"]["exception_type"].as_str().unwrap(), "ZeroDivisionError");
    assert_eq!(doc["data"]["exception_message"].as_str().unwrap(), "division by zero");
    assert!(doc["timestamp"].as_u64().unwrap() > 0);
    assert!(doc["data"]["raw"].as_str().unwrap().contains("Traceback"));
}

#[test]
fn test_parse_python_traceback_frames_parsed() {
    let doc = parse_python_traceback(SIMPLE_TB).unwrap();
    let frames = doc["data"]["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 2);

    assert_eq!(frames[0]["file"].as_str().unwrap(), "app.py");
    assert_eq!(frames[0]["line"].as_u64().unwrap(), 10);
    assert_eq!(frames[0]["function"].as_str().unwrap(), "<module>");
    assert_eq!(frames[0]["source"].as_str().unwrap(), "result = divide(10, 0)");

    assert_eq!(frames[1]["file"].as_str().unwrap(), "lib.py");
    assert_eq!(frames[1]["line"].as_u64().unwrap(), 5);
    assert_eq!(frames[1]["function"].as_str().unwrap(), "divide");
    assert_eq!(frames[1]["source"].as_str().unwrap(), "return a / b");
}

#[test]
fn test_parse_python_traceback_bare_exception_no_message() {
    let text = "Traceback (most recent call last):\n  File \"a.py\", line 1, in f\n    pass\nKeyboardInterrupt";
    let doc = parse_python_traceback(text).unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "KeyboardInterrupt");
    assert_eq!(doc["data"]["exception_message"].as_str().unwrap(), "");
}

#[test]
fn test_parse_python_traceback_result_passes_validate() {
    let doc = parse_python_traceback(SIMPLE_TB).unwrap();
    validate_telemetry(&doc).unwrap();
}

#[test]
fn test_parse_python_traceback_invalid_header_returns_err() {
    assert!(parse_python_traceback("not a traceback").is_err());
    assert!(parse_python_traceback("").is_err());
    let err = parse_python_traceback("not a traceback").err().unwrap().to_string();
    assert!(err.contains("not a Python traceback"));
}

#[test]
fn test_parse_python_traceback_no_exception_line_returns_err() {
    // Frames present but no exception line after them.
    let text = "Traceback (most recent call last):\n  File \"a.py\", line 1, in f\n    pass";
    let err = parse_python_traceback(text).err().unwrap().to_string();
    assert!(err.contains("no exception line"));
}

#[test]
fn test_parse_python_traceback_colon_in_message() {
    // Exception message itself contains ": " — only the first occurrence splits.
    let text = "Traceback (most recent call last):\n  File \"a.py\", line 1, in f\n    x = d[k]\nKeyError: 'foo: bar'";
    let doc = parse_python_traceback(text).unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "KeyError");
    assert_eq!(doc["data"]["exception_message"].as_str().unwrap(), "'foo: bar'");
}

#[test]
fn test_parse_python_traceback_namespaced_exception() {
    let text = "Traceback (most recent call last):\n  File \"a.py\", line 1, in f\n    raise x\npkg.module.CustomError: something failed";
    let doc = parse_python_traceback(text).unwrap();
    assert_eq!(doc["key"].as_str().unwrap(), "pkg.module.CustomError");
    assert_eq!(doc["data"]["exception_message"].as_str().unwrap(), "something failed");
}

#[test]
fn test_parse_python_traceback_chained_exception() {
    // Chained traceback: "During handling…" separator, second Traceback block wins.
    let text = "Traceback (most recent call last):\n  \
        File \"a.py\", line 1, in f\n    \
        raise ValueError(\"original\")\n\
        ValueError: original\n\n\
        During handling of the above exception, another exception occurred:\n\n\
        Traceback (most recent call last):\n  \
        File \"a.py\", line 3, in f\n    \
        raise RuntimeError(\"wrapped\")\n\
        RuntimeError: wrapped";
    let doc = parse_python_traceback(text).unwrap();
    // Last exception in the chain wins.
    assert_eq!(doc["key"].as_str().unwrap(), "RuntimeError");
    assert_eq!(doc["data"]["exception_message"].as_str().unwrap(), "wrapped");
    // Only the frames from the second block survive.
    let frames = doc["data"]["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["file"].as_str().unwrap(), "a.py");
    assert_eq!(frames[0]["line"].as_u64().unwrap(), 3);
}

// ── ingest_multiline_file ─────────────────────────────────────────────────────

#[test]
fn test_ingest_multiline_file_two_tracebacks() {
    let content = "Traceback (most recent call last):\n  \
        File \"a.py\", line 1, in f\n    \
        x = bad()\n\
        ValueError: bad value\n\
        Traceback (most recent call last):\n  \
        File \"b.py\", line 2, in g\n    \
        y = broken\n\
        RuntimeError: broken pipe";

    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("tracebacks.log");
    std::fs::write(&path, content).unwrap();

    let mut docs = Vec::new();
    ingest_multiline_file(
        is_python_traceback_start,
        parse_python_traceback,
        |doc| docs.push(doc),
        path.to_str().unwrap(),
    )
    .unwrap();

    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0]["key"].as_str().unwrap(), "ValueError");
    assert_eq!(docs[1]["key"].as_str().unwrap(), "RuntimeError");
}

#[test]
fn test_ingest_multiline_file_skips_non_traceback_blocks() {
    // A non-traceback block at the top is silently dropped.
    let content = "Some unrelated log line\n\
        Traceback (most recent call last):\n  \
        File \"app.py\", line 5, in main\n    \
        result = compute()\n\
        ZeroDivisionError: division by zero";

    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("mixed.log");
    std::fs::write(&path, content).unwrap();

    let mut count = 0usize;
    ingest_multiline_file(
        is_python_traceback_start,
        parse_python_traceback,
        |_| count += 1,
        path.to_str().unwrap(),
    )
    .unwrap();

    assert_eq!(count, 1);
}

#[test]
fn test_ingest_multiline_file_empty_file_ok() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("empty.log");
    std::fs::write(&path, "").unwrap();

    let mut count = 0usize;
    ingest_multiline_file(
        is_python_traceback_start,
        parse_python_traceback,
        |_| count += 1,
        path.to_str().unwrap(),
    )
    .unwrap();

    assert_eq!(count, 0);
}

#[test]
fn test_ingest_multiline_file_missing_file_returns_err() {
    let result = ingest_multiline_file(
        is_python_traceback_start,
        parse_python_traceback,
        |_| {},
        "/nonexistent/path/tracebacks.log",
    );
    assert!(result.is_err());
    assert!(result.err().unwrap().to_string().contains("cannot open"));
}

#[test]
fn test_ingest_multiline_file_all_docs_pass_validate() {
    let content = "Traceback (most recent call last):\n  \
        File \"x.py\", line 1, in f\n    \
        raise e\n\
        OSError: file not found\n\
        Traceback (most recent call last):\n  \
        File \"y.py\", line 9, in g\n    \
        do_thing()\n\
        TypeError: bad type";

    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("validate.log");
    std::fs::write(&path, content).unwrap();

    let mut docs = Vec::new();
    ingest_multiline_file(
        is_python_traceback_start,
        parse_python_traceback,
        |doc| docs.push(doc),
        path.to_str().unwrap(),
    )
    .unwrap();

    assert_eq!(docs.len(), 2);
    for doc in &docs {
        validate_telemetry(doc).unwrap();
    }
}
