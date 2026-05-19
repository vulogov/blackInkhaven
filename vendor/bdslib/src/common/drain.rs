use std::collections::HashMap;
use std::path::Path;
use regex::Regex;
use serde::{Deserialize, Serialize};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogCluster {
    pub template: Vec<String>,
    pub id: usize,
    pub size: usize,
}

/// Describes what happened to the cluster after a call to [`DrainParser::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeType {
    /// A brand-new cluster was created for this line.
    New,
    /// The line matched an existing cluster and caused at least one template
    /// token to be replaced with `"<*>"`.
    Updated,
    /// The line matched an existing cluster with no change to the template.
    None,
}

/// Return value of [`DrainParser::parse`].
///
/// Derefs to [`LogCluster`] so fields like `.id`, `.size`, and `.template` are
/// accessible directly without going through `.cluster`.
pub struct ParseResult<'a> {
    pub cluster: &'a LogCluster,
    pub change_type: ChangeType,
}

/// Owned return value of [`DrainParser::parse_json`].
///
/// Unlike [`ParseResult`] this type owns its data and does not borrow the
/// parser, so callers can use the cluster information after the parser has been
/// mutated again.
#[derive(Debug, Clone)]
pub struct ParseJsonResult {
    pub cluster_id: usize,
    pub template: Vec<String>,
    pub cluster_size: usize,
    pub change_type: ChangeType,
    /// UUID assigned by `store_fn` when a new or updated template was persisted.
    /// `None` for `ChangeType::None` (existing template, no change).
    pub stored_id: Option<uuid::Uuid>,
}

impl<'a> std::ops::Deref for ParseResult<'a> {
    type Target = LogCluster;
    fn deref(&self) -> &LogCluster {
        self.cluster
    }
}

// ── Internal tree ─────────────────────────────────────────────────────────────

enum Node {
    Internal(HashMap<String, Node>),
    Leaf(Vec<usize>),
}

// Serialisable mirror of Node — keeps serde off the hot-path runtime type.
#[derive(Serialize, Deserialize)]
enum NodeSer {
    Internal(HashMap<String, NodeSer>),
    Leaf(Vec<usize>),
}

impl NodeSer {
    fn from_ref(n: &Node) -> Self {
        match n {
            Node::Internal(m) => NodeSer::Internal(
                m.iter().map(|(k, v)| (k.clone(), NodeSer::from_ref(v))).collect(),
            ),
            Node::Leaf(v) => NodeSer::Leaf(v.clone()),
        }
    }
}

impl From<NodeSer> for Node {
    fn from(n: NodeSer) -> Self {
        match n {
            NodeSer::Internal(m) => {
                Node::Internal(m.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
            NodeSer::Leaf(v) => Node::Leaf(v),
        }
    }
}

// ── Serialisable snapshot ─────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct DrainSnapshot {
    depth: usize,
    sim_threshold: f64,
    max_children: usize,
    mask_patterns: Vec<String>,
    clusters: Vec<LogCluster>,
    next_id: usize,
    root: NodeSer,
}

// ── DrainParser ───────────────────────────────────────────────────────────────

pub struct DrainParser {
    depth: usize,
    sim_threshold: f64,
    max_children: usize,
    mask_patterns: Vec<String>,
    masks: Vec<Regex>,
    root: Node,
    clusters: Vec<LogCluster>,
    next_id: usize,
}

impl DrainParser {
    /// Create a parser with the default mask set (digit tokens and hex
    /// addresses) and the given tuning parameters.
    ///
    /// Use [`DrainParserBuilder`] when you need custom masking patterns.
    pub fn new(depth: usize, sim_threshold: f64, max_children: usize) -> Self {
        DrainParserBuilder::new()
            .depth(depth)
            .sim_threshold(sim_threshold)
            .max_children(max_children)
            .build()
            .expect("default mask patterns are valid")
    }

    // ── Core parsing ──────────────────────────────────────────────────────────

    fn preprocess(&self, content: &str) -> Vec<String> {
        content
            .split_whitespace()
            .map(|s| {
                let res = s.to_string();
                for re in &self.masks {
                    if re.is_match(&res) {
                        return "<*>".to_string();
                    }
                }
                res
            })
            .collect()
    }

    fn traverse_to_leaf(&mut self, tokens: &[String], log_len: usize) -> &mut Vec<usize> {
        let mut current_node = &mut self.root;

        current_node = match current_node {
            Node::Internal(children) => children
                .entry(log_len.to_string())
                .or_insert(Node::Internal(HashMap::new())),
            _ => unreachable!(),
        };

        for i in 0..(self.depth - 2).min(log_len) {
            let token = tokens[i].clone();
            let is_last = i == self.depth - 3;
            let make = move || {
                if is_last {
                    Node::Leaf(Vec::new())
                } else {
                    Node::Internal(HashMap::new())
                }
            };
            current_node = match current_node {
                Node::Internal(children) => {
                    if children.len() < self.max_children || children.contains_key(&token) {
                        children.entry(token).or_insert_with(make)
                    } else {
                        children.entry("<*>".to_string()).or_insert_with(make)
                    }
                }
                _ => unreachable!(),
            };
        }

        match current_node {
            Node::Leaf(indices) => indices,
            _ => panic!("Tree depth logic mismatch"),
        }
    }

    /// Process one log line.
    ///
    /// Returns a [`ParseResult`] containing a reference to the matched or newly
    /// created cluster and a [`ChangeType`] that indicates what happened.
    /// The result borrows `self`; drop it before calling `parse` again.
    pub fn parse(&mut self, content: &str) -> ParseResult<'_> {
        let tokens = self.preprocess(content);
        let log_len = tokens.len();

        // Snapshot leaf indices to release the mutable borrow on the tree.
        let candidates: Vec<usize> = self.traverse_to_leaf(&tokens, log_len).clone();

        let best_match = candidates
            .iter()
            .copied()
            .filter_map(|idx| {
                let sim = Self::similarity(&self.clusters[idx].template, &tokens);
                if sim >= self.sim_threshold {
                    Some((idx, sim))
                } else {
                    None
                }
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        if let Some((idx, _)) = best_match {
            let change_type = Self::change_type_for(&self.clusters[idx].template, &tokens);
            self.update_template(idx, &tokens);
            ParseResult { cluster: &self.clusters[idx], change_type }
        } else {
            let new_idx = self.clusters.len();
            self.clusters.push(LogCluster {
                template: tokens.clone(),
                id: self.next_id,
                size: 1,
            });
            self.next_id += 1;
            self.traverse_to_leaf(&tokens, log_len).push(new_idx);
            ParseResult { cluster: &self.clusters[new_idx], change_type: ChangeType::New }
        }
    }

    /// All clusters discovered so far, in creation order.
    pub fn clusters(&self) -> &[LogCluster] {
        &self.clusters
    }

    fn similarity(template: &[String], tokens: &[String]) -> f64 {
        if template.len() != tokens.len() {
            return 0.0;
        }
        let matches = template.iter().zip(tokens.iter()).filter(|(a, b)| a == b).count();
        matches as f64 / template.len() as f64
    }

    fn change_type_for(template: &[String], tokens: &[String]) -> ChangeType {
        for (t, tok) in template.iter().zip(tokens.iter()) {
            if t != "<*>" && t != tok {
                return ChangeType::Updated;
            }
        }
        ChangeType::None
    }

    fn update_template(&mut self, idx: usize, tokens: &[String]) {
        let cluster = &mut self.clusters[idx];
        cluster.size += 1;
        for (t, tok) in cluster.template.iter_mut().zip(tokens.iter()) {
            if t != tok {
                *t = "<*>".to_string();
            }
        }
    }

    // ── Seed / global-DB integration ─────────────────────────────────────────

    /// Inject a pre-built template directly into the parser without running the
    /// full parse pipeline.
    ///
    /// Use [`load_templates`] to bulk-seed a fresh parser from stored drain
    /// templates rather than building this method manually.
    pub fn seed_cluster(&mut self, template: Vec<String>) {
        let log_len = template.len();
        let idx = self.clusters.len();
        self.clusters.push(LogCluster {
            template: template.clone(),
            id: self.next_id,
            size: 1,
        });
        self.next_id += 1;
        self.traverse_to_leaf(&template, log_len).push(idx);
    }

    /// Parse a JSON document; call `store_fn` for any new or updated template.
    ///
    /// This is the low-level variant used by both [`parse_json`][Self::parse_json]
    /// (global DB) and [`ShardsManager::drain_parse_json`] (instance-scoped).
    ///
    /// # Content extraction
    ///
    /// The log string is extracted from `doc` in this order:
    /// 1. `doc["data"]` — plain string.
    /// 2. `doc["data"]["value"]` — object with a `"value"` key.
    /// 3. `doc["data"]["message"]` — object with a `"message"` key.
    ///
    /// # Timestamp
    ///
    /// Extracted from `doc["timestamp"]` (Unix seconds).  Falls back to `now`
    /// if the field is absent or invalid.
    ///
    /// # Storage
    ///
    /// For [`ChangeType::New`] and [`ChangeType::Updated`] results `store_fn` is
    /// called with `(metadata, body_bytes)`.  The metadata has the shape:
    /// ```json
    /// { "name": "<template>", "type": "drain_template",
    ///   "cluster_id": <n>, "timestamp": <unix>, "created_at": <unix> }
    /// ```
    pub fn parse_json_with_callback<F>(
        &mut self,
        doc: &serde_json::Value,
        store_fn: F,
    ) -> crate::common::error::Result<ParseJsonResult>
    where
        F: FnOnce(serde_json::Value, Vec<u8>) -> crate::common::error::Result<uuid::Uuid>,
    {
        // Extract log string.
        let content = match doc.get("data") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Object(obj)) => {
                if let Some(serde_json::Value::String(s)) = obj.get("value") {
                    s.clone()
                } else if let Some(serde_json::Value::String(s)) = obj.get("message") {
                    s.clone()
                } else {
                    return Err(crate::common::error::err_msg(
                        "doc['data'] object has neither 'value' nor 'message' string fields",
                    ));
                }
            }
            _ => {
                return Err(crate::common::error::err_msg(
                    "document must have a 'data' field that is a string or object",
                ));
            }
        };
        if content.is_empty() {
            return Err(crate::common::error::err_msg("extracted log string is empty"));
        }

        // Timestamp: best-effort, fallback to now.
        let ts_secs = match crate::common::time::extract_timestamp(doc) {
            Ok(t) => t
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            Err(_) => crate::common::time::now_secs(),
        };

        let result = self.parse(&content);
        let mut owned = ParseJsonResult {
            cluster_id: result.cluster.id,
            template: result.cluster.template.clone(),
            cluster_size: result.cluster.size,
            change_type: result.change_type.clone(),
            stored_id: None,
        };
        drop(result);

        if owned.change_type == ChangeType::New || owned.change_type == ChangeType::Updated {
            let template_str = owned.template.join(" ");
            let metadata = serde_json::json!({
                "name": template_str,
                "type": "drain_template",
                "cluster_id": owned.cluster_id,
                "timestamp": ts_secs,
                "created_at": crate::common::time::now_secs(),
            });
            let body = template_str.into_bytes();
            owned.stored_id = Some(store_fn(metadata, body)?);
        }

        Ok(owned)
    }

    /// Parse a JSON document and persist any new or updated templates to the
    /// global [`ShardsManager`].
    ///
    /// Delegates to [`parse_json_with_callback`][Self::parse_json_with_callback].
    /// For instance-scoped storage use [`ShardsManager::drain_parse_json`].
    pub fn parse_json(
        &mut self,
        doc: &serde_json::Value,
    ) -> crate::common::error::Result<ParseJsonResult> {
        self.parse_json_with_callback(doc, |meta, body| {
            crate::globals::get_db()?.tpl_add(meta, &body)
        })
    }

    /// Build a [`DrainParser`] pre-seeded from a list of `(id, metadata)` pairs
    /// previously returned by [`ShardsManager::tpl_list`].
    ///
    /// Only entries whose metadata has `"type": "drain_template"` are imported.
    /// The template string is read from the `"name"` field and split on whitespace
    /// to reconstruct the token sequence.
    ///
    /// Returns `(parser, cluster_map)` where `cluster_map` maps each seeded
    /// in-memory cluster ID (0, 1, 2 …) to the stored template UUID.  The caller
    /// can use this map to record frequency observations when the cluster is
    /// matched later via `parse_json_with_callback`.
    pub fn from_tpl_list(
        entries: Vec<(uuid::Uuid, serde_json::Value)>,
    ) -> crate::common::error::Result<(Self, HashMap<usize, uuid::Uuid>)> {
        let mut parser = DrainParserBuilder::new()
            .build()
            .map_err(|e| crate::common::error::err_msg(format!("failed to build parser: {e}")))?;
        let mut cluster_map: HashMap<usize, uuid::Uuid> = HashMap::new();
        for (uuid, meta) in entries {
            if meta.get("type").and_then(|v| v.as_str()) != Some("drain_template") {
                continue;
            }
            if let Some(tpl_str) = meta.get("name").and_then(|v| v.as_str()) {
                let tokens: Vec<String> = tpl_str.split_whitespace().map(str::to_owned).collect();
                if !tokens.is_empty() {
                    let cluster_id = parser.next_id;
                    parser.seed_cluster(tokens);
                    cluster_map.insert(cluster_id, uuid);
                }
            }
        }
        Ok((parser, cluster_map))
    }

    /// Build a [`DrainParser`] pre-seeded with all drain templates stored in the
    /// global [`ShardsManager`] for the given lookback `duration`.
    ///
    /// Delegates to [`from_tpl_list`][Self::from_tpl_list].
    /// For instance-scoped loading use [`ShardsManager::drain_load`].
    pub fn load_templates(duration: &str) -> crate::common::error::Result<Self> {
        let entries = crate::globals::get_db()?.tpl_list(duration)?;
        let (parser, _) = Self::from_tpl_list(entries)?;
        Ok(parser)
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    fn to_snapshot(&self) -> DrainSnapshot {
        DrainSnapshot {
            depth: self.depth,
            sim_threshold: self.sim_threshold,
            max_children: self.max_children,
            mask_patterns: self.mask_patterns.clone(),
            clusters: self.clusters.clone(),
            next_id: self.next_id,
            root: NodeSer::from_ref(&self.root),
        }
    }

    fn from_snapshot(snap: DrainSnapshot) -> Result<Self, regex::Error> {
        let masks = snap
            .mask_patterns
            .iter()
            .map(|p| Regex::new(p))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            depth: snap.depth,
            sim_threshold: snap.sim_threshold,
            max_children: snap.max_children,
            mask_patterns: snap.mask_patterns,
            masks,
            root: snap.root.into(),
            clusters: snap.clusters,
            next_id: snap.next_id,
        })
    }

    /// Serialise the full parser state to a compact JSON string.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(&self.to_snapshot())
    }

    /// Serialise the full parser state to a pretty-printed JSON string.
    pub fn to_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(&self.to_snapshot())
    }

    /// Restore a parser from a JSON string produced by [`Self::to_json`] or
    /// [`Self::to_json_pretty`].
    pub fn from_json(s: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let snap: DrainSnapshot = serde_json::from_str(s)?;
        Ok(Self::from_snapshot(snap)?)
    }

    /// Write the parser state to a JSON file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(path, self.to_json_pretty()?)?;
        Ok(())
    }

    /// Load a parser from a JSON file written by [`Self::save`].
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        Self::from_json(&std::fs::read_to_string(path)?)
    }
}

// ── Builder ───────────────────────────────────────────────────────────────────

/// Fluent builder for [`DrainParser`] with configurable masking patterns.
///
/// ```
/// use drainlib::DrainParserBuilder;
///
/// let mut parser = DrainParserBuilder::new()
///     .depth(4)
///     .sim_threshold(0.5)
///     .add_mask(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}")  // IPv4
///     .add_mask(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")  // UUID
///     .build()
///     .unwrap();
/// ```
pub struct DrainParserBuilder {
    depth: usize,
    sim_threshold: f64,
    max_children: usize,
    mask_patterns: Vec<String>,
}

impl Default for DrainParserBuilder {
    fn default() -> Self {
        Self {
            // depth=3 → route only on tokens[0] (the category/verb prefix).
            // This ensures that all log lines with the same leading word share
            // the same leaf and are similarity-compared regardless of how many
            // distinct values appear at later token positions.  depth=4 (route
            // on tokens[0] + tokens[1]) requires max_children distinct second
            // tokens before the wildcard bucket is used — unhelpful on small
            // datasets where variable positions sit at tokens[1].
            depth: 3,
            sim_threshold: 0.5,
            max_children: 100,
            mask_patterns: vec![
                r"\d+".to_string(),
                r"(0x)[0-9a-fA-F]+".to_string(),
            ],
        }
    }
}

impl DrainParserBuilder {
    /// Start with sensible defaults and the standard digit / hex masks.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn depth(mut self, d: usize) -> Self {
        self.depth = d;
        self
    }

    pub fn sim_threshold(mut self, t: f64) -> Self {
        self.sim_threshold = t;
        self
    }

    pub fn max_children(mut self, n: usize) -> Self {
        self.max_children = n;
        self
    }

    /// Append one regex pattern to the mask list.  Any token whose string
    /// representation matches the pattern (via `is_match`) is replaced with
    /// `"<*>"` during preprocessing.
    pub fn add_mask(mut self, pattern: impl Into<String>) -> Self {
        self.mask_patterns.push(pattern.into());
        self
    }

    /// Replace the entire mask list (including the defaults).
    pub fn mask_patterns(mut self, patterns: Vec<String>) -> Self {
        self.mask_patterns = patterns;
        self
    }

    /// Build the parser.  Returns `Err` if any mask pattern is not a valid
    /// regex.
    pub fn build(self) -> Result<DrainParser, regex::Error> {
        let masks = self
            .mask_patterns
            .iter()
            .map(|p| Regex::new(p))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DrainParser {
            depth: self.depth,
            sim_threshold: self.sim_threshold,
            max_children: self.max_children,
            mask_patterns: self.mask_patterns,
            masks,
            root: Node::Internal(HashMap::new()),
            clusters: Vec::new(),
            next_id: 0,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parser() -> DrainParser {
        DrainParser::new(4, 0.5, 100)
    }

    fn tpl(r: &ParseResult<'_>) -> String {
        r.template.join(" ")
    }

    // ── core parsing ──────────────────────────────────────────────────────────

    #[test]
    fn new_line_creates_cluster() {
        let mut p = parser();
        let r = p.parse("user login success");
        assert_eq!(r.id, 0);
        assert_eq!(r.size, 1);
        assert_eq!(tpl(&r), "user login success");
        assert_eq!(r.change_type, ChangeType::New);
    }

    #[test]
    fn identical_lines_merge() {
        let mut p = parser();
        let id = p.parse("connected to host").id;
        let r = p.parse("connected to host");
        assert_eq!(r.id, id);
        assert_eq!(r.size, 2);
        assert_eq!(r.change_type, ChangeType::None);
    }

    #[test]
    fn differing_token_becomes_wildcard() {
        // depth=4 routes on tokens[0] and tokens[1]; put the variable token at
        // position [3] so both lines reach the same leaf before similarity search.
        let mut p = parser();
        p.parse("user logged in alice");
        let r = p.parse("user logged in bob");
        assert_eq!(tpl(&r), "user logged in <*>");
        assert_eq!(r.size, 2);
        assert_eq!(r.change_type, ChangeType::Updated);
    }

    #[test]
    fn numeric_token_preprocessed_to_wildcard() {
        let mut p = parser();
        p.parse("request took 120 ms");
        let r = p.parse("request took 95 ms");
        assert_eq!(tpl(&r), "request took <*> ms");
        assert_eq!(r.size, 2);
    }

    #[test]
    fn hex_token_preprocessed_to_wildcard() {
        let mut p = parser();
        let r = p.parse("addr 0xdeadbeef allocated");
        assert_eq!(tpl(&r), "addr <*> allocated");
    }

    #[test]
    fn different_lengths_produce_separate_clusters() {
        let mut p = parser();
        let id1 = p.parse("disk full").id;
        let id2 = p.parse("disk almost full now").id;
        assert_ne!(id1, id2);
        assert_eq!(p.clusters().len(), 2);
    }

    #[test]
    fn below_threshold_creates_new_cluster() {
        // sim_threshold = 0.9 — only 1/4 tokens match, well below threshold
        let mut p = DrainParser::new(4, 0.9, 100);
        p.parse("alpha beta gamma delta");
        let size = p.parse("alpha zzzz yyyy xxxx").size;
        assert_eq!(p.clusters().len(), 2, "dissimilar line should not merge");
        assert_eq!(size, 1);
    }

    #[test]
    fn multiple_variables_in_template() {
        let mut p = parser();
        p.parse("ERROR port 8080 host db1 failed");
        let r = p.parse("ERROR port 9090 host db2 failed");
        assert_eq!(tpl(&r), "ERROR port <*> host <*> failed");
    }

    #[test]
    fn cluster_ids_are_monotone() {
        let mut p = parser();
        p.parse("alpha bravo charlie delta");
        p.parse("one two three four");
        p.parse("foo bar baz qux");
        let ids: Vec<usize> = p.clusters().iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![0, 1, 2]);
    }

    #[test]
    fn size_tracks_merged_count() {
        let mut p = parser();
        for i in 0..5usize {
            p.parse(&format!("worker {} started", i));
        }
        assert_eq!(p.clusters().len(), 1);
        assert_eq!(p.clusters()[0].size, 5);
    }

    // ── change type ───────────────────────────────────────────────────────────

    #[test]
    fn change_type_new_on_first_line() {
        let mut p = parser();
        assert_eq!(p.parse("service started ok").change_type, ChangeType::New);
    }

    #[test]
    fn change_type_none_on_repeat() {
        let mut p = parser();
        p.parse("service started ok");
        assert_eq!(p.parse("service started ok").change_type, ChangeType::None);
    }

    #[test]
    fn change_type_updated_on_new_variable() {
        let mut p = parser();
        p.parse("user logged in alice");
        assert_eq!(
            p.parse("user logged in bob").change_type,
            ChangeType::Updated,
        );
    }

    #[test]
    fn change_type_none_after_template_stabilises() {
        let mut p = parser();
        p.parse("user logged in alice");
        p.parse("user logged in bob"); // template now has <*>
        // A third distinct username: template already has <*> at that pos → None
        assert_eq!(
            p.parse("user logged in carol").change_type,
            ChangeType::None,
        );
    }

    // ── builder ───────────────────────────────────────────────────────────────

    #[test]
    fn builder_add_mask_replaces_matching_tokens() {
        let mut p = DrainParserBuilder::new()
            .add_mask(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}") // IPv4
            .build()
            .unwrap();
        let r = p.parse("connection from 192.168.1.10 accepted");
        assert_eq!(tpl(&r), "connection from <*> accepted");
    }

    #[test]
    fn builder_custom_patterns_replace_defaults() {
        // Replace defaults with a pattern that matches only pure uppercase words.
        let mut p = DrainParserBuilder::new()
            .mask_patterns(vec![r"^[A-Z]+$".to_string()])
            .build()
            .unwrap();
        // "ERROR" is all-caps → masked; "123" is not → kept as-is
        let r = p.parse("ERROR code 123");
        assert_eq!(tpl(&r), "<*> code 123");
    }

    #[test]
    fn builder_invalid_pattern_returns_err() {
        assert!(DrainParserBuilder::new().add_mask(r"[invalid").build().is_err());
    }

    // ── persistence ───────────────────────────────────────────────────────────

    #[test]
    fn json_round_trip_preserves_clusters() {
        let mut p = parser();
        p.parse("user logged in alice");
        p.parse("user logged in bob");

        let json = p.to_json().unwrap();
        let mut p2 = DrainParser::from_json(&json).unwrap();

        // Existing clusters survive.
        assert_eq!(p2.clusters().len(), 1);
        assert_eq!(p2.clusters()[0].template.join(" "), "user logged in <*>");
        assert_eq!(p2.clusters()[0].size, 2);

        // Continued parsing routes correctly and merges.
        let r = p2.parse("user logged in carol");
        assert_eq!(r.id, 0);
        assert_eq!(r.size, 3);
        assert_eq!(r.change_type, ChangeType::None);
    }

    #[test]
    fn json_round_trip_preserves_next_id() {
        let mut p = parser();
        p.parse("alpha bravo charlie delta");
        p.parse("one two three four");

        let mut p2 = DrainParser::from_json(&p.to_json().unwrap()).unwrap();
        let r = p2.parse("entirely new unique line here");
        assert_eq!(r.id, 2, "id must continue from where the original left off");
    }

    #[test]
    fn file_save_load_round_trip() {
        let path = std::env::temp_dir().join("drainlib_test.json");
        let mut p = parser();
        p.parse("disk usage at high level");
        p.parse("disk usage at low level");
        p.save(&path).unwrap();

        let mut p2 = DrainParser::load(&path).unwrap();
        assert_eq!(p2.clusters().len(), 1);
        let r = p2.parse("disk usage at critical level");
        assert_eq!(r.size, 3);
        assert_eq!(r.change_type, ChangeType::None);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn json_round_trip_preserves_custom_masks() {
        let mut p = DrainParserBuilder::new()
            .add_mask(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}")
            .build()
            .unwrap();
        p.parse("connect from 10.0.0.1 ok");

        let mut p2 = DrainParser::from_json(&p.to_json().unwrap()).unwrap();
        // IP mask must survive round-trip.
        let r = p2.parse("connect from 172.16.0.5 ok");
        assert_eq!(r.id, 0, "should merge into same cluster");
    }
}
