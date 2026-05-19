mod client;
mod error;
mod routes;
mod state;

use axum::{routing::{delete, get, post}, Router};
use clap::Parser;
use state::AppState;
use tower_http::compression::CompressionLayer;

#[derive(Parser)]
#[command(name = "bdsweb", about = "bdsnode web UI")]
struct Args {
    /// Address to bind the web server
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to bind the web server
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// bdsnode JSON-RPC endpoint
    #[arg(short, long, env = "BDSNODE_URL", default_value = "http://127.0.0.1:9000")]
    node: String,

    /// Path to bds.hjson config file (reads ollama_model for the Chat UI)
    #[arg(short, long, env = "BDS_CONFIG")]
    config: Option<String>,

    /// Log verbosity (0=warn, 1=info, 2=debug)
    #[arg(long, default_value_t = 1)]
    verbose: u8,
}

struct WebConfig {
    ollama_model:           String,
    dashboard_refresh_secs: u64,
}

fn load_config(config_path: Option<&str>) -> WebConfig {
    let defaults = WebConfig { ollama_model: "llama3.2".to_owned(), dashboard_refresh_secs: 30 };
    let path = match config_path {
        Some(p) => p,
        None => return defaults,
    };
    let raw = match std::fs::read_to_string(path) {
        Ok(r) => r,
        Err(_) => return defaults,
    };
    let val: serde_hjson::Value = match serde_hjson::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return defaults,
    };
    let obj = match val.as_object() {
        Some(o) => o,
        None => return defaults,
    };
    WebConfig {
        ollama_model: obj.get("ollama_model")
            .and_then(|v| v.as_str())
            .unwrap_or(&defaults.ollama_model)
            .to_owned(),
        dashboard_refresh_secs: obj.get("dashboard_refresh_secs")
            .and_then(|v| v.as_f64())
            .map(|n| n as u64)
            .unwrap_or(defaults.dashboard_refresh_secs)
            .max(1),
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let level = match args.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();

    let cfg = load_config(args.config.as_deref());
    let state = AppState::new(args.node.clone(), cfg.ollama_model, cfg.dashboard_refresh_secs);

    // Background poller: refreshes the cached Dashboard snapshot every N seconds.
    {
        let poller_state = state.clone();
        tokio::spawn(async move {
            let interval = std::time::Duration::from_secs(poller_state.dashboard_refresh_secs);
            log::info!(
                "dashboard background poller started (interval={}s)",
                poller_state.dashboard_refresh_secs
            );
            loop {
                match routes::dashboard::collect(&poller_state).await {
                    Ok(snap) => {
                        *poller_state.dashboard_cache.write().await = Some(snap);
                        log::debug!("dashboard cache refreshed");
                    }
                    Err(e) => {
                        log::warn!("dashboard background poll failed: {e}");
                    }
                }
                tokio::time::sleep(interval).await;
            }
        });
    }

    let app = Router::new()
        .route("/",                  get(routes::dashboard::page))
        .route("/dashboard/data",    get(routes::dashboard::data))
        .route("/dashboard/refresh", get(routes::dashboard::refresh))
        .route("/telemetry",         get(routes::telemetry::page))
        .route("/telemetry/results", get(routes::telemetry::results))
        .route("/telemetry/keys",    get(routes::telemetry::keys))
        .route("/logs",              get(routes::logs::page))
        .route("/logs/results",      get(routes::logs::results))
        .route("/logs/keys",         get(routes::logs::keys))
        .route("/logs/topics",       get(routes::logs::topics))
        .route("/docs",           get(routes::docs::page))
        .route("/docs/results",   get(routes::docs::results))
        .route("/search",         get(routes::search::page))
        .route("/search/results", get(routes::search::results))
        .route("/trends",           get(routes::trends::page))
        .route("/trends/results",   get(routes::trends::results))
        .route("/signals",          get(routes::signals::page))
        .route("/signals/results",  get(routes::signals::results))
        .route("/rca",              get(routes::rca::page))
        .route("/rca/results",    get(routes::rca::results))
        .route("/rca/templates",         get(routes::rca_templates::page))
        .route("/rca/templates/results", get(routes::rca_templates::results))
        .route("/templates",         get(routes::templates::page))
        .route("/templates/results", get(routes::templates::results))
        .route("/templates_summary",         get(routes::templates_summary::page))
        .route("/templates_summary/results", get(routes::templates_summary::results))
        .route("/primary_summary",         get(routes::primary_summary::page))
        .route("/primary_summary/results", get(routes::primary_summary::results))
        .route("/primary_query_summary",         get(routes::primary_query_summary::page))
        .route("/primary_query_summary/results", get(routes::primary_query_summary::results))
        .route("/primary_lsa_summary",         get(routes::primary_lsa_summary::page))
        .route("/primary_lsa_summary/results", get(routes::primary_lsa_summary::results))
        .route("/primary_lsa_query_summary",         get(routes::primary_lsa_query_summary::page))
        .route("/primary_lsa_query_summary/results", get(routes::primary_lsa_query_summary::results))
        .route("/anomaly_recent",         get(routes::anomaly_recent::page))
        .route("/anomaly_recent/results", get(routes::anomaly_recent::results))
        .route("/denoise_recent",         get(routes::denoise_recent::page))
        .route("/denoise_recent/results", get(routes::denoise_recent::results))
        .route("/knn",                    get(routes::knn::page))
        .route("/knn/results",            get(routes::knn::results))
        .route("/chat",           get(routes::chat::page))
        .route("/chat/query",     post(routes::chat::query))
        .route("/chat/new",       post(routes::chat::new_session))
        .route("/chat/reset",     get(routes::chat::reset))
        .route("/bund",           get(routes::bund::page))
        .route("/bund/eval",      post(routes::bund::eval))
        .route("/scripts",                get(routes::scripts::page))
        .route("/scripts/list",           get(routes::scripts::list))
        .route("/scripts/editor",         get(routes::scripts::editor_new))
        .route("/scripts/editor/{id}",    get(routes::scripts::editor_get))
        .route("/scripts/save",           post(routes::scripts::save))
        .route("/scripts/run",            post(routes::scripts::run))
        .route("/scripts/{id}",           delete(routes::scripts::delete))
        .route("/version",        get(routes::version::version))
        .layer(CompressionLayer::new())
        .with_state(state);

    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("cannot bind {addr}: {e}"));

    log::info!("bdsweb listening on http://{addr}  →  bdsnode at {}", args.node);
    axum::serve(listener, app).await.expect("server error");
}
