mod jsonrpc;
mod server;
mod status;

use anyhow::Context;
use bdslib::vm::workers::BundWorkerPool;
use clap::Parser;
use jsonrpsee::server::Server;
use std::sync::OnceLock;

/// Process-wide BUND worker pool.  Initialised in `main()` before the
/// JSON-RPC server starts.  Workers run for the lifetime of the process.
static WORKERS: OnceLock<BundWorkerPool> = OnceLock::new();

fn nofile_limit_from_config(config_path: Option<&str>) -> u64 {
    const DEFAULT: u64 = 4096;
    let path = match config_path {
        Some(p) => p,
        None => return DEFAULT,
    };
    let raw = match std::fs::read_to_string(path) {
        Ok(r) => r,
        Err(_) => return DEFAULT,
    };
    let val: serde_hjson::Value = match serde_hjson::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return DEFAULT,
    };
    val.as_object()
       .and_then(|o| o.get("nofile_limit"))
       .and_then(|v| v.as_f64())
       .map(|n| n as u64)
       .unwrap_or(DEFAULT)
}

fn n_workers_from_config(config_path: Option<&str>) -> usize {
    const DEFAULT: usize = 4;
    let path = match config_path {
        Some(p) => p,
        None => return DEFAULT,
    };
    let raw = match std::fs::read_to_string(path) {
        Ok(r) => r,
        Err(_) => return DEFAULT,
    };
    let val: serde_hjson::Value = match serde_hjson::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return DEFAULT,
    };
    val.as_object()
       .and_then(|o| o.get("n_workers"))
       .and_then(|v| v.as_f64())
       .map(|n| (n as usize).max(1))
       .unwrap_or(DEFAULT)
}

/// Capacity for the ingest channels (`ingest`, `ingest_file`,
/// `ingest_file_syslog`).
///
/// Returns the `ingest_channel_capacity` config value, or `100_000` if
/// unset.  `0` means "unbounded" (the legacy behaviour, susceptible to
/// OOM under producer pressure).
fn ingest_channel_capacity_from_config(config_path: Option<&str>) -> usize {
    const DEFAULT: usize = 100_000;
    let path = match config_path {
        Some(p) => p,
        None => return DEFAULT,
    };
    let raw = match std::fs::read_to_string(path) {
        Ok(r) => r,
        Err(_) => return DEFAULT,
    };
    let val: serde_hjson::Value = match serde_hjson::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return DEFAULT,
    };
    val.as_object()
       .and_then(|o| o.get("ingest_channel_capacity"))
       .and_then(|v| v.as_f64())
       .map(|n| n as usize)
       .unwrap_or(DEFAULT)
}

fn raise_nofile_limit(limit: u64) {
    match rlimit::increase_nofile_limit(limit) {
        Ok(n)  => log::info!("NOFILE soft limit raised to {n}"),
        Err(e) => log::warn!("could not raise NOFILE limit: {e}"),
    }
}

#[derive(Parser)]
#[command(name = "bdsnode", about = "BDS JSON-RPC 2.0 server")]
struct Cli {
    /// Path to the hjson configuration file (overrides BDS_CONFIG env var).
    #[arg(short, long, env = "BDS_CONFIG")]
    config: Option<String>,

    /// Address to bind the JSON-RPC listener.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port for the JSON-RPC listener.
    #[arg(short, long, default_value_t = 9000)]
    port: u16,

    /// Log verbosity (0=env default, 1=info, 2=debug, 3=trace).
    #[arg(short = 'd', long, default_value_t = 0)]
    debug: u32,

    /// Node identifier included in v2/status responses.
    ///
    /// Pass an explicit value for fixed cluster identities (e.g. a hostname or
    /// role name).  When omitted a UUID v7 is generated at startup.
    #[arg(long)]
    nodeid: Option<String>,

    /// Wipe the existing data store and start fresh before opening.
    ///
    /// Reads `dbpath` from the config file, removes the directory tree, then
    /// proceeds with normal initialisation. Use with care — all data is lost.
    #[arg(long, default_value_t = false)]
    new: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let node_id = cli.nodeid.clone()
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    status::init(node_id);

    bdslib::setloglevel::setloglevel(cli.debug);
    raise_nofile_limit(nofile_limit_from_config(cli.config.as_deref()));

    if cli.new {
        let dbpath = bdslib::dbpath_from_config(cli.config.as_deref())
            .map_err(|e| anyhow::anyhow!("{e}"))
            .context("failed to read dbpath from config for --new")?;
        if std::path::Path::new(&dbpath).exists() {
            std::fs::remove_dir_all(&dbpath)
                .with_context(|| format!("--new: failed to remove {dbpath}"))?;
            log::info!("--new: removed existing data store at {dbpath}");
        } else {
            log::info!("--new: {dbpath} does not exist, nothing to remove");
        }
    }

    bdslib::init_db(cli.config.as_deref())
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to initialise database")?;

    jsonrpc::chat_ollama::init(cli.config.as_deref())
        .context("failed to initialise Ollama config")?;

    bdslib::init_adam()
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to initialise BUND VM")?;

    let n_workers = n_workers_from_config(cli.config.as_deref());
    let pool = BundWorkerPool::start(n_workers)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to initialise BundWorkerPool")?;
    WORKERS.set(pool).ok();
    log::info!("BundWorkerPool started with {n_workers} worker(s)");

    bdslib::context::init(cli.config.as_deref())
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to initialise BUND context")?;

    // Bound the ingest channels so a producer flood (or a stalled
    // consumer thread) returns "full" to the RPC layer instead of
    // OOMing the process by silently growing an unbounded queue.
    // `0` means unbounded (back-compat for callers that need it).
    let ingest_capacity = ingest_channel_capacity_from_config(cli.config.as_deref());
    bdslib::pipe::init_with_capacity(&[
        ("ingest",              ingest_capacity),
        ("ingest_file",         ingest_capacity),
        ("ingest_file_syslog",  ingest_capacity),
    ])
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("failed to initialise pipe registry")?;

    let cleanup_cfg = server::bundcleanup::Config::from_config(cli.config.as_deref())
        .context("failed to read BUND cleanup config")?;
    let cleanup_handle = server::bundcleanup::start(cleanup_cfg);

    let results_cfg = server::results_sweeper::Config::from_config(cli.config.as_deref())
        .context("failed to read result-queue sweeper config")?;
    let results_sweeper_handle = server::results_sweeper::start(results_cfg);

    // Cron-driven script scheduler — fires stored BUND scripts whose
    // `schedule` metadata matches the current minute. Disabled by setting
    // `scheduler_interval_secs: 0` in bds.hjson.
    let scheduler_cfg = server::scheduler::Config::from_config(cli.config.as_deref())
        .context("failed to read scheduler config")?;
    let scheduler_handle = server::scheduler::start(scheduler_cfg);

    // Periodic global sync — checkpoints DuckDB WAL, commits Tantivy, flushes
    // VecStore on every open shard. Bounds recovery time after an unclean
    // exit. Disabled by setting `sync_interval_secs: 0` in bds.hjson.
    let sync_cfg = server::sync::Config::from_config(cli.config.as_deref())
        .context("failed to read sync config")?;
    let sync_handle = server::sync::start(sync_cfg);

    let add_handle = if let Some(cfg) = server::add::Config::from_config(cli.config.as_deref())
        .context("failed to read ingest config")?
    {
        Some(server::add::start(cfg))
    } else {
        None
    };

    let add_file_handle =
        if let Some(cfg) = server::add_file::Config::from_config(cli.config.as_deref())
            .context("failed to read file-ingest config")?
        {
            Some(server::add_file::start(cfg, status::get().current_file.clone()))
        } else {
            None
        };

    let add_file_syslog_handle =
        if let Some(cfg) = server::add_file_syslog::Config::from_config(cli.config.as_deref())
            .context("failed to read syslog file-ingest config")?
        {
            Some(server::add_file_syslog::start(cfg, status::get().current_syslog_file.clone()))
        } else {
            None
        };

    let addr = format!("{}:{}", cli.host, cli.port);

    let server = Server::builder()
        .build(&addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;

    let local_addr = server.local_addr()?;
    let handle = server.start(jsonrpc::build_module());

    log::info!("bdsnode listening on {local_addr}");

    tokio::signal::ctrl_c().await.context("ctrl-c signal error")?;

    log::info!("shutting down…");
    handle.stop()?;
    handle.stopped().await;

    cleanup_handle.stop().await;
    server::bundcleanup::vm_close();

    results_sweeper_handle.stop().await;
    scheduler_handle.stop().await;
    sync_handle.stop().await;

    // Drain ingest channels and join batch threads before checkpointing so
    // that no queued records are lost.
    if let Some(h) = add_file_syslog_handle {
        h.stop();
    }
    if let Some(h) = add_file_handle {
        h.stop();
    }
    if let Some(h) = add_handle {
        h.stop();
    }

    bdslib::sync_db().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}
