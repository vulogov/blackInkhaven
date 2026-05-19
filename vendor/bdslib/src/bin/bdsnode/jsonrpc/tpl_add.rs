use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct TplAddParams {
    #[allow(dead_code)]
    session:     String,
    /// Human-readable template name.
    name:        String,
    /// Template body text.
    body:        String,
    /// Unix timestamp (seconds).  Determines which time shard the template
    /// is stored in.  Defaults to the current wall-clock time when omitted.
    timestamp:   Option<u64>,
    /// Optional list of tags.
    #[serde(default)]
    tags:        Vec<String>,
    /// Optional description.
    #[serde(default)]
    description: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/tpl.add", |params, _ctx, _| async move {
            log::info!("v2/tpl.add: start");
            let p: TplAddParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                let now_secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let ts = p.timestamp.unwrap_or(now_secs);

                let metadata = serde_json::json!({
                    "name":        p.name,
                    "tags":        p.tags,
                    "description": p.description,
                    "type":        "template",
                    "created_at":  now_secs,
                    "timestamp":   ts,
                });

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let id = db
                    .tpl_add(metadata, p.body.as_bytes())
                    .map_err(|e| rpc_err(-32011, e))?;
                log::info!("v2/tpl.add: stored id={id}");
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "id": id.to_string() }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            result
        })
        .unwrap();
}
