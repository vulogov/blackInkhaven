use jsonrpsee::RpcModule;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/status", |_params, _ctx, _| async move {
            log::debug!("v2/status: start");

            let state = crate::status::get();

            let uptime_secs     = state.started_at.elapsed().as_secs();
            let timestamp       = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let logs_queue         = bdslib::pipe::len("ingest").unwrap_or(0);
            let json_file_queue    = bdslib::pipe::len("ingest_file").unwrap_or(0);
            let syslog_file_queue  = bdslib::pipe::len("ingest_file_syslog").unwrap_or(0);
            let json_file_name     = state.current_file
                .lock()
                .ok()
                .and_then(|g| g.clone());
            let syslog_file_name   = state.current_syslog_file
                .lock()
                .ok()
                .and_then(|g| g.clone());

            let (jsoncache_pct, jsoncache_len, jsoncache_capacity, embedding_model) =
                match bdslib::get_db() {
                    Ok(db) => (
                        db.jsoncache_utilization_pct(),
                        db.jsoncache_len() as u64,
                        db.jsoncache_capacity() as u64,
                        db.embedding_model_name(),
                    ),
                    Err(_) => (0, 0, 0, None),
                };

            // BUND runtime stats (BundWorkerPool + result queues + named contexts).
            let n_results = bdslib::vm::results().n_queues() as u64;
            let n_bunds   = bdslib::vm::context::n_contexts() as u64;

            let recent_scripts: Vec<serde_json::Value> = bdslib::vm::workers::recent_submissions()
                .into_iter()
                .map(|(id, ts)| serde_json::json!({
                    "id":           id.to_string(),
                    "submitted_at": ts,
                }))
                .collect();

            let running_scripts: Vec<serde_json::Value> = bdslib::vm::workers::running_snapshot()
                .into_iter()
                .map(|(worker_id, job_id)| serde_json::json!({
                    "worker": worker_id,
                    "id":     job_id.to_string(),
                }))
                .collect();

            let value = serde_json::json!({
                "node_id":           state.node_id,
                "hostname":          state.hostname,
                "version":           env!("CARGO_PKG_VERSION"),
                "uptime_secs":       uptime_secs,
                "timestamp":         timestamp,
                "logs_queue":        logs_queue,
                "json_file_queue":   json_file_queue,
                "json_file_name":    json_file_name,
                "syslog_file_queue": syslog_file_queue,
                "syslog_file_name":  syslog_file_name,
                "jsoncache_pct":      jsoncache_pct,
                "jsoncache_len":      jsoncache_len,
                "jsoncache_capacity": jsoncache_capacity,
                "embedding_model":    embedding_model,
                "n_results":          n_results,
                "n_bunds":            n_bunds,
                "recent_scripts":     recent_scripts,
                "running_scripts":    running_scripts,
            });

            log::debug!("v2/status: done");
            Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(value)
        })
        .unwrap();
}
