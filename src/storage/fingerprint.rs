//! JSON-tree fingerprinting used by the vector layer. Walks the tree
//! depth-first and emits `path: value` pairs for every leaf so that
//! field-name context survives the embedding step. The exact string
//! shape matters for on-disk compatibility — if you change anything
//! here, existing vector indexes will start to drift against new
//! embeddings.

use serde_json::Value as JsonValue;

/// Convert a JSON value to a flat, embedding-friendly string.
///
/// ```text
/// { "title": "Rust", "meta": { "year": 2015 } }
/// →  "title: Rust meta.year: 2015"
/// ```
pub fn json_fingerprint(json: &JsonValue) -> String {
    let mut parts = Vec::new();
    collect_leaves(json, "", &mut parts);
    parts.join(" ")
}

fn collect_leaves(value: &JsonValue, path: &str, out: &mut Vec<String>) {
    match value {
        JsonValue::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                collect_leaves(child, &child_path, out);
            }
        }
        JsonValue::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let child_path = if path.is_empty() {
                    format!("[{i}]")
                } else {
                    format!("{path}[{i}]")
                };
                collect_leaves(item, &child_path, out);
            }
        }
        JsonValue::String(s) => {
            if path.is_empty() {
                out.push(s.clone());
            } else {
                out.push(format!("{path}: {s}"));
            }
        }
        JsonValue::Number(n) => {
            if path.is_empty() {
                out.push(n.to_string());
            } else {
                out.push(format!("{path}: {n}"));
            }
        }
        JsonValue::Bool(b) => {
            if path.is_empty() {
                out.push(b.to_string());
            } else {
                out.push(format!("{path}: {b}"));
            }
        }
        JsonValue::Null => {}
    }
}
