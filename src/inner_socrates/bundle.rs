//! The `.isl` (Inner Socrates Ledger) bundle (RFC §3.16) — a portable,
//! single-file JSON archive of intent-ledger entries marked for series-level
//! reuse, so an author can carry a series' deliberate choices into the next book.
//! The export/parse here are pure + tested; the CLI wires them to the store.

use serde::{Deserialize, Serialize};

use super::intent::{IntentEntry, IntentKind, IntentScope, ScopeLevel};
use super::types::Category;
use super::{Result, SocratesError};

/// The bundle format tag + version.
const FORMAT: &str = "isl";
const VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct Bundle {
    format: String,
    version: u32,
    entries: Vec<BundleEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BundleEntry {
    id: String,
    kind: String,
    description: String,
    scope_type: String,
    scope_data: serde_json::Value,
    coverage: Vec<String>,
    scope_level: String,
}

/// Which entries an export includes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportScope {
    /// Only `scope_level: series` entries (the default — portable across books).
    Series,
    /// Only project-level entries.
    Project,
    /// Every entry.
    All,
}

/// Serialize the matching entries into an `.isl` bundle (pretty JSON).
pub fn export(entries: &[IntentEntry], scope: ExportScope) -> String {
    let chosen: Vec<BundleEntry> = entries
        .iter()
        .filter(|e| match scope {
            ExportScope::Series => e.scope_level == ScopeLevel::Series,
            ExportScope::Project => e.scope_level == ScopeLevel::Project,
            ExportScope::All => true,
        })
        .map(to_bundle_entry)
        .collect();
    let bundle = Bundle { format: FORMAT.into(), version: VERSION, entries: chosen };
    serde_json::to_string_pretty(&bundle).unwrap_or_else(|_| "{}".into())
}

/// Parse an `.isl` bundle into intent entries. Tolerant of unknown scope/kind
/// (those entries are skipped); errors only on a non-bundle / malformed file.
pub fn parse(body: &str) -> Result<Vec<IntentEntry>> {
    let bundle: Bundle =
        serde_json::from_str(body).map_err(|e| SocratesError::Parse(format!("not an .isl bundle: {e}")))?;
    if bundle.format != FORMAT {
        return Err(SocratesError::Parse(format!("unexpected bundle format `{}`", bundle.format)));
    }
    Ok(bundle.entries.iter().filter_map(from_bundle_entry).collect())
}

fn to_bundle_entry(e: &IntentEntry) -> BundleEntry {
    let (scope_type, scope_data) = scope_to_json(&e.scope);
    BundleEntry {
        id: e.id.clone(),
        kind: e.kind.id().into(),
        description: e.description.clone(),
        scope_type,
        scope_data,
        coverage: e.coverage.iter().map(|c| c.id().to_string()).collect(),
        scope_level: match e.scope_level {
            ScopeLevel::Series => "series".into(),
            ScopeLevel::Project => "project".into(),
        },
    }
}

fn from_bundle_entry(b: &BundleEntry) -> Option<IntentEntry> {
    Some(IntentEntry {
        id: b.id.clone(),
        kind: IntentKind::from_id(&b.kind)?,
        description: b.description.clone(),
        scope: scope_from_json(&b.scope_type, &b.scope_data)?,
        coverage: b.coverage.iter().filter_map(|c| Category::from_id(c)).collect(),
        scope_level: if b.scope_level == "series" { ScopeLevel::Series } else { ScopeLevel::Project },
    })
}

fn scope_to_json(scope: &IntentScope) -> (String, serde_json::Value) {
    use serde_json::json;
    let (ty, data) = match scope {
        IntentScope::Project => ("project", json!({})),
        IntentScope::Chapter(c) => ("chapter", json!({ "chapter": c })),
        IntentScope::ParagraphRange { from, to } => ("paragraph_range", json!({ "from": from, "to": to })),
        IntentScope::Character(c) => ("character", json!({ "character": c })),
        IntentScope::Scene(s) => ("scene", json!({ "scene": s })),
        IntentScope::TimelineRange { from, to } => ("timeline_range", json!({ "from": from, "to": to })),
    };
    (ty.to_string(), data)
}

fn scope_from_json(scope_type: &str, data: &serde_json::Value) -> Option<IntentScope> {
    let s = |k: &str| data.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Some(match scope_type {
        "project" => IntentScope::Project,
        "chapter" => IntentScope::Chapter(s("chapter")),
        "paragraph_range" => IntentScope::ParagraphRange { from: s("from"), to: s("to") },
        "character" => IntentScope::Character(s("character")),
        "scene" => IntentScope::Scene(s("scene")),
        "timeline_range" => IntentScope::TimelineRange { from: s("from"), to: s("to") },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, level: ScopeLevel, scope: IntentScope) -> IntentEntry {
        IntentEntry {
            id: id.into(),
            kind: IntentKind::DeliberateTemporalAmbiguity,
            description: "Years 90-95 intentionally unresolved".into(),
            scope,
            coverage: vec![Category::DramatizationGap, Category::TemporalDensity],
            scope_level: level,
        }
    }

    #[test]
    fn exports_only_series_by_default_and_roundtrips() {
        let entries = vec![
            entry("a", ScopeLevel::Series, IntentScope::TimelineRange { from: "1A.090".into(), to: "1A.095".into() }),
            entry("b", ScopeLevel::Project, IntentScope::Chapter("ch01".into())),
        ];
        let body = export(&entries, ExportScope::Series);
        let back = parse(&body).unwrap();
        assert_eq!(back.len(), 1, "only the series entry exports");
        assert_eq!(back[0].id, "a");
        assert_eq!(back[0].scope_level, ScopeLevel::Series);
        assert_eq!(back[0].coverage, vec![Category::DramatizationGap, Category::TemporalDensity]);
        assert!(matches!(&back[0].scope, IntentScope::TimelineRange { from, to }
            if from == "1A.090" && to == "1A.095"));
    }

    #[test]
    fn export_all_includes_both() {
        let entries = vec![
            entry("a", ScopeLevel::Series, IntentScope::Project),
            entry("b", ScopeLevel::Project, IntentScope::Project),
        ];
        assert_eq!(parse(&export(&entries, ExportScope::All)).unwrap().len(), 2);
    }

    #[test]
    fn rejects_non_bundle() {
        assert!(parse("{\"format\":\"nope\",\"version\":1,\"entries\":[]}").is_err());
        assert!(parse("not json").is_err());
    }
}
