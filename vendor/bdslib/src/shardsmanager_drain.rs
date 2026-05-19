//! Instance-scoped drain3 log-template mining on [`ShardsManager`].
//!
//! These methods mirror [`DrainParser::parse_json`] and [`DrainParser::load_templates`]
//! but operate against a specific `ShardsManager` instance rather than the
//! process-wide global singleton.  Use them when you hold a `ShardsManager`
//! directly (e.g. in tests or multi-tenant setups).

use crate::common::drain::{DrainParser, ParseJsonResult};
use crate::common::error::Result;
use crate::shardsmanager::ShardsManager;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

impl ShardsManager {
    /// Parse a JSON document with `parser` and store any newly discovered or
    /// updated drain templates in this manager's tplstorage.
    ///
    /// This is the instance-scoped equivalent of [`DrainParser::parse_json`].
    /// The same content-extraction rules apply: the log string is read from
    /// `doc["data"]`, `doc["data"]["value"]`, or `doc["data"]["message"]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `doc` does not contain a parseable log string, or if
    /// writing the discovered template to tplstorage fails.
    pub fn drain_parse_json(
        &self,
        parser: &mut DrainParser,
        doc: &JsonValue,
    ) -> Result<ParseJsonResult> {
        parser.parse_json_with_callback(doc, |meta, body| self.tpl_add(meta, &body))
    }

    /// Build a [`DrainParser`] pre-seeded with all drain templates stored in
    /// this manager for the given lookback `duration`.
    ///
    /// `duration` is a human-readable string such as `"1h"` or `"7days"`.
    /// This is the instance-scoped equivalent of [`DrainParser::load_templates`].
    ///
    /// Returns `(parser, cluster_map)` where `cluster_map` maps each seeded
    /// in-memory cluster ID to the stored template UUID.
    pub fn drain_load(&self, duration: &str) -> Result<(DrainParser, HashMap<usize, Uuid>)> {
        let entries = self.tpl_list(duration)?;
        DrainParser::from_tpl_list(entries)
    }
}
