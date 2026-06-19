//! Language-family tree (LANG-1 P4.2).
//!
//! Build + render the genealogical tree from each language's declared `proto`
//! (its parent). A language whose proto is unset (or isn't itself a language)
//! is a root. Pure ASCII rendering — no dependency; an SVG via resvg is a
//! later refinement.

use std::collections::{BTreeMap, BTreeSet};

/// Render the family forest from `(language, proto)` pairs as an indented
/// ASCII tree. Roots and siblings are sorted for a stable layout.
pub fn render_tree(pairs: &[(String, Option<String>)]) -> String {
    let names: BTreeSet<&str> = pairs.iter().map(|(n, _)| n.as_str()).collect();
    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut roots: Vec<&str> = Vec::new();
    for (name, proto) in pairs {
        match proto.as_deref().filter(|p| names.contains(p)) {
            Some(p) => children.entry(p).or_default().push(name),
            None => roots.push(name),
        }
    }
    for v in children.values_mut() {
        v.sort_unstable();
    }
    roots.sort_unstable();

    let mut out = String::new();
    for root in &roots {
        out.push_str(root);
        out.push('\n');
        render_children(root, "", &children, &mut out);
    }
    out
}

fn render_children(name: &str, prefix: &str, children: &BTreeMap<&str, Vec<&str>>, out: &mut String) {
    let Some(kids) = children.get(name) else { return };
    let n = kids.len();
    for (i, kid) in kids.iter().enumerate() {
        let last = i == n - 1;
        out.push_str(prefix);
        out.push_str(if last { "└─ " } else { "├─ " });
        out.push_str(kid);
        out.push('\n');
        let next = format!("{prefix}{}", if last { "   " } else { "│  " });
        render_children(kid, &next, children, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, proto: Option<&str>) -> (String, Option<String>) {
        (name.to_string(), proto.map(String::from))
    }

    #[test]
    fn renders_a_family_forest() {
        let pairs = vec![
            p("ProtoEldarin", None),
            p("Eldar", Some("ProtoEldarin")),
            p("Sindarin", Some("ProtoEldarin")),
            p("LowEldar", Some("Eldar")),
            p("Klingon", None), // a second, unrelated root
        ];
        let tree = render_tree(&pairs);
        let expected = "\
Klingon
ProtoEldarin
├─ Eldar
│  └─ LowEldar
└─ Sindarin
";
        assert_eq!(tree, expected);
    }

    #[test]
    fn a_proto_that_is_not_a_known_language_makes_a_root() {
        // "Eldar" names an unknown proto → Eldar is a root, not lost.
        let pairs = vec![p("Eldar", Some("Nonexistent"))];
        assert_eq!(render_tree(&pairs), "Eldar\n");
    }
}
