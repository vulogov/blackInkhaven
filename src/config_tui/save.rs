//! 1.2.10+ — surgical-splice save pipeline.
//!
//! Given the original HJSON `source`, a `path → span`
//! index built by `hjson_index::parse`, and an `edited`
//! `serde_json::Value`, compute the splice plan and
//! emit a new HJSON string that:
//!
//!   * Preserves every comment in the original source
//!     (including hand-written `# foo` lines we never
//!     parsed semantically).
//!   * Preserves unknown / user-added fields (anything
//!     in `source` but not in the schema; we simply
//!     don't touch those byte ranges).
//!   * Splices new values into changed leaves byte-for-
//!     byte at the span recorded in the index.
//!   * Appends newly-set leaves (paths the user
//!     edited that weren't present in the source) at
//!     the corresponding stanza's insertion point.
//!
//! Output is then atomically written + a timestamped
//! copy lands in `<project>/.config-backups/`.
//!
//! See proposal §6.5–6.6 for the design rationale.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;
use serde_json::Value;

use super::hjson_index::HjsonIndex;
use super::schema::SchemaNode;

/// One pending edit.  `path` is the dotted schema
/// path; `new_value` is the serde_json value the user
/// committed; `kind` records whether the path already
/// existed in the source (so we know whether to
/// splice or append).
#[derive(Debug, Clone)]
pub struct Edit {
    pub path: String,
    pub new_value: Value,
    pub kind: EditKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    /// Path is present in the live HJSON — splice the
    /// new value into its existing span.
    Splice,
    /// Path is NOT present in the live HJSON — append
    /// at the parent stanza's insertion point.
    Append,
}

/// Compare the schema tree (post-edit) against the
/// HJSON source index to compute the per-leaf edit
/// list.  Returns only paths whose `current` differs
/// from what the HJSON source actually says.
pub fn compute_edits(
    root: &SchemaNode,
    index: &HjsonIndex,
) -> Vec<Edit> {
    let mut out: Vec<Edit> = Vec::new();
    walk(root, index, &mut out);
    out
}

fn walk(node: &SchemaNode, index: &HjsonIndex, out: &mut Vec<Edit>) {
    if node.is_leaf() {
        if node.path.is_empty() {
            return;
        }
        // The leaf in the schema tree has the user's
        // edited `current`.  Compare it to the value
        // actually written in the source — if the
        // source has this leaf, the source's textual
        // value is the source of truth; if not, the
        // default acts as the baseline.
        let source_value = match index.leaves.get(&node.path) {
            Some(span) => {
                // The source carries a textual value;
                // re-parse it through serde_hjson so
                // the comparison is type-aware (not
                // string-equal).
                let raw = &index.source[span.value_range.clone()];
                serde_hjson::from_str::<Value>(raw)
                    .unwrap_or_else(|_| Value::String(raw.to_string()))
            }
            None => node.default.clone(),
        };
        if values_match(&node.current, &source_value) {
            return;
        }
        let kind = if index.leaves.contains_key(&node.path) {
            EditKind::Splice
        } else {
            EditKind::Append
        };
        out.push(Edit {
            path: node.path.clone(),
            new_value: node.current.clone(),
            kind,
        });
        return;
    }
    for child in &node.children {
        walk(child, index, out);
    }
}

/// Type-aware equality.  `serde_json::Value::PartialEq`
/// is exact (e.g. `1` ≠ `1.0`), which is good for
/// detecting genuine type changes.  We use it as-is;
/// HJSON-derived ambiguity is handled by serde_hjson at
/// parse time.
fn values_match(a: &Value, b: &Value) -> bool {
    // Numbers: collapse integer / float representations
    // so `5` (int) equals `5.0` (float) — common when
    // the user typed a default with a fractional zero.
    if let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) {
        if x.is_finite() && y.is_finite() {
            return (x - y).abs() < f64::EPSILON;
        }
    }
    a == b
}

/// Apply `edits` against `index.source` and return the
/// new HJSON text.  Splices are sorted by descending
/// start offset so earlier splices don't shift later
/// ones; appends are batched per-stanza.
pub fn apply_edits(index: &HjsonIndex, edits: &[Edit]) -> Result<String> {
    // Partition.
    let mut splices: Vec<&Edit> =
        edits.iter().filter(|e| e.kind == EditKind::Splice).collect();
    let appends: Vec<&Edit> =
        edits.iter().filter(|e| e.kind == EditKind::Append).collect();

    // Splices: sort by value_range.start descending
    // and rewrite in place.
    let mut source = index.source.clone();
    splices.sort_by(|a, b| {
        let a_start = index.leaves[&a.path].value_range.start;
        let b_start = index.leaves[&b.path].value_range.start;
        b_start.cmp(&a_start)
    });
    for edit in &splices {
        let span = &index.leaves[&edit.path];
        let new_text = render_value(&edit.new_value);
        source.replace_range(span.value_range.clone(), &new_text);
    }

    // Appends: group by parent stanza so we can splice
    // multiple new fields in one pass.  Phase 2 supports
    // root-level appends + nested-stanza appends both.
    let mut by_parent: BTreeMap<String, Vec<&Edit>> = BTreeMap::new();
    for edit in &appends {
        let parent = parent_path(&edit.path);
        by_parent
            .entry(parent.to_string())
            .or_default()
            .push(edit);
    }
    // Apply in reverse-document order (latest insertion
    // point first) so earlier insertions don't shift
    // later ones.  Rebuild the index in source so the
    // insertion points after each batch are still
    // valid.
    let mut insertion_plan: Vec<(usize, String)> = Vec::new();
    for (parent, edits_in_parent) in by_parent {
        let insertion_point = if parent.is_empty() {
            // Append at the top-level body end (just
            // inside the outer `}` or at EOF).
            //
            // The source has shifted because of the
            // splice pass — rebuild a fresh index for
            // a stable insertion point.
            let fresh = super::hjson_index::parse(&source)
                .context("re-parse source after splice")?;
            fresh.top_level_body_end
        } else {
            let fresh = super::hjson_index::parse(&source)
                .context("re-parse source after splice")?;
            match fresh.stanzas.get(&parent) {
                Some(span) => span.close_brace,
                None => continue, // parent stanza
                                  // missing — bail.
            }
        };
        let mut payload = String::new();
        for edit in edits_in_parent {
            let key = leaf_key(&edit.path);
            payload.push_str("\n  ");
            payload.push_str(&key);
            payload.push_str(": ");
            payload.push_str(&render_value(&edit.new_value));
        }
        payload.push('\n');
        insertion_plan.push((insertion_point, payload));
    }
    // Apply in descending offset.
    insertion_plan.sort_by(|a, b| b.0.cmp(&a.0));
    for (pos, payload) in insertion_plan {
        source.insert_str(pos, &payload);
    }
    Ok(source)
}

/// Render a JSON value as HJSON text.  Strings get
/// quotes when they contain whitespace, comma, brace,
/// bracket, or comment markers; otherwise unquoted.
/// Booleans + numbers + null straightforward.  Arrays
/// + objects fall back to `serde_json::to_string` and
/// the user can hand-tune the output via the HJSON
/// editor — they're rare enough as a single-leaf edit
/// that this isn't worth a full pretty-printer in
/// Phase 2.
fn render_value(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => if *b { "true" } else { "false" }.into(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if needs_quoting(s) {
                format!("\"{}\"", escape_string(s))
            } else {
                s.clone()
            }
        }
        // For arrays / objects fall back to compact
        // serde_json — Phase 2 doesn't typically
        // surgically rewrite these.
        other => serde_json::to_string(other)
            .unwrap_or_else(|_| "null".to_string()),
    }
}

fn needs_quoting(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    for c in s.chars() {
        if c == ' '
            || c == '\t'
            || c == '\n'
            || c == '\r'
            || c == ','
            || c == '{'
            || c == '}'
            || c == '['
            || c == ']'
            || c == ':'
            || c == '#'
            || c == '"'
            || c == '\\'
        {
            return true;
        }
    }
    // Avoid clashing with HJSON keywords + numeric
    // looks.
    if matches!(s, "true" | "false" | "null") {
        return true;
    }
    if s.parse::<f64>().is_ok() {
        return true;
    }
    // `//` and `/*` would start comments if at the
    // beginning of the value.
    if s.starts_with("//") || s.starts_with("/*") {
        return true;
    }
    false
}

fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn parent_path(path: &str) -> &str {
    match path.rfind('.') {
        Some(idx) => &path[..idx],
        None => "",
    }
}

fn leaf_key(path: &str) -> String {
    match path.rfind('.') {
        Some(idx) => path[idx + 1..].to_string(),
        None => path.to_string(),
    }
}

/// Atomic write: write to `path.tmp` then rename.
/// Returns the canonical path on success.
pub fn write_atomic(path: &Path, contents: &str) -> Result<PathBuf> {
    let mut tmp_path = PathBuf::from(path);
    let mut new_name = tmp_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    new_name.push_str(".tmp");
    tmp_path.set_file_name(&new_name);
    {
        let mut file = fs::File::create(&tmp_path)
            .with_context(|| format!("create {}", tmp_path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("write {}", tmp_path.display()))?;
        file.sync_all().ok();
    }
    fs::rename(&tmp_path, path)
        .with_context(|| format!("rename {} → {}", tmp_path.display(), path.display()))?;
    Ok(path.to_path_buf())
}

/// Snapshot the **post-save** file to
/// `<project>/.config-backups/inkhaven_YYYYMMDD_HHMMSS.hjson`.
/// Returns the backup path written.
pub fn write_backup(project_root: &Path, contents: &str) -> Result<PathBuf> {
    let dir = project_root.join(".config-backups");
    fs::create_dir_all(&dir)
        .with_context(|| format!("create {}", dir.display()))?;
    let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let name = format!("inkhaven_{ts}.hjson");
    let path = dir.join(&name);
    let mut file = fs::File::create(&path)
        .with_context(|| format!("create {}", path.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("write {}", path.display()))?;
    file.sync_all().ok();
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn splice_replaces_leaf_value_in_place() {
        let src = "{\n  // comment\n  port: 8080\n}";
        let idx = super::super::hjson_index::parse(src).unwrap();
        let edits = vec![Edit {
            path: "port".into(),
            new_value: json!(9090),
            kind: EditKind::Splice,
        }];
        let out = apply_edits(&idx, &edits).unwrap();
        // Comment preserved.
        assert!(out.contains("// comment"));
        // Value replaced.
        assert!(out.contains("port: 9090"));
        assert!(!out.contains("8080"));
    }

    #[test]
    fn splice_in_nested_stanza_preserves_outer_comments() {
        let src = "{\n  // top\n  outer: {\n    // inner\n    x: 1\n  }\n}";
        let idx = super::super::hjson_index::parse(src).unwrap();
        let edits = vec![Edit {
            path: "outer.x".into(),
            new_value: json!(99),
            kind: EditKind::Splice,
        }];
        let out = apply_edits(&idx, &edits).unwrap();
        assert!(out.contains("// top"));
        assert!(out.contains("// inner"));
        assert!(out.contains("x: 99"));
    }

    #[test]
    fn unknown_field_preserved_on_save() {
        let src = "{\n  experimental: { my_flag: true }\n  port: 8080\n}";
        let idx = super::super::hjson_index::parse(src).unwrap();
        let edits = vec![Edit {
            path: "port".into(),
            new_value: json!(9090),
            kind: EditKind::Splice,
        }];
        let out = apply_edits(&idx, &edits).unwrap();
        // Unknown field's full subtree untouched.
        assert!(out.contains("experimental"));
        assert!(out.contains("my_flag: true"));
        assert!(out.contains("port: 9090"));
    }

    #[test]
    fn append_into_existing_stanza_inserts_before_close_brace() {
        let src = "{\n  outer: {\n    x: 1\n  }\n}";
        let idx = super::super::hjson_index::parse(src).unwrap();
        let edits = vec![Edit {
            path: "outer.y".into(),
            new_value: json!("hello"),
            kind: EditKind::Append,
        }];
        let out = apply_edits(&idx, &edits).unwrap();
        assert!(out.contains("y: hello"), "got: {out:?}");
        // Outer brace structure intact.
        assert!(out.contains("x: 1"));
    }

    #[test]
    fn render_value_handles_string_with_spaces() {
        assert_eq!(render_value(&json!("hello world")), "\"hello world\"");
    }

    #[test]
    fn render_value_keeps_simple_identifier_unquoted() {
        assert_eq!(render_value(&json!("english")), "english");
    }

    #[test]
    fn render_value_quotes_reserved_words() {
        assert_eq!(render_value(&json!("true")), "\"true\"");
        assert_eq!(render_value(&json!("null")), "\"null\"");
    }

    #[test]
    fn render_value_quotes_numeric_lookalike() {
        assert_eq!(render_value(&json!("42")), "\"42\"");
    }

    #[test]
    fn end_to_end_realistic_hjson_only_changes_target_value() {
        // A representative chunk of a real
        // inkhaven.hjson — comments, nested stanzas,
        // unquoted scalars.  The splice should change
        // exactly the targeted byte range and leave
        // every comment / unknown field / unrelated
        // stanza byte-identical.
        let src = r#"// project config
{
  // primary writing language
  language: english

  embeddings: {
    // fastembed model
    model: MultilingualE5Small
    chunk_size: 800
    chunk_overlap: 0.15
  }

  // user-added field — should survive untouched
  my_custom_setting: hello
}"#;
        let idx = super::super::hjson_index::parse(src).unwrap();
        let edits = vec![Edit {
            path: "embeddings.chunk_size".into(),
            new_value: json!(1200),
            kind: EditKind::Splice,
        }];
        let out = apply_edits(&idx, &edits).unwrap();
        // Targeted change.
        assert!(out.contains("chunk_size: 1200"));
        assert!(!out.contains("chunk_size: 800"));
        // Every other surface byte-stable.
        assert!(out.contains("// primary writing language"));
        assert!(out.contains("language: english"));
        assert!(out.contains("model: MultilingualE5Small"));
        assert!(out.contains("chunk_overlap: 0.15"));
        // Unknown field preserved.
        assert!(out.contains("my_custom_setting: hello"));
        assert!(out.contains("// user-added field"));
    }

    #[test]
    fn end_to_end_append_into_nested_stanza_preserves_neighbours() {
        let src = r#"{
  embeddings: {
    model: MultilingualE5Small
    chunk_size: 800
  }
}"#;
        let idx = super::super::hjson_index::parse(src).unwrap();
        let edits = vec![Edit {
            path: "embeddings.chunk_overlap".into(),
            new_value: json!(0.15),
            kind: EditKind::Append,
        }];
        let out = apply_edits(&idx, &edits).unwrap();
        assert!(out.contains("chunk_overlap: 0.15"));
        // Pre-existing siblings still there.
        assert!(out.contains("model: MultilingualE5Small"));
        assert!(out.contains("chunk_size: 800"));
    }

    #[test]
    fn multiple_splices_apply_correctly() {
        let src = "{\n  a: 1\n  b: 2\n  c: 3\n}";
        let idx = super::super::hjson_index::parse(src).unwrap();
        let edits = vec![
            Edit { path: "a".into(), new_value: json!(11), kind: EditKind::Splice },
            Edit { path: "b".into(), new_value: json!(22), kind: EditKind::Splice },
            Edit { path: "c".into(), new_value: json!(33), kind: EditKind::Splice },
        ];
        let out = apply_edits(&idx, &edits).unwrap();
        assert!(out.contains("a: 11"));
        assert!(out.contains("b: 22"));
        assert!(out.contains("c: 33"));
    }
}
