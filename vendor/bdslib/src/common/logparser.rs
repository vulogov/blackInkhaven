use crate::common::error::{err_msg, Result};
use crate::common::time::now_secs;
use chrono::{Datelike, TimeZone, Utc};
use grok::{Grok, Pattern};
use serde_json::{json, Value as JsonValue};
use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader};
use std::sync::OnceLock;

// ── GrokParser ────────────────────────────────────────────────────────────────

/// A compiled grok pattern ready for repeated matching.
///
/// `GrokParser` wraps [`grok::Pattern`] and exposes the error type used
/// throughout `bdslib`.  All built-in logstash/grok pattern names
/// (`%{WORD}`, `%{IPORHOST}`, `%{GREEDYDATA}`, …) are available by default.
pub struct GrokParser {
    pattern: Pattern,
}

impl GrokParser {
    /// Compile `pattern_str` against the full default grok pattern library.
    ///
    /// Returns `Err` if any pattern reference cannot be resolved or the
    /// resulting regex does not compile.
    pub fn new(pattern_str: &str) -> Result<Self> {
        let grok = Grok::with_default_patterns();
        let pattern = grok
            .compile(pattern_str, true)
            .map_err(|e| err_msg(format!("grok compile error for '{pattern_str}': {e}")))?;
        Ok(Self { pattern })
    }

    /// Compile `pattern_str` with additional custom pattern definitions.
    ///
    /// `customs` is a slice of `(name, regex)` pairs that are added to the
    /// default library before compilation.  Custom names may be referenced
    /// inside `pattern_str` just like built-in names.
    pub fn with_patterns(customs: &[(&str, &str)], pattern_str: &str) -> Result<Self> {
        let mut grok = Grok::with_default_patterns();
        for (name, pat) in customs {
            grok.add_pattern(*name, *pat);
        }
        let pattern = grok
            .compile(pattern_str, true)
            .map_err(|e| err_msg(format!("grok compile error for '{pattern_str}': {e}")))?;
        Ok(Self { pattern })
    }

    /// Borrow the underlying compiled [`grok::Pattern`].
    pub fn pattern(&self) -> &Pattern {
        &self.pattern
    }
}

// ── Core API ──────────────────────────────────────────────────────────────────

/// Parse `line` with `parser` and return a JSON document ready for
/// [`ShardsManager::add`].
///
/// All grok captures are stored in `"data"` as a JSON object.
/// `"timestamp"` and `"key"` are resolved from the capture map using
/// well-known field names (see details below) and fall back to sensible
/// defaults if absent:
///
/// | Priority | `"timestamp"` source | Parsed as |
/// |---|---|---|
/// | 1 | capture named `timestamp` or `ts` | numeric Unix seconds |
/// | 2 | capture named `apache_ts` or `httpdate` | `dd/Mon/YYYY:HH:MM:SS ±HHMM` |
/// | 3 | capture named `syslog_ts` | `Mon d HH:MM:SS` (current year, UTC) |
/// | 4 | fallback | `SystemTime::now()` |
///
/// | Priority | `"key"` source |
/// |---|---|
/// | 1 | capture named `key` |
/// | 2 | capture named `program`, `method`, `severity`, or `level` |
/// | 3 | fallback `"log"` |
///
/// Returns `Err` if the pattern does not match, or if the resulting document
/// fails [`validate_telemetry`].
pub fn parse_log_line(parser: &GrokParser, line: &str) -> Result<JsonValue> {
    let matches = parser
        .pattern
        .match_against(line)
        .ok_or_else(|| err_msg(format!("grok pattern did not match: {line:?}")))?;

    let fields: BTreeMap<&str, &str> = matches.iter().collect();

    let timestamp = resolve_timestamp(&fields);
    let key = resolve_key(&fields).to_string();
    let data: serde_json::Map<String, JsonValue> = fields
        .iter()
        .map(|(k, v)| ((*k).to_string(), json!(*v)))
        .collect();

    let doc = json!({
        "timestamp": timestamp,
        "key": key,
        "data": JsonValue::Object(data),
    });

    validate_telemetry(&doc)?;
    Ok(doc)
}

/// Verify that `doc` is a valid telemetry entry for [`ShardsManager::add`].
///
/// Checks:
/// - `"timestamp"` — present and a non-negative integer (Unix seconds)
/// - `"key"` — present and a non-empty string
/// - `"data"` — present and not `null`
///
/// Returns `Ok(())` on success, `Err` describing the first violation found.
pub fn validate_telemetry(doc: &JsonValue) -> Result<()> {
    match doc.get("timestamp") {
        Some(JsonValue::Number(n)) if n.as_u64().is_some() => {}
        Some(JsonValue::Number(_)) => {
            return Err(err_msg(
                "'timestamp' must be a non-negative integer (Unix seconds)",
            ));
        }
        Some(_) => return Err(err_msg("'timestamp' must be a numeric Unix timestamp")),
        None => return Err(err_msg("missing required field 'timestamp'")),
    }

    match doc.get("key") {
        Some(JsonValue::String(s)) if !s.is_empty() => {}
        Some(JsonValue::String(_)) => return Err(err_msg("'key' must not be empty")),
        Some(_) => return Err(err_msg("'key' must be a string")),
        None => return Err(err_msg("missing required field 'key'")),
    }

    match doc.get("data") {
        None | Some(JsonValue::Null) => return Err(err_msg("missing required field 'data'")),
        Some(_) => {}
    }

    Ok(())
}

// ── Syslog ────────────────────────────────────────────────────────────────────

const SYSLOG_CUSTOMS: &[(&str, &str)] = &[
    (
        "SYSLOG_TS",
        r"(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) +\d{1,2} \d{2}:\d{2}:\d{2}",
    ),
    ("SYSLOG_PROG", r"[a-zA-Z0-9._\-/]+"),
    (
        "HOSTADDR",
        r"(?:(?:\d{1,3}\.){3}\d{1,3}|[a-zA-Z0-9][a-zA-Z0-9._-]*)",
    ),
];

const SYSLOG_PATTERN: &str = r"%{SYSLOG_TS:syslog_ts} %{HOSTADDR:host} %{SYSLOG_PROG:key}(?:\[%{POSINT:pid}\])?: %{GREEDYDATA:message}";

static SYSLOG_PARSER: OnceLock<GrokParser> = OnceLock::new();

fn syslog_parser() -> &'static GrokParser {
    SYSLOG_PARSER.get_or_init(|| {
        GrokParser::with_patterns(SYSLOG_CUSTOMS, SYSLOG_PATTERN)
            .expect("syslog grok pattern is invalid — this is a bug")
    })
}

/// Parse an RFC 3164 syslog line into a ShardsManager-ready JSON document.
///
/// ```text
/// Nov 16 23:01:55 myhost sshd[1234]: Accepted publickey for alice from 10.0.0.1
/// ```
///
/// | Field | Source |
/// |---|---|
/// | `"timestamp"` | syslog timestamp with current year, UTC assumed |
/// | `"key"` | program name (e.g. `"sshd"`) |
/// | `"data"` | object with `message`, `host`, and optionally `pid` |
pub fn parse_syslog(line: &str) -> Result<JsonValue> {
    let matches = syslog_parser()
        .pattern
        .match_against(line)
        .ok_or_else(|| err_msg(format!("syslog pattern did not match: {line:?}")))?;

    let fields: BTreeMap<&str, &str> = matches.iter().collect();

    let timestamp = fields
        .get("syslog_ts")
        .and_then(|s| parse_syslog_ts(s))
        .unwrap_or_else(now_secs);
    let key = fields.get("key").copied().unwrap_or("syslog");

    let mut data = serde_json::Map::new();
    data.insert("message".into(), json!(fields.get("message").copied().unwrap_or("")));
    if let Some(h) = fields.get("host") {
        data.insert("host".into(), json!(*h));
    }
    if let Some(pid) = fields.get("pid") {
        data.insert("pid".into(), json!(*pid));
    }
    data.insert("raw".into(), json!(line));

    let doc = json!({ "timestamp": timestamp, "key": key, "data": data });
    validate_telemetry(&doc)?;
    Ok(doc)
}

// ── HTTP access logs (Apache / Nginx / NCSA CLF) ──────────────────────────────

const HTTP_CUSTOMS: &[(&str, &str)] = &[
    (
        "APACHE_TS",
        r"\d{2}/\w{3}/\d{4}:\d{2}:\d{2}:\d{2} [+-]\d{4}",
    ),
    (
        "CLIENTIP",
        r"(?:(?:\d{1,3}\.){3}\d{1,3}|[0-9a-fA-F:]+|[a-zA-Z0-9._-]+)",
    ),
];

// NCSA Common Log Format (no referer / user-agent)
const CLF_PATTERN: &str = r#"%{CLIENTIP:client} %{NOTSPACE:ident} %{NOTSPACE:auth} \[%{APACHE_TS:apache_ts}\] "%{WORD:method} %{NOTSPACE:path} HTTP/%{NUMBER:http_version}" %{NUMBER:status} (?:%{NUMBER:bytes}|-)"#;

// Apache / Nginx Combined Log Format (includes referer + user-agent)
const COMBINED_PATTERN: &str = r#"%{CLIENTIP:client} %{NOTSPACE:ident} %{NOTSPACE:auth} \[%{APACHE_TS:apache_ts}\] "%{WORD:method} %{NOTSPACE:path} HTTP/%{NUMBER:http_version}" %{NUMBER:status} (?:%{NUMBER:bytes}|-) "(?:%{DATA:referrer}|-)" "%{DATA:user_agent}""#;

static CLF_PARSER: OnceLock<GrokParser> = OnceLock::new();
static APACHE_PARSER: OnceLock<GrokParser> = OnceLock::new();
static NGINX_PARSER: OnceLock<GrokParser> = OnceLock::new();

fn clf_parser() -> &'static GrokParser {
    CLF_PARSER.get_or_init(|| {
        GrokParser::with_patterns(HTTP_CUSTOMS, CLF_PATTERN)
            .expect("NCSA CLF grok pattern is invalid — this is a bug")
    })
}

fn apache_parser() -> &'static GrokParser {
    APACHE_PARSER.get_or_init(|| {
        GrokParser::with_patterns(HTTP_CUSTOMS, COMBINED_PATTERN)
            .expect("Apache combined grok pattern is invalid — this is a bug")
    })
}

fn nginx_parser() -> &'static GrokParser {
    NGINX_PARSER.get_or_init(|| {
        GrokParser::with_patterns(HTTP_CUSTOMS, COMBINED_PATTERN)
            .expect("Nginx combined grok pattern is invalid — this is a bug")
    })
}

/// Parse an NCSA Common Log Format (CLF) line.
///
/// ```text
/// 127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.0" 200 1024
/// ```
///
/// | Field | Source |
/// |---|---|
/// | `"timestamp"` | Apache-format timestamp (`dd/Mon/YYYY:HH:MM:SS ±HHMM`) |
/// | `"key"` | `"<METHOD> <path>"` (e.g. `"GET /index.html"`) |
/// | `"data"` | object with `client`, `method`, `path`, `status`, `bytes` |
pub fn parse_ncsa_clf(line: &str) -> Result<JsonValue> {
    parse_http_log(clf_parser(), line, None)
}

/// Parse an Apache Combined Log Format line (includes referer and user-agent).
///
/// ```text
/// 127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.0" 200 1024 "-" "Mozilla/5.0"
/// ```
pub fn parse_apache(line: &str) -> Result<JsonValue> {
    parse_http_log(apache_parser(), line, None)
}

/// Parse an Nginx combined access log line (same format as Apache combined).
///
/// ```text
/// 192.168.1.1 - - [10/Oct/2024:13:55:36 +0000] "POST /api/v1/data HTTP/1.1" 201 512 "https://example.com" "curl/7.81.0"
/// ```
pub fn parse_nginx(line: &str) -> Result<JsonValue> {
    parse_http_log(nginx_parser(), line, Some("nginx"))
}

fn parse_http_log(parser: &GrokParser, line: &str, server: Option<&str>) -> Result<JsonValue> {
    let matches = parser
        .pattern
        .match_against(line)
        .ok_or_else(|| err_msg(format!("HTTP log pattern did not match: {line:?}")))?;

    let fields: BTreeMap<&str, &str> = matches.iter().collect();

    let timestamp = fields
        .get("apache_ts")
        .and_then(|s| parse_apache_ts(s))
        .unwrap_or_else(now_secs);

    let method = fields.get("method").copied().unwrap_or("UNKNOWN");
    let path = fields.get("path").copied().unwrap_or("/");
    let key = format!("{method} {path}");

    let mut data = serde_json::Map::new();
    data.insert("client".into(), json!(fields.get("client").copied().unwrap_or("")));
    data.insert("method".into(), json!(method));
    data.insert("path".into(), json!(path));
    if let Some(v) = fields.get("status") {
        data.insert("status".into(), json!(*v));
    }
    if let Some(v) = fields.get("bytes").filter(|v| **v != "-") {
        data.insert("bytes".into(), json!(*v));
    }
    if let Some(v) = fields.get("http_version") {
        data.insert("http_version".into(), json!(*v));
    }
    if let Some(v) = fields.get("referrer").filter(|v| **v != "-" && !v.is_empty()) {
        data.insert("referrer".into(), json!(*v));
    }
    if let Some(v) = fields.get("user_agent").filter(|v| **v != "-" && !v.is_empty()) {
        data.insert("user_agent".into(), json!(*v));
    }
    if let Some(s) = server {
        data.insert("server".into(), json!(s));
    }
    data.insert("raw".into(), json!(line));

    let doc = json!({ "timestamp": timestamp, "key": key, "data": data });
    validate_telemetry(&doc)?;
    Ok(doc)
}

// ── Python traceback ──────────────────────────────────────────────────────────

/// Returns `true` if `line` is the first line of a Python traceback block.
///
/// Python tracebacks always begin with the literal string
/// `Traceback (most recent call last):`.
pub fn is_python_traceback_start(line: &str) -> bool {
    line.trim() == "Traceback (most recent call last):"
}

/// Parse a Python traceback block (multiline string) into a ShardsManager-ready
/// JSON document.
///
/// Input must begin with `Traceback (most recent call last):`.  Stack frames
/// are collected into a `frames` array; the final exception line supplies the
/// document `"key"`.  Chained tracebacks (preceded by
/// `During handling of the above exception, another exception occurred:`) are
/// supported — frames reset on each nested `Traceback` header and the last
/// exception wins.
///
/// ```text
/// Traceback (most recent call last):
///   File "app.py", line 42, in handler
///     result = compute(x)
///   File "lib.py", line 17, in compute
///     return a / b
/// ZeroDivisionError: division by zero
/// ```
///
/// | Field | Source |
/// |---|---|
/// | `"timestamp"` | `SystemTime::now()` |
/// | `"key"` | exception type (e.g. `"ZeroDivisionError"`) |
/// | `"data.exception_type"` | exception class name |
/// | `"data.exception_message"` | text after the first `": "` separator |
/// | `"data.frames"` | array of `{file, line, function[, source]}` objects |
/// | `"data.raw"` | original text unchanged |
pub fn parse_python_traceback(text: &str) -> Result<JsonValue> {
    let lines: Vec<&str> = text.lines().collect();
    let header = lines.first().map(|l| l.trim()).unwrap_or("");
    if header != "Traceback (most recent call last):" {
        return Err(err_msg(format!(
            "not a Python traceback: first line is {header:?}"
        )));
    }

    let mut frames: Vec<JsonValue> = Vec::new();
    let mut exception_type = String::new();
    let mut exception_message = String::new();

    let mut i = 1usize;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        if trimmed.starts_with("File \"") {
            // Frame header: `  File "filename", line N, in function`
            let source = if i + 1 < lines.len() && lines[i + 1].starts_with("    ") {
                i += 1;
                lines[i].trim().to_string()
            } else {
                String::new()
            };
            frames.push(tb_parse_frame(trimmed, &source));
        } else if !line.starts_with(' ') && !line.starts_with('\t') {
            if trimmed == "Traceback (most recent call last):" {
                // Nested chained traceback — reset frames, keep going
                frames.clear();
            } else if tb_looks_like_exception(trimmed) {
                let (t, m) = tb_split_exception(trimmed);
                exception_type = t;
                exception_message = m;
            }
            // Non-matching non-indented lines (e.g. "During handling…") are ignored
        }

        i += 1;
    }

    if exception_type.is_empty() {
        return Err(err_msg("Python traceback has no exception line"));
    }

    let doc = json!({
        "timestamp": now_secs(),
        "key": exception_type.clone(),
        "data": {
            "exception_type": exception_type,
            "exception_message": exception_message,
            "frames": frames,
            "raw": text,
        }
    });
    validate_telemetry(&doc)?;
    Ok(doc)
}

fn tb_parse_frame(trimmed: &str, source: &str) -> JsonValue {
    let file = trimmed
        .find('"')
        .and_then(|s| trimmed[s + 1..].find('"').map(|e| trimmed[s + 1..s + 1 + e].to_string()))
        .unwrap_or_default();

    let line_no: u64 = trimmed
        .find(", line ")
        .map(|pos| {
            let rest = &trimmed[pos + 7..];
            let end = rest.find([',', ' ']).unwrap_or(rest.len());
            rest[..end].parse().unwrap_or(0)
        })
        .unwrap_or(0);

    let function = trimmed
        .rfind(", in ")
        .map(|pos| trimmed[pos + 5..].to_string())
        .unwrap_or_default();

    if source.is_empty() {
        json!({"file": file, "line": line_no, "function": function})
    } else {
        json!({"file": file, "line": line_no, "function": function, "source": source})
    }
}

fn tb_looks_like_exception(s: &str) -> bool {
    let name_end = s.find(": ").unwrap_or(s.len());
    let name = &s[..name_end];
    !name.is_empty()
        && name.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

fn tb_split_exception(s: &str) -> (String, String) {
    match s.find(": ") {
        Some(pos) => (s[..pos].to_string(), s[pos + 2..].to_string()),
        None => (s.to_string(), String::new()),
    }
}

// ── Stdin reader ──────────────────────────────────────────────────────────────

/// Read log lines from stdin, parse each with `parser`, and pass the result to
/// `callback`.
///
/// Lines that fail to parse are skipped silently.  Returns `Err` only on an
/// I/O error reading stdin.
///
/// The `parser` parameter accepts any function with signature
/// `Fn(&str) -> Result<JsonValue>`, including the format helpers provided by
/// this module (`parse_syslog`, `parse_apache`, `parse_nginx`,
/// `parse_ncsa_clf`) and custom closures wrapping [`parse_log_line`].
///
/// ```no_run
/// use bdslib::common::logparser::{ingest_stdin, parse_syslog};
///
/// ingest_stdin(parse_syslog, |doc| {
///     println!("{}", serde_json::to_string_pretty(&doc).unwrap());
/// }).unwrap();
/// ```
pub fn ingest_stdin<P, C>(parser: P, mut callback: C) -> Result<()>
where
    P: Fn(&str) -> Result<JsonValue>,
    C: FnMut(JsonValue),
{
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| err_msg(format!("stdin read error: {e}")))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(doc) = parser(trimmed) {
            callback(doc);
        }
    }
    Ok(())
}

/// Read log lines from a file, parse each with `parser`, and pass the result
/// to `callback`.
///
/// Lines that fail to parse are skipped silently.  Returns `Err` if the file
/// cannot be opened or if an I/O error occurs during reading.
///
/// The `parser` parameter accepts the same types as [`ingest_stdin`]:
/// any `Fn(&str) -> Result<JsonValue>`, including the format helpers in this
/// module and custom closures wrapping [`parse_log_line`].
///
/// ```no_run
/// use bdslib::common::logparser::{ingest_file, parse_apache};
///
/// ingest_file(parse_apache, |doc| {
///     println!("{}", serde_json::to_string_pretty(&doc).unwrap());
/// }, "/var/log/apache2/access.log").unwrap();
/// ```
pub fn ingest_file<P, C>(parser: P, mut callback: C, filename: &str) -> Result<()>
where
    P: Fn(&str) -> Result<JsonValue>,
    C: FnMut(JsonValue),
{
    let file = std::fs::File::open(filename)
        .map_err(|e| err_msg(format!("cannot open '{filename}': {e}")))?;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| err_msg(format!("read error in '{filename}': {e}")))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(doc) = parser(trimmed) {
            callback(doc);
        }
    }
    Ok(())
}

// ── Multiline reader ──────────────────────────────────────────────────────────

fn ingest_multiline_impl<R, S, P, C>(
    reader: R,
    is_start: S,
    parser: P,
    mut callback: C,
) -> Result<()>
where
    R: BufRead,
    S: Fn(&str) -> bool,
    P: Fn(&str) -> Result<JsonValue>,
    C: FnMut(JsonValue),
{
    let mut buffer: Vec<String> = Vec::new();

    for line_result in reader.lines() {
        let line = line_result.map_err(|e| err_msg(format!("I/O read error: {e}")))?;
        if is_start(&line) && !buffer.is_empty() {
            let text = buffer.join("\n");
            if let Ok(doc) = parser(&text) {
                callback(doc);
            }
            buffer.clear();
        }
        buffer.push(line);
    }
    if !buffer.is_empty() {
        let text = buffer.join("\n");
        if let Ok(doc) = parser(&text) {
            callback(doc);
        }
    }
    Ok(())
}

/// Read multiline log entries from stdin.
///
/// Lines are accumulated into a block until `is_start(line)` returns `true`
/// on a subsequent line, at which point the accumulated block (lines joined
/// with `'\n'`) is forwarded to `parser`.  The final block is flushed at
/// EOF.  Blocks that fail to parse are silently dropped.  Returns `Err` only
/// on I/O error.
///
/// ```no_run
/// use bdslib::common::logparser::{
///     ingest_multiline_stdin, is_python_traceback_start, parse_python_traceback,
/// };
///
/// ingest_multiline_stdin(
///     is_python_traceback_start,
///     parse_python_traceback,
///     |doc| println!("{}", serde_json::to_string_pretty(&doc).unwrap()),
/// ).unwrap();
/// ```
pub fn ingest_multiline_stdin<S, P, C>(is_start: S, parser: P, callback: C) -> Result<()>
where
    S: Fn(&str) -> bool,
    P: Fn(&str) -> Result<JsonValue>,
    C: FnMut(JsonValue),
{
    let stdin = io::stdin();
    ingest_multiline_impl(stdin.lock(), is_start, parser, callback)
}

/// Read multiline log entries from `filename`.
///
/// Works like [`ingest_multiline_stdin`] but reads from a file.
/// Returns `Err` if the file cannot be opened or if an I/O error occurs.
///
/// ```no_run
/// use bdslib::common::logparser::{
///     ingest_multiline_file, is_python_traceback_start, parse_python_traceback,
/// };
///
/// ingest_multiline_file(
///     is_python_traceback_start,
///     parse_python_traceback,
///     |doc| println!("{}", serde_json::to_string_pretty(&doc).unwrap()),
///     "/var/log/app/errors.log",
/// ).unwrap();
/// ```
pub fn ingest_multiline_file<S, P, C>(
    is_start: S,
    parser: P,
    callback: C,
    filename: &str,
) -> Result<()>
where
    S: Fn(&str) -> bool,
    P: Fn(&str) -> Result<JsonValue>,
    C: FnMut(JsonValue),
{
    let file = std::fs::File::open(filename)
        .map_err(|e| err_msg(format!("cannot open '{filename}': {e}")))?;
    ingest_multiline_impl(BufReader::new(file), is_start, parser, callback)
}

// ── internal helpers ──────────────────────────────────────────────────────────

fn resolve_timestamp(fields: &BTreeMap<&str, &str>) -> u64 {
    for name in &["timestamp", "ts", "epoch"] {
        if let Some(s) = fields.get(*name) {
            if let Ok(n) = s.parse::<u64>() {
                return n;
            }
        }
    }
    if let Some(ts) = fields
        .get("apache_ts")
        .or(fields.get("httpdate"))
        .and_then(|s| parse_apache_ts(s))
    {
        return ts;
    }
    if let Some(ts) = fields.get("syslog_ts").and_then(|s| parse_syslog_ts(s)) {
        return ts;
    }
    now_secs()
}

fn resolve_key<'a>(fields: &BTreeMap<&str, &'a str>) -> &'a str {
    for name in &["key", "program", "method", "severity", "level", "type"] {
        if let Some(v) = fields.get(*name) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    "log"
}

/// Parse `dd/Mon/YYYY:HH:MM:SS ±HHMM` (Apache/Nginx timestamp) to Unix seconds.
fn parse_apache_ts(s: &str) -> Option<u64> {
    chrono::DateTime::parse_from_str(s, "%d/%b/%Y:%H:%M:%S %z")
        .ok()
        .map(|dt| dt.timestamp() as u64)
}

/// Parse `Mon d HH:MM:SS` or `Mon dd HH:MM:SS` (syslog timestamp) to Unix
/// seconds.
///
/// The current year is injected and UTC is assumed.  Syslog lines from late
/// December may map to the wrong year when processed in early January.
fn parse_syslog_ts(s: &str) -> Option<u64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let month = month_num(parts[0])?;
    let day: u32 = parts[1].parse().ok()?;
    let time: Vec<u32> = parts[2]
        .split(':')
        .map(|p| p.parse().unwrap_or(0))
        .collect();
    if time.len() < 3 {
        return None;
    }
    let year = Utc::now().year();
    let date = chrono::NaiveDate::from_ymd_opt(year, month, day)?;
    let dt = date.and_hms_opt(time[0], time[1], time[2])?;
    Some(Utc.from_utc_datetime(&dt).timestamp() as u64)
}

fn month_num(s: &str) -> Option<u32> {
    match s {
        "Jan" | "January" => Some(1),
        "Feb" | "February" => Some(2),
        "Mar" | "March" => Some(3),
        "Apr" | "April" => Some(4),
        "May" => Some(5),
        "Jun" | "June" => Some(6),
        "Jul" | "July" => Some(7),
        "Aug" | "August" => Some(8),
        "Sep" | "September" => Some(9),
        "Oct" | "October" => Some(10),
        "Nov" | "November" => Some(11),
        "Dec" | "December" => Some(12),
        _ => None,
    }
}
