//! RESRCH-3 (R3-A) — `/wikidata`: query Wikidata's **structured** entity claims.
//!
//! Keyless (`reqwest`, already present), three calls: `wbsearchentities` → top
//! entity → `wbgetentities` for its labels/descriptions/claims → one batched
//! label resolve for the property- and value-Q-IDs. Returns citable triples, not
//! prose — the top of the trust ladder, so a `/fact` from it skips the gate.
//!
//! **Wikipedia is deliberately excluded**: its narrative carries editorial bias
//! (politics / economics / history); Wikidata's per-statement triples don't.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use serde_json::Value as Json;

use crate::config::WikidataConfig;

/// A resolved Wikidata entity: its Q-ID, label, description, and structured
/// `(property, value)` statements (labels resolved, values rendered).
#[derive(Debug, Clone)]
pub(super) struct WdEntity {
    pub qid: String,
    pub label: String,
    pub description: String,
    pub statements: Vec<(String, String)>,
}

/// Whether `/wikidata` can run (keyless — just the master switch).
pub(super) fn available(cfg: &WikidataConfig) -> bool {
    cfg.enabled
}

/// Query Wikidata for the top entity matching `query`, in `language`. Owned args
/// so it can be spawned onto a tokio task.
pub(super) async fn fetch(cfg: WikidataConfig, query: String, language: String) -> Result<WdEntity> {
    let base = cfg.endpoint.trim_end_matches('/').to_string();
    let api = format!("{base}/w/api.php");
    let client = reqwest::Client::builder()
        .user_agent("inkhaven-research/1.0 (https://crates.io/crates/inkhaven)")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow!("http client: {e}"))?;

    // 1. Search for the best-matching item.
    let search: Json = client
        .get(&api)
        .query(&[
            ("action", "wbsearchentities"),
            ("search", query.as_str()),
            ("language", language.as_str()),
            ("uselang", language.as_str()),
            ("type", "item"),
            ("limit", "1"),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|e| anyhow!("wikidata search: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("wikidata search decode: {e}"))?;
    let qid = search
        .get("search")
        .and_then(|s| s.as_array())
        .and_then(|a| a.first())
        .and_then(|e| e.get("id"))
        .and_then(|i| i.as_str())
        .ok_or_else(|| anyhow!("no Wikidata entity found for `{query}`"))?
        .to_string();

    // 2. Fetch the entity's labels, descriptions and claims.
    let entities: Json = client
        .get(&api)
        .query(&[
            ("action", "wbgetentities"),
            ("ids", qid.as_str()),
            ("props", "labels|descriptions|claims"),
            ("languages", language.as_str()),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|e| anyhow!("wikidata entity: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("wikidata entity decode: {e}"))?;
    let entity = entities
        .get("entities")
        .and_then(|e| e.get(&qid))
        .ok_or_else(|| anyhow!("wikidata: entity {qid} missing from response"))?;

    let label = lang_value(entity, "labels", &language).unwrap_or_else(|| qid.clone());
    let description = lang_value(entity, "descriptions", &language).unwrap_or_default();

    // 3. Collect the (property, raw-value) pairs and the ids needing labels.
    let claims = entity.get("claims").and_then(|c| c.as_object());
    let mut raw: Vec<(String, RawVal)> = Vec::new();
    let mut ids_to_resolve: Vec<String> = Vec::new();
    if let Some(claims) = claims {
        for (prop, statements) in claims {
            ids_to_resolve.push(prop.clone());
            let Some(arr) = statements.as_array() else { continue };
            for st in arr.iter().take(3) {
                if let Some(v) = mainsnak_value(st) {
                    if let RawVal::Entity(ref q) = v {
                        ids_to_resolve.push(q.clone());
                    }
                    raw.push((prop.clone(), v));
                    break; // one value per property keeps it compact
                }
            }
            if raw.len() >= cfg.max_statements {
                break;
            }
        }
    }

    // 4. One batched label resolve for property- and entity-value ids.
    let labels = resolve_labels(&client, &api, &language, &ids_to_resolve).await;

    let statements: Vec<(String, String)> = raw
        .into_iter()
        .map(|(prop, val)| {
            let pl = labels.get(&prop).cloned().unwrap_or_else(|| prop.clone());
            (pl, render_value(&val, &labels))
        })
        .collect();

    Ok(WdEntity { qid, label, description, statements })
}

/// A raw claim value before label resolution.
#[derive(Debug, Clone)]
enum RawVal {
    Entity(String),
    Text(String),
    Time(String),
    Quantity(String),
    Coordinate(f64, f64),
}

fn mainsnak_value(statement: &Json) -> Option<RawVal> {
    let snak = statement.get("mainsnak")?;
    // Skip external-identifier properties (VIAF, ISNI, catalog IDs, …) — cruft,
    // not the substantive triples we want to surface.
    if snak.get("datatype").and_then(|d| d.as_str()) == Some("external-id") {
        return None;
    }
    let dv = snak.get("datavalue")?;
    let ty = dv.get("type")?.as_str()?;
    let v = dv.get("value")?;
    match ty {
        "wikibase-entityid" => Some(RawVal::Entity(v.get("id")?.as_str()?.to_string())),
        "string" => Some(RawVal::Text(v.as_str()?.to_string())),
        "monolingualtext" => Some(RawVal::Text(v.get("text")?.as_str()?.to_string())),
        "time" => Some(RawVal::Time(format_time(v.get("time")?.as_str()?))),
        "quantity" => Some(RawVal::Quantity(v.get("amount")?.as_str()?.trim_start_matches('+').to_string())),
        "globecoordinate" => {
            Some(RawVal::Coordinate(v.get("latitude")?.as_f64()?, v.get("longitude")?.as_f64()?))
        }
        _ => None,
    }
}

fn render_value(val: &RawVal, labels: &BTreeMap<String, String>) -> String {
    match val {
        RawVal::Entity(q) => labels.get(q).cloned().unwrap_or_else(|| q.clone()),
        RawVal::Text(s) => s.clone(),
        RawVal::Time(t) => t.clone(),
        RawVal::Quantity(a) => a.clone(),
        RawVal::Coordinate(lat, lon) => format!("{lat:.4}, {lon:.4}"),
    }
}

/// `+1879-03-14T00:00:00Z` → `1879-03-14`; leaves other shapes readable.
fn format_time(t: &str) -> String {
    let t = t.trim_start_matches('+');
    t.split('T').next().unwrap_or(t).to_string()
}

/// The label/description string for `language`, from a `{labels:{lang:{value}}}`
/// block, falling back to English then any available language.
fn lang_value(entity: &Json, key: &str, language: &str) -> Option<String> {
    let block = entity.get(key)?.as_object()?;
    for lang in [language, "en"] {
        if let Some(v) = block.get(lang).and_then(|v| v.get("value")).and_then(|v| v.as_str()) {
            return Some(v.to_string());
        }
    }
    block.values().next().and_then(|v| v.get("value")).and_then(|v| v.as_str()).map(str::to_string)
}

/// Batch-resolve labels for a set of P-/Q-ids (deduped) → id → label.
async fn resolve_labels(
    client: &reqwest::Client,
    api: &str,
    language: &str,
    ids: &[String],
) -> BTreeMap<String, String> {
    let mut unique: Vec<String> = ids.to_vec();
    unique.sort();
    unique.dedup();
    let mut out = BTreeMap::new();
    // Wikibase caps ids at 50 per call.
    for chunk in unique.chunks(50) {
        let joined = chunk.join("|");
        let resp = client
            .get(api)
            .query(&[
                ("action", "wbgetentities"),
                ("ids", joined.as_str()),
                ("props", "labels"),
                ("languages", language),
                ("format", "json"),
            ])
            .send()
            .await;
        let Ok(resp) = resp else { continue };
        let Ok(json) = resp.json::<Json>().await else { continue };
        let Some(entities) = json.get("entities").and_then(|e| e.as_object()) else { continue };
        for (id, ent) in entities {
            if let Some(label) = lang_value(ent, "labels", language) {
                out.insert(id.clone(), label);
            }
        }
    }
    out
}

/// Render an entity as the chat body: a header, the description, and one line per
/// statement, ending with the Q-ID citation.
pub(super) fn render(entity: &WdEntity) -> String {
    let mut s = format!("{} ({})", entity.label, entity.qid);
    if !entity.description.is_empty() {
        s.push_str(&format!(" — {}", entity.description));
    }
    s.push('\n');
    for (prop, val) in &entity.statements {
        s.push_str(&format!("• {prop}: {val}\n"));
    }
    s.push_str(&format!("\nSource: Wikidata {} · https://www.wikidata.org/wiki/{}", entity.qid, entity.qid));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_and_time() {
        let mut c = WikidataConfig::default();
        assert!(available(&c));
        c.enabled = false;
        assert!(!available(&c));
        assert_eq!(format_time("+1879-03-14T00:00:00Z"), "1879-03-14");
    }

    #[test]
    fn renders_statements_and_citation() {
        let e = WdEntity {
            qid: "Q937".to_string(),
            label: "Albert Einstein".to_string(),
            description: "theoretical physicist".to_string(),
            statements: vec![
                ("occupation".to_string(), "physicist".to_string()),
                ("date of birth".to_string(), "1879-03-14".to_string()),
            ],
        };
        let r = render(&e);
        assert!(r.contains("Albert Einstein (Q937)"));
        assert!(r.contains("• occupation: physicist"));
        assert!(r.contains("Source: Wikidata Q937"));
    }

    #[test]
    fn mainsnak_parses_value_types() {
        let time = serde_json::json!({"mainsnak":{"datavalue":{"type":"time","value":{"time":"+1879-03-14T00:00:00Z"}}}});
        assert!(matches!(mainsnak_value(&time), Some(RawVal::Time(t)) if t == "1879-03-14"));
        let ent = serde_json::json!({"mainsnak":{"datavalue":{"type":"wikibase-entityid","value":{"id":"Q5"}}}});
        assert!(matches!(mainsnak_value(&ent), Some(RawVal::Entity(q)) if q == "Q5"));
        let qty = serde_json::json!({"mainsnak":{"datavalue":{"type":"quantity","value":{"amount":"+42"}}}});
        assert!(matches!(mainsnak_value(&qty), Some(RawVal::Quantity(a)) if a == "42"));
    }
}
