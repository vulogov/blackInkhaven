use serde_json::Value as JsonValue;

/// Convert a JSON value into a flat, human-readable fingerprint string suitable
/// for embedding or full-text indexing.
///
/// The algorithm walks the JSON tree recursively and emits `path: value` pairs
/// for every leaf, preserving field-name context at every depth:
///
/// ```text
/// { "title": "Rust",
///   "meta": { "year": 2015, "tags": ["systems", "safe"] } }
/// →
/// "title: Rust meta.year: 2015 meta.tags[0]: systems meta.tags[1]: safe"
/// ```
///
/// Rules:
/// - **Objects** — recurse with dot-separated path prefix.
/// - **Arrays** — recurse with `[i]` index appended to the path.
/// - **Strings** — emitted as `path: value` (field name retained for context).
/// - **Numbers / booleans** — emitted as `path: value`.
/// - **Null** — skipped (carries no semantic content).
/// - **Top-level primitives** — emitted as-is without a path prefix.
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
        JsonValue::Null => {} // no semantic content
    }
}

/// Extract a scalar value from `doc` by following the dot-notation `path`.
///
/// Each `.`-separated segment is used as an object key. Returns `None` if any
/// segment is missing or the resolved value is an object or array (non-scalar).
///
/// ```text
/// extract_key(&json!({"meta": {"id": "x"}}), "meta.id")  →  Some("x")
/// extract_key(&json!({"meta": {}}), "meta.id")            →  None
/// ```
pub fn extract_key(doc: &JsonValue, path: &str) -> Option<String> {
    let mut cur = doc;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    match cur {
        JsonValue::String(s) => Some(s.clone()),
        JsonValue::Number(n) => Some(n.to_string()),
        JsonValue::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}
