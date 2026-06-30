//! RESRCH-1 (R-P9) — the `/command` namespace (RFC §9). A pure parser over the
//! query-prompt text; the dispatcher in `app.rs` routes the parsed `Command`.
//! Unknown commands surface a status-bar error and do nothing.

/// A parsed `/command`. Arguments are pre-split; the app interprets them.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Command {
    /// `/fact "prompt" [→ path]` — extract last response → Facts (R-P10).
    Fact { prompt: Option<String>, path: Option<String> },
    /// `/note "prompt" [→ path]` — extract → Notes (R-P11).
    Note { prompt: Option<String>, path: Option<String> },
    /// `/goto facts/path/slug` — navigate the Facts tree (R-P12).
    Goto(String),
    /// `/diff` — semantic similarity check (R-P13).
    Diff,
    /// `/verify` — LLM confidence probe on the last response (R-P14).
    Verify,
    /// `/factcheck` — audit the whole Facts corpus for truth + mutual
    /// consistency (multi-call).
    FactCheck,
    /// `/sources` — list each fact's recorded provenance (RESRCH-2.1).
    Sources,
    /// `/import [path]` — ingest a document as a research source, or list the
    /// imported sources when bare (RESRCH-2 / R2-B).
    Import(Option<String>),
    /// `/forget <name>` — remove an imported research source.
    Forget(String),
    /// `/promote [notes/path] [→ facts/path]` — turn a Note into a verified Fact.
    Promote { note: Option<String>, path: Option<String> },
    /// `/chain q1 → q2 → q3` — sequential research pipeline (R-P15).
    Chain(Vec<String>),
    /// `/rag [facts+full|facts|full]` — switch RAG mode.
    Rag(Option<String>),
    /// `/clear` — clear the chat window.
    Clear,
    /// `/save [name]` — save / rename the current thread.
    Save(Option<String>),
    /// An unrecognised `/word`.
    Unknown(String),
}

/// Split a command argument into its first quoted string and an optional
/// `→ path` (the arrow may be the unicode `→` or the ASCII `->`).
fn split_prompt_and_path(rest: &str) -> (Option<String>, Option<String>) {
    let prompt = first_quoted(rest);
    let path = arrow_tail(rest);
    (prompt, path)
}

/// The first `"…"` substring, unquoted.
fn first_quoted(s: &str) -> Option<String> {
    let start = s.find('"')?;
    let after = &s[start + 1..];
    let end = after.find('"')?;
    let inner = after[..end].trim();
    if inner.is_empty() { None } else { Some(inner.to_string()) }
}

/// The text after the first `→` / `->`, trimmed (the insertion path).
fn arrow_tail(s: &str) -> Option<String> {
    let idx = s.find('→').map(|i| (i, '→'.len_utf8())).or_else(|| s.find("->").map(|i| (i, 2)));
    let (i, w) = idx?;
    let tail = s[i + w..].trim();
    if tail.is_empty() { None } else { Some(tail.to_string()) }
}

/// Parse `/command …`. Returns `None` for non-command input (no leading `/`).
pub(super) fn parse(input: &str) -> Option<Command> {
    let input = input.trim();
    let body = input.strip_prefix('/')?;
    let (name, rest) = match body.split_once(char::is_whitespace) {
        Some((n, r)) => (n, r.trim()),
        None => (body, ""),
    };
    let cmd = match name.to_ascii_lowercase().as_str() {
        "fact" => {
            let (prompt, path) = split_prompt_and_path(rest);
            // Allow a bare unquoted prompt (`/fact extract the figure`) too.
            let prompt = prompt.or_else(|| {
                let head = arrow_head(rest);
                if head.is_empty() { None } else { Some(head) }
            });
            Command::Fact { prompt, path }
        }
        "note" => {
            let (prompt, path) = split_prompt_and_path(rest);
            let prompt = prompt.or_else(|| {
                let head = arrow_head(rest);
                if head.is_empty() { None } else { Some(head) }
            });
            Command::Note { prompt, path }
        }
        "goto" => Command::Goto(rest.to_string()),
        "diff" => Command::Diff,
        "verify" => Command::Verify,
        "factcheck" => Command::FactCheck,
        "sources" => Command::Sources,
        "import" => Command::Import(if rest.is_empty() { None } else { Some(rest.to_string()) }),
        "forget" => Command::Forget(rest.to_string()),
        "promote" => {
            // `/promote [notes/path] [→ facts/path]` — both optional.
            let note = arrow_head(rest);
            let path = arrow_tail(rest);
            Command::Promote {
                note: if note.is_empty() { None } else { Some(note) },
                path,
            }
        }
        "chain" => {
            let steps: Vec<String> = split_steps(rest);
            Command::Chain(steps)
        }
        "rag" => Command::Rag(if rest.is_empty() { None } else { Some(rest.to_string()) }),
        "clear" => Command::Clear,
        "save" => Command::Save(if rest.is_empty() { None } else { Some(rest.to_string()) }),
        other => Command::Unknown(other.to_string()),
    };
    Some(cmd)
}

/// The portion of `rest` before any `→` (for a bare unquoted `/fact` prompt).
fn arrow_head(s: &str) -> String {
    let cut = s
        .find('→')
        .or_else(|| s.find("->"))
        .unwrap_or(s.len());
    s[..cut].trim().to_string()
}

/// Split a `/chain` argument on `→` / `->`, trimming and dropping empties.
fn split_steps(s: &str) -> Vec<String> {
    s.split('→')
        .flat_map(|seg| seg.split("->"))
        .map(|seg| seg.trim().to_string())
        .filter(|seg| !seg.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_command_is_none() {
        assert!(parse("hello world").is_none());
        assert!(parse("  not a command").is_none());
    }

    #[test]
    fn fact_with_prompt_and_path() {
        let c = parse("/fact \"extract the capacity figure\" → Facts/Rome/Engineering").unwrap();
        assert_eq!(
            c,
            Command::Fact {
                prompt: Some("extract the capacity figure".into()),
                path: Some("Facts/Rome/Engineering".into()),
            }
        );
    }

    #[test]
    fn fact_bare_prompt_no_path() {
        let c = parse("/fact extract the figure").unwrap();
        assert_eq!(c, Command::Fact { prompt: Some("extract the figure".into()), path: None });
    }

    #[test]
    fn fact_empty_argument() {
        assert_eq!(parse("/fact").unwrap(), Command::Fact { prompt: None, path: None });
    }

    #[test]
    fn goto_and_simple_commands() {
        assert_eq!(parse("/goto facts/rome/eng").unwrap(), Command::Goto("facts/rome/eng".into()));
        assert_eq!(parse("/diff").unwrap(), Command::Diff);
        assert_eq!(parse("/verify").unwrap(), Command::Verify);
        assert_eq!(parse("/factcheck").unwrap(), Command::FactCheck);
        assert_eq!(parse("/sources").unwrap(), Command::Sources);
        assert_eq!(parse("/import /docs/rome.md").unwrap(), Command::Import(Some("/docs/rome.md".into())));
        assert_eq!(parse("/import").unwrap(), Command::Import(None));
        assert_eq!(parse("/forget rome").unwrap(), Command::Forget("rome".into()));
        assert_eq!(
            parse("/promote notes/rome/idea → Facts/Rome").unwrap(),
            Command::Promote { note: Some("notes/rome/idea".into()), path: Some("Facts/Rome".into()) }
        );
        assert_eq!(parse("/promote").unwrap(), Command::Promote { note: None, path: None });
        assert_eq!(parse("/clear").unwrap(), Command::Clear);
        assert_eq!(parse("/rag facts").unwrap(), Command::Rag(Some("facts".into())));
        assert_eq!(parse("/save rome").unwrap(), Command::Save(Some("rome".into())));
    }

    #[test]
    fn chain_splits_on_arrows() {
        let c = parse("/chain a? → b? -> c?").unwrap();
        assert_eq!(c, Command::Chain(vec!["a?".into(), "b?".into(), "c?".into()]));
        // Empty steps rejected.
        let c2 = parse("/chain a → → b").unwrap();
        assert_eq!(c2, Command::Chain(vec!["a".into(), "b".into()]));
    }

    #[test]
    fn unknown_command() {
        assert_eq!(parse("/frobnicate x").unwrap(), Command::Unknown("frobnicate".into()));
    }
}
