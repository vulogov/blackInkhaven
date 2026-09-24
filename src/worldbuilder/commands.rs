//! WBLD-1 (WB-P4) — the worldbuilder command namespace.
//!
//! A `/command` typed in the Query prompt parses to a [`Command`]. Shaping
//! commands produce one or more [`Op`]s — structured edits to `world.hjson` —
//! which the app previews before accepting into the session's pending delta.
//! `/write` folds the pending ops into `world.hjson`; `/undo` drops the last.
//!
//! WB-P4 ships the delta mechanic plus a representative command set (`/set` — the
//! generic dot-path escape hatch — and `/star`, `/tilt`, `/moon`, `/nation`);
//! the rest of the RFC's shaping vocabulary is mechanical follow-up over the same
//! `Op` engine.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A single structured edit to the world's `serde_json::Value`. Serialisable so
/// the pending delta survives a quit in the session sidecar (WB-P10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum Op {
    /// Set a dot-path leaf (creating intermediate objects).
    Set { path: Vec<String>, value: Value },
    /// Append to the array at a dot-path (creating it if absent).
    Push { path: Vec<String>, value: Value },
    /// Remove the element at `index` from the array at a dot-path (MAPED-P2).
    RemoveAt { path: Vec<String>, index: usize },
}

impl Op {
    /// A one-line HJSON-ish preview of the edit.
    pub(super) fn preview(&self) -> String {
        match self {
            Op::Set { path, value } => format!("{} = {}", path.join("."), compact(value)),
            Op::Push { path, value } => format!("{}[] += {}", path.join("."), compact(value)),
            Op::RemoveAt { path, index } => format!("{}[{index}] removed", path.join(".")),
        }
    }

    /// Apply this edit to the world root value in place.
    pub(super) fn apply(&self, root: &mut Value) {
        match self {
            Op::Set { path, value } => set_path(root, path, value.clone()),
            Op::Push { path, value } => push_path(root, path, value.clone()),
            Op::RemoveAt { path, index } => remove_at_path(root, path, *index),
        }
    }
}

/// A parsed worldbuilder command.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Command {
    /// One or more shaping edits, with a human label for the preview/status.
    Shape { label: String, ops: Vec<Op> },
    Write,
    Undo,
    Reset,
    Diff,
    /// Compile the pure layer chain and report the compiled world state to Chat.
    Compile,
    /// Run the deterministic plausibility lints and report warnings to Chat.
    Validate,
    /// Record an author-decided world fact into the Facts book (tagged fact:world).
    Wfact(String),
    /// Retrieve related Facts for a query into the Research pane.
    Research(String),
    /// Start the guided world interview.
    Interview,
    /// Show the session timeline (the Worldbuilding Journey) in the Chat pane.
    Journey,
    /// List the project's worldbuilder sessions.
    Sessions,
    /// Export a readable world dossier. `pdf` also renders a PDF via Typst.
    Export { pdf: bool },
    /// Switch to another worldbuilder session by name (WS-P3).
    Switch(String),
    /// Compile `n` candidate worlds on derived seeds and compare them (WS-P1).
    Roll(usize),
    /// Render the world map with plakat and show it in the Map pane (WS-P2).
    Map,
    /// Check the declared map layer against the compiled world (MAPED-P5).
    MapCheck,
    /// Write the sculpted terrain as a DEM heightmap + set geology.dem (MAPED-P7).
    Terrain,
    /// Unrecognised / malformed — carries a message for the status bar.
    Unknown(String),
}

/// Parse a `/command …` line (the leading `/` optional).
pub(super) fn parse(input: &str) -> Command {
    let body = input.trim().strip_prefix('/').unwrap_or(input.trim());
    let (cmd, rest) = body
        .split_once(char::is_whitespace)
        .map(|(a, b)| (a, b.trim()))
        .unwrap_or((body, ""));
    match cmd.to_ascii_lowercase().as_str() {
        "write" => Command::Write,
        "undo" => Command::Undo,
        "reset" => Command::Reset,
        "diff" => Command::Diff,
        "compile" => Command::Compile,
        "validate" | "check" => Command::Validate,
        "interview" => Command::Interview,
        "journey" => Command::Journey,
        "sessions" => Command::Sessions,
        "export" => {
            let pdf = rest.split_whitespace().any(|w| w.eq_ignore_ascii_case("--pdf") || w.eq_ignore_ascii_case("pdf"));
            Command::Export { pdf }
        }
        "switch" => {
            if rest.trim().is_empty() {
                Command::Unknown("usage: /switch <session-name>".into())
            } else {
                Command::Switch(rest.trim().to_string())
            }
        }

        "map" => Command::Map,
        "mapcheck" => Command::MapCheck,
        "terrain" => Command::Terrain,

        "roll" => {
            // `/roll [n]` — n candidate seeds (default 4, clamped 1..=8).
            let n = rest.split_whitespace().next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(4);
            Command::Roll(n.clamp(1, 8))
        }

        "adopt" => {
            // `/adopt <seed>` — decimal or 0x-hex. Written as a hex STRING so any
            // u64 round-trips through SeedValue (untagged Int(i64)|Str).
            let t = rest.trim();
            let parsed = t
                .strip_prefix("0x")
                .or_else(|| t.strip_prefix("0X"))
                .and_then(|h| u64::from_str_radix(h, 16).ok())
                .or_else(|| t.parse::<u64>().ok());
            match parsed {
                Some(seed) => Command::Shape {
                    label: format!("seed → 0x{seed:x}"),
                    ops: vec![Op::Set {
                        path: vec!["seed".into()],
                        value: json!(format!("0x{seed:x}")),
                    }],
                },
                None => Command::Unknown("usage: /adopt <seed> (decimal or 0x-hex)".into()),
            }
        }
        "wfact" | "fact" => {
            if rest.is_empty() {
                Command::Unknown("usage: /wfact <statement> — records an author fact:world".into())
            } else {
                Command::Wfact(rest.to_string())
            }
        }
        "research" | "wresearch" => {
            if rest.is_empty() {
                Command::Unknown("usage: /research <query> — retrieve related Facts".into())
            } else {
                Command::Research(rest.to_string())
            }
        }

        "set" => {
            let (path_s, val_s) = rest
                .split_once(char::is_whitespace)
                .map(|(a, b)| (a, b.trim()))
                .unwrap_or((rest, ""));
            if path_s.is_empty() {
                return Command::Unknown("usage: /set <dot.path> <value>".into());
            }
            let path: Vec<String> = path_s.split('.').map(|s| s.to_string()).collect();
            let value = parse_scalar(val_s);
            Command::Shape {
                label: format!("{path_s} = {}", compact(&value)),
                ops: vec![Op::Set { path, value }],
            }
        }

        "star" => {
            if rest.is_empty() {
                return Command::Unknown("usage: /star <type> (e.g. G, K, M)".into());
            }
            // Accept the prompt's own words ("orange", "red dwarf", "Sun-like")
            // as well as a spectral class ("K", "G2V", "M5"); anything else is
            // refused rather than guessed from its first letter ("orange" ≠ O).
            let Some(sc) = star_class_of(rest) else {
                return Command::Unknown(
                    "usage: /star <class> — G (Sun-like/yellow), K (orange), M (red dwarf), F, A, B, O, or e.g. G2V".into(),
                );
            };
            // The schema nests the star (`astronomy.star.class`), and its
            // required `luminosity_solar` drives the compiler — so a K/M dwarf
            // must not be left as bright as the Sun. Set a class-typical value.
            let mut ops = vec![Op::Set {
                path: vec!["astronomy".into(), "star".into(), "class".into()],
                value: json!(sc),
            }];
            let label = match typical_luminosity(&sc) {
                Some(lum) => {
                    ops.push(Op::Set {
                        path: vec!["astronomy".into(), "star".into(), "luminosity_solar".into()],
                        value: json!(lum),
                    });
                    format!("star → {sc} ({lum} L☉)")
                }
                None => format!("star → {sc}"),
            };
            Command::Shape { label, ops }
        }

        "tilt" => match rest.parse::<f64>() {
            Ok(v) => Command::Shape {
                label: format!("axial tilt → {v}°"),
                ops: vec![Op::Set {
                    path: vec!["astronomy".into(), "planet".into(), "axial_tilt_deg".into()],
                    value: json!(v),
                }],
            },
            Err(_) => Command::Unknown("usage: /tilt <degrees>".into()),
        },

        "moon" => {
            // `<name…> [period_days]` — a trailing number is the period, the rest
            // (any number of words) is the name. `period_days` is required by the
            // schema (a moon without it makes world.hjson unparseable); default to
            // Earth's Moon when omitted, and give it a lunar mass so it raises tides.
            let mut toks: Vec<&str> = rest.split_whitespace().collect();
            let period = match toks.last().and_then(|s| s.parse::<f64>().ok()) {
                Some(p) if p.is_finite() && p > 0.0 && toks.len() > 1 => {
                    toks.pop();
                    p
                }
                _ => 27.32,
            };
            let name = toks.join(" ");
            if name.is_empty() {
                return Command::Unknown("usage: /moon <name> [period_days]".into());
            }
            let moon = json!({ "name": name, "period_days": period, "mass_lunar": 1.0 });
            Command::Shape {
                label: format!("moon {name} ({period} d)"),
                ops: vec![Op::Push {
                    path: vec!["astronomy".into(), "moons".into()],
                    value: moon,
                }],
            }
        }

        "magic" => match rest.to_ascii_lowercase().as_str() {
            "on" | "true" | "enabled" => Command::Shape {
                label: "magic → enabled".into(),
                ops: vec![Op::Set {
                    path: vec!["magic".into(), "enabled".into()],
                    value: json!(true),
                }],
            },
            "off" | "false" | "disabled" => Command::Shape {
                label: "magic → disabled".into(),
                ops: vec![Op::Set {
                    path: vec!["magic".into(), "enabled".into()],
                    value: json!(false),
                }],
            },
            _ => Command::Unknown("usage: /magic on|off".into()),
        },

        "rule" => {
            // /rule <kind> <cover1,cover2,…> [description…]
            let mut it = rest.splitn(3, char::is_whitespace);
            let kind = it.next().unwrap_or("").trim();
            let covers_s = it.next().unwrap_or("").trim();
            let desc = it.next().unwrap_or("").trim();
            if kind.is_empty() || covers_s.is_empty() {
                return Command::Unknown(
                    "usage: /rule <kind> <category,category> [description] (enables magic)".into(),
                );
            }
            let covers: Vec<String> = covers_s
                .split(',')
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .collect();
            let mut rule = json!({ "kind": kind, "covers": covers });
            if !desc.is_empty() {
                rule["description"] = json!(desc);
            }
            Command::Shape {
                label: format!("magic rule {kind} (covers {covers_s})"),
                ops: vec![
                    // A rule with the ledger disabled suppresses nothing — enable it.
                    Op::Set { path: vec!["magic".into(), "enabled".into()], value: json!(true) },
                    Op::Push { path: vec!["magic".into(), "rules".into()], value: rule },
                ],
            }
        }

        "nation" => {
            // /nation <name…> [x y] — the schema's `NationDef` is {name, capital?,
            // relations}. A trailing pair of integers is the capital cell; without
            // one the compiler seats the nation at the largest unclaimed
            // settlement. (The old era/polity_kind/traits fields never existed in
            // the schema — they were silently dropped and, with no `capital`,
            // broke the parse.)
            let toks: Vec<&str> = rest.split_whitespace().collect();
            let (name_toks, capital) = match toks.as_slice() {
                [head @ .., x, y] if !head.is_empty() => match (x.parse::<usize>(), y.parse::<usize>()) {
                    (Ok(cx), Ok(cy)) => (head, Some([cx, cy])),
                    _ => (toks.as_slice(), None),
                },
                _ => (toks.as_slice(), None),
            };
            let name = name_toks.join(" ");
            if name.is_empty() {
                return Command::Unknown("usage: /nation <name> [capital-x capital-y]".into());
            }
            let mut n = json!({ "name": name });
            let label = match capital {
                Some([x, y]) => {
                    n["capital"] = json!([x, y]);
                    format!("nation {name} (capital {x},{y})")
                }
                None => format!("nation {name}"),
            };
            Command::Shape {
                label,
                ops: vec![Op::Push { path: vec!["nations".into()], value: n }],
            }
        }

        other => Command::Unknown(format!(
            "unknown command `/{other}` — supports /interview /roll /adopt /map /mapcheck /terrain /journey /sessions /switch /export[ --pdf] /set /star /tilt /moon /nation /magic /rule /wfact /research /compile /validate /write /undo /reset /diff"
        )),
    }
}

/// Typical main-sequence bolometric luminosity (solar units) for a spectral
/// class letter — so `/star K` doesn't leave a K-dwarf as bright as the Sun.
/// Resolve a `/star` answer to a spectral class: a class token (`K`, `g2v`,
/// `M5`) or one of the interview's words (`sun-like`, `yellow`, `orange`,
/// `red`, `red dwarf`, `blue`, `white`). `None` when it is neither.
fn star_class_of(answer: &str) -> Option<String> {
    let t = answer.trim();
    let lower = t.to_ascii_lowercase();
    let by_word = match lower.as_str() {
        "sun-like" | "sunlike" | "sun like" | "yellow" | "sun" | "yellow dwarf" | "g-type" => Some("G"),
        "orange" | "orange dwarf" | "k-type" => Some("K"),
        "red" | "red dwarf" | "m-type" => Some("M"),
        "yellow-white" | "yellow white" | "f-type" => Some("F"),
        "white" | "a-type" => Some("A"),
        "blue-white" | "blue white" | "b-type" => Some("B"),
        "blue" | "blue giant" | "o-type" => Some("O"),
        _ => None,
    };
    if let Some(c) = by_word {
        return Some(c.to_string());
    }
    // A spectral class token: letter O/B/A/F/G/K/M, optional subtype digit,
    // optional luminosity class (V, IV, III, II, I).
    let up = t.to_ascii_uppercase();
    let mut chars = up.chars();
    let first = chars.next()?;
    if !"OBAFGKM".contains(first) {
        return None;
    }
    let rest: String = chars.collect();
    let rest_ok = rest.is_empty()
        || rest
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .trim_start_matches(['.', ' '])
            .chars()
            .all(|c| matches!(c, 'I' | 'V'));
    if rest_ok && up.len() <= 6 { Some(up) } else { None }
}

fn typical_luminosity(class: &str) -> Option<f64> {
    match class.chars().next()?.to_ascii_uppercase() {
        'O' => Some(30000.0),
        'B' => Some(1000.0),
        'A' => Some(20.0),
        'F' => Some(3.0),
        'G' => Some(1.0),
        'K' => Some(0.4),
        'M' => Some(0.05),
        _ => None,
    }
}

/// Fold `ops` onto `base` and **validate** the result parses as a
/// [`crate::world::types::WorldDefinition`] — the gate that keeps every shaping
/// path (interview, `/set`, map tools) from ever writing an unloadable
/// `world.hjson`. On `Err` nothing should be written; the message is the schema
/// error for the author to act on.
pub(super) fn fold_ops(mut base: Value, ops: &[Op]) -> Result<Value, String> {
    for op in ops {
        op.apply(&mut base);
    }
    serde_json::from_value::<crate::world::types::WorldDefinition>(base.clone())
        .map_err(|e| e.to_string())?;
    Ok(base)
}

/// Best-effort scalar parse for `/set` values: bool → int → float → string.
/// A value wrapped in matching quotes is always a string (so `/set name "1984"`
/// keeps the digits as a name); `yes`/`no`/`on`/`off` read as booleans (the
/// interview asks "Is there magic?"); non-finite floats (`nan`, `inf`) are
/// strings, never a JSON `null` the schema would reject as a missing field.
/// [`validate_ops`] retries a scalar as a string when the schema wants one.
fn parse_scalar(s: &str) -> Value {
    let t = s.trim();
    if t.len() >= 2 {
        let (a, z) = (t.as_bytes()[0], t.as_bytes()[t.len() - 1]);
        if (a == b'"' && z == b'"') || (a == b'\'' && z == b'\'') {
            return json!(t[1..t.len() - 1]);
        }
    }
    match t.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => return json!(true),
        "false" | "no" | "off" => return json!(false),
        _ => {}
    }
    if let Ok(i) = t.parse::<i64>() {
        return json!(i);
    }
    if let Ok(f) = t.parse::<f64>() {
        if f.is_finite() {
            return json!(f);
        }
        return json!(t);
    }
    json!(t)
}

/// Accept-time validation of a shaping delta against the world as it stands
/// (`base` = `world.hjson` + the pending ops). Returns the ops to record — a
/// scalar the schema wants as a string is retried as one (`/set name 1984`,
/// `/set name True`), so the typed text lands rather than a bool/number the
/// schema refuses. Errors name the offending delta, so the author can tell
/// which op is at fault instead of meeting a bare serde error at `/write`.
/// A `Set` whose path is not a schema field is refused too (serde would drop
/// it silently and the stray key would land on disk).
pub(super) fn validate_ops(base: &Value, ops: Vec<Op>) -> Result<Vec<Op>, String> {
    let canonical = |v: Value| -> Result<Value, String> {
        let def: crate::world::types::WorldDefinition =
            serde_json::from_value(v).map_err(|e| e.to_string())?;
        serde_json::to_value(def).map_err(|e| e.to_string())
    };
    canonical(base.clone())?;
    let mut out = Vec::with_capacity(ops.len());
    let mut cur = base.clone();
    for op in ops {
        let mut try_op = op.clone();
        let mut ok = fold_ops(cur.clone(), std::slice::from_ref(&try_op));
        if ok.is_err() {
            if let Op::Set { path, value } = &op {
                if !value.is_string() && !value.is_object() && !value.is_array() && !value.is_null() {
                    let as_text = match value {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    try_op = Op::Set { path: path.clone(), value: json!(as_text) };
                    ok = fold_ops(cur.clone(), std::slice::from_ref(&try_op));
                }
            }
        }
        let next = match ok {
            Ok(v) => v,
            Err(e) => return Err(format!("`{}` refused — {e}", op.preview())),
        };
        if let Op::Set { path, value } = &try_op {
            // Unknown path: the fold changed nothing the schema can see.
            let before = canonical(cur.clone())?;
            let after = canonical(next.clone())?;
            let raw_before = path.iter().fold(Some(&cur), |v, k| v.and_then(|v| v.get(k)));
            if before == after && raw_before != Some(value) {
                return Err(format!(
                    "`{}` is not a world.hjson field (typo? see Documentation/WORLDBUILDING.md)",
                    path.join(".")
                ));
            }
        }
        cur = next;
        out.push(try_op);
    }
    Ok(out)
}

fn compact(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn set_path(root: &mut Value, path: &[String], value: Value) {
    if path.is_empty() {
        *root = value;
        return;
    }
    if !root.is_object() {
        *root = Value::Object(serde_json::Map::new());
    }
    let obj = root.as_object_mut().expect("just ensured object");
    if path.len() == 1 {
        obj.insert(path[0].clone(), value);
    } else {
        let child = obj
            .entry(path[0].clone())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        set_path(child, &path[1..], value);
    }
}

fn push_path(root: &mut Value, path: &[String], value: Value) {
    if path.is_empty() {
        return;
    }
    if !root.is_object() {
        *root = Value::Object(serde_json::Map::new());
    }
    let obj = root.as_object_mut().expect("just ensured object");
    if path.len() == 1 {
        let arr = obj
            .entry(path[0].clone())
            .or_insert_with(|| Value::Array(Vec::new()));
        if !arr.is_array() {
            *arr = Value::Array(Vec::new());
        }
        arr.as_array_mut().expect("just ensured array").push(value);
    } else {
        let child = obj
            .entry(path[0].clone())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        push_path(child, &path[1..], value);
    }
}

/// Remove the element at `index` from the array at `path`. A missing path,
/// non-array, or out-of-range index is a silent no-op (the delta simply does
/// nothing rather than corrupting the world).
fn remove_at_path(root: &mut Value, path: &[String], index: usize) {
    let Some(obj) = root.as_object_mut() else { return };
    let Some(first) = path.first() else { return };
    if path.len() == 1 {
        if let Some(Value::Array(arr)) = obj.get_mut(first) {
            if index < arr.len() {
                arr.remove(index);
            }
        }
    } else if let Some(child) = obj.get_mut(first) {
        remove_at_path(child, &path[1..], index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_sets_the_nested_class_and_a_typical_luminosity() {
        match parse("/star k") {
            Command::Shape { ops, .. } => {
                assert_eq!(
                    ops,
                    vec![
                        Op::Set {
                            path: vec!["astronomy".into(), "star".into(), "class".into()],
                            value: json!("K"),
                        },
                        Op::Set {
                            path: vec!["astronomy".into(), "star".into(), "luminosity_solar".into()],
                            value: json!(0.4),
                        },
                    ]
                );
            }
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    #[test]
    fn tilt_sets_the_planet_axial_tilt() {
        match parse("/tilt 28.5") {
            Command::Shape { ops, .. } => {
                assert_eq!(
                    ops,
                    vec![Op::Set {
                        path: vec!["astronomy".into(), "planet".into(), "axial_tilt_deg".into()],
                        value: json!(28.5),
                    }]
                );
            }
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    #[test]
    fn moon_pushes_a_schema_shaped_moon_with_a_default_period() {
        match parse("/moon Selene") {
            Command::Shape { ops, .. } => {
                let Op::Push { path, value } = &ops[0] else { panic!("expected Push") };
                assert_eq!(path, &vec!["astronomy".to_string(), "moons".to_string()]);
                assert_eq!(value["name"], json!("Selene"));
                assert_eq!(value["period_days"], json!(27.32));
                assert_eq!(value["mass_lunar"], json!(1.0));
            }
            other => panic!("expected Shape, got {other:?}"),
        }
        match parse("/moon Selene 12.5") {
            Command::Shape { ops, .. } => {
                let Op::Push { value, .. } = &ops[0] else { panic!("expected Push") };
                assert_eq!(value["period_days"], json!(12.5));
            }
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    #[test]
    fn nation_pushes_name_and_optional_capital_only() {
        match parse("/nation Velmari") {
            Command::Shape { ops, .. } => {
                assert_eq!(ops.len(), 1);
                let Op::Push { path, value } = &ops[0] else { panic!("expected Push") };
                assert_eq!(path, &vec!["nations".to_string()]);
                assert_eq!(value, &json!({ "name": "Velmari" }));
            }
            other => panic!("expected Shape, got {other:?}"),
        }
        match parse("/nation Free Cities of Velmari 12 40") {
            Command::Shape { ops, .. } => {
                let Op::Push { value, .. } = &ops[0] else { panic!("expected Push") };
                assert_eq!(value["name"], json!("Free Cities of Velmari"));
                assert_eq!(value["capital"], json!([12, 40]));
                assert!(value.get("era").is_none() && value.get("traits").is_none());
            }
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    /// The full interview's answers, folded onto the starter base, must yield a
    /// definition the schema loads with every answer landing where the
    /// compilers read it (the bug: `/star`, `/tilt`, `/moon`, `/nation` wrote
    /// stray keys or non-schema shapes and the file no longer parsed).
    #[test]
    fn interview_answers_fold_onto_the_starter_base_into_a_valid_world() {
        let base: Value =
            serde_hjson::from_str(&crate::world::starter_template("Thalor")).expect("starter parses");
        let mut ops = Vec::new();
        for line in [
            "/star M",
            "/tilt 31",
            "/moon Selene 9.5",
            "/set geology.generated.continents 4",
            "/set geology.generated.sea_level 0.55",
            "/set geology.generated.mountain_orogeny active",
            "/set primary_language Russian",
            "/nation Velmari 3 4",
            "/nation Karon",
            "/set magic.enabled true",
        ] {
            match parse(line) {
                Command::Shape { ops: o, .. } => ops.extend(o),
                other => panic!("{line}: expected Shape, got {other:?}"),
            }
        }
        let folded = fold_ops(base, &ops).expect("folded world is schema-valid");
        let world: crate::world::types::WorldDefinition =
            serde_json::from_value(folded).expect("parses as WorldDefinition");
        assert_eq!(world.name, "Thalor");
        assert_eq!(world.astronomy.star.class, "M");
        assert!((world.astronomy.star.luminosity_solar - 0.05).abs() < 1e-9);
        assert!((world.astronomy.planet.axial_tilt_deg - 31.0).abs() < 1e-9);
        let selene = world.astronomy.moons.iter().find(|m| m.name == "Selene").expect("moon pushed");
        assert!((selene.period_days - 9.5).abs() < 1e-9);
        let generated = world.geology.as_ref().and_then(|g| g.generated.as_ref()).expect("generated geology");
        assert_eq!(generated.continents, 4);
        assert_eq!(world.nations.len(), 2);
        assert_eq!(world.nations[0].capital, Some([3, 4]));
        assert_eq!(world.nations[1].capital, None);
        assert!(world.magic.as_ref().map(|m| m.enabled).unwrap_or(false));
    }

    #[test]
    fn star_accepts_the_prompt_words_and_refuses_guesses() {
        for (ans, class, lum) in [("orange", "K", 0.4), ("red dwarf", "M", 0.05), ("Sun-like", "G", 1.0), ("g2v", "G2V", 1.0)] {
            match parse(&format!("/star {ans}")) {
                Command::Shape { ops, .. } => {
                    assert_eq!(ops[0], Op::Set { path: vec!["astronomy".into(), "star".into(), "class".into()], value: json!(class) });
                    assert_eq!(ops[1], Op::Set { path: vec!["astronomy".into(), "star".into(), "luminosity_solar".into()], value: json!(lum) });
                }
                other => panic!("{ans}: expected Shape, got {other:?}"),
            }
        }
        assert!(matches!(parse("/star purple"), Command::Unknown(_)));
        assert!(matches!(parse("/star Xenon"), Command::Unknown(_)));
    }

    #[test]
    fn moon_takes_a_multi_word_name_with_a_trailing_period() {
        match parse("/moon Selene Minor 12") {
            Command::Shape { ops, .. } => {
                let Op::Push { value, .. } = &ops[0] else { panic!("expected Push") };
                assert_eq!(value["name"], json!("Selene Minor"));
                assert_eq!(value["period_days"], json!(12.0));
            }
            other => panic!("expected Shape, got {other:?}"),
        }
        match parse("/moon 42") {
            Command::Shape { ops, .. } => {
                let Op::Push { value, .. } = &ops[0] else { panic!("expected Push") };
                assert_eq!(value["name"], json!("42"), "a lone number is a name, not a period");
                assert_eq!(value["period_days"], json!(27.32));
            }
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    #[test]
    fn scalars_keep_names_that_look_like_numbers_or_bools() {
        assert_eq!(parse_scalar("\"1984\""), json!("1984"));
        assert_eq!(parse_scalar("nan"), json!("nan"));
        assert_eq!(parse_scalar("Infinity"), json!("Infinity"));
        assert_eq!(parse_scalar("yes"), json!(true));
        assert_eq!(parse_scalar("Off"), json!(false));
        assert_eq!(parse_scalar("0.6"), json!(0.6));
        assert_eq!(parse_scalar("3"), json!(3));
    }

    #[test]
    fn validate_ops_retries_scalars_as_strings_and_names_the_bad_op() {
        let base: Value =
            serde_hjson::from_str(&crate::world::starter_template("Thalor")).expect("starter parses");
        // A name that parsed as a number lands as text.
        let Command::Shape { ops, .. } = parse("/set name 1984") else { panic!() };
        let ops = validate_ops(&base, ops).expect("retried as a string");
        assert_eq!(ops[0], Op::Set { path: vec!["name".into()], value: json!("1984") });
        // A field the schema types as u32 refuses text, naming the op.
        let Command::Shape { ops, .. } = parse("/set geology.generated.continents three") else { panic!() };
        let err = validate_ops(&base, ops).unwrap_err();
        assert!(err.contains("geology.generated.continents"), "{err}");
        // A typo path is refused instead of landing as a stray key.
        let Command::Shape { ops, .. } = parse("/set astronmy.star.class K") else { panic!() };
        let err = validate_ops(&base, ops).unwrap_err();
        assert!(err.contains("not a world.hjson field"), "{err}");
        // A real path passes untouched.
        let Command::Shape { ops, .. } = parse("/set astronomy.star.class K") else { panic!() };
        assert!(validate_ops(&base, ops).is_ok());
        // yes/no reach a bool field.
        let Command::Shape { ops, .. } = parse("/set magic.enabled yes") else { panic!() };
        let ops = validate_ops(&base, ops).expect("yes → true");
        assert_eq!(ops[0], Op::Set { path: vec!["magic".into(), "enabled".into()], value: json!(true) });
    }

    #[test]
    fn fold_ops_refuses_a_delta_that_breaks_the_schema() {
        let base: Value =
            serde_hjson::from_str(&crate::world::starter_template("Thalor")).expect("starter parses");
        let ops = vec![Op::Set {
            path: vec!["astronomy".into(), "star".into(), "luminosity_solar".into()],
            value: json!("bright"),
        }];
        assert!(fold_ops(base, &ops).is_err());
    }

    #[test]
    fn set_and_push_apply_to_a_value() {
        let mut root = json!({});
        Op::Set { path: vec!["astronomy".into(), "star_class".into()], value: json!("K") }.apply(&mut root);
        Op::Push { path: vec!["nations".into()], value: json!({ "name": "Velmari" }) }.apply(&mut root);
        Op::Push { path: vec!["nations".into()], value: json!({ "name": "Eastreach" }) }.apply(&mut root);
        assert_eq!(root["astronomy"]["star_class"], json!("K"));
        assert_eq!(root["nations"].as_array().unwrap().len(), 2);
        assert_eq!(root["nations"][1]["name"], json!("Eastreach"));
    }

    #[test]
    fn remove_at_deletes_the_indexed_element_and_is_bounds_safe() {
        let mut root = json!({ "geography": { "landmarks": [
            { "name": "A" }, { "name": "B" }, { "name": "C" }
        ]}});
        Op::RemoveAt { path: vec!["geography".into(), "landmarks".into()], index: 1 }.apply(&mut root);
        let arr = root["geography"]["landmarks"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], json!("A"));
        assert_eq!(arr[1]["name"], json!("C"));
        // Out-of-range and missing paths are no-ops, not panics.
        Op::RemoveAt { path: vec!["geography".into(), "landmarks".into()], index: 9 }.apply(&mut root);
        assert_eq!(root["geography"]["landmarks"].as_array().unwrap().len(), 2);
        Op::RemoveAt { path: vec!["nope".into()], index: 0 }.apply(&mut root);
    }

    #[test]
    fn set_parses_scalar_types() {
        assert_eq!(parse_scalar("42"), json!(42));
        assert_eq!(parse_scalar("3.5"), json!(3.5));
        assert_eq!(parse_scalar("true"), json!(true));
        assert_eq!(parse_scalar("Aldoria"), json!("Aldoria"));
    }

    #[test]
    fn session_commands_and_unknown() {
        assert_eq!(parse("/write"), Command::Write);
        assert_eq!(parse("/undo"), Command::Undo);
        assert!(matches!(parse("/frobnicate"), Command::Unknown(_)));
        assert!(matches!(parse("/set"), Command::Unknown(_)));
    }

    #[test]
    fn compile_and_validate_parse() {
        assert_eq!(parse("/compile"), Command::Compile);
        assert_eq!(parse("/validate"), Command::Validate);
        assert_eq!(parse("/check"), Command::Validate); // alias
    }

    #[test]
    fn rule_enables_magic_and_pushes_a_rule() {
        match parse("/rule messenger_birds travel_time Royal pelicans fly day and night") {
            Command::Shape { ops, .. } => {
                assert_eq!(ops.len(), 2);
                assert_eq!(
                    ops[0],
                    Op::Set {
                        path: vec!["magic".into(), "enabled".into()],
                        value: json!(true),
                    }
                );
                let Op::Push { path, value } = &ops[1] else { panic!("expected Push") };
                assert_eq!(path, &vec!["magic".to_string(), "rules".to_string()]);
                assert_eq!(value["kind"], json!("messenger_birds"));
                assert_eq!(value["covers"], json!(["travel_time"]));
                assert_eq!(value["description"], json!("Royal pelicans fly day and night"));
            }
            other => panic!("expected Shape, got {other:?}"),
        }
        // Multiple covers split on comma.
        match parse("/rule seer astronomy,climate") {
            Command::Shape { ops, .. } => {
                let Op::Push { value, .. } = &ops[1] else { panic!("expected Push") };
                assert_eq!(value["covers"], json!(["astronomy", "climate"]));
            }
            other => panic!("expected Shape, got {other:?}"),
        }
        assert!(matches!(parse("/rule"), Command::Unknown(_)));
        assert!(matches!(parse("/magic on"), Command::Shape { .. }));
        assert!(matches!(parse("/magic sideways"), Command::Unknown(_)));
    }

    #[test]
    fn roll_defaults_and_clamps_and_adopt_writes_hex_seed() {
        assert_eq!(parse("/roll"), Command::Roll(4));
        assert_eq!(parse("/roll 3"), Command::Roll(3));
        assert_eq!(parse("/roll 99"), Command::Roll(8)); // clamped
        assert_eq!(parse("/roll 0"), Command::Roll(1)); // clamped
        // /adopt writes the seed as a 0x hex string leaf.
        match parse("/adopt 20818") {
            Command::Shape { ops, .. } => {
                assert_eq!(
                    ops,
                    vec![Op::Set { path: vec!["seed".into()], value: json!("0x5152") }]
                );
            }
            other => panic!("expected Shape, got {other:?}"),
        }
        assert_eq!(parse("/adopt 0x5152"), parse("/adopt 20818"));
        assert!(matches!(parse("/adopt nope"), Command::Unknown(_)));
        assert_eq!(parse("/map"), Command::Map);
        assert_eq!(parse("/mapcheck"), Command::MapCheck);
    }

    #[test]
    fn export_pdf_flag_and_switch_parse() {
        assert_eq!(parse("/export"), Command::Export { pdf: false });
        assert_eq!(parse("/export --pdf"), Command::Export { pdf: true });
        assert_eq!(parse("/export pdf"), Command::Export { pdf: true });
        assert_eq!(parse("/switch aldoria-v2"), Command::Switch("aldoria-v2".into()));
        assert!(matches!(parse("/switch"), Command::Unknown(_)));
    }

    #[test]
    fn wfact_and_research_carry_their_argument() {
        assert_eq!(
            parse("/wfact The tides run backwards at the equinox"),
            Command::Wfact("The tides run backwards at the equinox".into())
        );
        assert_eq!(parse("/research tidal harbours"), Command::Research("tidal harbours".into()));
        // Argument required.
        assert!(matches!(parse("/wfact"), Command::Unknown(_)));
        assert!(matches!(parse("/research"), Command::Unknown(_)));
    }
}
