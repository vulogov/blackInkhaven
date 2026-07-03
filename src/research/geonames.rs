//! RESRCH-6-lite (GeoNames) — real-world **places** as a structured research
//! source. `/geonames <query>` looks a place up in the GeoNames gazetteer and
//! returns a compact card (region, country, feature type, coordinates,
//! population), cited by GeoNames id. A `/fact` from it records `origin=geonames`
//! (structured tier — gate-skipped, like `/wikidata`).
//!
//! GeoNames needs a free **username** (not a key, but registration-gated), so
//! `available` also requires `research.geonames.username` to be set. `reqwest`
//! only — no new crates.

use anyhow::{Result, anyhow};
use serde_json::Value as Json;

use crate::config::GeonamesConfig;

/// One gazetteer hit.
#[derive(Debug, Clone)]
pub(super) struct GeoPlace {
    pub id: i64,
    pub name: String,
    pub country: String,
    pub admin1: String,
    pub feature: String,
    pub lat: String,
    pub lng: String,
    pub population: i64,
}

pub(super) fn available(cfg: &GeonamesConfig) -> bool {
    cfg.enabled && !cfg.username.trim().is_empty()
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("inkhaven-research/1.0 (https://crates.io/crates/inkhaven)")
        .build()
        .map_err(|e| anyhow!("http client: {e}"))
}

/// Look up the top matching place. Owned args → spawnable.
pub(super) async fn fetch(cfg: GeonamesConfig, query: String, language: String) -> Result<GeoPlace> {
    let base = cfg.endpoint.trim_end_matches('/').to_string();
    let client = client()?;
    let mut q: Vec<(&str, String)> = vec![
        ("q", query.clone()),
        ("maxRows", "1".to_string()),
        ("style", "FULL".to_string()),
        ("username", cfg.username.clone()),
    ];
    if !language.is_empty() {
        q.push(("lang", language));
    }
    let json: Json = client
        .get(format!("{base}/searchJSON"))
        .query(&q)
        .send()
        .await
        .map_err(|e| anyhow!("geonames search: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("geonames decode: {e}"))?;
    // GeoNames signals bad username / rate limit via a `status` object.
    if let Some(status) = json.get("status") {
        let msg = status.get("message").and_then(|m| m.as_str()).unwrap_or("error");
        return Err(anyhow!("geonames: {msg}"));
    }
    let first = json
        .get("geonames")
        .and_then(|g| g.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| anyhow!("no place found for `{query}`"))?;
    Ok(parse_place(first))
}

/// A GeoNames result object → a `GeoPlace`. Numeric fields may arrive as JSON
/// numbers or strings depending on the API `style`, so both are tolerated.
fn parse_place(r: &Json) -> GeoPlace {
    let s = |k: &str| r.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let num = |k: &str| {
        r.get(k)
            .map(|v| match v {
                Json::Number(n) => n.as_i64().unwrap_or(0),
                Json::String(st) => st.parse().unwrap_or(0),
                _ => 0,
            })
            .unwrap_or(0)
    };
    let name = {
        let t = s("toponymName");
        if t.is_empty() { s("name") } else { t }
    };
    GeoPlace {
        id: num("geonameId"),
        name,
        country: s("countryName"),
        admin1: s("adminName1"),
        feature: s("fcodeName"),
        lat: s("lat"),
        lng: s("lng"),
        population: num("population"),
    }
}

/// Render the place as a compact chat card, ending with the GeoNames citation.
pub(super) fn render(p: &GeoPlace) -> String {
    let mut s = String::new();
    let loc: Vec<&str> = [p.admin1.as_str(), p.country.as_str()].into_iter().filter(|x| !x.is_empty()).collect();
    if loc.is_empty() {
        s.push_str(&format!("{}\n", p.name));
    } else {
        s.push_str(&format!("{} — {}\n", p.name, loc.join(", ")));
    }
    if !p.feature.is_empty() {
        s.push_str(&format!("Type: {}\n", p.feature));
    }
    if !p.lat.is_empty() && !p.lng.is_empty() {
        s.push_str(&format!("Coordinates: {}, {}\n", p.lat, p.lng));
    }
    if p.population > 0 {
        s.push_str(&format!("Population: {}\n", p.population));
    }
    s.push_str(&format!("\nSource: GeoNames #{} · https://www.geonames.org/{}", p.id, p.id));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(user: &str) -> GeonamesConfig {
        GeonamesConfig { enabled: true, endpoint: "http://api.geonames.org".into(), username: user.into() }
    }

    #[test]
    fn availability_requires_username() {
        assert!(available(&cfg("demo")));
        assert!(!available(&cfg("")));
        let mut off = cfg("demo");
        off.enabled = false;
        assert!(!available(&off));
    }

    #[test]
    fn parses_and_renders_a_place() {
        let j = serde_json::json!({
            "geonameId": 3169070,
            "toponymName": "Roma",
            "name": "Rome",
            "countryName": "Italy",
            "adminName1": "Latium",
            "fcodeName": "capital of a political entity",
            "lat": "41.89193",
            "lng": "12.51133",
            "population": 2318895
        });
        let p = parse_place(&j);
        assert_eq!(p.id, 3169070);
        assert_eq!(p.name, "Roma");
        assert_eq!(p.country, "Italy");
        assert_eq!(p.population, 2318895);
        let card = render(&p);
        assert!(card.contains("Roma — Latium, Italy"));
        assert!(card.contains("capital of a political entity"));
        assert!(card.contains("GeoNames #3169070"));
    }

    #[test]
    fn population_tolerates_string_numbers() {
        let j = serde_json::json!({ "geonameId": "1", "name": "X", "population": "500" });
        let p = parse_place(&j);
        assert_eq!(p.id, 1);
        assert_eq!(p.population, 500);
    }
}
