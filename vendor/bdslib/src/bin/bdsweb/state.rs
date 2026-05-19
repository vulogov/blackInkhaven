use std::sync::Arc;
use tokio::sync::RwLock;

/// Latest set of dashboard RPC results, populated by the background poller in
/// `main` and consumed by `routes::dashboard::data`.  `None` means the
/// background task has not yet completed its first successful fetch — the
/// dashboard route renders a "Wait" placeholder in that case.
#[derive(Clone, Debug)]
pub struct DashboardSnapshot {
    pub status:   serde_json::Value,
    pub count:    serde_json::Value,
    pub timeline: serde_json::Value,
    pub shards:   serde_json::Value,
}

#[derive(Clone)]
pub struct AppState {
    pub node_url:     Arc<String>,
    pub http:         reqwest::Client,
    /// Ollama model name read from bds.hjson (for display in the Chat UI).
    pub ollama_model: Arc<String>,
    /// Background-poll interval for the cached Dashboard snapshot, in seconds.
    pub dashboard_refresh_secs: u64,
    /// Most-recent Dashboard snapshot collected by the background task.
    pub dashboard_cache: Arc<RwLock<Option<DashboardSnapshot>>>,
}

impl AppState {
    pub fn new(node_url: String, ollama_model: String, dashboard_refresh_secs: u64) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("failed to build HTTP client");
        Self {
            node_url:     Arc::new(node_url),
            http,
            ollama_model: Arc::new(ollama_model),
            dashboard_refresh_secs,
            dashboard_cache: Arc::new(RwLock::new(None)),
        }
    }
}
