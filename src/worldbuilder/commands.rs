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
}

impl Op {
    /// A one-line HJSON-ish preview of the edit.
    pub(super) fn preview(&self) -> String {
        match self {
            Op::Set { path, value } => format!("{} = {}", path.join("."), compact(value)),
            Op::Push { path, value } => format!("{}[] += {}", path.join("."), compact(value)),
        }
    }

    /// Apply this edit to the world root value in place.
    pub(super) fn apply(&self, root: &mut Value) {
        match self {
            Op::Set { path, value } => set_path(root, path, value.clone()),
            Op::Push { path, value } => push_path(root, path, value.clone()),
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
            let sc = rest.to_uppercase();
            Command::Shape {
                label: format!("star → {sc}"),
                ops: vec![Op::Set {
                    path: vec!["astronomy".into(), "star_class".into()],
                    value: json!(sc),
                }],
            }
        }

        "tilt" => match rest.parse::<f64>() {
            Ok(v) => Command::Shape {
                label: format!("axial tilt → {v}°"),
                ops: vec![Op::Set {
                    path: vec!["astronomy".into(), "axial_tilt".into()],
                    value: json!(v),
                }],
            },
            Err(_) => Command::Unknown("usage: /tilt <degrees>".into()),
        },

        "moon" => {
            let mut it = rest.splitn(2, char::is_whitespace);
            let name = it.next().unwrap_or("").trim();
            if name.is_empty() {
                return Command::Unknown("usage: /moon <name> <period>".into());
            }
            let mut moon = json!({ "name": name });
            if let Some(p) = it.next().and_then(|s| s.trim().parse::<f64>().ok()) {
                moon["period"] = json!(p);
            }
            Command::Shape {
                label: format!("moon {name}"),
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
            let mut it = rest.split_whitespace();
            let name = it.next().unwrap_or("").to_string();
            if name.is_empty() {
                return Command::Unknown("usage: /nation <name> [era] [polity_kind] [traits…]".into());
            }
            let mut n = json!({ "name": name });
            if let Some(era) = it.next() {
                n["era"] = json!(era);
            }
            if let Some(kind) = it.next() {
                n["polity_kind"] = json!(kind);
            }
            let traits: Vec<&str> = it.collect();
            if !traits.is_empty() {
                n["traits"] = json!(traits);
            }
            Command::Shape {
                label: format!("nation {name}"),
                ops: vec![Op::Push { path: vec!["nations".into()], value: n }],
            }
        }

        other => Command::Unknown(format!(
            "unknown command `/{other}` — supports /interview /journey /sessions /set /star /tilt /moon /nation /magic /rule /wfact /research /compile /validate /write /undo /reset /diff"
        )),
    }
}

/// Best-effort scalar parse for `/set` values: bool → int → float → string.
fn parse_scalar(s: &str) -> Value {
    let t = s.trim();
    if t.eq_ignore_ascii_case("true") {
        return json!(true);
    }
    if t.eq_ignore_ascii_case("false") {
        return json!(false);
    }
    if let Ok(i) = t.parse::<i64>() {
        return json!(i);
    }
    if let Ok(f) = t.parse::<f64>() {
        return json!(f);
    }
    json!(t)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_sets_the_astronomy_class() {
        match parse("/star k") {
            Command::Shape { ops, .. } => {
                assert_eq!(
                    ops,
                    vec![Op::Set {
                        path: vec!["astronomy".into(), "star_class".into()],
                        value: json!("K"),
                    }]
                );
            }
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    #[test]
    fn nation_with_traits_builds_a_valid_push() {
        match parse("/nation Velmari bronze_age confederation seafaring trade") {
            Command::Shape { ops, .. } => {
                assert_eq!(ops.len(), 1);
                let Op::Push { path, value } = &ops[0] else { panic!("expected Push") };
                assert_eq!(path, &vec!["nations".to_string()]);
                assert_eq!(value["name"], json!("Velmari"));
                assert_eq!(value["era"], json!("bronze_age"));
                assert_eq!(value["polity_kind"], json!("confederation"));
                assert_eq!(value["traits"], json!(["seafaring", "trade"]));
            }
            other => panic!("expected Shape, got {other:?}"),
        }
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
