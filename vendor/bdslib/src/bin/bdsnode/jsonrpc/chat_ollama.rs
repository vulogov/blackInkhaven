use std::sync::OnceLock;
use jsonrpsee::RpcModule;
use super::params::rpc_err;

struct OllamaNodeConfig {
    url:           String,
    model:         String,
    system_prompt: String,
}

static OLLAMA_CFG: OnceLock<OllamaNodeConfig> = OnceLock::new();

fn default_system_prompt() -> &'static str {
    "You are an expert site reliability engineer and telemetry analyst with access to real observability data. Analyse the provided context and answer the operator's question concisely and accurately."
}

fn get_cfg() -> &'static OllamaNodeConfig {
    OLLAMA_CFG.get_or_init(|| OllamaNodeConfig {
        url:           "http://localhost:11434".to_owned(),
        model:         "llama3.2".to_owned(),
        system_prompt: default_system_prompt().to_owned(),
    })
}

/// Read Ollama settings from bds.hjson and store in the process-wide singleton.
/// Safe to call multiple times; only the first call takes effect.
pub fn init(config_path: Option<&str>) -> anyhow::Result<()> {
    let path = match config_path {
        Some(p) => p.to_owned(),
        None => match std::env::var("BDS_CONFIG") {
            Ok(p) => p,
            Err(_) => {
                get_cfg(); // ensure defaults are set
                return Ok(());
            }
        },
    };

    let raw = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("cannot read config {path:?}: {e}"))?;
    let val: serde_hjson::Value = serde_hjson::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("hjson parse error: {e}"))?;
    let obj = val.as_object()
        .ok_or_else(|| anyhow::anyhow!("config must be a JSON object"))?;

    let url = obj.get("ollama_url")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:11434")
        .to_owned();
    let model = obj.get("ollama_model")
        .and_then(|v| v.as_str())
        .unwrap_or("llama3.2")
        .to_owned();
    let system_prompt = obj.get("ollama_system_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or(default_system_prompt())
        .to_owned();

    OLLAMA_CFG.get_or_init(|| OllamaNodeConfig { url, model, system_prompt });
    Ok(())
}

// ── RPC params ────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct ChatOllamaParams {
    /// Existing session UUID; omit or null to start a new session.
    chat_id:  Option<String>,
    /// Lookback window for RAG context, e.g. "1h".
    duration: String,
    /// The user's natural-language question (sent to the LLM).
    query:    String,
    /// Pre-built RAG context string supplied by the caller (e.g. from
    /// v2/primaries.explore).  When present, aggregationsearch is skipped
    /// entirely and this string is used verbatim as the observability context.
    context:  Option<String>,
}

// ── RAG context builder ───────────────────────────────────────────────────────

fn build_rag_context(telemetry: &[serde_json::Value], documents: &[serde_json::Value]) -> String {
    let mut parts: Vec<String> = Vec::new();

    for (i, item) in telemetry.iter().take(30).enumerate() {
        let fp = bdslib::json_fingerprint(item);
        if !fp.is_empty() {
            parts.push(format!("[telemetry {}] {}", i + 1, fp));
        }
    }

    for (i, item) in documents.iter().take(10).enumerate() {
        let fp = bdslib::json_fingerprint(item);
        if !fp.is_empty() {
            parts.push(format!("[document {}] {}", i + 1, fp));
        }
    }

    parts.join("\n")
}

// ── Handler registration ──────────────────────────────────────────────────────

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/chat.ollama", |params, _ctx, _| async move {
            log::info!("v2/chat.ollama: received request");
            let p: ChatOllamaParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                let cfg = get_cfg();

                // Resolve or create chat session.
                let is_new_session = p.chat_id.is_none();
                let chat_id = match &p.chat_id {
                    Some(id) => uuid::Uuid::parse_str(id)
                        .map_err(|e| rpc_err(-32600, format!("invalid chat_id: {e}")))?,
                    None => {
                        log::info!("v2/chat.ollama: creating new session");
                        bdslib::ai::ollama::new_chat_session(&cfg.model, &cfg.system_prompt)
                            .map_err(|e| rpc_err(-32001, e))?
                    }
                };

                log::info!(
                    "v2/chat.ollama: session={} new={} duration={}",
                    chat_id, is_new_session, p.duration
                );

                // Resolve RAG context: caller may supply a pre-built context
                // (e.g. from v2/primaries.explore); otherwise run aggregationsearch.
                let (rag_context, telemetry_count, document_count) = if let Some(ctx) = p.context {
                    log::info!("v2/chat.ollama: using caller-supplied context ({} chars)", ctx.len());
                    (ctx, 0usize, 0usize)
                } else {
                    let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;

                    log::info!("v2/chat.ollama: aggregationsearch duration={} query={:?}",
                        p.duration, p.query);
                    let agg = db.aggregationsearch(&p.duration, &p.query)
                        .map_err(|e| rpc_err(-32004, e))?;

                    let telemetry_hits: Vec<serde_json::Value> = agg
                        .get("observability")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let doc_hits: Vec<serde_json::Value> = agg
                        .get("documents")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();

                    let n_tel = telemetry_hits.len();
                    let n_doc = doc_hits.len();
                    log::info!("v2/chat.ollama: RAG context: {n_tel} telemetry, {n_doc} docs");

                    (build_rag_context(&telemetry_hits, &doc_hits), n_tel, n_doc)
                };

                // Build the enriched user message.
                let enriched = if rag_context.is_empty() {
                    p.query.clone()
                } else {
                    format!(
                        "Relevant observability context (last {}):\n\n{}\n\n---\n\nUser question: {}",
                        p.duration, rag_context, p.query
                    )
                };

                // Call Ollama and persist history.
                log::info!("v2/chat.ollama: sending to Ollama model={}", cfg.model);
                let response = bdslib::ai::ollama::chat(
                    chat_id,
                    &cfg.url,
                    &cfg.model,
                    &cfg.system_prompt,
                    &enriched,
                ).map_err(|e| rpc_err(-32004, e))?;

                log::info!("v2/chat.ollama: done session={chat_id}");

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject<'static>>(
                    serde_json::json!({
                        "chat_id":        chat_id.to_string(),
                        "response":       response,
                        "is_new_session": is_new_session,
                        "telemetry_count": telemetry_count,
                        "document_count":  document_count,
                    })
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            result
        })
        .unwrap();
}
