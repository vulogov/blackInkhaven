use crate::common::time::now_secs;
use chrono::{TimeZone, Utc};
use rand::Rng;
use serde_json::{json, Value as JsonValue};
use uuid::Uuid;

// ── Metric catalogue ──────────────────────────────────────────────────────────

struct MetricSpec {
    key: &'static str,
    unit: &'static str,
    min: f64,
    max: f64,
}

const METRICS: &[MetricSpec] = &[
    MetricSpec { key: "cpu.usage",            unit: "percent",  min: 0.0,   max: 100.0   },
    MetricSpec { key: "cpu.iowait",           unit: "percent",  min: 0.0,   max: 60.0    },
    MetricSpec { key: "cpu.steal",            unit: "percent",  min: 0.0,   max: 20.0    },
    MetricSpec { key: "mem.used_pct",         unit: "percent",  min: 10.0,  max: 95.0    },
    MetricSpec { key: "mem.free_bytes",       unit: "bytes",    min: 0.0,   max: 3.4e10  },
    MetricSpec { key: "mem.swap_used_bytes",  unit: "bytes",    min: 0.0,   max: 2.1e9   },
    MetricSpec { key: "disk.read_bytes_sec",  unit: "bytes/s",  min: 0.0,   max: 5.0e8   },
    MetricSpec { key: "disk.write_bytes_sec", unit: "bytes/s",  min: 0.0,   max: 5.0e8   },
    MetricSpec { key: "disk.iowait",          unit: "percent",  min: 0.0,   max: 80.0    },
    MetricSpec { key: "net.rx_bytes_sec",     unit: "bytes/s",  min: 0.0,   max: 1.25e9  },
    MetricSpec { key: "net.tx_bytes_sec",     unit: "bytes/s",  min: 0.0,   max: 1.25e9  },
    MetricSpec { key: "net.dropped_packets",  unit: "count",    min: 0.0,   max: 1000.0  },
    MetricSpec { key: "process.cpu_pct",      unit: "percent",  min: 0.0,   max: 100.0   },
    MetricSpec { key: "process.mem_rss",      unit: "bytes",    min: 1.0e6, max: 4.0e9   },
    MetricSpec { key: "queue.depth",          unit: "count",    min: 0.0,   max: 5000.0  },
    MetricSpec { key: "queue.lag_ms",         unit: "ms",       min: 0.0,   max: 30000.0 },
    MetricSpec { key: "db.query_latency_ms",  unit: "ms",       min: 0.1,   max: 5000.0  },
    MetricSpec { key: "db.connections",       unit: "count",    min: 0.0,   max: 500.0   },
    MetricSpec { key: "http.request_rate",    unit: "req/s",    min: 0.0,   max: 10000.0 },
    MetricSpec { key: "http.error_rate",      unit: "percent",  min: 0.0,   max: 30.0    },
    MetricSpec { key: "http.p99_latency_ms",  unit: "ms",       min: 1.0,   max: 10000.0 },
    MetricSpec { key: "cache.hit_rate",       unit: "percent",  min: 50.0,  max: 100.0   },
    MetricSpec { key: "cache.evictions_sec",  unit: "count/s",  min: 0.0,   max: 1000.0  },
];

// ── Static data tables ────────────────────────────────────────────────────────

const HOSTS: &[&str] = &[
    "web-01", "web-02", "web-03", "api-01", "api-02",
    "db-primary", "db-replica-01", "cache-01",
    "worker-01", "worker-02", "k8s-node-01", "k8s-node-02",
];
const REGIONS: &[&str] = &["us-east-1", "us-west-2", "eu-west-1", "ap-southeast-1"];
const ENVS:    &[&str] = &["prod", "staging", "dev"];

const SYSLOG_PROGRAMS: &[&str] = &[
    "sshd", "nginx", "apache2", "postgres", "systemd",
    "kernel", "cron", "sudo", "dockerd", "kubelet",
];
const SYSLOG_USERS: &[&str] = &["alice", "bob", "carol", "deploy", "admin", "ubuntu"];

const HTTP_METHODS:  &[&str] = &["GET", "POST", "PUT", "DELETE", "PATCH"];
const HTTP_STATUSES: &[&str] = &[
    "200", "201", "204", "301", "302",
    "400", "401", "403", "404", "500", "502", "503",
];
const HTTP_PATHS: &[&str] = &[
    "/", "/index.html", "/api/v1/users", "/api/v1/data",
    "/api/v2/events", "/health", "/metrics",
    "/api/v1/auth/login", "/api/v1/auth/logout",
    "/api/v1/items/42", "/api/v1/search?q=test",
    "/admin/dashboard", "/static/app.js",
];
const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/120.0.0.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Firefox/121.0",
    "curl/7.81.0",
    "HTTPie/3.2.1",
    "python-requests/2.31.0",
    "Go-http-client/2.0",
    "kube-probe/1.27",
    "Prometheus/2.47.0",
];
const REFERRERS: &[&str] = &[
    "https://example.com/",
    "https://app.example.com/dashboard",
    "https://google.com/search?q=test",
    "-",
];

const PYTHON_EXCEPTIONS: &[&str] = &[
    "ValueError", "TypeError", "KeyError", "IndexError", "AttributeError",
    "RuntimeError", "OSError", "ImportError", "PermissionError",
    "ConnectionError", "TimeoutError", "NotImplementedError",
    "ZeroDivisionError", "RecursionError", "AssertionError", "MemoryError",
];
const EXCEPTION_MESSAGES: &[&str] = &[
    "invalid literal for int() with base 10: 'none'",
    "unsupported operand type(s) for +: 'int' and 'str'",
    "'config' key not found in response",
    "list index out of range",
    "'NoneType' object has no attribute 'get'",
    "connection refused to 127.0.0.1:5432",
    "timed out after 30s waiting for response",
    "division by zero",
    "no module named 'yaml'",
    "permission denied: /var/run/app.sock",
    "maximum recursion depth exceeded",
    "assertion failed: expected 200, got 500",
];
const PYTHON_FILES: &[&str] = &[
    "app.py", "main.py", "handler.py", "views.py",
    "models.py", "utils.py", "db.py", "worker.py",
    "api/routes.py", "core/processor.py", "lib/client.py",
];
const PYTHON_FUNCS: &[&str] = &[
    "handle_request", "process_event", "load_config", "connect",
    "fetch_data", "parse_response", "update_record", "validate",
    "run_query", "dispatch", "main", "<module>",
];
const PYTHON_SOURCES: &[&str] = &[
    "result = int(value)",
    "return a + b",
    "cfg = data['config']",
    "item = items[idx]",
    "return obj.field",
    "conn = db.connect(host, port)",
    "resp = await client.get(url, timeout=30)",
    "x = numerator / denominator",
    "assert status == 200, f'got {status}'",
];

// Vocabulary for $word and $name placeholders
const WORDS: &[&str] = &[
    "alpha", "bravo", "charlie", "delta", "echo", "foxtrot",
    "golf", "hotel", "india", "juliet", "kilo", "lima",
    "mike", "november", "oscar", "papa", "quebec", "romeo",
    "sierra", "tango", "uniform", "victor", "whiskey", "zulu",
];
const FIRST_NAMES: &[&str] = &[
    "Alice", "Bob", "Carol", "Dave", "Eve", "Frank",
    "Grace", "Heidi", "Ivan", "Judy", "Karl", "Laura",
    "Mallory", "Niaj", "Oscar", "Peggy", "Quentin", "Rupert",
    "Sybil", "Trent", "Ursula", "Victor", "Wendy", "Xerxes",
];
const LAST_NAMES: &[&str] = &[
    "Smith", "Jones", "Brown", "Davis", "Miller", "Wilson",
    "Moore", "Taylor", "Anderson", "Thomas", "Jackson", "White",
    "Harris", "Martin", "Garcia", "Martinez", "Robinson", "Clark",
];

// ── Log format selector ───────────────────────────────────────────────────────

/// Selects which log-entry variant [`Generator::log_entries`] produces.
///
/// Pass to [`Generator::with_log_format`] before calling `log_entries` or
/// `mixed`.  Defaults to [`LogFormat::Random`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Pick a format at random for each document (default).
    Random,
    /// RFC-3164 syslog style.
    Syslog,
    /// Apache Combined Log Format.
    Http,
    /// Nginx access log format.
    HttpNginx,
    /// Python exception traceback.
    Traceback,
}

// ── Generator ─────────────────────────────────────────────────────────────────

/// Produces random JSON documents suitable for ingestion into `ShardsManager`.
///
/// All time windows are expressed as [humantime] duration strings such as
/// `"1h"`, `"30m"`, `"2d 4h"`.  Each method call resolves the window to
/// `[now − duration, now]` at the moment it is called.
///
/// An explicit timestamp range can be pinned via [`Generator::with_time_range`];
/// when set it overrides the `duration` argument in every method call,
/// which is useful for tests that need deterministic timestamp bounds.
///
/// # Example
///
/// ```
/// use bdslib::common::generator::Generator;
///
/// let g = Generator::new();
/// let telemetry = g.telemetry("1h", 50);
/// let logs      = g.log_entries("30m", 50);
/// let mixed     = g.mixed("2h", 100, 0.7); // 70 % telemetry, 30 % log entries
///
/// assert_eq!(telemetry.len(), 50);
/// assert_eq!(logs.len(), 50);
/// assert_eq!(mixed.len(), 100);
/// ```
///
/// [humantime]: https://docs.rs/humantime
#[derive(Debug, Clone)]
pub struct Generator {
    /// When `Some`, overrides the `duration` argument in all method calls.
    time_override: Option<(u64, u64)>,
    /// When `Some`, telemetry documents are locked to this metric key.
    key_filter: Option<String>,
    /// Controls which log-entry variant is produced.
    log_format: LogFormat,
}

impl Default for Generator {
    fn default() -> Self {
        Self::new()
    }
}

impl Generator {
    /// Create a generator that derives its time window from the `duration`
    /// argument passed to each method.
    pub fn new() -> Self {
        Generator { time_override: None, key_filter: None, log_format: LogFormat::Random }
    }

    /// Create a generator with a fixed `[start, end]` timestamp range
    /// (Unix seconds).  This overrides the `duration` argument in every
    /// method call.
    pub fn with_time_range(start: u64, end: u64) -> Self {
        Generator { time_override: Some((start, end)), key_filter: None, log_format: LogFormat::Random }
    }

    /// Restrict telemetry generation to a specific metric key (e.g. `"cpu.usage"`).
    ///
    /// If the key is not in the built-in catalogue a random metric is used instead.
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key_filter = Some(key.into());
        self
    }

    /// Restrict log-entry generation to a specific format.
    pub fn with_log_format(mut self, fmt: LogFormat) -> Self {
        self.log_format = fmt;
        self
    }

    /// Generate `n` random metric-style telemetry documents spread over
    /// the given `duration` window.
    ///
    /// Each document has the shape:
    /// ```json
    /// { "timestamp": 1700000042, "key": "cpu.usage",
    ///   "data": { "value": 72.4, "unit": "percent",
    ///             "host": "web-01", "region": "us-east-1", "env": "prod" } }
    /// ```
    pub fn telemetry(&self, duration: &str, n: usize) -> Vec<JsonValue> {
        let tr = self.time_range_for(duration);
        let mut rng = rand::thread_rng();
        (0..n).map(|_| self.random_telemetry(&mut rng, tr)).collect()
    }

    /// Generate `n` random log-entry documents in the exact format produced
    /// by `src/common/logparser` (syslog, Apache/Nginx access log, Python
    /// traceback).  All documents pass `validate_telemetry`.
    pub fn log_entries(&self, duration: &str, n: usize) -> Vec<JsonValue> {
        let tr = self.time_range_for(duration);
        let mut rng = rand::thread_rng();
        (0..n).map(|_| self.random_log_entry(&mut rng, tr)).collect()
    }

    /// Generate `n` documents mixing telemetry and log entries over
    /// `duration`.
    ///
    /// `telemetry_ratio ∈ [0.0, 1.0]` is the fraction of telemetry
    /// documents; the remainder are log entries.  Values outside that range
    /// are clamped.
    pub fn mixed(&self, duration: &str, n: usize, telemetry_ratio: f64) -> Vec<JsonValue> {
        let ratio = telemetry_ratio.clamp(0.0, 1.0);
        let tr = self.time_range_for(duration);
        let mut rng = rand::thread_rng();
        (0..n)
            .map(|_| {
                if rng.gen_range(0.0f64..1.0) < ratio {
                    self.random_telemetry(&mut rng, tr)
                } else {
                    self.random_log_entry(&mut rng, tr)
                }
            })
            .collect()
    }

    /// Generate `n` raw RFC-3164 syslog lines ready to be parsed by `parse_syslog`.
    ///
    /// Each line has the form:
    /// ```text
    /// Nov 16 23:01:55 web-01 sshd[12345]: Accepted publickey for alice from 10.0.0.1 port 54321
    /// ```
    pub fn syslog_lines(&self, duration: &str, n: usize) -> Vec<String> {
        let tr = self.time_range_for(duration);
        let mut rng = rand::thread_rng();
        (0..n)
            .map(|_| {
                let program  = pick(&mut rng, SYSLOG_PROGRAMS);
                let host     = pick(&mut rng, HOSTS);
                let pid: u32 = rng.gen_range(100..65535);
                let message  = syslog_message(&mut rng, program);
                let ts       = self.rand_ts(&mut rng, tr);
                format!("{} {} {}[{}]: {}", syslog_ts_str(ts), host, program, pid, message)
            })
            .collect()
    }

    /// Generate `n` documents from a JSON template over `duration`.
    ///
    /// `template` must be a valid JSON string.  String values that begin
    /// with `$` are treated as placeholders and replaced with random data:
    ///
    /// | Placeholder | Output |
    /// |---|---|
    /// | `"$timestamp"` | Random `u64` Unix timestamp within the duration window |
    /// | `"$int(min,max)"` | Random integer in `[min, max]` |
    /// | `"$float(min,max)"` | Random float in `[min, max]` (2 decimal places) |
    /// | `"$choice(a,b,c)"` | One of the comma-separated strings |
    /// | `"$bool"` | JSON `true` or `false` |
    /// | `"$uuid"` | UUID v4 string |
    /// | `"$ip"` | Random IPv4 address string |
    /// | `"$word"` | Random lowercase word |
    /// | `"$name"` | Random `"Firstname Lastname"` string |
    ///
    /// Non-placeholder values (including nested objects and arrays) are
    /// passed through unchanged.  Placeholders are processed recursively at
    /// any nesting depth.
    ///
    /// # Panics
    ///
    /// Panics if `template` is not valid JSON.
    ///
    /// # Example
    ///
    /// ```
    /// use bdslib::common::generator::Generator;
    ///
    /// let template = r#"{
    ///     "timestamp": "$timestamp",
    ///     "key": "$choice(cpu,mem,disk)",
    ///     "data": { "value": "$float(0.0,100.0)", "host": "$word", "alert": "$bool" }
    /// }"#;
    ///
    /// let docs = Generator::new().templated("1h", template, 10);
    /// assert_eq!(docs.len(), 10);
    /// ```
    pub fn templated(&self, duration: &str, template: &str, n: usize) -> Vec<JsonValue> {
        let tr = self.time_range_for(duration);
        let tmpl: JsonValue = serde_json::from_str(template)
            .unwrap_or_else(|e| panic!("templated(): invalid JSON template: {e}"));
        let mut rng = rand::thread_rng();
        (0..n)
            .map(|_| process_template_value(&tmpl, &mut rng, tr))
            .collect()
    }

    // ── private ───────────────────────────────────────────────────────────────

    fn time_range_for(&self, duration: &str) -> (u64, u64) {
        if let Some(pair) = self.time_override {
            return pair;
        }
        parse_duration_to_range(duration)
    }

    fn rand_ts<R: Rng>(&self, rng: &mut R, tr: (u64, u64)) -> u64 {
        let (s, e) = tr;
        if s >= e { return s; }
        rng.gen_range(s..=e)
    }

    fn random_telemetry<R: Rng>(&self, rng: &mut R, tr: (u64, u64)) -> JsonValue {
        let m = self.key_filter
            .as_deref()
            .and_then(|k| METRICS.iter().find(|m| m.key == k))
            .unwrap_or(&METRICS[rng.gen_range(0..METRICS.len())]);
        let host   = pick(rng, HOSTS);
        let region = pick(rng, REGIONS);
        let env    = pick(rng, ENVS);
        let value  = round2(rng.gen_range(m.min..=m.max));
        let ts     = self.rand_ts(rng, tr);

        json!({
            "timestamp": ts,
            "key": m.key,
            "data": { "value": value, "unit": m.unit,
                      "host": host, "region": region, "env": env }
        })
    }

    fn random_log_entry<R: Rng>(&self, rng: &mut R, tr: (u64, u64)) -> JsonValue {
        match self.log_format {
            LogFormat::Syslog    => self.gen_syslog(rng, tr),
            LogFormat::Http      => self.gen_http_access(rng, tr, None),
            LogFormat::HttpNginx => self.gen_http_access(rng, tr, Some("nginx")),
            LogFormat::Traceback => self.gen_python_traceback(rng, tr),
            LogFormat::Random    => match rng.gen_range(0u8..4) {
                0 => self.gen_syslog(rng, tr),
                1 => self.gen_http_access(rng, tr, None),
                2 => self.gen_http_access(rng, tr, Some("nginx")),
                _ => self.gen_python_traceback(rng, tr),
            },
        }
    }

    fn gen_syslog<R: Rng>(&self, rng: &mut R, tr: (u64, u64)) -> JsonValue {
        let program = pick(rng, SYSLOG_PROGRAMS);
        let host    = pick(rng, HOSTS);
        let pid: u32 = rng.gen_range(100..65535);
        let message = syslog_message(rng, program);
        let ts      = self.rand_ts(rng, tr);
        let raw     = format!("{} {} {}[{}]: {}", syslog_ts_str(ts), host, program, pid, message);

        json!({
            "timestamp": ts, "key": program,
            "data": { "message": message, "host": host,
                      "pid": pid.to_string(), "raw": raw }
        })
    }

    fn gen_http_access<R: Rng>(&self, rng: &mut R, tr: (u64, u64), server: Option<&str>) -> JsonValue {
        let method  = pick(rng, HTTP_METHODS);
        let path    = pick(rng, HTTP_PATHS);
        let status  = pick(rng, HTTP_STATUSES);
        let client  = rand_ipv4(rng);
        let ua      = pick(rng, USER_AGENTS);
        let bytes: u32 = rng.gen_range(64..131072);
        let ts      = self.rand_ts(rng, tr);
        let key     = format!("{method} {path}");
        let raw     = format!(
            r#"{} - - [-] "{} {} HTTP/1.1" {} {} "-" "{}""#,
            client, method, path, status, bytes, ua
        );

        let mut data = serde_json::Map::new();
        data.insert("client".into(),       json!(client));
        data.insert("method".into(),       json!(method));
        data.insert("path".into(),         json!(path));
        data.insert("status".into(),       json!(status));
        data.insert("bytes".into(),        json!(bytes.to_string()));
        data.insert("http_version".into(), json!("1.1"));
        let referrer = pick(rng, REFERRERS);
        if referrer != "-" {
            data.insert("referrer".into(), json!(referrer));
        }
        data.insert("user_agent".into(), json!(ua));
        if let Some(s) = server {
            data.insert("server".into(), json!(s));
        }
        data.insert("raw".into(), json!(raw));

        json!({ "timestamp": ts, "key": key, "data": data })
    }

    fn gen_python_traceback<R: Rng>(&self, rng: &mut R, tr: (u64, u64)) -> JsonValue {
        let exc    = pick(rng, PYTHON_EXCEPTIONS);
        let file   = pick(rng, PYTHON_FILES);
        let func   = pick(rng, PYTHON_FUNCS);
        let lineno: u32 = rng.gen_range(1..500);
        let source = pick(rng, PYTHON_SOURCES);
        let msg    = pick(rng, EXCEPTION_MESSAGES);
        let ts     = self.rand_ts(rng, tr);

        let mut frames = vec![json!({
            "file": file, "line": lineno,
            "function": func, "source": source,
        })];
        if rng.gen_bool(0.4) {
            let outer_file = pick(rng, PYTHON_FILES);
            let outer_func = pick(rng, PYTHON_FUNCS);
            let outer_line: u32 = rng.gen_range(1..500);
            let outer_src  = pick(rng, PYTHON_SOURCES);
            frames.insert(0, json!({
                "file": outer_file, "line": outer_line,
                "function": outer_func, "source": outer_src,
            }));
        }

        let raw = build_traceback_text(&frames, exc, msg);

        json!({
            "timestamp": ts, "key": exc,
            "data": {
                "exception_type": exc, "exception_message": msg,
                "frames": frames, "raw": raw,
            }
        })
    }
}

// ── Template processing ───────────────────────────────────────────────────────

/// Recursively walk a JSON value, replacing `"$placeholder"` strings with
/// random data.
fn process_template_value<R: Rng>(v: &JsonValue, rng: &mut R, tr: (u64, u64)) -> JsonValue {
    match v {
        JsonValue::String(s) if s.starts_with('$') => resolve_placeholder(s, rng, tr),
        JsonValue::Object(map) => {
            let processed = map
                .iter()
                .map(|(k, val)| (k.clone(), process_template_value(val, rng, tr)))
                .collect();
            JsonValue::Object(processed)
        }
        JsonValue::Array(arr) => {
            JsonValue::Array(arr.iter().map(|val| process_template_value(val, rng, tr)).collect())
        }
        other => other.clone(),
    }
}

/// Resolve a `$name` or `$name(args)` placeholder string to a JSON value.
fn resolve_placeholder<R: Rng>(s: &str, rng: &mut R, tr: (u64, u64)) -> JsonValue {
    let inner = &s[1..]; // strip leading `$`

    // Split into (name, args): `$name(args)` or `$name`
    let (name, args) = if let Some(paren) = inner.find('(') {
        if inner.ends_with(')') {
            (&inner[..paren], &inner[paren + 1..inner.len() - 1])
        } else {
            (inner, "")
        }
    } else {
        (inner, "")
    };

    match name {
        "timestamp" => {
            let (start, end) = tr;
            let ts = if start >= end { start } else { rng.gen_range(start..=end) };
            json!(ts)
        }
        "int" => {
            let (min, max) = split_two::<i64>(args, 0, 100);
            let v = if min >= max { min } else { rng.gen_range(min..=max) };
            json!(v)
        }
        "float" => {
            let (min, max) = split_two::<f64>(args, 0.0, 1.0);
            let v = if min >= max { min } else { round2(rng.gen_range(min..=max)) };
            json!(v)
        }
        "choice" => {
            let opts: Vec<&str> = args.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
            if opts.is_empty() { return json!(""); }
            json!(opts[rng.gen_range(0..opts.len())])
        }
        "bool" => json!(rng.gen_bool(0.5)),
        "uuid" => json!(Uuid::new_v4().to_string()),
        "ip"   => json!(rand_ipv4(rng)),
        "word" => json!(pick(rng, WORDS)),
        "name" => json!(format!("{} {}", pick(rng, FIRST_NAMES), pick(rng, LAST_NAMES))),
        // Unknown placeholder: pass through unchanged
        _ => json!(s),
    }
}

/// Parse `"min,max"` into a pair, falling back to `(default_min, default_max)`.
fn split_two<T: std::str::FromStr + Copy>(args: &str, default_min: T, default_max: T) -> (T, T) {
    let mut parts = args.splitn(2, ',');
    let min = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(default_min);
    let max = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(default_max);
    (min, max)
}

// ── Free-function helpers ─────────────────────────────────────────────────────

/// Derive `(start, end)` from a humantime duration string relative to now.
/// Falls back to a 1-hour window if the string cannot be parsed.
fn parse_duration_to_range(duration: &str) -> (u64, u64) {
    let secs = humantime::parse_duration(duration)
        .unwrap_or_else(|_| std::time::Duration::from_secs(3_600))
        .as_secs();
    let now = now_secs();
    (now.saturating_sub(secs), now)
}

fn pick<'a, R: Rng>(rng: &mut R, slice: &[&'a str]) -> &'a str {
    slice[rng.gen_range(0..slice.len())]
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn rand_ipv4<R: Rng>(rng: &mut R) -> String {
    format!(
        "{}.{}.{}.{}",
        rng.gen_range(1u8..254),
        rng.gen_range(0u8..255),
        rng.gen_range(0u8..255),
        rng.gen_range(1u8..254),
    )
}

fn syslog_ts_str(ts: u64) -> String {
    Utc.timestamp_opt(ts as i64, 0)
        .single()
        .unwrap_or_default()
        .format("%b %e %H:%M:%S")
        .to_string()
}

fn syslog_message<R: Rng>(rng: &mut R, program: &str) -> String {
    match program {
        "sshd" => {
            let user  = pick(rng, SYSLOG_USERS);
            let ip    = rand_ipv4(rng);
            let port: u16 = rng.gen_range(1024..65535);
            [
                format!("Accepted publickey for {user} from {ip} port {port}"),
                format!("Failed password for {user} from {ip} port {port}"),
                format!("session opened for user {user} by (uid=0)"),
                format!("Connection closed by {ip} port {port}"),
                format!("Invalid user {user} from {ip} port {port}"),
            ][rng.gen_range(0..5)].clone()
        }
        "nginx" | "apache2" => [
            "worker process started",
            "graceful shutdown in progress",
            "upstream server temporarily disabled",
            "no live upstreams while connecting to upstream",
        ][rng.gen_range(0..4)].to_string(),
        "postgres" => [
            "database system is ready to accept connections",
            "checkpoint complete: wrote 1024 buffers",
            "autovacuum: processing table public.events",
            "connection received: host=127.0.0.1 port=5432",
        ][rng.gen_range(0..4)].to_string(),
        "kernel" => [
            "OOM killer invoked for process with oom_score_adj 1000",
            "TCP: Possible SYN flooding on port 80. Sending cookies",
            "EXT4-fs (sda1): re-mounted. Opts: errors=remount-ro",
            "audit: rate limit exceeded",
        ][rng.gen_range(0..4)].to_string(),
        "cron" => {
            let user = pick(rng, SYSLOG_USERS);
            [
                format!("({user}) CMD (backup.sh)"),
                format!("({user}) CMD (cleanup-logs.sh)"),
                format!("({user}) RELOAD (crontabs/root)"),
            ][rng.gen_range(0..3)].clone()
        }
        _ => [
            "started", "stopped", "reloaded configuration",
            "received SIGTERM", "process exited normally",
        ][rng.gen_range(0..5)].to_string(),
    }
}

fn build_traceback_text(frames: &[JsonValue], exc: &str, msg: &str) -> String {
    let mut lines = vec!["Traceback (most recent call last):".to_string()];
    for f in frames {
        let file = f["file"].as_str().unwrap_or("unknown.py");
        let line = f["line"].as_u64().unwrap_or(1);
        let func = f["function"].as_str().unwrap_or("<module>");
        let src  = f["source"].as_str().unwrap_or("");
        lines.push(format!("  File \"{file}\", line {line}, in {func}"));
        if !src.is_empty() {
            lines.push(format!("    {src}"));
        }
    }
    lines.push(format!("{exc}: {msg}"));
    lines.join("\n")
}
