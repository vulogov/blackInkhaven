use bdslib::vm::helpers::file_helper::get_snippet;
use bdslib::vm::helpers::print_error::print_error_plain;
use bdslib::{
    bund_eval, dbpath_from_config, get_db, init_db, sync_db, LdaConfig, LogFormat,
    TelemetryTrend, TopicSummary,
};
use clap::{Parser, Subcommand};
use std::process;

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name    = "bdscli",
    about   = "BDS command-line interface",
    version,
    propagate_version = true
)]
struct Cli {
    /// Path to the hjson configuration file.
    /// Falls back to the BDS_CONFIG environment variable when omitted.
    #[arg(short, long, env = "BDS_CONFIG", global = true)]
    config: Option<String>,

    /// Suppress ANSI colour codes in error output.
    #[arg(long, global = true, default_value_t = false)]
    nocolor: bool,

    /// Log verbosity (0=env default, 1=info, 2=debug, 3=trace).
    #[arg(long, global = true, default_value_t = 0)]
    debug: u32,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Evaluate a BUND script.
    Eval {
        /// Read script from stdin.
        #[arg(long, conflicts_with_all = ["eval", "file", "url"])]
        stdin: bool,

        /// Evaluate an inline BUND expression.
        #[arg(short, long, conflicts_with_all = ["stdin", "file", "url"])]
        eval: Option<String>,

        /// Read script from a local file path.
        #[arg(short, long, conflicts_with_all = ["stdin", "eval", "url"])]
        file: Option<String>,

        /// Read script from a URL (http/https/ftp/file).
        #[arg(short, long, conflicts_with_all = ["stdin", "eval", "file"])]
        url: Option<String>,
    },

    /// Run analytical computations over a time-windowed key corpus.
    Analyze {
        #[command(subcommand)]
        mode: AnalyzeMode,
    },

    /// Generate synthetic documents and print them as JSON (or ingest into DB).
    Generate {
        /// Fraction of generated documents to re-emit as exact duplicates
        /// (same key and data, different timestamp).  Range: 0.0–1.0.
        /// E.g. 0.2 adds 20 duplicate records for every 100 generated.
        #[arg(long, default_value_t = 0.0)]
        duplicate: f64,

        #[command(subcommand)]
        mode: GenerateMode,
    },

    /// Search the document store.
    Search {
        #[command(subcommand)]
        mode: SearchMode,
    },

    /// Query documents from the store.
    Get {
        /// Time window to scan (e.g. `1h`, `30min`).
        /// When omitted all shards are scanned.
        #[arg(short, long)]
        duration: Option<String>,

        /// Return only primary records.
        #[arg(long, conflicts_with_all = ["secondary", "duplication_timestamps"])]
        primary: bool,

        /// Return secondaries for the primary given by --primary-id.
        #[arg(long, conflicts_with_all = ["primary", "duplication_timestamps"], requires = "primary_id")]
        secondary: bool,

        /// Show exact-match deduplication timestamps.
        /// Without --primary-id: list every primary that has duplicates with
        ///   its UUID, key, and the timestamps of each duplicate submission.
        /// With --primary-id: list only the duplicate timestamps for that record.
        #[arg(long, conflicts_with_all = ["primary", "secondary"])]
        duplication_timestamps: bool,

        /// UUID of the primary to scope --secondary or --duplication-timestamps.
        #[arg(long)]
        primary_id: Option<String>,
    },

    /// Flush all open shards to disk.
    Sync,

    /// Open (or create) the DB described by the config.
    /// With --new the existing DB directory is wiped first.
    Init {
        /// Remove the existing DB directory before initialising.
        #[arg(long, default_value_t = false)]
        new: bool,
    },
}

/// Log-entry format selector for `generate log`.
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum LogFormatArg {
    /// Pick a format at random for each document (default).
    Random,
    /// RFC-3164 syslog style.
    Syslog,
    /// Apache Combined Log Format.
    Http,
    /// Nginx access log format.
    #[value(name = "http-nginx")]
    HttpNginx,
    /// Python exception traceback.
    Traceback,
}

#[derive(Subcommand)]
enum GenerateMode {
    /// Generate syslog / HTTP / traceback log-entry documents.
    Log {
        /// Time window for generated timestamps (humantime, e.g. `1h`, `30min`).
        #[arg(short, long, default_value = "1h")]
        duration: String,

        /// Number of documents to generate.
        #[arg(short = 'n', long, default_value_t = 100)]
        count: usize,

        /// Log-entry format to produce (default: random per document).
        #[arg(short, long, default_value = "random")]
        format: LogFormatArg,

        /// Ingest the generated documents into the DB instead of printing them.
        #[arg(long, default_value_t = false)]
        ingest: bool,
    },

    /// Generate metric telemetry documents with dotted keys and numeric values.
    Telemetry {
        /// Time window for generated timestamps (humantime, e.g. `1h`, `30min`).
        #[arg(short, long, default_value = "1h")]
        duration: String,

        /// Number of documents to generate.
        #[arg(short = 'n', long, default_value_t = 100)]
        count: usize,

        /// Restrict output to a specific metric key (e.g. `cpu.usage`).
        /// When omitted a random metric is chosen per document.
        #[arg(short, long)]
        key: Option<String>,

        /// Ingest the generated documents into the DB instead of printing them.
        #[arg(long, default_value_t = false)]
        ingest: bool,
    },

    /// Generate a mix of telemetry and log-entry documents.
    Mixed {
        /// Time window for generated timestamps (humantime, e.g. `1h`, `30min`).
        #[arg(short, long, default_value = "1h")]
        duration: String,

        /// Number of documents to generate.
        #[arg(short = 'n', long, default_value_t = 100)]
        count: usize,

        /// Fraction of documents that are telemetry (0.0 = all logs, 1.0 = all telemetry).
        #[arg(short, long, default_value_t = 0.5)]
        ratio: f64,

        /// Ingest the generated documents into the DB instead of printing them.
        #[arg(long, default_value_t = false)]
        ingest: bool,
    },

    /// Generate documents from a custom JSON template with $placeholder substitution.
    Templated {
        /// Time window for generated timestamps (humantime, e.g. `1h`, `30min`).
        #[arg(short, long, default_value = "1h")]
        duration: String,

        /// Number of documents to generate.
        #[arg(short = 'n', long, default_value_t = 100)]
        count: usize,

        /// Inline JSON template string.
        /// Use $timestamp, $int(min,max), $float(min,max), $choice(a,b,c),
        /// $bool, $uuid, $ip, $word, $name as placeholders.
        #[arg(long, conflicts_with = "template_file")]
        template: Option<String>,

        /// Path to a file containing the JSON template.
        #[arg(long, conflicts_with = "template")]
        template_file: Option<String>,

        /// Ingest the generated documents into the DB instead of printing them.
        #[arg(long, default_value_t = false)]
        ingest: bool,
    },

    /// Generate raw RFC-3164 syslog lines parseable by `parse_syslog`.
    /// Each line is printed to stdout as plain text (not JSON).
    Syslog {
        /// Time window for generated timestamps (humantime, e.g. `1h`, `30min`).
        #[arg(short, long, default_value = "1h")]
        duration: String,

        /// Number of lines to generate.
        #[arg(short = 'n', long, default_value_t = 100)]
        count: usize,
    },

    /// Generate realistic IT operational documents and tickets for the docstore.
    ///
    /// Produces runbooks, incident tickets, post-mortems, KB articles, and
    /// change requests covering common failure modes (OOM, database overload,
    /// certificate expiry, deployment rollback, network partitions, etc.).
    /// Without --ingest the documents are printed as JSON lines; with --ingest
    /// they are stored directly in the ShardsManager docstore.
    Docs {
        /// Number of documents to generate.
        #[arg(short = 'n', long, default_value_t = 20)]
        count: usize,

        /// Document category to produce.  "all" produces a random mix.
        #[arg(long, default_value = "all")]
        doc_type: DocTypeArg,

        /// Store generated documents in the docstore instead of printing them.
        #[arg(long, default_value_t = false)]
        ingest: bool,
    },
}

/// Document-type selector for `generate docs`.
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum DocTypeArg {
    /// Random mix of all categories (default).
    All,
    /// Step-by-step operational runbooks.
    Runbook,
    /// Structured incident or bug tickets.
    Ticket,
    /// Post-incident post-mortem reports.
    Postmortem,
    /// Knowledge-base how-to articles.
    Kb,
    /// Planned change requests / maintenance windows.
    Change,
}

#[derive(Subcommand)]
enum AnalyzeMode {
    /// Compute telemetry trend statistics for a key over a time window.
    Trend {
        /// Metric key to analyse (e.g. `cpu.usage`).
        #[arg(short, long)]
        key: String,

        /// Lookback duration in humantime notation (e.g. `1h`, `30min`, `7days`).
        /// Ignored when --start and --end are both supplied.
        #[arg(short, long, default_value = "1h")]
        duration: String,

        /// Absolute window start (Unix seconds).  Must be paired with --end.
        #[arg(long, requires = "end")]
        start: Option<u64>,

        /// Absolute window end (Unix seconds).  Must be paired with --start.
        #[arg(long, requires = "start")]
        end: Option<u64>,
    },

    /// Run LDA topic modelling for a key and print the discovered keywords.
    Topics {
        /// Metric key to analyse.
        #[arg(short, long)]
        key: String,

        /// Lookback duration in humantime notation.
        /// Ignored when --start and --end are both supplied.
        #[arg(short, long, default_value = "1h")]
        duration: String,

        /// Absolute window start (Unix seconds).  Must be paired with --end.
        #[arg(long, requires = "end")]
        start: Option<u64>,

        /// Absolute window end (Unix seconds).  Must be paired with --start.
        #[arg(long, requires = "start")]
        end: Option<u64>,

        /// Number of topics.
        #[arg(long, default_value_t = 3)]
        k: usize,

        /// Number of Gibbs sampling iterations.
        #[arg(long, default_value_t = 200)]
        iters: usize,

        /// Top words extracted per topic.
        #[arg(long, default_value_t = 10)]
        top_n: usize,

        /// Dirichlet prior for document-topic distributions.
        #[arg(long, default_value_t = 0.1)]
        alpha: f64,

        /// Dirichlet prior for topic-word distributions.
        #[arg(long, default_value_t = 0.01)]
        beta: f64,

        /// RNG seed for reproducible runs.
        #[arg(long, default_value_t = 42)]
        seed: u64,
    },
}

#[derive(Subcommand)]
enum SearchMode {
    /// Full-text keyword search.
    Fts {
        /// Tantivy query string (e.g. `cpu AND usage`, `"disk full"`).
        #[arg(short, long)]
        query: String,

        /// Lookback duration in humantime notation.
        #[arg(short, long, default_value = "1h")]
        duration: String,

        /// Maximum number of results to display.
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },

    /// Semantic vector search.
    Vector {
        /// Free-form description of what you are looking for.
        #[arg(short, long)]
        query: String,

        /// Lookback duration in humantime notation.
        #[arg(short, long, default_value = "1h")]
        duration: String,

        /// Maximum number of results to display.
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    bdslib::setloglevel::setloglevel(cli.debug);

    if let Err(e) = run(cli) {
        print_error_plain(e);
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), easy_error::Error> {
    match cli.command {
        Command::Eval { stdin, eval, file, url } => {
            cmd_eval(cli.config.as_deref(), stdin, eval, file, url)
        }
        Command::Analyze { mode } => {
            setup_db(cli.config.as_deref())?;
            match mode {
                AnalyzeMode::Trend { key, duration, start, end } => {
                    cmd_trend(&key, &duration, start, end)
                }
                AnalyzeMode::Topics { key, duration, start, end, k, iters, top_n, alpha, beta, seed } => {
                    cmd_topics(&key, &duration, start, end, LdaConfig { k, iters, top_n, alpha, beta, seed })
                }
            }
        }
        Command::Generate { duplicate, mode } => {
            cmd_generate(cli.config.as_deref(), duplicate, mode)
        }
        Command::Search { mode } => {
            setup_db(cli.config.as_deref())?;
            cmd_search(mode)
        }
        Command::Get { duration, primary, secondary, duplication_timestamps, primary_id } => {
            setup_db(cli.config.as_deref())?;
            cmd_get(duration, primary, secondary, duplication_timestamps, primary_id)
        }
        Command::Sync => {
            setup_db(cli.config.as_deref())?;
            cmd_sync()
        }
        Command::Init { new } => {
            cmd_init(cli.config.as_deref(), new)
        }
    }
}

// ── shared setup ──────────────────────────────────────────────────────────────

fn setup_db(config: Option<&str>) -> Result<(), easy_error::Error> {
    init_db(config)
        .map_err(|e| easy_error::err_msg(format!("DB init failed: {e}")))?;
    bdslib::init_adam()
        .map_err(|e| easy_error::err_msg(format!("VM init failed: {e}")))?;
    bdslib::context::init(config)
        .map_err(|e| easy_error::err_msg(format!("BUND context init failed: {e}")))?;
    Ok(())
}

// ── eval ──────────────────────────────────────────────────────────────────────

fn cmd_eval(config: Option<&str>, stdin: bool, eval: Option<String>, file: Option<String>, url: Option<String>) -> Result<(), easy_error::Error> {
    bdslib::init_adam()
        .map_err(|e| easy_error::err_msg(format!("VM init failed: {e}")))?;
    bdslib::context::init(config)
        .map_err(|e| easy_error::err_msg(format!("BUND context init failed: {e}")))?;
    let snippet = match get_snippet(stdin, eval, file, url) {
        Some(s) => s,
        None => {
            return Err(easy_error::err_msg(
                "no script source: supply --stdin, --eval, --file, or --url",
            ));
        }
    };
    bund_eval(&snippet)
}

// ── trend ─────────────────────────────────────────────────────────────────────

fn cmd_trend(
    key: &str,
    duration: &str,
    start: Option<u64>,
    end: Option<u64>,
) -> Result<(), easy_error::Error> {
    let t = match (start, end) {
        (Some(s), Some(e)) => TelemetryTrend::query(key, s, e)?,
        _ => TelemetryTrend::query_window(key, duration)?,
    };

    println!("key        : {}", t.key);
    println!("window     : [{}, {})", t.start, t.end);
    println!("samples    : {}", t.n);

    if t.n == 0 {
        println!("(no data found)");
        return Ok(());
    }

    println!("min / max  : {:.6} / {:.6}", t.min, t.max);
    println!("mean       : {:.6}", t.mean);
    println!("median     : {:.6}", t.median);
    println!("std_dev    : {:.6}", t.std_dev);
    println!("variability: {:.6}  (CV)", t.variability);

    if t.anomalies.is_empty() {
        println!("anomalies  : none");
    } else {
        println!("anomalies  : {} flagged", t.anomalies.len());
        for p in &t.anomalies {
            println!("  [{}]  ts={}  value={:.6}", p.index, p.timestamp, p.value);
        }
    }

    if t.breakouts.is_empty() {
        println!("breakouts  : none");
    } else {
        println!("breakouts  : {} detected", t.breakouts.len());
        for p in &t.breakouts {
            println!("  [{}]  ts={}  value={:.6}", p.index, p.timestamp, p.value);
        }
    }

    Ok(())
}

// ── topics ────────────────────────────────────────────────────────────────────

fn cmd_topics(
    key: &str,
    duration: &str,
    start: Option<u64>,
    end: Option<u64>,
    config: LdaConfig,
) -> Result<(), easy_error::Error> {
    let s = match (start, end) {
        (Some(s), Some(e)) => TopicSummary::query(key, s, e, config)?,
        _ => TopicSummary::query_window(key, duration, config)?,
    };

    println!("key      : {}", s.key);
    println!("window   : [{}, {})", s.start, s.end);
    println!("docs     : {}", s.n_docs);
    println!("topics   : {}", s.n_topics);
    println!("keywords : {}", s.keywords);

    Ok(())
}

// ── search ────────────────────────────────────────────────────────────────────

fn cmd_search(mode: SearchMode) -> Result<(), easy_error::Error> {
    let db = get_db()?;
    match mode {
        SearchMode::Fts { query, duration, limit } => {
            let results = db.search_fts(&duration, &query)?;
            let shown = results.len().min(limit);
            println!("fts query  : {query:?}");
            println!("duration   : {duration}");
            println!("hits       : {}  (showing {})", results.len(), shown);
            for doc in results.iter().take(shown) {
                print_doc(doc);
            }
        }
        SearchMode::Vector { query, duration, limit } => {
            let q = serde_json::json!({ "data": query });
            let results = db.search_vector(&duration, &q)?;
            let shown = results.len().min(limit);
            println!("vector query : {query:?}");
            println!("duration     : {duration}");
            println!("hits         : {}  (showing {})", results.len(), shown);
            for doc in results.iter().take(shown) {
                print_doc(doc);
            }
        }
    }
    Ok(())
}

fn print_doc(doc: &serde_json::Value) {
    let key   = doc["key"].as_str().unwrap_or("?");
    let ts    = doc["timestamp"].as_u64().unwrap_or(0);
    let score = doc.get("_score").and_then(|v| v.as_f64());
    match score {
        Some(s) => println!("  [{ts}]  score={s:.4}  key={key}"),
        None    => println!("  [{ts}]  key={key}"),
    }
}

// ── generate ──────────────────────────────────────────────────────────────────

/// Return the duration string from any GenerateMode without consuming it.
fn mode_duration(mode: &GenerateMode) -> &str {
    match mode {
        GenerateMode::Log       { duration, .. } => duration,
        GenerateMode::Telemetry { duration, .. } => duration,
        GenerateMode::Mixed     { duration, .. } => duration,
        GenerateMode::Templated { duration, .. } => duration,
        GenerateMode::Syslog    { duration, .. } => duration,
        GenerateMode::Docs      { .. } => "1h", // unreachable: Docs exits early in cmd_generate
    }
}

/// Append `(docs.len() * pct).round()` duplicate records to `docs`.
/// Each duplicate copies the `key` and `data` of a randomly chosen source
/// document and receives a new timestamp within `[now - duration_secs, now]`.
fn inject_duplicates(docs: &mut Vec<serde_json::Value>, pct: f64, duration_secs: u64) {
    if pct <= 0.0 || docs.is_empty() {
        return;
    }
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let dup_count = ((docs.len() as f64) * pct.min(1.0)).round() as usize;
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let n_src = docs.len();
    for _ in 0..dup_count {
        let mut dup = docs[rng.gen_range(0..n_src)].clone();
        let ts = now_secs.saturating_sub(rng.gen_range(0..=duration_secs));
        if let Some(obj) = dup.as_object_mut() {
            obj.insert("timestamp".to_string(), serde_json::json!(ts));
        }
        docs.push(dup);
    }
}

fn cmd_generate(config: Option<&str>, duplicate: f64, mode: GenerateMode) -> Result<(), easy_error::Error> {
    if !(0.0..=1.0).contains(&duplicate) {
        return Err(easy_error::err_msg("--duplicate must be between 0.0 and 1.0"));
    }

    // Syslog emits raw RFC-3164 text lines, not JSON — handle before the JSON path.
    if let GenerateMode::Syslog { ref duration, count } = mode {
        let lines = bdslib::Generator::new().syslog_lines(duration, count);
        for line in lines {
            println!("{line}");
        }
        return Ok(());
    }

    // Docs stores into the docstore — completely separate path (no timestamp, no duplicate injection).
    if let GenerateMode::Docs { count, doc_type, ingest } = mode {
        return cmd_generate_docs(config, count, doc_type, ingest);
    }

    let duration_secs = humantime::parse_duration(mode_duration(&mode))
        .map(|d| d.as_secs())
        .unwrap_or(3600);

    let (mut docs, ingest) = match mode {
        GenerateMode::Log { duration, count, format, ingest } => {
            let fmt = match format {
                LogFormatArg::Random    => LogFormat::Random,
                LogFormatArg::Syslog    => LogFormat::Syslog,
                LogFormatArg::Http      => LogFormat::Http,
                LogFormatArg::HttpNginx => LogFormat::HttpNginx,
                LogFormatArg::Traceback => LogFormat::Traceback,
            };
            let docs = bdslib::Generator::new().with_log_format(fmt).log_entries(&duration, count);
            (docs, ingest)
        }
        GenerateMode::Telemetry { duration, count, key, ingest } => {
            let mut g = bdslib::Generator::new();
            if let Some(k) = key { g = g.with_key(k); }
            let docs = g.telemetry(&duration, count);
            (docs, ingest)
        }
        GenerateMode::Mixed { duration, count, ratio, ingest } => {
            let docs = bdslib::Generator::new().mixed(&duration, count, ratio);
            (docs, ingest)
        }
        GenerateMode::Templated { duration, count, template, template_file, ingest } => {
            let tmpl = resolve_template(template, template_file)?;
            let docs = bdslib::Generator::new().templated(&duration, &tmpl, count);
            (docs, ingest)
        }
        GenerateMode::Syslog { .. } => unreachable!(),
        GenerateMode::Docs { .. }   => unreachable!(),
    };

    inject_duplicates(&mut docs, duplicate, duration_secs);

    emit_or_ingest(config, docs, ingest)
}

fn resolve_template(
    inline: Option<String>,
    path: Option<String>,
) -> Result<String, easy_error::Error> {
    if let Some(t) = inline {
        return Ok(t);
    }
    if let Some(p) = path {
        return std::fs::read_to_string(&p)
            .map_err(|e| easy_error::err_msg(format!("cannot read template file {p:?}: {e}")));
    }
    Err(easy_error::err_msg(
        "templated: supply --template or --template-file",
    ))
}

fn emit_or_ingest(
    config: Option<&str>,
    docs: Vec<serde_json::Value>,
    ingest: bool,
) -> Result<(), easy_error::Error> {
    if ingest {
        setup_db(config)?;
        let db = get_db()?;
        let n = docs.len();
        db.add_batch(docs)
            .map_err(|e| easy_error::err_msg(format!("ingest failed: {e}")))?;
        sync_db().map_err(|e| easy_error::err_msg(format!("sync failed: {e}")))?;
        println!("ingested: {n}");
    } else {
        for doc in &docs {
            println!("{}", doc);
        }
    }
    Ok(())
}

// ── generate docs ─────────────────────────────────────────────────────────────

fn cmd_generate_docs(
    config: Option<&str>,
    count: usize,
    doc_type: DocTypeArg,
    ingest: bool,
) -> Result<(), easy_error::Error> {
    let docs = build_doc_corpus(count, doc_type);

    if ingest {
        setup_db(config)?;
        let db = bdslib::get_db()?;
        for (meta, content) in &docs {
            db.doc_add(meta.clone(), content.as_bytes())
                .map_err(|e| easy_error::err_msg(format!("doc_add failed: {e}")))?;
        }
        db.doc_sync()
            .map_err(|e| easy_error::err_msg(format!("doc_sync failed: {e}")))?;
        println!("ingested: {} documents into docstore", docs.len());
    } else {
        for (meta, content) in &docs {
            println!("{}", serde_json::json!({"metadata": meta, "content": content}));
        }
    }
    Ok(())
}

// ── document corpus generator ──────────────────────────────────────────────────

fn build_doc_corpus(count: usize, doc_type: DocTypeArg) -> Vec<(serde_json::Value, String)> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut out = Vec::with_capacity(count);

    // Weight distribution for "all" mode
    let weights: &[(DocTypeArg, u32)] = &[
        (DocTypeArg::Runbook,   35),
        (DocTypeArg::Ticket,    30),
        (DocTypeArg::Postmortem, 15),
        (DocTypeArg::Kb,        15),
        (DocTypeArg::Change,     5),
    ];
    let total_weight: u32 = weights.iter().map(|(_, w)| w).sum();

    for _ in 0..count {
        let chosen = match doc_type {
            DocTypeArg::All => {
                let roll: u32 = rng.gen_range(0..total_weight);
                let mut acc = 0u32;
                let mut picked = DocTypeArg::Runbook;
                for (t, w) in weights {
                    acc += w;
                    if roll < acc { picked = *t; break; }
                }
                picked
            }
            other => other,
        };
        let (meta, content) = match chosen {
            DocTypeArg::Runbook    => gen_runbook(&mut rng),
            DocTypeArg::Ticket     => gen_ticket(&mut rng),
            DocTypeArg::Postmortem => gen_postmortem(&mut rng),
            DocTypeArg::Kb         => gen_kb(&mut rng),
            DocTypeArg::Change     => gen_change(&mut rng),
            DocTypeArg::All        => unreachable!(),
        };
        out.push((meta, content));
    }
    out
}

// ── static data used across generators ────────────────────────────────────────

const SERVICES: &[&str] = &[
    "payment-api", "auth-service", "notification-service", "user-data-api",
    "order-processor", "inventory-service", "search-service", "recommendation-engine",
    "billing-service", "analytics-pipeline", "event-bus", "file-storage",
    "gateway", "config-service", "scheduler", "report-generator",
];

const TEAMS: &[&str] = &[
    "platform-ops", "backend-sre", "data-engineering", "security",
    "database-admin", "network-ops", "release-engineering", "ml-infra",
];

const SEVERITIES: &[(&str, &str)] = &[
    ("P1", "critical"), ("P1", "critical"), ("P2", "high"),
    ("P2", "high"), ("P3", "medium"), ("P3", "medium"), ("P4", "low"),
];

const ENVS_PROD: &[&str] = &["production", "production", "production", "staging", "dev"];

fn pick<'a, T>(rng: &mut impl rand::Rng, items: &'a [T]) -> &'a T {
    &items[rng.gen_range(0..items.len())]
}

fn ticket_id(rng: &mut impl rand::Rng) -> String {
    format!("INC-{}", rng.gen_range(10000u32..99999))
}

fn change_id(rng: &mut impl rand::Rng) -> String {
    format!("CHG-{}", rng.gen_range(1000u32..9999))
}

// ── runbook generator ──────────────────────────────────────────────────────────

struct RunbookTemplate {
    name:     &'static str,
    category: &'static str,
    topic:    &'static str,
    body:     &'static str,
}

const RUNBOOKS: &[RunbookTemplate] = &[
    RunbookTemplate {
        name: "Database Connection Pool Exhaustion",
        category: "runbook", topic: "database",
        body: "\
Symptoms: application logs show 'connection pool exhausted' or 'no connections available'; \
query latency exceeds SLA; HTTP 500/503 errors increase on endpoints that hit the database.\n\n\
Immediate triage:\n\
1. Check pg_stat_activity: SELECT count(*), state FROM pg_stat_activity GROUP BY state;\n\
2. Identify long-running queries: SELECT pid, now()-pg_stat_activity.query_start AS duration, \
query FROM pg_stat_activity WHERE state = 'active' ORDER BY duration DESC LIMIT 20;\n\
3. Kill blocking queries older than 5 minutes: SELECT pg_terminate_backend(pid) FROM \
pg_stat_activity WHERE duration > interval '5 minutes' AND state = 'active';\n\n\
Recovery: pool recovers within one pool_recycle_interval (default 30 s) after blocking queries \
are terminated. Monitor active connection count until it falls below 70 pct of max pool size. \
Temporarily reduce pool_max_size for non-critical background jobs (reconciliation, analytics) \
to free connections for user-facing handlers.\n\n\
Prevention: add a LimitRange on namespace pool allocation; alert on connections > 80 pct of max.",
    },
    RunbookTemplate {
        name: "Memory Pressure — Pod Eviction Response",
        category: "runbook", topic: "kubernetes",
        body: "\
Alert: MemoryPressure=True on node; BestEffort pods evicted; Burstable pods at risk.\n\n\
Triage steps:\n\
1. kubectl describe node <name> — check Conditions and Allocatable vs Requests/Limits.\n\
2. kubectl top pods --all-namespaces --sort-by=memory — identify top consumers.\n\
3. kubectl top node — confirm per-node real-time usage.\n\n\
Mitigation:\n\
- Cordon the pressured node: kubectl cordon <node>\n\
- Evict the top non-critical pod: kubectl delete pod <name> --grace-period=30\n\
- If multiple nodes are affected simultaneously, check for a recent Deployment rollout \
that increased memory footprint — roll it back and tighten resource requests.\n\
- Do not restart all replicas at once. Use rolling restart.\n\n\
After pressure clears: uncordon with kubectl uncordon <node>. Confirm memory usage \
stabilises below 60 pct within 5 min. Add a LimitRange to the offending namespace.",
    },
    RunbookTemplate {
        name: "SSL/TLS Certificate Renewal Procedure",
        category: "runbook", topic: "security",
        body: "\
Trigger: certificate expiry alert fires with less than 30 days remaining; or TLS handshake \
failures appear in service logs.\n\n\
Renewal steps (Let's Encrypt / ACME):\n\
1. Verify current expiry: echo | openssl s_client -connect <host>:443 2>/dev/null | \
openssl x509 -noout -dates\n\
2. Run certbot: sudo certbot renew --cert-name <domain> --dry-run  (verify first)\n\
3. If dry-run passes: sudo certbot renew --cert-name <domain>\n\
4. Reload the web server: sudo systemctl reload nginx  (or apache2)\n\
5. Verify new expiry date with the openssl command above.\n\n\
If using cert-manager in Kubernetes:\n\
kubectl describe certificate <name> -n <namespace>  # check status\n\
kubectl annotate certificate <name> -n <namespace> cert-manager.io/issue-temporary-certificate=true\n\n\
Post-renewal: update the certificate expiry date in the monitoring dashboard. \
Confirm the renewed certificate is served on all load balancer endpoints.",
    },
    RunbookTemplate {
        name: "Service Circuit Breaker — Open State Response",
        category: "runbook", topic: "resilience",
        body: "\
A circuit breaker in OPEN state stops all calls to the downstream service, failing fast \
rather than queueing requests that will time out. The breaker resets automatically on \
the probe_interval if consecutive probes succeed.\n\n\
Do not force-close the breaker manually unless explicitly authorised by the downstream \
service team. Forced closure under an ongoing incident will restart the error cascade.\n\n\
Triage:\n\
1. Check the downstream service health dashboard.\n\
2. Review error logs for the upstream service for the last 15 minutes.\n\
3. Determine if the incident is a network issue, application error, or database failure.\n\n\
Mitigation options:\n\
- If the downstream service is recovering: wait for the half-open probe cycle (default 30 s). \
The breaker closes automatically when consecutive probes succeed.\n\
- If a deployment caused the issue: roll back the deployment immediately; do not wait for a fix.\n\
- If the upstream processor is the root cause: engage the secondary failover endpoint \
via the feature flag store.\n\n\
Recovery verification: confirm error rate on the upstream endpoint is below 1 pct for a \
sustained 5-minute window before declaring the incident resolved.",
    },
    RunbookTemplate {
        name: "Redis Cache Flush and Restart",
        category: "runbook", topic: "caching",
        body: "\
Use this runbook when: cache contains stale or corrupt data causing incorrect application \
behaviour; cache memory usage exceeds the configured maxmemory limit and eviction is \
degrading performance; after a schema migration that invalidates all cached objects.\n\n\
Steps:\n\
1. Notify the on-call team that cache will be flushed — expect a temporary cache miss spike.\n\
2. Check current cache size: redis-cli INFO memory | grep used_memory_human\n\
3. Count keys: redis-cli DBSIZE\n\
4. Flush the cache: redis-cli FLUSHALL ASYNC  (ASYNC avoids blocking the event loop)\n\
5. Restart Redis if required: sudo systemctl restart redis\n\
6. Verify connection: redis-cli PING  (expect PONG)\n\n\
Post-flush monitoring:\n\
- Cache hit rate will drop to near zero immediately — this is expected.\n\
- Expect elevated database load for 5-10 minutes while the cache warms.\n\
- Monitor hit rate recovery: redis-cli INFO stats | grep keyspace_hits\n\
- Alert if hit rate does not recover above 60 pct within 30 minutes.",
    },
    RunbookTemplate {
        name: "Nginx Configuration Reload",
        category: "runbook", topic: "webserver",
        body: "\
Use when deploying a new virtual host configuration, updating upstream weights, or \
rotating TLS certificates without a full restart.\n\n\
Pre-flight:\n\
1. Validate the new configuration: sudo nginx -t\n\
   If validation fails, do NOT proceed — fix the config error first.\n\
2. Check the current worker processes: ps aux | grep nginx\n\n\
Reload (zero-downtime):\n\
sudo systemctl reload nginx\n\
# or: sudo nginx -s reload\n\n\
Verify:\n\
1. Check error log for any post-reload errors: sudo tail -20 /var/log/nginx/error.log\n\
2. Confirm new config is active: curl -I https://<host>/health\n\
3. Check worker process count matches expected value.\n\n\
Rollback: if the reload introduced a regression, revert the config file and reload again. \
Keep the previous config as <file>.bak before making changes. If nginx is in a bad state \
and reload fails, do a full restart: sudo systemctl restart nginx (brief downtime).",
    },
    RunbookTemplate {
        name: "Kubernetes Deployment Rollback",
        category: "runbook", topic: "kubernetes",
        body: "\
Trigger: post-deployment error rate spike; pod CrashLoopBackOff; health check failures; \
latency p99 exceeds SLO threshold by more than 50 pct.\n\n\
Rollback procedure:\n\
1. Confirm the deployment is the cause: check deployment timestamp vs incident start time.\n\
2. Perform rollback: kubectl rollout undo deployment/<name> -n <namespace>\n\
3. Monitor rollout: kubectl rollout status deployment/<name> -n <namespace>\n\
4. Verify pod health: kubectl get pods -n <namespace> -l app=<name>\n\n\
If rollback does not resolve the issue:\n\
- Check if the rollback deployed to a bad earlier version: \
kubectl rollout history deployment/<name>\n\
- Specify a known-good revision: kubectl rollout undo deployment/<name> --to-revision=<N>\n\n\
Post-rollback:\n\
- Confirm error rate returns to baseline within 5 min of pods becoming Ready.\n\
- Open an incident ticket with the deployment diff and the timeline.\n\
- Block the failed image tag from being promoted to production until root cause is understood.",
    },
    RunbookTemplate {
        name: "Disk Space — Emergency Cleanup",
        category: "runbook", topic: "storage",
        body: "\
Alert threshold: disk usage above 85 pct on any production volume.\n\n\
Immediate triage:\n\
1. Identify top consumers: du -sh /* 2>/dev/null | sort -rh | head -20\n\
2. Check log directory: du -sh /var/log/*\n\
3. Check for large core dumps: find / -name 'core.*' -size +100M 2>/dev/null\n\
4. Check for stale Docker images: docker system df\n\n\
Safe cleanup actions (execute in order, check free space after each):\n\
1. Rotate and compress logs: sudo logrotate -f /etc/logrotate.conf\n\
2. Remove old journal entries: sudo journalctl --vacuum-time=7d\n\
3. Remove stale Docker data: docker system prune -f  (containers, networks, dangling images)\n\
4. Remove unused Docker images: docker image prune -a --filter 'until=72h'\n\n\
Do NOT delete: database WAL files, application data directories, \
config files, or any file without confirming its purpose.\n\n\
Post-cleanup: confirm disk usage below 70 pct. Add a monitoring alert for 80 pct.",
    },
    RunbookTemplate {
        name: "On-Call Escalation Procedure",
        category: "runbook", topic: "process",
        body: "\
Use this runbook when an incident cannot be resolved within 30 minutes by the \
initial responder, or when the blast radius exceeds the scope of a single service team.\n\n\
Escalation thresholds:\n\
- P1 (critical): escalate to tech lead within 15 minutes if not mitigated.\n\
- P2 (high): escalate to senior engineer within 30 minutes.\n\
- Any incident affecting customer payment processing: escalate immediately regardless of severity.\n\n\
Escalation steps:\n\
1. Page the next level via PagerDuty: pd trigger --service <service-id> --message '<summary>'\n\
2. Post in the #incidents Slack channel: tag the service owner and on-call lead.\n\
3. Start a Zoom war-room bridge and post the link in #incidents.\n\
4. Transfer incident command: brief the incoming lead on current state, what has been tried, \
blast radius, and next investigation steps.\n\n\
Do not abandon the incident until formal handoff is acknowledged by the incoming lead.",
    },
    RunbookTemplate {
        name: "Queue Backpressure Activation",
        category: "runbook", topic: "messaging",
        body: "\
Trigger: queue depth exceeds 50,000 items; consumer lag growing faster than throughput; \
downstream processor confirmed degraded and not expected to recover within 10 minutes.\n\n\
Backpressure mechanism: when enabled, the service returns HTTP 503 with a Retry-After \
header to new incoming requests rather than enqueuing them. This caps memory growth \
and prevents OOM kills on the processing workers.\n\n\
Activation steps:\n\
1. Confirm that the backpressure feature flag is available: \
curl -s https://flags.internal/api/v1/flags/queue_backpressure\n\
2. Enable the flag: PUT https://flags.internal/api/v1/flags/queue_backpressure \
with body {\"enabled\": true, \"rollout\": 100}\n\
3. Verify that new incoming requests receive HTTP 503: \
curl -s -o /dev/null -w '%{http_code}' https://<service>/api/submit\n\
4. Monitor queue depth: it should stabilise or decrease within 2 minutes.\n\n\
Deactivation: once the downstream processor has recovered and queue depth is below \
10,000 items, disable the flag. Monitor queue depth to confirm it does not grow again.",
    },
    RunbookTemplate {
        name: "Database Failover — Promote Replica",
        category: "runbook", topic: "database",
        body: "\
Use when: primary database is unresponsive; replication lag has caused primary to be \
unreachable; physical host failure on the primary node.\n\n\
Pre-failover checklist:\n\
1. Confirm primary is actually down (not a false alarm): pg_isready -h <primary-host>\n\
2. Check replica lag: SELECT now() - pg_last_xact_replay_timestamp() AS lag; on replica.\n\
3. Alert the application and DBA teams before proceeding.\n\n\
Failover steps:\n\
1. Promote replica to primary: sudo -u postgres pg_ctl promote -D /var/lib/postgresql/data\n\
2. Update DNS/load balancer to point the database endpoint to the new primary IP.\n\
3. Update the application config or connection string if the endpoint is hardcoded.\n\
4. Verify application connectivity: check that error rates on database-dependent \
endpoints return to baseline within 5 minutes.\n\n\
Post-failover: set up a new replica from the promoted primary as soon as possible. \
Do not operate without a replica for more than 2 hours.",
    },
    RunbookTemplate {
        name: "Rate Limiter Bypass Procedure",
        category: "runbook", topic: "api",
        body: "\
Use this only for: internal tooling that legitimately requires higher throughput than \
the public rate limit; post-incident catch-up processing; authorised load tests.\n\
Do not bypass rate limiting for external clients.\n\n\
Rate limit exemption steps:\n\
1. Confirm the request is authorised by a service owner and documented in the incident ticket.\n\
2. Add the client IP or API key to the bypass allowlist:\n\
   redis-cli SADD rate_limit_bypass_set <client-id>\n\
3. Set an expiry on the bypass (maximum 4 hours):\n\
   redis-cli EXPIREAT rate_limit_bypass_set $(date -d '+4 hours' +%s)\n\
4. Document the bypass in the incident ticket with start time, end time, and justification.\n\n\
Removal: the bypass expires automatically after 4 hours. If it needs to be removed sooner:\n\
redis-cli SREM rate_limit_bypass_set <client-id>\n\n\
Review: all bypass events are logged in the security audit trail. \
The security team reviews these weekly.",
    },
];

// ── ticket generator ────────────────────────────────────────────────────────────

struct TicketTemplate {
    title_prefix: &'static str,
    topic:        &'static str,
    body:         &'static str,
}

const TICKETS: &[TicketTemplate] = &[
    TicketTemplate {
        title_prefix: "Production: database slow queries blocking payment processing",
        topic: "database",
        body: "\
Environment: production\n\
Affected service: payment-api\n\n\
Summary: p99 query latency on the payments database has exceeded 5 seconds for the \
last 45 minutes. The payment-api is timing out on checkout requests, causing HTTP 504 \
responses for approximately 12 pct of users.\n\n\
Evidence:\n\
- Datadog shows db.query_latency_ms p99 = 8,200 ms (SLO: 500 ms)\n\
- pg_stat_activity shows 14 queries waiting on lock: SELECT * FROM pg_stat_activity \
WHERE wait_event_type = 'Lock';\n\
- The lock holder is a bulk UPDATE from the analytics exporter running since 02:14 UTC.\n\n\
Immediate action taken: terminated the analytics exporter query (pid 31774). \
Query latency returning to normal.\n\n\
Root cause hypothesis: analytics exporter holds a ShareLock on the payments table \
during bulk export, blocking concurrent writes from payment-api workers.\n\n\
Follow-up: add NOWAIT or advisory lock to analytics exporter; schedule it outside \
peak transaction hours.",
    },
    TicketTemplate {
        title_prefix: "Alert: SSL certificate expiring in 14 days on api.prod",
        topic: "security",
        body: "\
Certificate details:\n\
- Host: api.prod.example.com\n\
- Issuer: Let's Encrypt Authority X3\n\
- Expiry: in 14 days\n\
- Last renewed: 76 days ago\n\n\
Impact if not renewed: TLS handshake failures for all HTTPS clients; \
health checks from the load balancer will fail; the service will become unreachable.\n\n\
Renewal procedure: see the SSL/TLS Certificate Renewal Runbook. \
The cert-manager CertificateRequest was created but the ACME challenge is failing \
because the HTTP-01 challenge path is blocked by the current WAF rule set.\n\n\
Workaround: temporarily allow HTTP-01 challenge traffic at path /.well-known/acme-challenge/* \
on the WAF before running certbot. Remove the WAF exception immediately after renewal.\n\n\
Assignee: @infra-security  Priority: P2",
    },
    TicketTemplate {
        title_prefix: "Incident: OOM kills on worker-02 and worker-03 — payment processing degraded",
        topic: "memory",
        body: "\
Start time: detected at 03:47 UTC\n\
Duration: 38 minutes\n\
Impact: payment processing degraded, approximately 12,000 transactions affected\n\n\
Sequence of events:\n\
1. Nightly ETL job submitted to analytics namespace without memory limits.\n\
2. ETL process grew to 14 GiB on a 16 GiB node, triggering kubelet eviction.\n\
3. Payment worker pods (BestEffort QoS) were evicted first.\n\
4. OOM killer fired on worker-02 and worker-03, causing restart loops.\n\
5. Circuit breaker opened on payment-api.\n\n\
Mitigation: cancelled the ETL job via kubectl delete job. \
Memory usage began dropping. Circuit breaker closed 20 minutes later.\n\n\
Root cause: missing resource limits on analytics ETL job + BestEffort QoS on payment workers.\n\n\
Action items:\n\
- [ ] Add resource limits to analytics namespace LimitRange\n\
- [ ] Set resource requests on payment-worker Deployment\n\
- [ ] Add OPA policy to reject limit-free workloads",
    },
    TicketTemplate {
        title_prefix: "Deployment failed: auth-service v2.4.1 — pods in CrashLoopBackOff",
        topic: "deployment",
        body: "\
Deployment: auth-service v2.4.1 to production\n\
Rollout started: 14:22 UTC\n\
Failure detected: 14:29 UTC\n\n\
Observed behaviour:\n\
- New pods exit with code 1 immediately after startup.\n\
- kubectl logs auth-service-7d9f8b-xxx --previous shows:\n\
  FATAL: cannot load config: SECRET_KEY_BASE is not set\n\
- The deployment diff introduced a new required environment variable SECRET_KEY_BASE \
that was not added to the Kubernetes Secret before the rollout.\n\n\
Immediate action: kubectl rollout undo deployment/auth-service\n\
Rollback completed at 14:35 UTC. All pods healthy.\n\n\
Root cause: deployment checklist did not include a step to verify all required \
env vars are present in the target environment before promoting the image.\n\n\
Fix: add SECRET_KEY_BASE to production Secret; add env-var validation step to the \
deployment runbook and CI pipeline gate.",
    },
    TicketTemplate {
        title_prefix: "Alert: replication lag on standby-db exceeded 60 seconds",
        topic: "database",
        body: "\
Alert: pg_replication_lag > 60 s on standby-db-01.prod\n\
Detected: 09:18 UTC  Duration so far: 22 minutes\n\n\
Observed metrics:\n\
- Replication lag: 87 s (threshold: 30 s)\n\
- WAL sender: active — data is flowing\n\
- Standby pg_stat_replication: streaming, sent_lsn ahead of replay_lsn by ~450 MB\n\n\
Hypothesis: standby is under heavy I/O pressure from the nightly VACUUM FULL that \
was scheduled at 09:00 UTC on the standby (separate from the primary). VACUUM FULL \
holds an exclusive lock on the tables it processes, pausing WAL replay.\n\n\
Verification: SELECT query, now()-query_start FROM pg_stat_activity WHERE state='active' \
on standby shows VACUUM FULL running on payments_archive table.\n\n\
Action: cancel VACUUM FULL on standby: SELECT pg_cancel_backend(<pid>). \
Reschedule for a lower-traffic window (03:00–05:00 UTC).\n\n\
Assignee: @db-admin",
    },
    TicketTemplate {
        title_prefix: "Network: intermittent packet loss between app tier and database tier",
        topic: "network",
        body: "\
Reported by: payment-api SRE  Severity: P2\n\n\
Symptoms:\n\
- TCP retransmit rate on eth0 of db-primary elevated at 4.2 pct (threshold: 1 pct)\n\
- Intermittent database query timeouts (not consistent — affects ~5 pct of queries)\n\
- ping from app-tier hosts to db-primary shows 0 pct packet loss but 12-18 ms jitter\n\n\
Investigation:\n\
1. MTU mismatch suspected between app-tier VMs (MTU 9001, jumbo frames) and \
database tier hosts (MTU 1500). Confirmed with: ip link show eth0 on both sides.\n\
2. Large queries with result sets > 1 packet are most affected — consistent with \
MTU fragmentation causing retransmits.\n\n\
Fix: configure MSS clamping on the security group / iptables rule between tiers:\n\
iptables -t mangle -A FORWARD -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu\n\n\
Applied at 11:34 UTC. Retransmit rate dropped to 0.1 pct within 3 minutes. \
Query timeout rate returned to baseline.",
    },
    TicketTemplate {
        title_prefix: "Autoscaler not scaling down — zombie pods consuming cluster resources",
        topic: "kubernetes",
        body: "\
Observed: HPA for recommendation-engine is reporting 0 pct CPU utilisation but \
pods are not being scaled down to the minReplicas value (2). Current replica count: 18.\n\n\
Investigation:\n\
1. kubectl describe hpa recommendation-engine shows: \
'unable to fetch metrics for resource cpu: no metrics returned from resource metrics API'\n\
2. metrics-server pod in kube-system is in CrashLoopBackOff — it has not been \
reporting metrics for 47 minutes.\n\
3. HPA scale-down is blocked: when metrics are unavailable, the HPA controller \
uses the last known value and does not scale down to avoid thrashing.\n\n\
Fix:\n\
1. Restart metrics-server: kubectl rollout restart deployment/metrics-server -n kube-system\n\
2. Wait for metrics-server to become Ready (usually 60–90 s).\n\
3. Verify HPA is receiving metrics again: kubectl describe hpa recommendation-engine\n\
4. HPA will scale down automatically once metrics are restored (scale-down stabilisation \
window: 5 minutes by default).\n\n\
Root cause: metrics-server was OOM-killed and had its restart count capped at 5 with \
an exponential backoff, delaying recovery.",
    },
    TicketTemplate {
        title_prefix: "Config drift detected: notification-service using wrong SMTP endpoint",
        topic: "configuration",
        body: "\
Detection method: automated config drift check (weekly)\n\
Service: notification-service  Environment: production\n\n\
Finding: notification-service is configured to use smtp.legacy.internal:25 but the \
approved SMTP relay endpoint is smtp.prod.internal:587 (TLS required since Q2 policy update).\n\n\
Impact: emails sent via the legacy SMTP relay are not TLS-encrypted in transit and \
are routing through a server that is scheduled for decommission on the 15th of next month. \
If the legacy server is decommissioned without this being fixed, all email notifications \
will fail silently.\n\n\
Remediation:\n\
1. Update the SMTP_HOST and SMTP_PORT values in the notification-service Secret.\n\
2. Add SMTP_TLS=true to the environment.\n\
3. Roll out the updated config: kubectl rollout restart deployment/notification-service\n\
4. Verify email delivery via the /api/v1/notify/test endpoint.\n\n\
Priority: P3 — must be resolved before the legacy SMTP server decommission date.",
    },
    TicketTemplate {
        title_prefix: "Security: API keys found in public GitHub repository",
        topic: "security",
        body: "\
Detection: GitHub secret scanning alert received at 08:14 UTC\n\
Repository: github.com/example-org/frontend-app (public)\n\
Commit: a3f8d21  File: src/config/api-keys.js  Line: 14\n\n\
Exposed credentials:\n\
- STRIPE_SECRET_KEY: sk_live_xxxx (production Stripe secret key)\n\
- SENDGRID_API_KEY: SG.xxxx (production SendGrid key)\n\n\
Immediate actions (already taken):\n\
1. Revoked both keys in the respective provider dashboards at 08:22 UTC.\n\
2. Generated replacement keys.\n\
3. Updated the production Kubernetes Secret with new keys.\n\
4. Rolled out notification-service and billing-service with updated credentials.\n\n\
Investigation: key was committed by a contractor in a local development config file \
that was incorrectly added to .gitignore. The commit has been removed from history \
via git-filter-repo.\n\n\
Follow-up: add pre-commit hook for secret scanning to all repositories; \
rotate secrets on a 90-day schedule.",
    },
    TicketTemplate {
        title_prefix: "Performance: search-service query latency degraded after index rebuild",
        topic: "performance",
        body: "\
Detected: 16:05 UTC  Service: search-service  Environment: production\n\n\
Symptom: after the scheduled weekly Elasticsearch index rebuild completed at 15:50 UTC, \
p99 search latency increased from 120 ms to 1,450 ms. Hit rate on the result cache \
dropped from 78 pct to 12 pct.\n\n\
Root cause: index rebuild produced a new index name (search_v7_20241115). The \
search-service is configured to use an alias (search_alias) that was not updated \
to point to the new index. Queries are still hitting search_v7_20241108 (old index) \
which was not refreshed during the rebuild, causing stale/slow results and cache misses.\n\n\
Fix:\n\
1. Update the alias: POST /_aliases with actions to remove search_v7_20241108 and add search_v7_20241115.\n\
2. Verify: GET /_alias/search_alias should return only the new index.\n\
3. Cache hit rate should recover within 15 minutes as the new index warms.\n\n\
Prevention: add alias update as a mandatory step in the index rebuild runbook; \
add a monitor that alerts if the alias target is more than 7 days old.",
    },
];

// ── post-mortem generator ───────────────────────────────────────────────────────

struct PostmortemTemplate {
    title: &'static str,
    topic: &'static str,
    body:  &'static str,
}

const POSTMORTEMS: &[PostmortemTemplate] = &[
    PostmortemTemplate {
        title: "Post-Mortem: Database Outage — Payment Service Unavailable 47 Minutes",
        topic: "database",
        body: "\
Executive Summary: A misconfigured connection pool setting deployed in a routine config \
update caused all database connections to be exhausted within 90 seconds of deployment, \
resulting in 47 minutes of payment service unavailability affecting approximately 8,400 users.\n\n\
Timeline:\n\
T+0  Config update deployed to payment-api (pool_max_size reduced from 50 to 5 — intended for staging).\n\
T+2  Connection pool exhausted. Payment requests begin failing with HTTP 503.\n\
T+8  On-call engineer paged. Initial investigation focuses on database server health.\n\
T+22 Config diff review identifies pool_max_size change as root cause.\n\
T+25 Rollback deployed. Connection pool recovers immediately.\n\
T+47 All metrics back to baseline. Incident closed.\n\n\
Root Cause: The staging config value for pool_max_size (5) was accidentally applied to \
the production deployment manifest during a copy-paste error in the CI pipeline configuration.\n\n\
Corrective Actions:\n\
- Add config diff review to the deployment checklist for pool-related settings.\n\
- Add a pre-deployment gate that rejects pool_max_size values below a minimum threshold.\n\
- Separate staging and production config repositories.",
    },
    PostmortemTemplate {
        title: "Post-Mortem: Full Disk on Logging Host Caused 3-Hour Log Gap",
        topic: "storage",
        body: "\
Executive Summary: The primary log aggregation host ran out of disk space due to \
log rotation misconfiguration introduced 6 weeks prior. Log ingestion stopped for \
3 hours and 14 minutes, creating a gap in the audit trail that required forensic \
reconstruction.\n\n\
Timeline:\n\
T-6 weeks  Log rotation config updated to increase retention from 7 to 30 days without \
           increasing disk allocation.\n\
T+0  Disk usage reaches 100 pct on /var/log volume. Fluentd cannot write new log files.\n\
T+45 min  Alert fires for missing log metrics in Datadog (log count dropped to 0).\n\
T+1h 20m  On-call engineer identifies full disk and begins manual log rotation.\n\
T+3h 14m  Log volume freed. Fluentd resumes. Gap reconstructed from application-side \
           in-memory buffers where available.\n\n\
Root Cause: The 30-day retention change increased required disk space by 4x. \
No capacity check was performed before the change. The disk usage alert threshold was \
set at 95 pct but disk was already at 88 pct before the retention increase.\n\n\
Corrective Actions:\n\
- Lower disk usage alert to 80 pct.\n\
- Add a pre-flight capacity check to log retention config changes.\n\
- Use log compression (gzip) for archives older than 3 days.",
    },
    PostmortemTemplate {
        title: "Post-Mortem: Kubernetes Cluster Upgrade Caused 22-Minute Service Disruption",
        topic: "kubernetes",
        body: "\
Executive Summary: A Kubernetes control plane upgrade from 1.26 to 1.27 removed a \
deprecated beta API (networking.k8s.io/v1beta1) that was still referenced by three \
production Ingress objects. The Ingress controller failed to load the objects after \
the upgrade, making all three services unreachable for 22 minutes.\n\n\
Timeline:\n\
T+0  Control plane upgrade to 1.27 completed.\n\
T+3  nginx-ingress-controller pod restarts with error: 'no kind is registered for the \
     type Ingress in group networking.k8s.io/v1beta1'.\n\
T+7  Alert fires: health checks failing on three production services.\n\
T+17 Root cause identified: Ingress manifests using deprecated API version.\n\
T+22 Manifests updated to networking.k8s.io/v1. Services recover.\n\n\
Root Cause: API version deprecation checks in the pre-upgrade validation script only \
scanned the kube-system namespace. The three affected Ingress objects were in the \
payments and notifications namespaces, which were not scanned.\n\n\
Corrective Actions:\n\
- Update the pre-upgrade validation script to scan all namespaces.\n\
- Add a CI gate that fails if deprecated API versions are referenced in any manifest.\n\
- Schedule control plane upgrades during the 02:00–06:00 UTC low-traffic window.",
    },
    PostmortemTemplate {
        title: "Post-Mortem: CDN Misconfiguration Served Stale Auth Tokens for 2 Hours",
        topic: "security",
        body: "\
Executive Summary: A CDN cache TTL misconfiguration caused authentication tokens to be \
cached and served to wrong users for 2 hours and 8 minutes, affecting up to 340 user \
sessions. All affected sessions were invalidated as an emergency measure.\n\n\
Timeline:\n\
T+0  CDN caching rules updated to improve performance on /api/v1/auth endpoints.\n\
T+12 First user reports receiving another user's session data.\n\
T+31 Security team confirms: CDN is caching /api/v1/auth/refresh responses (which \
     contain JWT tokens) because the response was missing a Cache-Control: no-store header.\n\
T+45 CDN caching for /api/v1/auth/* purged and disabled.\n\
T+2h 8m All auth tokens issued during the window revoked. Users prompted to re-authenticate.\n\n\
Root Cause: The new CDN caching rule matched on path prefix /api/v1/ and overrode \
the existing /api/v1/auth/* no-cache rule due to rule priority ordering. \
The auth endpoints did not set Cache-Control: no-store in the application layer as a \
defence-in-depth measure.\n\n\
Corrective Actions:\n\
- Add Cache-Control: no-store, no-cache to all authentication endpoints at the \
application layer.\n\
- Add CDN rule ordering tests to the integration test suite.\n\
- Notify affected users per the breach notification policy.",
    },
    PostmortemTemplate {
        title: "Post-Mortem: Message Queue Poison Pill Caused Consumer Group Stall",
        topic: "messaging",
        body: "\
Executive Summary: A single malformed message (poison pill) entered the payment \
processing Kafka topic. Every consumer in the group failed to deserialise it, \
triggering the retry policy. After exhausting retries, the consumer offset was not \
advanced, stalling the entire partition for 1 hour and 53 minutes.\n\n\
Timeline:\n\
T+0  A payment event was published with an incorrect schema (missing the 'currency' field) \
     due to a client-side validation regression introduced in v3.1.0 of the publisher library.\n\
T+4  All three consumer instances log deserialization errors on offset 1,847,293.\n\
T+12 Consumer lag alert fires: lag growing at 800 msg/min.\n\
T+1h Alert acknowledged. Engineer identifies poison pill from consumer logs.\n\
T+1h 14m Message skipped via kafka-consumer-groups.sh --reset-offsets.\n\
T+1h 53m Consumer lag cleared. Normal processing resumed.\n\n\
Root Cause: Publisher library v3.1.0 removed the 'currency' field from the \
PaymentEvent schema without a corresponding consumer schema update. The consumer \
used strict deserialization with no tolerance for missing fields.\n\n\
Corrective Actions:\n\
- Add schema registry enforcement (Confluent Schema Registry) to reject schema-incompatible messages at publish time.\n\
- Implement a dead-letter queue for messages that fail deserialization after N retries.",
    },
];

// ── KB article generator ────────────────────────────────────────────────────────

struct KbTemplate {
    title: &'static str,
    topic: &'static str,
    body:  &'static str,
}

const KB_ARTICLES: &[KbTemplate] = &[
    KbTemplate {
        title: "How to Check and Resolve Pod CrashLoopBackOff in Kubernetes",
        topic: "kubernetes",
        body: "\
A pod in CrashLoopBackOff is restarting repeatedly. Kubernetes applies an exponential \
backoff (10s, 20s, 40s, up to 5 minutes) between restart attempts.\n\n\
Diagnosis commands:\n\
kubectl describe pod <name> -n <namespace>  # check Last State and Events\n\
kubectl logs <pod> --previous               # logs from the crashed container\n\
kubectl get events -n <namespace> --sort-by=.metadata.creationTimestamp\n\n\
Common exit codes and their meaning:\n\
- Exit 0: container exited cleanly — check if it is a one-shot job or a probe misconfiguration.\n\
- Exit 1: application error — read the previous logs for the actual error message.\n\
- Exit 137: OOM kill (SIGKILL sent by the kernel). Increase memory limits.\n\
- Exit 143: SIGTERM — container did not shut down within terminationGracePeriodSeconds.\n\
- Exit 255: container could not start — often a missing environment variable or bad image.\n\n\
Quick fixes:\n\
- Missing ConfigMap or Secret key: verify with kubectl get secret/configmap and check all keys.\n\
- OOM: kubectl patch deployment <name> -p '{\"spec\":{\"template\":{\"spec\":{\"containers\":[{\"name\":\"<n>\",\"resources\":{\"limits\":{\"memory\":\"512Mi\"}}}]}}}}'.\n\
- Image pull failure: kubectl describe pod shows ImagePullBackOff — verify image name and tag.",
    },
    KbTemplate {
        title: "Debugging Slow PostgreSQL Queries — Step-by-Step Guide",
        topic: "database",
        body: "\
Use this guide when database query latency is elevated but no obvious cause is visible.\n\n\
Step 1 — identify currently running slow queries:\n\
SELECT pid, now()-query_start AS duration, state, query\n\
FROM pg_stat_activity\n\
WHERE state = 'active' AND now()-query_start > interval '1 second'\n\
ORDER BY duration DESC;\n\n\
Step 2 — check for lock waits:\n\
SELECT pid, wait_event_type, wait_event, query\n\
FROM pg_stat_activity\n\
WHERE wait_event_type IS NOT NULL;\n\n\
Step 3 — inspect the query plan for a slow query:\n\
EXPLAIN (ANALYZE, BUFFERS) <slow query here>;\n\
Look for: Seq Scan on large tables (missing index), high actual rows vs estimated rows \
(stale statistics), nested loop joins with large outer input.\n\n\
Step 4 — update statistics if estimates are wrong:\n\
ANALYZE <table_name>;\n\n\
Step 5 — check index usage:\n\
SELECT schemaname, tablename, indexname, idx_scan\n\
FROM pg_stat_user_indexes\n\
WHERE idx_scan = 0 ORDER BY tablename;\n\
Unused indexes waste write performance. Consider dropping them after confirming they are not \
used for uniqueness constraints.\n\n\
Step 6 — check for bloat:\n\
SELECT relname, n_live_tup, n_dead_tup, last_autovacuum\n\
FROM pg_stat_user_tables ORDER BY n_dead_tup DESC LIMIT 20;\n\
If n_dead_tup is large: VACUUM ANALYZE <table>;",
    },
    KbTemplate {
        title: "Understanding Kubernetes Resource Requests and Limits",
        topic: "kubernetes",
        body: "\
Resource requests tell the scheduler how much CPU/memory to reserve for a pod. \
Resource limits tell the kernel the maximum it is allowed to use. Understanding the \
difference prevents two of the most common production incidents: OOM kills and CPU throttling.\n\n\
CPU:\n\
- Request: guaranteed CPU time allocated by the scheduler. 1 CPU = 1000m (millicores).\n\
- Limit: maximum CPU time. If the container exceeds its limit it is throttled (not killed).\n\
- Rule of thumb: set limit to 2-4x the request for burst headroom.\n\n\
Memory:\n\
- Request: reserved by the scheduler. Affects QoS class.\n\
- Limit: hard cap. If the container exceeds its limit it is OOM-killed (exit 137).\n\
- Rule of thumb: set limit to 1.2-1.5x the observed peak. Leaves no burst headroom — \
size requests and limits based on profiling, not guesses.\n\n\
QoS classes:\n\
- Guaranteed: requests == limits for all resources. Highest eviction priority protection.\n\
- Burstable: requests < limits (or only some resources have limits set).\n\
- BestEffort: no requests or limits set. Evicted first under memory pressure.\n\n\
For production services always set requests and limits. For batch jobs: set memory limits \
to prevent runaway consumption; leave CPU limit unset to allow burst.",
    },
    KbTemplate {
        title: "How to Rotate Database Credentials Without Downtime",
        topic: "database",
        body: "\
This procedure ensures zero-downtime credential rotation by running old and new \
credentials in parallel during the transition window.\n\n\
Step 1 — create the new credentials:\n\
CREATE USER app_user_v2 WITH PASSWORD 'new-secure-password';\n\
GRANT ALL PRIVILEGES ON DATABASE app_db TO app_user_v2;\n\
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO app_user_v2;\n\
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO app_user_v2;\n\n\
Step 2 — update the application to use the new credentials:\n\
- Update the Kubernetes Secret: kubectl create secret generic db-creds \\\n\
  --from-literal=DB_USER=app_user_v2 \\\n\
  --from-literal=DB_PASSWORD='new-secure-password' \\\n\
  --dry-run=client -o yaml | kubectl apply -f -\n\
- Rolling restart: kubectl rollout restart deployment/<app-name>\n\
- Verify: check application logs for successful database connections.\n\n\
Step 3 — verify old user is no longer connected:\n\
SELECT usename, count(*) FROM pg_stat_activity GROUP BY usename;\n\
Confirm app_user_v1 connection count is 0.\n\n\
Step 4 — remove the old user:\n\
REVOKE ALL ON DATABASE app_db FROM app_user_v1;\n\
DROP USER app_user_v1;\n\n\
Rollback: if the rollout fails, kubectl rollout undo deployment/<app-name> to revert \
to the old credential secret.",
    },
    KbTemplate {
        title: "Kafka Consumer Lag — Diagnosis and Resolution",
        topic: "messaging",
        body: "\
Consumer lag is the number of messages in a partition that have been produced but not \
yet consumed. Growing lag means your consumers cannot keep up with the producer rate.\n\n\
Measuring lag:\n\
kafka-consumer-groups.sh --bootstrap-server <host>:9092 \\\n\
  --describe --group <consumer-group-name>\n\
Columns: TOPIC, PARTITION, CURRENT-OFFSET, LOG-END-OFFSET, LAG, CONSUMER-ID\n\n\
Common causes:\n\
1. Consumer processing is too slow: a single slow downstream call (e.g. database insert) \
   blocks message processing. Profile the consumer loop; add async processing or batching.\n\
2. Insufficient consumer instances: increase the replica count up to the number of partitions.\n\
   More replicas than partitions does not help — extra replicas are idle.\n\
3. Poison pill: a single undeserializable message blocks a partition.\n\
   Check consumer logs for repeated deserialization errors on the same offset.\n\
4. GC pressure: long GC pauses cause the consumer to fail to commit offsets in time,\n\
   triggering rebalances. Add -Xmx and -Xms JVM flags; tune GC.\n\n\
Resetting offsets (use with care — skips unprocessed messages):\n\
kafka-consumer-groups.sh --bootstrap-server <host>:9092 \\\n\
  --group <group> --topic <topic> --reset-offsets --to-latest --execute",
    },
    KbTemplate {
        title: "Setting Up Prometheus Alerting Rules — Best Practices",
        topic: "observability",
        body: "\
Good alerting rules fire for customer-impacting conditions, not for internal metrics \
that may be noisy or normal. These guidelines reduce alert fatigue.\n\n\
Rule structure:\n\
groups:\n\
  - name: service-slo\n\
    rules:\n\
      - alert: HighErrorRate\n\
        expr: rate(http_requests_total{status=~'5..'}[5m]) / rate(http_requests_total[5m]) > 0.01\n\
        for: 5m\n\
        labels:\n\
          severity: critical\n\
        annotations:\n\
          summary: 'Error rate > 1% for {{ $labels.service }}'\n\n\
Best practices:\n\
- Use 'for' duration to avoid flapping on transient spikes. 1m for P1, 5m for P2.\n\
- Alert on symptoms (error rate, latency, availability) not causes (CPU %).\n\
- Every alert must have a runbook link in annotations.runbook_url.\n\
- Test alert rules with promtool check rules <file.yaml>.\n\
- Use 'without' rather than 'by' in aggregations to avoid cardinality explosions.\n\
- Inhibit lower-severity alerts when a higher-severity alert is already firing for \n\
  the same service — reduces noise during an incident.",
    },
];

// ── change request generator ────────────────────────────────────────────────────

struct ChangeTemplate {
    title:  &'static str,
    topic:  &'static str,
    body:   &'static str,
}

const CHANGES: &[ChangeTemplate] = &[
    ChangeTemplate {
        title: "Upgrade PostgreSQL 14 to PostgreSQL 16 — Production Database Cluster",
        topic: "database",
        body: "\
Change type: standard  Risk: medium  Downtime: < 5 minutes (failover window)\n\n\
Motivation: PostgreSQL 14 reaches end-of-life in November 2025. Version 16 provides \
improved query planner performance (estimated 15-20 pct improvement on analytical queries), \
logical replication improvements, and security patches.\n\n\
Preparation:\n\
1. Upgrade the staging database cluster first and run the full integration test suite.\n\
2. Verify all application ORM versions are compatible with PostgreSQL 16.\n\
3. Review pg_upgrade documentation for any breaking changes.\n\
4. Create a full pg_dump backup immediately before the upgrade window.\n\n\
Upgrade procedure:\n\
1. Upgrade standby replica first (no traffic impact).\n\
2. Test replication is working on the upgraded standby.\n\
3. Perform controlled failover: promote standby, redirect read/write endpoint.\n\
4. Upgrade the now-standby (former primary) in-place.\n\
5. Verify replication is re-established.\n\n\
Rollback: if issues are found within 30 minutes of failover, fail back to the original primary (still on PG14).\n\n\
Scheduled window: Sunday 03:00–05:00 UTC",
    },
    ChangeTemplate {
        title: "Scale Worker Fleet from 4 to 8 Nodes Ahead of Peak Season",
        topic: "infrastructure",
        body: "\
Change type: standard  Risk: low  Downtime: none\n\n\
Business justification: historical data shows a 2.3x traffic spike during the upcoming \
peak season (Black Friday to Christmas). Current 4-node fleet will be saturated at \
1.8x normal traffic based on load test results.\n\n\
Implementation:\n\
1. Update the worker Deployment replicas from 4 to 8 in the production manifest.\n\
2. Apply: kubectl scale deployment/order-processor --replicas=8\n\
3. Monitor pod startup: kubectl rollout status deployment/order-processor\n\
4. Verify all 8 pods are passing readiness probes before considering the change complete.\n\n\
Cost impact: 4 additional m5.2xlarge instances ($0.384/hr each) = $1.54/hr additional cost \
for the duration of the scale-up period.\n\n\
Scale-down: schedule a follow-up change request for January to return to 4 replicas \
after peak season.\n\n\
Rollback: kubectl scale deployment/order-processor --replicas=4",
    },
    ChangeTemplate {
        title: "Migrate Internal Service Communication to mTLS via Istio",
        topic: "security",
        body: "\
Change type: major  Risk: high  Downtime: possible brief disruption during sidecar injection\n\n\
Motivation: audit finding — inter-service traffic inside the cluster is currently \
unencrypted. Compliance requires encrypted internal communication by end of Q2.\n\n\
Rollout plan (staged to minimise risk):\n\
Phase 1 (week 1): install Istio control plane in permissive mode. Sidecars injected \
but mTLS not enforced. Verify all services continue to operate normally.\n\
Phase 2 (week 2): enable STRICT mTLS policy for non-production namespaces. \
Run integration tests.\n\
Phase 3 (week 3): enable STRICT mTLS for production namespaces during a maintenance window.\n\
Phase 4 (week 4): remove plain-text service mesh exceptions. Monitor for certificate errors.\n\n\
Risks:\n\
- Services that do not support mTLS client authentication will fail in Phase 3.\n\
- Certificate rotation must be automated via cert-manager before Phase 3.\n\n\
Rollback: set PeerAuthentication to PERMISSIVE mode and redeploy affected services.",
    },
];

// ── per-type generators ────────────────────────────────────────────────────────

fn gen_runbook(rng: &mut impl rand::Rng) -> (serde_json::Value, String) {
    let t  = pick(rng, RUNBOOKS);
    let svc = pick(rng, SERVICES);
    let team = pick(rng, TEAMS);
    let (sev, _) = pick(rng, SEVERITIES);
    let env = pick(rng, ENVS_PROD);
    let meta = serde_json::json!({
        "name":     t.name,
        "category": t.category,
        "topic":    t.topic,
        "service":  svc,
        "team":     team,
        "severity": sev,
        "env":      env,
    });
    (meta, t.body.to_string())
}

fn gen_ticket(rng: &mut impl rand::Rng) -> (serde_json::Value, String) {
    let t    = pick(rng, TICKETS);
    let svc  = pick(rng, SERVICES);
    let team = pick(rng, TEAMS);
    let (sev, pri) = pick(rng, SEVERITIES);
    let env  = pick(rng, ENVS_PROD);
    let id   = ticket_id(rng);
    let meta = serde_json::json!({
        "name":     format!("[{}] {}", id, t.title_prefix),
        "category": "ticket",
        "topic":    t.topic,
        "ticket_id": id,
        "service":  svc,
        "team":     team,
        "severity": sev,
        "priority": pri,
        "status":   "open",
        "env":      env,
    });
    (meta, t.body.to_string())
}

fn gen_postmortem(rng: &mut impl rand::Rng) -> (serde_json::Value, String) {
    let t    = pick(rng, POSTMORTEMS);
    let svc  = pick(rng, SERVICES);
    let team = pick(rng, TEAMS);
    let meta = serde_json::json!({
        "name":     t.title,
        "category": "postmortem",
        "topic":    t.topic,
        "service":  svc,
        "team":     team,
        "status":   "closed",
    });
    (meta, t.body.to_string())
}

fn gen_kb(rng: &mut impl rand::Rng) -> (serde_json::Value, String) {
    let t    = pick(rng, KB_ARTICLES);
    let team = pick(rng, TEAMS);
    let meta = serde_json::json!({
        "name":     t.title,
        "category": "kb",
        "topic":    t.topic,
        "team":     team,
    });
    (meta, t.body.to_string())
}

fn gen_change(rng: &mut impl rand::Rng) -> (serde_json::Value, String) {
    let t    = pick(rng, CHANGES);
    let team = pick(rng, TEAMS);
    let id   = change_id(rng);
    let env  = pick(rng, ENVS_PROD);
    let meta = serde_json::json!({
        "name":      format!("[{}] {}", id, t.title),
        "category":  "change-request",
        "topic":     t.topic,
        "change_id": id,
        "team":      team,
        "env":       env,
        "status":    "approved",
    });
    (meta, t.body.to_string())
}

// ── sync ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

fn cmd_sync() -> Result<(), easy_error::Error> {
    sync_db()?;
    println!("sync: OK");
    Ok(())
}

// ── init ──────────────────────────────────────────────────────────────────────

fn cmd_init(config: Option<&str>, recreate: bool) -> Result<(), easy_error::Error> {
    if recreate {
        let dbpath = dbpath_from_config(config)
            .map_err(|e| easy_error::err_msg(format!("config error: {e}")))?;
        let p = std::path::Path::new(&dbpath);
        if p.exists() {
            std::fs::remove_dir_all(p)
                .map_err(|e| easy_error::err_msg(format!("cannot remove {dbpath:?}: {e}")))?;
            println!("removed: {dbpath}");
        }
    }
    setup_db(config)?;
    println!("init: OK");
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

fn cmd_get(
    duration: Option<String>,
    primary: bool,
    secondary: bool,
    duplication_timestamps: bool,
    primary_id: Option<String>,
) -> Result<(), easy_error::Error> {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    // Runtime validation: --primary-id is only meaningful with --secondary or
    // --duplication-timestamps.
    if primary_id.is_some() && !secondary && !duplication_timestamps {
        return Err(easy_error::err_msg(
            "--primary-id requires --secondary or --duplication-timestamps",
        ));
    }

    let db = get_db()?;
    let cache = db.cache();
    let info = cache.info();

    // ── duplication-timestamps mode ───────────────────────────────────────────
    if duplication_timestamps {
        let all_infos = info.list_all()
            .map_err(|e| easy_error::err_msg(format!("catalog error: {e}")))?;

        if let Some(ref raw) = primary_id {
            // Scoped to one primary
            let pid = Uuid::parse_str(raw)
                .map_err(|e| easy_error::err_msg(format!("invalid UUID {raw:?}: {e}")))?;

            let mut found = false;
            for si in &all_infos {
                let shard = cache.shard(si.start_time)
                    .map_err(|e| easy_error::err_msg(format!("shard error: {e}")))?;
                let times = shard.observability()
                    .get_duplicate_timestamps_by_id(pid)
                    .map_err(|e| easy_error::err_msg(format!("query error: {e}")))?;
                if !times.is_empty() {
                    let ts_list: Vec<u64> = times.iter()
                        .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
                        .collect();
                    println!("{}", serde_json::json!({
                        "primary_id": raw,
                        "duplicate_timestamps": ts_list,
                    }));
                    found = true;
                    break;
                }
            }
            if !found {
                log::debug!("no duplicate timestamps for {raw}");
            }
        } else {
            // All primaries across all shards that have duplicates
            let mut total = 0usize;
            for si in &all_infos {
                let shard = cache.shard(si.start_time)
                    .map_err(|e| easy_error::err_msg(format!("shard error: {e}")))?;
                let entries = shard.observability()
                    .list_all_dedup_entries()
                    .map_err(|e| easy_error::err_msg(format!("query error: {e}")))?;
                for (id, key, times) in entries {
                    let ts_list: Vec<u64> = times.iter()
                        .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
                        .collect();
                    println!("{}", serde_json::json!({
                        "primary_id": id.to_string(),
                        "key": key,
                        "duplicate_timestamps": ts_list,
                    }));
                    total += 1;
                }
            }
            log::debug!("dedup entries: {total}");
        }
        return Ok(());
    }

    // ── secondary mode ────────────────────────────────────────────────────────
    if secondary {
        let raw = primary_id.unwrap();
        let pid = Uuid::parse_str(&raw)
            .map_err(|e| easy_error::err_msg(format!("invalid UUID {raw:?}: {e}")))?;

        let all_infos = info.list_all()
            .map_err(|e| easy_error::err_msg(format!("catalog error: {e}")))?;

        for si in &all_infos {
            let shard = cache.shard(si.start_time)
                .map_err(|e| easy_error::err_msg(format!("shard error: {e}")))?;
            let obs = shard.observability();

            if obs.is_primary(pid).unwrap_or(false) {
                let sec_ids = obs.list_secondaries(pid)
                    .map_err(|e| easy_error::err_msg(format!("query error: {e}")))?;
                let n = sec_ids.len();
                for sid in sec_ids {
                    if let Some(doc) = obs.get_by_id(sid)
                        .map_err(|e| easy_error::err_msg(format!("fetch error: {e}")))?
                    {
                        println!("{doc}");
                    }
                }
                log::debug!("secondaries: {n}");
                return Ok(());
            }
        }
        return Err(easy_error::err_msg(format!("primary {raw} not found in any shard")));
    }

    // ── primary / all-records modes ───────────────────────────────────────────
    let (shard_infos, start_opt, end_opt) = if let Some(ref d) = duration {
        let secs = humantime::parse_duration(d)
            .map_err(|e| easy_error::err_msg(format!("invalid duration {d:?}: {e}")))?
            .as_secs();
        let end = SystemTime::now();
        let start = end - Duration::from_secs(secs);
        let si = info.shards_in_range(start, end)
            .map_err(|e| easy_error::err_msg(format!("catalog error: {e}")))?;
        (si, Some(start), Some(end))
    } else {
        let si = info.list_all()
            .map_err(|e| easy_error::err_msg(format!("catalog error: {e}")))?;
        (si, None, None)
    };

    let mut total = 0usize;
    for si in &shard_infos {
        let shard = cache.shard(si.start_time)
            .map_err(|e| easy_error::err_msg(format!("shard error: {e}")))?;
        let obs = shard.observability();

        let ids: Vec<Uuid> = if primary {
            match (start_opt, end_opt) {
                (Some(s), Some(e)) => obs.list_primaries_in_range(s, e),
                _ => obs.list_primaries(),
            }
        } else {
            // use UNIX_EPOCH sentinel as "all time" lower bound when no duration given
            let range_start = start_opt.unwrap_or(UNIX_EPOCH);
            let range_end   = end_opt.unwrap_or(si.end_time);
            obs.list_ids_by_time_range(range_start, range_end)
        }.map_err(|e| easy_error::err_msg(format!("query error: {e}")))?;

        for id in ids {
            if let Some(doc) = obs.get_by_id(id)
                .map_err(|e| easy_error::err_msg(format!("fetch error: {e}")))?
            {
                println!("{doc}");
                total += 1;
            }
        }
    }
    log::debug!("total: {total}");
    Ok(())
}
