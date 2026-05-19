use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct TplTemplateByIdParams {
    #[allow(dead_code)]
    session: String,
    id: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/tpl.template_by_id", |params, _ctx, _| async move {
            log::info!("v2/tpl.template_by_id: start");
            let p: TplTemplateByIdParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                log::info!("v2/tpl.template_by_id: id={}", p.id);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let template = db
                    .template_by_id(&p.id)
                    .map_err(|e| rpc_err(-32011, e))?;
                log::info!("v2/tpl.template_by_id: found={}", template.is_some());
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "template": template }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            result
        })
        .unwrap();
}
