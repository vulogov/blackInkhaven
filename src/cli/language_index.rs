//! The `inkhaven language` command index (LANG-IMPROVE-1 LI-10).
//!
//! [`render_markdown`] walks clap's own tree, so the index in
//! `Documentation/CONLANG.md` (pasted in by the release checklist via the
//! hidden `inkhaven language _index-markdown`) cannot name a command that does
//! not exist.  The `GROUPS` table below is the only hand-maintained part: it
//! files every subcommand under a heading, and a test fails when a new command
//! is not filed (or an old one is still listed).  The drift test at the bottom
//! runs the check the other way: every `inkhaven language <name>` and every
//! `--flag` written in `CONLANG.md`, `CLI.md` and the tutorials must exist.

use clap::Command;

/// Heading → the subcommands filed under it, in the order the index prints.
pub const GROUPS: &[(&str, &[&str])] = &[
    ("Set-up and lexicon", &[
        "init", "list", "doctor", "scaffold", "suggest-phonemes", "define-rule", "grammar",
        "add-word", "remove-word", "query", "sort-dictionary", "import", "export",
        "generate-lexicon", "derive-lexicon", "derive", "gaps", "audit", "stats", "scan-manuscript",
    ]),
    ("Phonology", &[
        "generate-word", "syllabify", "ipa", "stress", "romanize", "tone", "scan", "trace",
    ]),
    ("Morphology and syntax", &[
        "paradigm", "agree", "sentence", "relative", "complement", "coordinate", "gloss", "igt",
        "parse", "tree", "movement", "binding", "link", "check", "check-clause", "check-agreement",
    ]),
    ("Analysis", &[
        "metrics", "universals", "grammar-check", "pairs", "naturalness", "harmony",
        "distribution", "sketch",
    ]),
    ("Hypotheses", &[
        "hypothesize", "hypotheses", "hypothesis", "hypothesis-status", "hypothesis-check",
    ]),
    ("Corpus and texts", &["texts", "frequency", "concordance", "collocations", "corpus", "eval"]),
    ("Translation", &["translate", "reverse", "cross", "remember", "memory", "export-translation"]),
    ("Diachronics", &["family-tree", "cognates", "sound-change", "reconstruct", "realism-check"]),
    ("Varieties and contact", &[
        "varieties", "lect", "dialects", "propose-dialect", "borrow", "propose-loans", "areal",
        "areal-check", "idiolect", "link-place", "link-character", "speakers", "ecology",
    ]),
    ("Creative text and idiom", &["compose", "idioms", "idiom-add", "metaphor-add"]),
    ("Books", &["dictionary", "grammar-book", "tutorial"]),
    ("Writing systems and fonts", &[
        "font-config", "font-templates", "font-import-glyph", "font-build", "font-compose",
        "transliterate", "spatial-typst", "glyph-draft", "glyph-lint",
    ]),
];

/// The `language` subcommand tree.
pub fn language_command() -> Command {
    use clap::CommandFactory;
    super::Cli::command()
        .find_subcommand("language")
        .expect("`language` subcommand")
        .clone()
}

/// Visible subcommand names, in clap's order.
pub fn visible_names(language: &Command) -> Vec<String> {
    language
        .get_subcommands()
        .filter(|c| !c.is_hide_set() && c.get_name() != "help")
        .map(|c| c.get_name().to_string())
        .collect()
}

/// One line from a subcommand's `about`: the tracker tag (`LANG-1 P1.4 — `,
/// `1.7 LING-1 (Wave 4) — `) dropped, the first sentence kept.
pub fn one_line_about(cmd: &Command) -> String {
    let about = cmd.get_about().map(|s| s.to_string()).unwrap_or_default();
    let about = about.split_whitespace().collect::<Vec<_>>().join(" ");
    let body = match about.split_once(" — ") {
        Some((tag, rest))
            if tag.split_whitespace().count() <= 4 && tag.chars().any(|c| c.is_ascii_digit()) =>
        {
            rest
        }
        _ => about.as_str(),
    };
    let mut first = body.to_string();
    if let Some(i) = first.find(". ") {
        first.truncate(i);
    }
    let first = first.trim_end_matches(['.', ' ']).to_string();
    let mut chars = first.chars();
    match chars.next() {
        Some(c) if c.is_lowercase() => c.to_uppercase().collect::<String>() + chars.as_str(),
        _ => first,
    }
}

/// The markdown section, ready to paste into `Documentation/CONLANG.md`.
pub fn render_markdown() -> String {
    let lang = language_command();
    let mut out = String::new();
    out.push_str("## Command index\n\n");
    out.push_str(
        "Every `inkhaven language` subcommand, one line each, generated from the CLI's own \
         help by `inkhaven language _index-markdown` (regenerate it with the README at \
         release time; the `docs_language_surface` test fails when this file names a \
         command or flag that does not exist). `--json` marks a command whose result \
         also comes as JSON; `-o` marks one that writes a file.\n\n",
    );
    // Anything visible that GROUPS has not filed lands under "Other" rather
    // than vanishing (the test below fails on it, so the table gets fixed).
    let filed: std::collections::BTreeSet<&str> = GROUPS.iter().flat_map(|(_, n)| n.iter().copied()).collect();
    let visible = visible_names(&lang);
    let other: Vec<&str> = visible.iter().map(String::as_str).filter(|n| !filed.contains(n)).collect();
    let mut sections: Vec<(&str, Vec<&str>)> = GROUPS.iter().map(|(h, n)| (*h, n.to_vec())).collect();
    if !other.is_empty() {
        sections.push(("Other", other));
    }
    for (heading, names) in &sections {
        out.push_str(&format!("### {heading}\n\n"));
        for name in names {
            let Some(cmd) = lang.find_subcommand(name) else { continue };
            let mut marks = Vec::new();
            if cmd.get_arguments().any(|a| a.get_long() == Some("json")) {
                marks.push("`--json`");
            }
            if cmd.get_arguments().any(|a| a.get_long() == Some("output")) {
                marks.push("`-o`");
            }
            let marks = if marks.is_empty() { String::new() } else { format!(" · {}", marks.join(" ")) };
            out.push_str(&format!("- `{name}` — {}{marks}\n", one_line_about(cmd)));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// `Cli::command()` builds several hundred subcommands; give the walk room.
    fn with_big_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn(f)
            .expect("spawn")
            .join()
            .expect("test body panicked");
    }

    #[test]
    fn every_visible_subcommand_is_filed_once() {
        with_big_stack(|| {
            let lang = language_command();
            let visible: BTreeSet<String> = visible_names(&lang).into_iter().collect();
            let mut filed: BTreeSet<String> = BTreeSet::new();
            for (heading, names) in GROUPS {
                for n in *names {
                    assert!(filed.insert(n.to_string()), "`{n}` filed twice (under {heading})");
                    assert!(visible.contains(*n), "`{n}` under {heading} is not a visible subcommand");
                }
            }
            let missing: Vec<_> = visible.difference(&filed).collect();
            assert!(missing.is_empty(), "subcommands not filed in GROUPS: {missing:?}");
        });
    }

    #[test]
    fn the_about_line_drops_the_tracker_tag() {
        let c = Command::new("x").about("LANG-1 P1.4 — place primary stress on a word per the language's rule. More.");
        assert_eq!(one_line_about(&c), "Place primary stress on a word per the language's rule");
        let c = Command::new("x").about("1.7 IGT-1 (Wave 4) — interlinear glossed text: auto-gloss a sentence");
        assert_eq!(one_line_about(&c), "Interlinear glossed text: auto-gloss a sentence");
        let c = Command::new("x").about("health report for a language");
        assert_eq!(one_line_about(&c), "Health report for a language");
    }

    #[test]
    fn the_rendered_index_names_only_real_commands() {
        with_big_stack(|| {
            let md = render_markdown();
            let lang = language_command();
            for line in md.lines().filter(|l| l.starts_with("- `")) {
                let name = line.trim_start_matches("- `").split('`').next().unwrap();
                assert!(lang.find_subcommand(name).is_some(), "{name}");
            }
            assert!(md.contains("- `sort-dictionary`"));
        });
    }

    // ── the drift test ─────────────────────────────────────────────────────

    /// Every `inkhaven language <name> …` (or `` `language <name>`` in a
    /// table) found in the docs, with the `--flags` written in the same
    /// invocation.
    fn documented_invocations(text: &str) -> Vec<(String, Vec<String>)> {
        let joined = text.replace("\\\n", " ");
        let mut out = Vec::new();
        for (i, _) in joined.match_indices("language ") {
            let before = &joined[..i];
            let anchored = before.ends_with("inkhaven ") || before.ends_with('`');
            if !anchored {
                continue;
            }
            let rest = &joined[i + "language ".len()..];
            let name: String = rest.chars().take_while(|c| c.is_ascii_lowercase() || *c == '-').collect();
            if name.is_empty() || !rest[name.len()..].starts_with(|c: char| c.is_whitespace() || c == '`' || c == ']' || c == ')' || c == '.' || c == ',' || c == ';' || c == ':') {
                continue;
            }
            // Flags up to the end of the invocation: a closing backtick, a
            // line end, a pipe or `&&`.
            let tail = &rest[name.len()..];
            let end = tail.find(['`', '\n', '|']).unwrap_or(tail.len());
            let tail = tail[..end].split("&&").next().unwrap_or("");
            let flags = tail
                .split_whitespace()
                .filter_map(|t| t.trim_matches(['[', ']', '(', ')', ',', '.', ';']).strip_prefix("--"))
                .map(|f| f.split(['=', '<']).next().unwrap_or(f).to_string())
                .filter(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
                .collect();
            out.push((name, flags));
        }
        out
    }

    fn docs() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Documentation");
        let mut files = vec![root.join("CONLANG.md"), root.join("CLI.md")];
        let mut tutorials: Vec<_> = std::fs::read_dir(root.join("Tutorials"))
            .expect("Documentation/Tutorials")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .collect();
        tutorials.sort();
        files.extend(tutorials);
        files
            .into_iter()
            .map(|p| (p.display().to_string(), std::fs::read_to_string(&p).expect("read doc")))
            .collect()
    }

    /// Every `inkhaven language <name>` and `--flag` the docs mention exists
    /// on `LanguageCommand`. Fails the build on the next drift instead of
    /// waiting for a reader.
    #[test]
    fn docs_language_surface() {
        with_big_stack(|| {
            let lang = language_command();
            let mut problems = Vec::new();
            for (file, text) in docs() {
                for (name, flags) in documented_invocations(&text) {
                    let Some(cmd) = lang.find_subcommand(&name) else {
                        problems.push(format!("{file}: `inkhaven language {name}` is not a command"));
                        continue;
                    };
                    for f in flags {
                        if f == "help" || f == "version" {
                            continue;
                        }
                        let known = cmd.get_arguments().any(|a| {
                            a.get_long() == Some(f.as_str())
                                || a.get_all_aliases().is_some_and(|al| al.contains(&f.as_str()))
                        });
                        if !known {
                            problems.push(format!("{file}: `inkhaven language {name}` has no `--{f}`"));
                        }
                    }
                }
            }
            assert!(problems.is_empty(), "documentation drift:\n  {}", problems.join("\n  "));
        });
    }

    #[test]
    fn invocation_scanner_reads_prose_tables_and_continuations() {
        let text = "Run `inkhaven language export Avesha --format anki -o d.csv` then\n\
                    | `language audit [--json]` | …|\n\
                    inkhaven language sentence Avesha \\\n  --subject kira --verb nami | head\n\
                    the language list is long.";
        let got = documented_invocations(text);
        assert_eq!(
            got,
            vec![
                ("export".to_string(), vec!["format".to_string()]),
                ("audit".to_string(), vec!["json".to_string()]),
                ("sentence".to_string(), vec!["subject".to_string(), "verb".to_string()]),
            ]
        );
    }
}
