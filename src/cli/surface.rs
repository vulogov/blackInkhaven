//! The CLI surface as clap sees it (3.20, CLI-1): a walker over
//! `Cli::command()` behind the convention tests and the docs-drift test that
//! keeps every `inkhaven …` invocation in the documentation pointing at a
//! command and flags that exist.
#![cfg(test)]

use clap::{Command, CommandFactory};

/// The whole command tree, built (so global arguments such as
/// `-p/--project-directory` are propagated onto every subcommand).
pub(super) fn cli_command() -> Command {
    let mut c = super::Cli::command();
    c.build();
    c
}

/// Every leaf command with its path, e.g. `["canon", "read"]`. The automatic
/// `help` subcommands are skipped.
pub(super) fn leaves(cmd: &Command) -> Vec<(Vec<String>, &Command)> {
    fn walk<'a>(cmd: &'a Command, path: Vec<String>, out: &mut Vec<(Vec<String>, &'a Command)>) {
        let mut any = false;
        for sub in cmd.get_subcommands() {
            if sub.get_name() == "help" {
                continue;
            }
            any = true;
            let mut p = path.clone();
            p.push(sub.get_name().to_string());
            walk(sub, p, out);
        }
        if !any && !path.is_empty() {
            out.push((path, cmd));
        }
    }
    let mut out = Vec::new();
    walk(cmd, Vec::new(), &mut out);
    out
}

fn has_long(cmd: &Command, long: &str) -> bool {
    cmd.get_arguments().any(|a| {
        a.get_long() == Some(long) || a.get_all_aliases().is_some_and(|al| al.contains(&long))
    })
}

fn find_sub<'a>(cmd: &'a Command, name: &str) -> Option<&'a Command> {
    cmd.get_subcommands().find(|s| {
        s.get_name() == name || s.get_all_aliases().any(|a| a == name)
    })
}

/// One documented invocation: where it was found and its tokens after
/// `inkhaven`, up to the end of the command line.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Invocation {
    pub line: usize,
    pub tokens: Vec<String>,
}

/// Every `inkhaven …` command line a document shows: anchored by a backtick
/// anywhere, or by a `$ ` prompt or the start of a line inside a fenced code
/// block (a prose sentence that happens to start with the program's name is
/// not a command); it ends at a closing backtick, the line end, a pipe,
/// `&&`, `;` or a `#` comment. Continuation lines (`\` + newline) are joined
/// first. A fenced line whose tokens hold a bare `:` / `—` / `=` is program
/// output (`inkhaven version : 1.2.15`), not an invocation.
pub(super) fn documented_invocations(text: &str) -> Vec<Invocation> {
    let joined = text.replace("\\\n", " ");
    let mut out = Vec::new();
    let mut line = 1usize;
    let mut cursor = 0usize;
    while let Some(rel) = joined[cursor..].find("inkhaven ") {
        let i = cursor + rel;
        line += joined[cursor..i].matches('\n').count();
        cursor = i + "inkhaven ".len();
        let before = &joined[..i];
        let in_fence = before.lines().filter(|l| l.trim_start().starts_with("```")).count() % 2 == 1;
        let at_line_start = before.is_empty()
            || before.ends_with('\n')
            || before.trim_end_matches([' ', '\t']).ends_with('\n');
        let anchored = before.ends_with('`') || (in_fence && (before.ends_with("$ ") || at_line_start));
        if !anchored {
            continue;
        }
        let rest = &joined[cursor..];
        let end = rest.find(['`', '\n', '|']).unwrap_or(rest.len());
        let mut cmdline = &rest[..end];
        for stop in ["&&", ";", " #"] {
            if let Some(p) = cmdline.find(stop) {
                cmdline = &cmdline[..p];
            }
        }
        // Program output quoted in a fence (`inkhaven version : 1.2.15`), not
        // a command line.
        if [" : ", " — ", " = "].iter().any(|s| cmdline.contains(s)) {
            continue;
        }
        let tokens: Vec<String> = cmdline
            .split_whitespace()
            .map(|t| t.trim_matches(['[', ']', '(', ')', ',', '.', ';', ':', '"', '\'']).to_string())
            .filter(|t| !t.is_empty())
            .collect();
        if tokens.is_empty() {
            continue;
        }
        out.push(Invocation { line, tokens });
    }
    out
}

/// Check one invocation against the command tree. `Ok(())` when the command
/// path resolves and every `--flag` exists on the resolved command (or on an
/// ancestor, for `inkhaven --flag sub` spellings); otherwise the problem.
pub(super) fn check_invocation(root: &Command, tokens: &[String]) -> Result<(), String> {
    let looks_like_name = |t: &str| {
        t.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    };
    let mut chain: Vec<&Command> = vec![root];
    let mut i = 0usize;
    let mut descending = true;
    let mut flags: Vec<String> = Vec::new();
    while i < tokens.len() {
        let t = tokens[i].as_str();
        if t == "-p" || t == "--project-directory" {
            i += 2;
            continue;
        }
        if let Some(f) = t.strip_prefix("--") {
            let name = f.split(['=', '<']).next().unwrap_or(f);
            if !name.is_empty() && name != "help" && name != "version" {
                flags.push(name.to_string());
            }
            i += 1;
            continue;
        }
        if t.starts_with('-') {
            i += 1;
            continue;
        }
        if descending {
            let cur = *chain.last().unwrap();
            if let Some(sub) = find_sub(cur, t) {
                chain.push(sub);
                i += 1;
                continue;
            }
            if chain.len() == 1 {
                if looks_like_name(t) {
                    return Err(format!("`inkhaven {t}` is not a command"));
                }
                // `inkhaven <command>` and similar placeholders.
                return Ok(());
            }
            if cur.has_subcommands() && looks_like_name(t) && !t.contains('_') {
                let path: Vec<&str> = chain.iter().skip(1).map(|c| c.get_name()).collect();
                return Err(format!("`inkhaven {} {t}` is not a command", path.join(" ")));
            }
            descending = false;
        }
        i += 1;
    }
    let path: Vec<&str> = chain.iter().skip(1).map(|c| c.get_name()).collect();
    for f in flags {
        if !chain.iter().any(|c| has_long(c, &f)) {
            return Err(format!("`inkhaven {}` has no `--{f}`", path.join(" ")));
        }
    }
    Ok(())
}

/// The documents whose command lines must match the binary: the
/// README, every top-level guide, the Bund reference and the tutorials.
/// Release notes and proposals are history and are not checked.
pub(crate) fn docs() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let docs = root.join("Documentation");
    let mut files = vec![root.join("README.md")];
    let mut collect = |dir: &std::path::Path| {
        let mut v: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .collect();
        v.sort();
        files.extend(v);
    };
    collect(&docs);
    collect(&docs.join("Bund"));
    collect(&docs.join("Tutorials"));
    files
        .into_iter()
        .map(|p| {
            let rel = p.strip_prefix(root).unwrap_or(&p).display().to_string();
            (rel, std::fs::read_to_string(&p).expect("read doc"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_big_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .expect("spawn")
            .join()
            .expect("join");
    }

    #[test]
    fn scanner_reads_prose_tables_prompts_and_continuations() {
        let text = "Run `inkhaven canon read --json` then\n\
                    | `inkhaven doctor [--fix]` | … |\n\
                    ```sh\n$ inkhaven export \\\n  --format pdf -o out.pdf | head\n\
                    the inkhaven binary reads nothing here.\n\
                    inkhaven -p /tmp/p list # comment\n\
                    inkhaven version : 1.2.15\n```\n\
                    inkhaven speaks the configured text at startup.\n";
        let got = documented_invocations(text);
        let tokens: Vec<Vec<String>> = got.iter().map(|i| i.tokens.clone()).collect();
        assert_eq!(
            tokens,
            vec![
                vec!["canon", "read", "--json"],
                vec!["doctor", "--fix"],
                vec!["export", "--format", "pdf", "-o", "out.pdf"],
                vec!["-p", "/tmp/p", "list"],
            ]
            .into_iter()
            .map(|v| v.into_iter().map(String::from).collect::<Vec<_>>())
            .collect::<Vec<_>>()
        );
        assert_eq!(got[0].line, 1);
        assert_eq!(got[3].line, 6);
    }

    #[test]
    fn checker_resolves_paths_aliases_and_globals() {
        with_big_stack(|| {
            let root = cli_command();
            let ok = |s: &str| {
                let t: Vec<String> = s.split_whitespace().map(String::from).collect();
                check_invocation(&root, &t)
            };
            assert_eq!(ok("list"), Ok(()));
            assert_eq!(ok("-p /tmp/x canon read --json"), Ok(()));
            assert_eq!(ok("language add-word Qya mara --project-directory ."), Ok(()));
            assert_eq!(ok("<command> --help"), Ok(()));
            assert!(ok("no-such-command").is_err());
            assert!(ok("canon no-such-sub").is_err());
            assert!(ok("canon read --no-such-flag").is_err());
        });
    }

    /// Every `inkhaven …` command line in the documentation names a command
    /// and flags the binary has. Fails on the next drift instead of waiting
    /// for a reader.
    #[test]
    fn docs_cli_surface() {
        with_big_stack(|| {
            let root = cli_command();
            let mut problems = Vec::new();
            for (file, text) in docs() {
                for inv in documented_invocations(&text) {
                    if let Err(e) = check_invocation(&root, &inv.tokens) {
                        problems.push(format!("{file}:{}: {e}", inv.line));
                    }
                }
            }
            assert!(problems.is_empty(), "documentation drift ({}):\n  {}", problems.len(), problems.join("\n  "));
        });
    }

    /// Every output-path flag is spelled `-o/--output` (or `--output-dir`
    /// for a directory); `--out` survives only as a hidden alias (3.20,
    /// CLI-1 — the convention `CLI.md` states).
    #[test]
    fn output_flags_are_spelled_output() {
        with_big_stack(|| {
            let root = cli_command();
            let mut problems = Vec::new();
            for (path, cmd) in leaves(&root) {
                for a in cmd.get_arguments() {
                    let Some(long) = a.get_long() else { continue };
                    if long == "out" || long == "out-dir" {
                        problems.push(format!("`inkhaven {}` spells its output flag `--{long}`", path.join(" ")));
                    }
                    if long == "output" && a.get_short() != Some('o') {
                        problems.push(format!("`inkhaven {}` has `--output` without `-o`", path.join(" ")));
                    }
                }
            }
            assert!(problems.is_empty(), "{}", problems.join("\n"));
        });
    }

    /// The whole surface, one line per leaf command: whether it takes
    /// `--json`, and every argument spelled like an output path. Run with
    /// `--ignored --nocapture` to read it.
    #[test]
    #[ignore = "inventory printer; run with --ignored --nocapture cli_surface_inventory"]
    fn cli_surface_inventory() {
        with_big_stack(|| {
            let root = cli_command();
            let leaves = leaves(&root);
            let mut json = 0usize;
            let mut out_like = 0usize;
            for (path, cmd) in &leaves {
                let has_json = has_long(cmd, "json");
                json += has_json as usize;
                let outs: Vec<String> = cmd
                    .get_arguments()
                    .filter(|a| {
                        a.get_long().is_some_and(|l| {
                            matches!(l, "out" | "output" | "out-dir" | "output-dir" | "outdir" | "dest" | "destination" | "to" | "file")
                                || l.ends_with("-out")
                                || l.starts_with("out-")
                                || l.starts_with("output-")
                        })
                    })
                    .map(|a| {
                        format!(
                            "--{}{}{}{}",
                            a.get_long().unwrap_or(""),
                            a.get_short().map(|s| format!("/-{s}")).unwrap_or_default(),
                            a.get_all_aliases().map(|al| format!(" (alias {})", al.join(","))).unwrap_or_default(),
                            if a.is_hide_set() { " hidden" } else { "" }
                        )
                    })
                    .collect();
                out_like += outs.len();
                println!(
                    "{:<48} {} {}{}",
                    path.join(" "),
                    if has_json { "json" } else { "    " },
                    outs.join(" "),
                    if cmd.is_hide_set() { " [hidden]" } else { "" }
                );
            }
            println!("{} leaf commands · {} take --json · {} output-like arguments", leaves.len(), json, out_like);
        });
    }
}
