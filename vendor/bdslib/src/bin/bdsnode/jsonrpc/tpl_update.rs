use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct TplUpdateParams {
    #[allow(dead_code)]
    session:     String,
    id:          String,
    /// New name; omit to leave unchanged.
    name:        Option<String>,
    /// New body; omit to leave unchanged.
    body:        Option<String>,
    /// New tag list; omit to leave unchanged.
    tags:        Option<Vec<String>>,
    /// New description; omit to leave unchanged.
    description: Option<String>,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/tpl.update", |params, _ctx, _| async move {
            log::info!("v2/tpl.update: start");
            let p: TplUpdateParams = params.parse()?;
            let id = uuid::Uuid::parse_str(&p.id)
                .map_err(|e| rpc_err(-32600, format!("invalid UUID {:?}: {e}", p.id)))?;
            let result = tokio::task::spawn_blocking(move || {
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;

                // Load and merge metadata if any metadata fields changed.
                let meta_changed = p.name.is_some() || p.tags.is_some() || p.description.is_some();
                if meta_changed {
                    let mut meta = db
                        .tpl_get_metadata(id)
                        .map_err(|e| rpc_err(-32011, e))?
                        .ok_or_else(|| rpc_err(-32010, format!("template {id} not found")))?;

                    let obj = meta.as_object_mut()
                        .ok_or_else(|| rpc_err(-32011, "stored metadata is not a JSON object".to_owned()))?;

                    if let Some(name) = p.name {
                        obj.insert("name".to_owned(), serde_json::Value::String(name));
                    }
                    if let Some(tags) = p.tags {
                        obj.insert("tags".to_owned(), serde_json::json!(tags));
                    }
                    if let Some(desc) = p.description {
                        obj.insert("description".to_owned(), serde_json::Value::String(desc));
                    }

                    db.tpl_update_metadata(id, meta)
                        .map_err(|e| rpc_err(-32011, e))?;
                }

                // Update body separately if provided.
                if let Some(body) = p.body {
                    db.tpl_update_body(id, body.as_bytes())
                        .map_err(|e| rpc_err(-32011, e))?;
                }

                log::info!("v2/tpl.update: updated id={id}");
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "updated": true }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            result
        })
        .unwrap();
}
