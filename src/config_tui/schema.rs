//! 1.2.10+ — schema model for the standalone TUI
//! configuration editor.
//!
//! Phase 1: types + a tree builder that walks
//! `Config::default()` serialised to `serde_json::Value`
//! to derive every leaf path + default + type.  Metadata
//! (enum variants, min/max ranges, help anchor) lives in
//! a small hand-rolled table in `metadata.rs` and is
//! layered on top of the auto-derived tree at build time.
//!
//! Read-only in Phase 1 — no widgets that mutate, no
//! save, no backup.  Just a *config explorer*.

use serde_json::Value;
use std::collections::BTreeMap;

/// Type of a config leaf (or `Stanza` for branches).
/// Phase 1 carries only the minimum the read-only
/// renderer needs; richer constraints (min/max, enum
/// variants) land in Phase 2.
#[derive(Debug, Clone)]
pub enum ConfigType {
    Bool,
    Int,
    Float,
    String,
    /// Array of strings — common shape for the
    /// `extra_words` / `*_stop_words` / per-language
    /// lists.
    StringList,
    /// Any array we can't narrow further.  Renders as
    /// JSON.
    Array,
    /// A nested stanza — has children, not a leaf.
    Stanza,
    /// A JSON object that isn't a `Stanza` (i.e. a map
    /// of dynamic keys, like `llm.providers`).  Phase 2
    /// gives it a dedicated map widget.
    #[allow(dead_code)]
    Map,
    /// Anything else — JSON null, mixed, etc.
    Unknown,
}

impl ConfigType {
    pub fn infer(value: &Value) -> Self {
        match value {
            Value::Null => Self::Unknown,
            Value::Bool(_) => Self::Bool,
            Value::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    Self::Int
                } else {
                    Self::Float
                }
            }
            Value::String(_) => Self::String,
            Value::Array(arr) => {
                if arr.iter().all(|v| v.is_string()) {
                    Self::StringList
                } else {
                    Self::Array
                }
            }
            Value::Object(_) => Self::Stanza,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Float => "float",
            Self::String => "string",
            Self::StringList => "list of strings",
            Self::Array => "array",
            Self::Stanza => "stanza",
            Self::Map => "map",
            Self::Unknown => "unknown",
        }
    }
}

/// Source of a leaf's current value at file-load time.
///
///   * `Default`      — not present in `inkhaven.hjson`;
///                      the displayed value is the
///                      built-in default.
///   * `Configured`   — present in HJSON, value taken
///                      from the file.
///   * `Unknown`      — present in HJSON, but **not in
///                      the schema** — i.e. a user-added
///                      field outside the inkhaven
///                      schema.  Preserved on save (see
///                      proposal §6.6) but not editable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueSource {
    Default,
    Configured,
    // Reserved for Phase 2 — the surgical-rewrite save
    // pipeline will tag values that came from outside
    // the schema.  Phase 1 routes those into the
    // `unknowns` Vec instead.
    #[allow(dead_code)]
    Unknown,
}

/// One node in the config tree.  Both leaves and
/// stanzas use the same shape; `children` is empty for
/// leaves.
#[derive(Debug, Clone)]
pub struct SchemaNode {
    /// Dotted path from the root of `Config`, e.g.
    /// `editor.style_warnings.enabled`.  Empty string
    /// for the synthetic root.
    pub path: String,
    /// Display name — the last path segment, or
    /// `"<root>"` for the root.
    pub display: String,
    /// Inferred type.  Stanzas have `Stanza`; leaves
    /// have their JSON-derived type.
    pub ty: ConfigType,
    /// Default value as JSON.  For stanzas: `Object`
    /// (recursive defaults of children).  For leaves:
    /// the scalar / array default.
    pub default: Value,
    /// Current effective value: configured value if
    /// present in HJSON, else default.
    pub current: Value,
    /// Where `current` came from.
    pub source: ValueSource,
    /// Child nodes for stanzas; empty for leaves.
    /// Sorted by display name.
    pub children: Vec<SchemaNode>,
}

impl SchemaNode {
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
            && !matches!(self.ty, ConfigType::Stanza | ConfigType::Map)
    }

    /// Flatten the tree into `(path, depth, &node)` in
    /// pre-order, skipping subtrees whose path is in
    /// `collapsed`.  Used by the tree renderer.
    pub fn flatten<'a>(
        &'a self,
        collapsed: &std::collections::HashSet<String>,
        out: &mut Vec<(usize, &'a SchemaNode)>,
        depth: usize,
    ) {
        out.push((depth, self));
        if collapsed.contains(&self.path) {
            return;
        }
        for child in &self.children {
            child.flatten(collapsed, out, depth + 1);
        }
    }
}

/// Build a schema tree by walking the JSON
/// representation of `Config::default()`.
///
/// `live` is the parsed HJSON from the project's
/// `inkhaven.hjson` (or `Value::Object(Map::new())` when
/// the file doesn't exist).  Each tree node's `source`
/// is set by checking whether the path is present in
/// `live`:
///   * Present + scalar match: `Configured` (value =
///     live value).
///   * Absent: `Default` (value = built-in default).
///   * Stanza: `source` rolls up via the children — a
///     stanza is `Configured` if any leaf descendant is.
///
/// Unknown fields (present in `live` but absent from the
/// defaults tree) are returned separately as a flat
/// `Vec<(path, Value)>` so the top-bar chip can show
/// the count; they do NOT appear in the schema tree
/// (see proposal §6.6).
pub fn build(
    defaults: &Value,
    live: &Value,
) -> (SchemaNode, Vec<(String, Value)>) {
    let mut unknowns: Vec<(String, Value)> = Vec::new();
    let root = build_node(
        "",
        "<root>",
        defaults,
        live,
        &mut unknowns,
    );
    (root, unknowns)
}

fn build_node(
    path: &str,
    display: &str,
    default: &Value,
    live: &Value,
    unknowns: &mut Vec<(String, Value)>,
) -> SchemaNode {
    let ty = ConfigType::infer(default);
    match default {
        Value::Object(default_map) => {
            // Build children from the default map; walk
            // live in parallel to find unknown keys.
            let live_map = live.as_object();
            let mut children: Vec<SchemaNode> =
                Vec::with_capacity(default_map.len());
            let mut any_configured = false;
            for (key, child_default) in default_map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                let child_live = live_map
                    .and_then(|m| m.get(key))
                    .cloned()
                    .unwrap_or_else(|| child_default.clone());
                let child = build_node(
                    &child_path,
                    key,
                    child_default,
                    &child_live,
                    unknowns,
                );
                if child.source == ValueSource::Configured {
                    any_configured = true;
                }
                children.push(child);
            }
            // Detect unknown keys in live (present in
            // HJSON but not in defaults).
            if let Some(map) = live_map {
                for (key, value) in map {
                    if !default_map.contains_key(key) {
                        let unknown_path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{path}.{key}")
                        };
                        collect_unknowns(&unknown_path, value, unknowns);
                    }
                }
            }
            // Sort children deterministically — keep
            // serde_json's key order for now (HJSON
            // field order tends to match struct field
            // order).  Already sorted by BTreeMap or
            // declaration order.
            children.sort_by(|a, b| a.display.cmp(&b.display));
            let source = if any_configured {
                ValueSource::Configured
            } else {
                ValueSource::Default
            };
            SchemaNode {
                path: path.to_string(),
                display: display.to_string(),
                ty,
                default: default.clone(),
                current: live.clone(),
                source,
                children,
            }
        }
        _ => {
            // Leaf: classify by source.
            let source = if live == default {
                // Could be "user set it explicitly to
                // the same value as the default" — but
                // without the byte-range index (Phase 2)
                // we can't distinguish.  Phase 1
                // policy: equal-to-default counts as
                // Default.  Phase 2's surgical rewrite
                // tracks explicit presence.
                ValueSource::Default
            } else {
                ValueSource::Configured
            };
            SchemaNode {
                path: path.to_string(),
                display: display.to_string(),
                ty,
                default: default.clone(),
                current: live.clone(),
                source,
                children: Vec::new(),
            }
        }
    }
}

fn collect_unknowns(
    path: &str,
    value: &Value,
    unknowns: &mut Vec<(String, Value)>,
) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                collect_unknowns(&child_path, child, unknowns);
            }
        }
        _ => unknowns.push((path.to_string(), value.clone())),
    }
}

/// Walk the schema tree and return a map `path → node`
/// for direct lookup (used by the help pane + future
/// surgical-rewrite save pipeline).  Skips the synthetic
/// root.  Unused in Phase 1; the help-pane lookup goes
/// through `App::current_node()`.  Kept public so Phase
/// 2's save pipeline can index without re-walking.
#[allow(dead_code)]
pub fn index_by_path(root: &SchemaNode) -> BTreeMap<String, &SchemaNode> {
    let mut out = BTreeMap::new();
    fn walk<'a>(
        node: &'a SchemaNode,
        out: &mut BTreeMap<String, &'a SchemaNode>,
    ) {
        if !node.path.is_empty() {
            out.insert(node.path.clone(), node);
        }
        for child in &node.children {
            walk(child, out);
        }
    }
    walk(root, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_live_marks_everything_default() {
        let defaults = json!({
            "editor": {
                "autosave_seconds": 5,
                "wrap": true,
            },
        });
        let live = json!({});
        let (root, unknowns) = build(&defaults, &live);
        let editor = &root.children[0];
        assert_eq!(editor.path, "editor");
        assert_eq!(editor.source, ValueSource::Default);
        for leaf in &editor.children {
            assert_eq!(leaf.source, ValueSource::Default);
        }
        assert!(unknowns.is_empty());
    }

    #[test]
    fn configured_leaf_marked_configured() {
        let defaults = json!({
            "editor": { "autosave_seconds": 5 },
        });
        let live = json!({
            "editor": { "autosave_seconds": 30 },
        });
        let (root, _) = build(&defaults, &live);
        let editor = &root.children[0];
        assert_eq!(editor.source, ValueSource::Configured);
        let leaf = &editor.children[0];
        assert_eq!(leaf.source, ValueSource::Configured);
        assert_eq!(leaf.current, json!(30));
        assert_eq!(leaf.default, json!(5));
    }

    #[test]
    fn unknown_field_collected_not_in_tree() {
        let defaults = json!({ "editor": { "wrap": true } });
        let live = json!({
            "editor": { "wrap": false },
            "experimental": { "my_custom_flag": "yes" },
        });
        let (root, unknowns) = build(&defaults, &live);
        // Only "editor" in the tree.
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].display, "editor");
        // The unknown field is reported separately.
        assert_eq!(unknowns.len(), 1);
        assert_eq!(unknowns[0].0, "experimental.my_custom_flag");
    }

    #[test]
    fn type_inference_recognises_string_list() {
        let defaults = json!({
            "editor": { "extra_words": ["just", "very"] },
        });
        let (root, _) = build(&defaults, &json!({}));
        let editor = &root.children[0];
        let leaf = &editor.children[0];
        assert!(matches!(leaf.ty, ConfigType::StringList));
    }

    #[test]
    fn flatten_skips_collapsed_subtree() {
        let defaults = json!({
            "a": { "b": 1, "c": 2 },
            "d": 3,
        });
        let (root, _) = build(&defaults, &json!({}));
        let mut out = Vec::new();
        let mut collapsed = std::collections::HashSet::new();
        collapsed.insert("a".to_string());
        root.flatten(&collapsed, &mut out, 0);
        // root + a + d (a's children skipped)
        let displays: Vec<&str> =
            out.iter().map(|(_, n)| n.display.as_str()).collect();
        assert_eq!(displays, vec!["<root>", "a", "d"]);
    }

    #[test]
    fn index_by_path_skips_root() {
        let defaults = json!({
            "editor": { "wrap": true },
        });
        let (root, _) = build(&defaults, &json!({}));
        let idx = index_by_path(&root);
        assert!(idx.contains_key("editor"));
        assert!(idx.contains_key("editor.wrap"));
        assert!(!idx.contains_key(""));
    }
}
