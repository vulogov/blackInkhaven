//! WBLD-1 (WB-P11) — the world dossier export.
//!
//! `/export` assembles a single readable Markdown document from everything the
//! worldbuilder knows: the compiled world state, the plausibility report, the
//! magic ledger, the recorded `fact:world` facts, and the Worldbuilding Journey
//! (the session timeline). It is a *record*, not generated prose — every line is
//! the author's own material or a deterministic measurement. Markdown keeps it
//! portable (no Typst-compile step) and naturally Unicode-clean for any project
//! language.

use crate::world::plausibility::Warning;
use crate::world::types::MagicLedger;

use super::session::SessionTurn;

/// Everything the dossier renders. Borrowed so the app assembles it without
/// cloning the world.
pub(super) struct DossierInput<'a> {
    pub world_name: &'a str,
    pub generated_at: &'a str,
    /// The compiled-state summary (`summarise_compiled`), if a world compiled.
    pub compiled: Option<&'a str>,
    pub score: Option<u8>,
    pub warnings: &'a [Warning],
    pub ledger: Option<&'a MagicLedger>,
    /// `(title, body)` of each `fact:world` paragraph, in tree order.
    pub facts: &'a [(String, String)],
    pub journey: &'a [SessionTurn],
}

/// Render the dossier to a Markdown string.
pub(super) fn build_dossier(input: &DossierInput) -> String {
    let mut s = String::new();
    let name = if input.world_name.trim().is_empty() { "Untitled world" } else { input.world_name.trim() };
    s.push_str(&format!("# World Dossier — {name}\n\n"));
    s.push_str(&format!("_Generated {} by the Inkhaven worldbuilder._\n\n", input.generated_at));

    // Plausibility headline.
    if let Some(score) = input.score {
        s.push_str(&format!("**Plausibility:** {score}/100\n\n"));
    }

    // Compiled world state.
    s.push_str("## Compiled world state\n\n");
    match input.compiled {
        Some(c) if !c.trim().is_empty() => {
            s.push_str("```\n");
            s.push_str(c.trim());
            s.push_str("\n```\n\n");
        }
        _ => s.push_str("_No world compiled yet — declare one (interview or `/set`), then `/compile`._\n\n"),
    }

    // Plausibility detail.
    if !input.warnings.is_empty() {
        s.push_str("## Plausibility warnings\n\n");
        for w in input.warnings {
            let sev = match w.severity {
                crate::world::plausibility::Severity::High => "HIGH",
                crate::world::plausibility::Severity::Medium => "MEDIUM",
                crate::world::plausibility::Severity::Low => "LOW",
            };
            s.push_str(&format!("- **[{sev}]** {}\n", w.text.trim()));
        }
        s.push('\n');
    }

    // Magic ledger.
    if let Some(l) = input.ledger {
        s.push_str("## Magic ledger\n\n");
        s.push_str(&format!(
            "Ledger is **{}** with {} rule(s).\n\n",
            if l.enabled { "enabled" } else { "disabled" },
            l.rules.len()
        ));
        for r in &l.rules {
            let kind = if r.kind.trim().is_empty() { "(no kind)" } else { r.kind.trim() };
            let covers = if r.covers.is_empty() { "—".to_string() } else { r.covers.join(", ") };
            s.push_str(&format!("- **{kind}** — covers {covers}"));
            if !r.description.trim().is_empty() {
                s.push_str(&format!(": {}", r.description.trim()));
            }
            s.push('\n');
        }
        s.push('\n');
    }

    // Recorded world facts.
    s.push_str("## World facts\n\n");
    if input.facts.is_empty() {
        s.push_str("_None recorded yet — capture them with `/wfact`._\n\n");
    } else {
        for (title, body) in input.facts {
            s.push_str(&format!("### {}\n\n", title.trim()));
            let body = body.trim();
            if !body.is_empty() {
                s.push_str(body);
                s.push_str("\n\n");
            }
        }
    }

    // The journey.
    s.push_str("## Worldbuilding Journey\n\n");
    if input.journey.is_empty() {
        s.push_str("_No steps recorded yet._\n\n");
    } else {
        for t in input.journey {
            let when = t.at.get(..16).unwrap_or(&t.at);
            let arc = match (t.plausibility_before, t.plausibility_after) {
                (Some(b), Some(a)) if a != b => format!(" · ★{b}→{a}"),
                (_, Some(a)) => format!(" · ★{a}"),
                _ => String::new(),
            };
            s.push_str(&format!(
                "{}. `{when}` **{}** — {}{arc}\n",
                t.seq,
                t.user.trim(),
                t.assistant_summary.trim(),
            ));
        }
        s.push('\n');
    }

    s
}

/// A Typst string literal (`"…"`, quotes included) with the only two characters
/// that can break a string escaped, and control characters flattened to spaces.
/// Rendering user prose as a *string* (not markup) makes injection impossible —
/// no `#`, `*`, `$`, `@`, `<` in a fact body is ever interpreted.
fn ts(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Render the dossier as a self-contained, compilable Typst document (WS-P3).
/// Structure is native Typst; every piece of author/measured text is emitted as
/// a string literal via [`ts`], so any project language and any punctuation is
/// safe. Compiled to PDF in-process by `/export --pdf`.
pub(super) fn build_dossier_typst(input: &DossierInput) -> String {
    let name = if input.world_name.trim().is_empty() { "Untitled world" } else { input.world_name.trim() };
    let mut s = String::new();
    s.push_str("#set document(title: ");
    s.push_str(&ts(&format!("World Dossier — {name}")));
    s.push_str(")\n");
    s.push_str("#set page(paper: \"a4\", margin: 2cm, numbering: \"1\")\n");
    s.push_str("#set text(font: (\"Libertinus Serif\", \"New Computer Modern\"), size: 11pt)\n");
    s.push_str("#set heading(numbering: \"1.\")\n\n");
    s.push_str(&format!("#text(size: 20pt, weight: \"bold\")[World Dossier — #({})]\n\n", ts(name)));
    s.push_str(&format!("#emph[Generated #({}) by the Inkhaven worldbuilder.]\n\n", ts(input.generated_at)));

    if let Some(score) = input.score {
        s.push_str(&format!("*Plausibility:* {score}/100\n\n"));
    }

    s.push_str("= Compiled world state\n\n");
    match input.compiled {
        Some(c) if !c.trim().is_empty() => {
            s.push_str(&format!("#raw(block: true, {})\n\n", ts(c.trim())));
        }
        _ => s.push_str("#emph[No world compiled yet.]\n\n"),
    }

    if !input.warnings.is_empty() {
        s.push_str("= Plausibility warnings\n\n");
        for w in input.warnings {
            let sev = match w.severity {
                crate::world::plausibility::Severity::High => "HIGH",
                crate::world::plausibility::Severity::Medium => "MEDIUM",
                crate::world::plausibility::Severity::Low => "LOW",
            };
            s.push_str(&format!("- *[{sev}]* #({})\n", ts(w.text.trim())));
        }
        s.push('\n');
    }

    if let Some(l) = input.ledger {
        s.push_str("= Magic ledger\n\n");
        s.push_str(&format!(
            "Ledger is *{}* with {} rule(s).\n\n",
            if l.enabled { "enabled" } else { "disabled" },
            l.rules.len()
        ));
        for r in &l.rules {
            let kind = if r.kind.trim().is_empty() { "(no kind)".to_string() } else { r.kind.trim().to_string() };
            let covers = if r.covers.is_empty() { "—".to_string() } else { r.covers.join(", ") };
            s.push_str(&format!("- *#({})* — covers #({})", ts(&kind), ts(&covers)));
            if !r.description.trim().is_empty() {
                s.push_str(&format!(": #({})", ts(r.description.trim())));
            }
            s.push('\n');
        }
        s.push('\n');
    }

    s.push_str("= World facts\n\n");
    if input.facts.is_empty() {
        s.push_str("#emph[None recorded yet.]\n\n");
    } else {
        for (title, body) in input.facts {
            s.push_str(&format!("== #({})\n\n", ts(title.trim())));
            let body = body.trim();
            if !body.is_empty() {
                s.push_str(&format!("#({})\n\n", ts(body)));
            }
        }
    }

    s.push_str("= Worldbuilding Journey\n\n");
    if input.journey.is_empty() {
        s.push_str("#emph[No steps recorded yet.]\n\n");
    } else {
        for t in input.journey {
            let when = t.at.get(..16).unwrap_or(&t.at);
            let arc = match (t.plausibility_before, t.plausibility_after) {
                (Some(b), Some(a)) if a != b => format!(" · {b}→{a}"),
                (_, Some(a)) => format!(" · {a}"),
                _ => String::new(),
            };
            s.push_str(&format!(
                "+ #({}) — #({}){}\n",
                ts(&format!("{} {}", when, t.user.trim())),
                ts(t.assistant_summary.trim()),
                arc,
            ));
        }
        s.push('\n');
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dossier_has_every_section_and_reports_empty_states() {
        let input = DossierInput {
            world_name: "Aldoria",
            generated_at: "2026-07-29T10:00",
            compiled: Some("World: Aldoria\nAstronomy: G star"),
            score: Some(88),
            warnings: &[Warning::medium("nations: capital is landlocked")],
            ledger: None,
            facts: &[],
            journey: &[],
        };
        let md = build_dossier(&input);
        assert!(md.contains("# World Dossier — Aldoria"));
        assert!(md.contains("**Plausibility:** 88/100"));
        assert!(md.contains("## Compiled world state"));
        assert!(md.contains("G star"));
        assert!(md.contains("**[MEDIUM]** nations: capital is landlocked"));
        assert!(md.contains("## World facts"));
        assert!(md.contains("capture them with `/wfact`"));
        assert!(md.contains("## Worldbuilding Journey"));
        assert!(md.contains("_No steps recorded yet._"));
    }

    #[test]
    fn typst_dossier_escapes_prose_as_string_literals() {
        // A fact body full of Typst-hostile characters must be neutralised.
        let input = DossierInput {
            world_name: "Aldoria",
            generated_at: "t",
            compiled: None,
            score: Some(90),
            warnings: &[],
            ledger: None,
            facts: &[("Injection".into(), "danger: #set page(width: 1pt) \"quote\" \\ end".into())],
            journey: &[],
        };
        let typ = build_dossier_typst(&input);
        // Structure is present.
        assert!(typ.contains("#set document(title:"));
        assert!(typ.contains("= World facts"));
        // The whole hostile body is wrapped in ONE `#("…")` string literal with
        // its quotes and backslashes escaped — so the `#set` inside it is inert
        // text, not a live directive.
        assert!(typ.contains(r#"#("danger: #set page(width: 1pt) \"quote\" \\ end")"#));
    }

    #[test]
    fn dossier_renders_facts_and_journey_when_present() {
        let input = DossierInput {
            world_name: "Aldoria",
            generated_at: "t",
            compiled: None,
            score: None,
            warnings: &[],
            ledger: None,
            facts: &[("Tidal harbours".into(), "The moons drive double tides.".into())],
            journey: &[SessionTurn {
                seq: 1,
                at: "2026-07-29T09:30:00Z".into(),
                user: "/star K".into(),
                assistant_summary: "shaping delta accepted".into(),
                plausibility_before: Some(100),
                plausibility_after: Some(95),
                ..Default::default()
            }],
        };
        let md = build_dossier(&input);
        assert!(md.contains("### Tidal harbours"));
        assert!(md.contains("The moons drive double tides."));
        assert!(md.contains("1. `2026-07-29T09:30` **/star K** — shaping delta accepted · ★100→95"));
        // No compiled world → the hint shows.
        assert!(md.contains("declare one"));
    }
}
