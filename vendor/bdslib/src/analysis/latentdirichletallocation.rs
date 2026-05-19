use crate::common::error::{err_msg, Result};
use crate::common::jsonfingerprint::json_fingerprint;
use crate::globals::get_db;
use latentdirichletallocation::Lda;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── configuration ─────────────────────────────────────────────────────────────

/// All tunable parameters for the LDA `from_documents` call and training loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdaConfig {
    /// Number of topics (`k`).  Clamped to `n_docs` when the corpus is small.
    pub k: usize,
    /// Dirichlet prior for document-topic distributions (`alpha`).
    /// Smaller values produce sparser, more focused topic assignments.
    pub alpha: f64,
    /// Dirichlet prior for topic-word distributions (`beta`).
    /// Smaller values produce sparser per-topic vocabulary.
    pub beta: f64,
    /// RNG seed passed to `from_documents` for reproducible runs.
    pub seed: u64,
    /// Number of collapsed Gibbs sampling iterations (`iters`).
    pub iters: usize,
    /// Top-N words extracted per topic before merging into the final keyword set.
    pub top_n: usize,
}

impl Default for LdaConfig {
    fn default() -> Self {
        Self {
            k: 3,
            alpha: 0.1,
            beta: 0.01,
            seed: 42,
            iters: 200,
            top_n: 10,
        }
    }
}

// ── output ────────────────────────────────────────────────────────────────────

/// Result of a topic-modelling pass over a telemetry/log corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicSummary {
    /// Metric key that was queried.
    pub key: String,
    /// Start of the queried window (Unix seconds, inclusive).
    pub start: u64,
    /// End of the queried window (Unix seconds, exclusive).
    pub end: u64,
    /// Number of documents used as input.
    pub n_docs: usize,
    /// Number of LDA topics that were modelled (≤ `config.k`).
    pub n_topics: usize,
    /// Sorted, comma-separated keywords distilled from all topics.
    ///
    /// Keywords appear at most once regardless of how many topics surface them.
    /// Empty string when the corpus is empty or contains no tokenisable words.
    pub keywords: String,
}

impl TopicSummary {
    /// Run LDA over documents for `key` in the absolute window
    /// `[start_secs, end_secs)`.
    ///
    /// Requires [`init_db`](crate::init_db) to have been called.
    pub fn query(key: &str, start_secs: u64, end_secs: u64, config: LdaConfig) -> Result<Self> {
        let db = get_db()?;

        let start_ts = UNIX_EPOCH + Duration::from_secs(start_secs);
        let end_ts = UNIX_EPOCH + Duration::from_secs(end_secs);

        let mut texts: Vec<String> = Vec::new();
        for info in db.cache().info().shards_in_range(start_ts, end_ts)? {
            let shard = db.cache().shard(info.start_time)?;
            for doc in shard.get_primaries_by_key(key)? {
                let ts = doc["timestamp"].as_u64().unwrap_or(0);
                if ts >= start_secs && ts < end_secs {
                    let text = doc_to_text(&doc);
                    if !text.trim().is_empty() {
                        texts.push(text);
                    }
                }
            }
        }

        build(key, start_secs, end_secs, texts, config)
    }

    /// Run LDA over documents for `key` in the lookback window
    /// `[now − duration, now)`.
    ///
    /// `duration` uses humantime notation (`"1h"`, `"30min"`, `"7days"`).
    /// Requires [`init_db`](crate::init_db) to have been called.
    pub fn query_window(key: &str, duration: &str, config: LdaConfig) -> Result<Self> {
        let dur = humantime::parse_duration(duration)
            .map_err(|e| err_msg(format!("invalid duration '{duration}': {e}")))?;
        let now = SystemTime::now();
        let start = now.checked_sub(dur).unwrap_or(UNIX_EPOCH);
        let start_secs = start.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let end_secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        Self::query(key, start_secs, end_secs, config)
    }

    /// Run LDA over every distinct primary key found in the lookback window
    /// `[now − duration, now)`, returning one [`TopicSummary`] per key.
    ///
    /// Keys are collected from all shards that overlap the window, deduplicated,
    /// and processed in alphabetical order. The same `config` is used for every
    /// key. Requires [`init_db`](crate::init_db) to have been called.
    pub fn query_all_keys(duration: &str, config: LdaConfig) -> Result<Vec<Self>> {
        let dur = humantime::parse_duration(duration)
            .map_err(|e| err_msg(format!("invalid duration '{duration}': {e}")))?;
        let now = SystemTime::now();
        let start = now.checked_sub(dur).unwrap_or(UNIX_EPOCH);
        let start_secs = start.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let end_secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        let db = get_db()?;
        let start_ts = UNIX_EPOCH + Duration::from_secs(start_secs);
        let end_ts = UNIX_EPOCH + Duration::from_secs(end_secs);

        let mut all_keys = std::collections::BTreeSet::new();
        for info in db.cache().info().shards_in_range(start_ts, end_ts)? {
            let shard = db.cache().shard(info.start_time)?;
            let keys = shard.observability().list_primary_keys_in_range(start_ts, end_ts)?;
            all_keys.extend(keys);
        }

        let mut summaries = Vec::with_capacity(all_keys.len());
        for key in all_keys {
            summaries.push(Self::query(&key, start_secs, end_secs, config.clone())?);
        }
        Ok(summaries)
    }
}

// ── core ──────────────────────────────────────────────────────────────────────

fn build(
    key: &str,
    start: u64,
    end: u64,
    texts: Vec<String>,
    config: LdaConfig,
) -> Result<TopicSummary> {
    let n_docs = texts.len();

    if n_docs == 0 {
        return Ok(TopicSummary {
            key: key.to_string(),
            start,
            end,
            n_docs: 0,
            n_topics: 0,
            keywords: String::new(),
        });
    }

    // LDA requires k ≥ 1 and makes little sense with k > n_docs.
    let k = config.k.max(1).min(n_docs);

    let doc_refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let mut lda = Lda::from_documents(k, config.alpha, config.beta, &doc_refs, config.seed);
    lda.train(config.iters);

    // Collect top_n keywords from every topic, deduplicate, sort alphabetically.
    let mut seen = std::collections::HashSet::new();
    let mut keywords: Vec<String> = lda
        .top_words(config.top_n)
        .into_iter()
        .flat_map(|topic| topic.into_iter().map(|(word, _score)| word))
        .filter(|word| seen.insert(word.clone()))
        .collect();
    keywords.sort_unstable();

    Ok(TopicSummary {
        key: key.to_string(),
        start,
        end,
        n_docs,
        n_topics: k,
        keywords: keywords.join(", "),
    })
}

// ── text extraction ───────────────────────────────────────────────────────────

/// Convert a stored document into a plain-text string for LDA tokenisation.
///
/// The key name (dots/underscores/hyphens replaced by spaces) is prepended to
/// a `json_fingerprint` of the `data` subtree.  `json_fingerprint` emits
/// `"field: value"` pairs for every leaf — strings, numbers, and booleans —
/// so LDA receives both field-name context and content signals for all value
/// types, not just string leaves.
///
/// Example for a syslog document:
/// ```text
/// "syslog  program: sshd  message: session opened  pid: 1234  host: server-01"
/// ```
fn doc_to_text(doc: &JsonValue) -> String {
    let key_part = doc["key"]
        .as_str()
        .map(|k| k.replace(['.', '_', '-'], " "))
        .unwrap_or_default();

    let data_fp = json_fingerprint(&doc["data"]);

    match (key_part.is_empty(), data_fp.is_empty()) {
        (true, _) => data_fp,
        (_, true) => key_part,
        _ => format!("{key_part}  {data_fp}"),
    }
}
