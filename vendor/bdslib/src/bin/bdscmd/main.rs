mod client;
mod cmd;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "bdscmd", about = "bdsnode JSON-RPC 2.0 client")]
struct Cli {
    /// bdsnode address (host:port or full URL)
    #[arg(short, long, default_value = "http://127.0.0.1:9000", env = "BDSCMD_ADDR")]
    address: String,

    /// Session UUID (auto-generated if omitted)
    #[arg(short, long, env = "BDSCMD_SESSION")]
    session: Option<String>,

    /// Print raw JSON result without pretty-printing
    #[arg(short, long)]
    raw: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show live node status (no server check performed)
    Status(cmd::status::Cmd),

    /// Ingest a single telemetry document
    Add(cmd::add::Cmd),

    /// Ingest a batch of telemetry documents
    AddBatch(cmd::add_batch::Cmd),

    /// Queue an NDJSON file for ingestion
    AddFile(cmd::add_file::Cmd),

    /// Queue a syslog (RFC 3164) file for ingestion
    AddFileSyslog(cmd::add_file_syslog::Cmd),

    /// Show the timestamp range of all stored data
    Timeline(cmd::timeline::Cmd),

    /// Count stored records
    Count(cmd::count::Cmd),

    /// List shards
    Shards(cmd::shards::Cmd),

    /// List distinct keys in a time window
    Keys(cmd::keys::Cmd),

    /// List all keys matching a pattern
    KeysAll(cmd::keys_all::Cmd),

    /// Get primaries for keys matching a pattern
    KeysGet(cmd::keys_get::Cmd),

    /// Count primaries in a time window
    Primaries(cmd::primaries::Cmd),

    /// Explore primaries grouped by key
    PrimariesExplore(cmd::primaries_explore::Cmd),

    /// Explore telemetry primaries grouped by key
    PrimariesExploreTelemetry(cmd::primaries_explore_telemetry::Cmd),

    /// Get primaries for a specific key
    PrimariesGet(cmd::primaries_get::Cmd),

    /// Get telemetry primaries for a specific key
    PrimariesGetTelemetry(cmd::primaries_get_telemetry::Cmd),

    /// Fetch a single primary record by UUID
    Primary(cmd::primary::Cmd),

    /// List secondaries for a primary UUID
    Secondaries(cmd::secondaries::Cmd),

    /// Fetch a single secondary record by UUID
    Secondary(cmd::secondary::Cmd),

    /// List duplicate records in a time window
    Duplicates(cmd::duplicates::Cmd),

    /// Full-text search (returns IDs and BM25 scores)
    Fulltext(cmd::fulltext::Cmd),

    /// Full-text search returning full documents
    FulltextGet(cmd::fulltext_get::Cmd),

    /// Full-text search returning newest results first
    FulltextRecent(cmd::fulltext_recent::Cmd),

    /// Semantic vector search (returns IDs and scores)
    Search(cmd::search::Cmd),

    /// Semantic vector search returning full documents
    SearchGet(cmd::search_get::Cmd),

    /// Statistical trend analysis for a key
    Trends(cmd::trends::Cmd),

    /// LDA topic analysis for a key
    Topics(cmd::topics::Cmd),

    /// LDA topic analysis across all keys
    TopicsAll(cmd::topics_all::Cmd),

    /// Root cause analysis
    Rca(cmd::rca::Cmd),

    /// Root cause analysis on drain3 template observations
    RcaTemplates(cmd::rca_templates::Cmd),

    /// Extractive TextRank summary of every drain3 template observed in a window
    TextrankTemplates(cmd::textrank_templates::Cmd),

    /// N-gram anomaly detection over recent primary records (phrase-structure outliers)
    AnomalyRecent(cmd::anomaly_recent::Cmd),

    /// N-gram noise removal over recent primary records (signal vs noise split)
    DenoiseRecent(cmd::denoise_recent::Cmd),

    /// k-NN clustering + isolation analysis over recent primary records
    Knn(cmd::knn::Cmd),

    /// Number of result queues currently tracked, with their UUIDs
    ResultsLen(cmd::results_len::Cmd),

    /// Push a JSON value into the result queue identified by --id
    ResultsPush(cmd::results_push::Cmd),

    /// Pop the front value from the result queue identified by --id
    ResultsPull(cmd::results_pull::Cmd),

    /// Number of elements in the result queue identified by --id
    ResultsEmpty(cmd::results_empty::Cmd),

    /// Evaluate a BUND script
    Eval(cmd::eval::Cmd),

    /// Submit a BUND script to the worker pool and return the result queue id
    EvalQueued(cmd::eval_queued::Cmd),

    // ── template store ────────────────────────────────────────────────────────

    /// Store a drain3 template document
    TplAdd(cmd::tpl_add::Cmd),

    /// Fetch a template document by UUID
    TplGet(cmd::tpl_get::Cmd),

    /// Delete a template document by UUID
    TplDelete(cmd::tpl_delete::Cmd),

    /// List template documents in a time window
    TplList(cmd::tpl_list::Cmd),

    /// Semantic search over template documents
    TplSearch(cmd::tpl_search::Cmd),

    /// Update a template document's metadata or body
    TplUpdate(cmd::tpl_update::Cmd),

    /// Rebuild the template store vector index
    TplReindex(cmd::tpl_reindex::Cmd),

    /// Fetch a template document by UUID via FrequencyTracking (cross-shard)
    TplTemplateById(cmd::tpl_template_by_id::Cmd),

    /// List templates whose FrequencyTracking observation falls in a Unix-second range
    TplTemplatesByTimestamp(cmd::tpl_templates_by_timestamp::Cmd),

    /// List templates whose FrequencyTracking observation falls in a humantime lookback window
    TplTemplatesRecent(cmd::tpl_templates_recent::Cmd),

    // ── document store ────────────────────────────────────────────────────────

    /// Store a document with JSON metadata and text content
    DocAdd(cmd::doc_add::Cmd),

    /// Load a text file, chunk it, and store it in the document store
    DocAddFile(cmd::doc_add_file::Cmd),

    /// Retrieve metadata and content for a document by UUID
    DocGet(cmd::doc_get::Cmd),

    /// Retrieve only the metadata for a document by UUID
    DocGetMetadata(cmd::doc_get_metadata::Cmd),

    /// Retrieve only the content text for a document by UUID
    DocGetContent(cmd::doc_get_content::Cmd),

    /// Replace the metadata of a document in-place
    DocUpdateMetadata(cmd::doc_update_metadata::Cmd),

    /// Replace the content text of a document in-place
    DocUpdateContent(cmd::doc_update_content::Cmd),

    /// Remove a document from the document store
    DocDelete(cmd::doc_delete::Cmd),

    /// Rebuild the document store vector index from persisted metadata and blobs
    DocReindex(cmd::doc_reindex::Cmd),

    /// Semantic search in the document store by plain-text query
    DocSearch(cmd::doc_search::Cmd),

    /// Semantic search in the document store by JSON query object
    DocSearchJson(cmd::doc_search_json::Cmd),

    /// Semantic search returning results as json_fingerprint strings
    DocSearchStrings(cmd::doc_search_strings::Cmd),

    /// Parallel telemetry vector search + document store semantic search in one call
    AggregationSearch(cmd::aggregationsearch::Cmd),

    // ── signal store ──────────────────────────────────────────────────────────

    /// Emit a signal with name, severity, and timestamp
    SignalEmit(cmd::signal_emit::Cmd),

    /// Replace the metadata of a signal in-place
    SignalUpdate(cmd::signal_update::Cmd),

    /// List signals observed within a humantime lookback window
    Signals(cmd::signals::Cmd),

    /// Semantic search over signals by plain-text query
    SignalsQuery(cmd::signals_query::Cmd),
}

fn normalise_url(addr: &str) -> String {
    if addr.starts_with("http://") || addr.starts_with("https://") {
        addr.to_string()
    } else {
        format!("http://{addr}")
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let url = normalise_url(&cli.address);
    let session = cli
        .session
        .unwrap_or_else(|| Uuid::now_v7().to_string());

    let needs_check = !matches!(cli.command, Commands::Status(_));
    if needs_check {
        client::check_server(&url)
            .with_context(|| format!("server pre-flight check failed for {url}"))?;
    }

    let result = match cli.command {
        Commands::Status(a)                   => cmd::status::run(&url, &session, a),
        Commands::Add(a)                      => cmd::add::run(&url, &session, a),
        Commands::AddBatch(a)                 => cmd::add_batch::run(&url, &session, a),
        Commands::AddFile(a)                  => cmd::add_file::run(&url, &session, a),
        Commands::AddFileSyslog(a)            => cmd::add_file_syslog::run(&url, &session, a),
        Commands::Timeline(a)                 => cmd::timeline::run(&url, &session, a),
        Commands::Count(a)                    => cmd::count::run(&url, &session, a),
        Commands::Shards(a)                   => cmd::shards::run(&url, &session, a),
        Commands::Keys(a)                     => cmd::keys::run(&url, &session, a),
        Commands::KeysAll(a)                  => cmd::keys_all::run(&url, &session, a),
        Commands::KeysGet(a)                  => cmd::keys_get::run(&url, &session, a),
        Commands::Primaries(a)                => cmd::primaries::run(&url, &session, a),
        Commands::PrimariesExplore(a)         => cmd::primaries_explore::run(&url, &session, a),
        Commands::PrimariesExploreTelemetry(a)=> cmd::primaries_explore_telemetry::run(&url, &session, a),
        Commands::PrimariesGet(a)             => cmd::primaries_get::run(&url, &session, a),
        Commands::PrimariesGetTelemetry(a)    => cmd::primaries_get_telemetry::run(&url, &session, a),
        Commands::Primary(a)                  => cmd::primary::run(&url, &session, a),
        Commands::Secondaries(a)              => cmd::secondaries::run(&url, &session, a),
        Commands::Secondary(a)                => cmd::secondary::run(&url, &session, a),
        Commands::Duplicates(a)               => cmd::duplicates::run(&url, &session, a),
        Commands::Fulltext(a)                 => cmd::fulltext::run(&url, &session, a),
        Commands::FulltextGet(a)              => cmd::fulltext_get::run(&url, &session, a),
        Commands::FulltextRecent(a)           => cmd::fulltext_recent::run(&url, &session, a),
        Commands::Search(a)                   => cmd::search::run(&url, &session, a),
        Commands::SearchGet(a)                => cmd::search_get::run(&url, &session, a),
        Commands::Trends(a)                   => cmd::trends::run(&url, &session, a),
        Commands::Topics(a)                   => cmd::topics::run(&url, &session, a),
        Commands::TopicsAll(a)                => cmd::topics_all::run(&url, &session, a),
        Commands::Rca(a)                      => cmd::rca::run(&url, &session, a),
        Commands::RcaTemplates(a)             => cmd::rca_templates::run(&url, &session, a),
        Commands::TextrankTemplates(a)        => cmd::textrank_templates::run(&url, &session, a),
        Commands::AnomalyRecent(a)            => cmd::anomaly_recent::run(&url, &session, a),
        Commands::DenoiseRecent(a)            => cmd::denoise_recent::run(&url, &session, a),
        Commands::Knn(a)                      => cmd::knn::run(&url, &session, a),
        Commands::ResultsLen(a)               => cmd::results_len::run(&url, &session, a),
        Commands::ResultsPush(a)              => cmd::results_push::run(&url, &session, a),
        Commands::ResultsPull(a)              => cmd::results_pull::run(&url, &session, a),
        Commands::ResultsEmpty(a)             => cmd::results_empty::run(&url, &session, a),
        Commands::Eval(a)                     => cmd::eval::run(&url, &session, a),
        Commands::EvalQueued(a)               => cmd::eval_queued::run(&url, &session, a),
        Commands::TplAdd(a)                   => cmd::tpl_add::run(&url, &session, a),
        Commands::TplGet(a)                   => cmd::tpl_get::run(&url, &session, a),
        Commands::TplDelete(a)                => cmd::tpl_delete::run(&url, &session, a),
        Commands::TplList(a)                  => cmd::tpl_list::run(&url, &session, a),
        Commands::TplSearch(a)                => cmd::tpl_search::run(&url, &session, a),
        Commands::TplUpdate(a)                => cmd::tpl_update::run(&url, &session, a),
        Commands::TplReindex(a)               => cmd::tpl_reindex::run(&url, &session, a),
        Commands::TplTemplateById(a)          => cmd::tpl_template_by_id::run(&url, &session, a),
        Commands::TplTemplatesByTimestamp(a)  => cmd::tpl_templates_by_timestamp::run(&url, &session, a),
        Commands::TplTemplatesRecent(a)       => cmd::tpl_templates_recent::run(&url, &session, a),
        Commands::DocAdd(a)                   => cmd::doc_add::run(&url, &session, a),
        Commands::DocAddFile(a)               => cmd::doc_add_file::run(&url, &session, a),
        Commands::DocGet(a)                   => cmd::doc_get::run(&url, &session, a),
        Commands::DocGetMetadata(a)           => cmd::doc_get_metadata::run(&url, &session, a),
        Commands::DocGetContent(a)            => cmd::doc_get_content::run(&url, &session, a),
        Commands::DocUpdateMetadata(a)        => cmd::doc_update_metadata::run(&url, &session, a),
        Commands::DocUpdateContent(a)         => cmd::doc_update_content::run(&url, &session, a),
        Commands::DocDelete(a)                => cmd::doc_delete::run(&url, &session, a),
        Commands::DocReindex(a)               => cmd::doc_reindex::run(&url, &session, a),
        Commands::DocSearch(a)                => cmd::doc_search::run(&url, &session, a),
        Commands::DocSearchJson(a)            => cmd::doc_search_json::run(&url, &session, a),
        Commands::DocSearchStrings(a)         => cmd::doc_search_strings::run(&url, &session, a),
        Commands::AggregationSearch(a)        => cmd::aggregationsearch::run(&url, &session, a),
        Commands::SignalEmit(a)               => cmd::signal_emit::run(&url, &session, a),
        Commands::SignalUpdate(a)             => cmd::signal_update::run(&url, &session, a),
        Commands::Signals(a)                  => cmd::signals::run(&url, &session, a),
        Commands::SignalsQuery(a)             => cmd::signals_query::run(&url, &session, a),
    }?;

    if cli.raw {
        println!("{result}");
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(())
}
