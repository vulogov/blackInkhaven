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
    /// `/undisputed` — common-sense check of the authorial (undisputed) facts
    /// (RESRCH-UNDISPUTED; read-only, never rewrites).
    Undisputed,
    /// `/synthesize <topic>` — grounded, cited synthesis over the Facts corpus
    /// (RESRCH-5 / R5-A).
    Synthesize(String),
    /// `/outline <topic>` — a structured, fact-citing outline (R5-B).
    Outline(String),
    /// `/gaps <topic>` — open questions the corpus doesn't answer (R5-C).
    Gaps(String),
    /// `/bibliography` — emit the Sources Research chapter as BibTeX (R5-D).
    Bibliography,
    /// `/upgrade [facts/path]` — try to re-ground a `model`-origin fact on a
    /// structured source and raise its provenance tier (R5-E; non-destructive).
    Upgrade(Option<String>),
    /// `/stale [days]` — list `model`/`web` facts older than N days (R5-F).
    Stale(Option<String>),
    /// `/deadsources` — check each `web` URL / `document` file source and report
    /// the ones that no longer resolve (R5-F, dead-source detection).
    DeadSources,
    /// `/sources` — list each fact's recorded provenance (RESRCH-2.1).
    Sources,
    /// `/import [path]` — ingest a document as a research source, or list the
    /// imported sources when bare (RESRCH-2 / R2-B).
    Import(Option<String>),
    /// `/forget <name>` — remove an imported research source.
    Forget(String),
    /// `/web [--ingest|--chat] <query>` — web search & fetch (RESRCH-2 / R2-C).
    /// `ingest` overrides the configured default pipeline (`None` = use default).
    Web { ingest: Option<bool>, query: String },
    /// `/calc <expr>` — evaluate a deterministic Bund expression (R3-C).
    Calc(String),
    /// `/world [layer]` — surface the project's World simulation facts (R3-C).
    World(String),
    /// `/wikidata <query>` — fetch a Wikidata entity's structured claims (R3-A).
    Wikidata(String),
    /// `/geonames <query>` — look up a real-world place in the GeoNames
    /// gazetteer (RESRCH-6-lite; structured tier).
    Geonames(String),
    /// `/gutenberg <query>` — ingest a public-domain Project Gutenberg book as a
    /// research source (RESRCH-GUTENBERG).
    Gutenberg(String),
    /// `/openalex <query>` — fetch the top OpenAlex paper (R3-B).
    OpenAlex(String),
    /// `/arxiv <query>` — fetch the top arXiv paper (R3-B).
    Arxiv(String),
    /// `/triangulate [claim]` — cross-check a claim across the structured sources
    /// (Wikidata + OpenAlex + arXiv) and report agreement (R3-E). Bare → the last
    /// response.
    Triangulate(String),
    /// `/whatswrong [facts/path]` — AI explanation of a fact flagged by
    /// `/factcheck` (RE-P5); bare → the selected/cursor fact.
    WhatsWrong(Option<String>),
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

/// RESRCH-UX (UX-P1) — one command's name, one-line summary, and usage. The
/// single source for the command palette, the live hints bar, and `Ctrl+B h`.
pub(super) struct CommandSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub usage: &'static str,
}

/// Every `/command`, for completion + hints. Order = display order.
pub(super) const SPECS: &[CommandSpec] = &[
    CommandSpec { name: "fact", summary: "extract last response → Facts (confirm)", usage: "/fact \"…\" [→ path]" },
    CommandSpec { name: "note", summary: "extract → Notes (speculative)", usage: "/note \"…\" [→ path]" },
    CommandSpec { name: "goto", summary: "jump the tree to a node", usage: "/goto facts/path" },
    CommandSpec { name: "diff", summary: "similar facts already in the corpus", usage: "/diff" },
    CommandSpec { name: "verify", summary: "confidence-probe the last response", usage: "/verify" },
    CommandSpec { name: "factcheck", summary: "audit the corpus (truth + consistency)", usage: "/factcheck" },
    CommandSpec { name: "undisputed", summary: "common-sense check of authorial facts", usage: "/undisputed" },
    CommandSpec { name: "synthesize", summary: "grounded, cited synthesis from your facts", usage: "/synthesize <topic>" },
    CommandSpec { name: "outline", summary: "a structured, fact-citing outline", usage: "/outline <topic>" },
    CommandSpec { name: "gaps", summary: "open questions the corpus doesn't answer", usage: "/gaps <topic>" },
    CommandSpec { name: "bibliography", summary: "Sources Research chapter → BibTeX", usage: "/bibliography" },
    CommandSpec { name: "upgrade", summary: "re-ground a model fact on a structured source", usage: "/upgrade [facts/path]" },
    CommandSpec { name: "stale", summary: "list aging model/web facts", usage: "/stale [days]" },
    CommandSpec { name: "deadsources", summary: "check web/document sources for link rot", usage: "/deadsources" },
    CommandSpec { name: "whatswrong", summary: "explain a flagged fact (AI)", usage: "/whatswrong [facts/path]" },
    CommandSpec { name: "sources", summary: "list each fact's provenance", usage: "/sources" },
    CommandSpec { name: "import", summary: "ingest a file / folder / .bib / .json", usage: "/import [path]" },
    CommandSpec { name: "forget", summary: "remove an imported source", usage: "/forget <name>" },
    CommandSpec { name: "web", summary: "web search & fetch", usage: "/web [--ingest|--chat] <query>" },
    CommandSpec { name: "wikidata", summary: "structured triples (Q-ID)", usage: "/wikidata <query>" },
    CommandSpec { name: "geonames", summary: "real-world places (GeoNames)", usage: "/geonames <query>" },
    CommandSpec { name: "gutenberg", summary: "ingest a public-domain book (Project Gutenberg)", usage: "/gutenberg <query>" },
    CommandSpec { name: "openalex", summary: "scholarly paper (DOI)", usage: "/openalex <query>" },
    CommandSpec { name: "arxiv", summary: "arXiv preprint", usage: "/arxiv <query>" },
    CommandSpec { name: "triangulate", summary: "cross-check a claim across sources", usage: "/triangulate [claim]" },
    CommandSpec { name: "calc", summary: "deterministic calc / units / world.get", usage: "/calc <expr>" },
    CommandSpec { name: "world", summary: "your World simulation facts", usage: "/world [layer]" },
    CommandSpec { name: "promote", summary: "turn a Note into a verified Fact", usage: "/promote [note] [→ path]" },
    CommandSpec { name: "chain", summary: "sequential research pipeline", usage: "/chain a → b → c" },
    CommandSpec { name: "rag", summary: "switch RAG mode", usage: "/rag [facts+full|facts|full]" },
    CommandSpec { name: "clear", summary: "clear the chat window", usage: "/clear" },
    CommandSpec { name: "save", summary: "save / rename the thread", usage: "/save [name]" },
];

/// The usage/summary hint for a fully-typed command word, or a list of matches
/// while it's still being typed. `None` when the input isn't a `/command`.
pub(super) fn hint_for(input: &str) -> Option<String> {
    let body = input.trim_start().strip_prefix('/')?;
    let (word, rest) = match body.split_once(char::is_whitespace) {
        Some((w, r)) => (w, Some(r)),
        None => (body, None),
    };
    let word_lc = word.to_ascii_lowercase();
    // A completed command (there's a space after it) → show its usage + summary.
    if rest.is_some() {
        if let Some(s) = SPECS.iter().find(|s| s.name == word_lc) {
            return Some(format!("{}  — {}", s.usage, s.summary));
        }
        return None;
    }
    // Still typing the command word → show matches.
    let matches: Vec<&str> = SPECS.iter().map(|s| s.name).filter(|n| n.starts_with(&word_lc)).collect();
    match matches.len() {
        0 => Some(format!("/{word_lc} — unknown command (Ctrl+B h for the list)")),
        1 => SPECS.iter().find(|s| s.name == matches[0]).map(|s| format!("{}  — {}", s.usage, s.summary)),
        _ => Some(format!("{} commands: {}", matches.len(), matches.join(" · "))),
    }
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
        "undisputed" => Command::Undisputed,
        "synthesize" => Command::Synthesize(rest.to_string()),
        "outline" => Command::Outline(rest.to_string()),
        "gaps" => Command::Gaps(rest.to_string()),
        "bibliography" | "bib" => Command::Bibliography,
        "upgrade" => Command::Upgrade(if rest.is_empty() { None } else { Some(rest.to_string()) }),
        "stale" => Command::Stale(if rest.is_empty() { None } else { Some(rest.to_string()) }),
        "deadsources" | "deadlinks" => Command::DeadSources,
        "sources" => Command::Sources,
        "import" => Command::Import(if rest.is_empty() { None } else { Some(rest.to_string()) }),
        "forget" => Command::Forget(rest.to_string()),
        "web" => {
            // Optional leading --ingest / --chat flag.
            let (ingest, q) = if let Some(r) = rest.strip_prefix("--ingest") {
                (Some(true), r.trim())
            } else if let Some(r) = rest.strip_prefix("--chat") {
                (Some(false), r.trim())
            } else {
                (None, rest)
            };
            Command::Web { ingest, query: q.to_string() }
        }
        "calc" => Command::Calc(rest.to_string()),
        "world" => Command::World(rest.to_string()),
        "wikidata" => Command::Wikidata(rest.to_string()),
        "geonames" | "geo" => Command::Geonames(rest.to_string()),
        "gutenberg" | "pg" => Command::Gutenberg(rest.to_string()),
        "openalex" => Command::OpenAlex(rest.to_string()),
        "arxiv" => Command::Arxiv(rest.to_string()),
        "triangulate" | "tri" => Command::Triangulate(rest.to_string()),
        "whatswrong" => Command::WhatsWrong(if rest.is_empty() { None } else { Some(rest.to_string()) }),
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
        assert_eq!(parse("/undisputed").unwrap(), Command::Undisputed);
        assert_eq!(parse("/synthesize roman roads").unwrap(), Command::Synthesize("roman roads".into()));
        assert_eq!(parse("/outline the siege").unwrap(), Command::Outline("the siege".into()));
        assert_eq!(parse("/gaps the siege").unwrap(), Command::Gaps("the siege".into()));
        assert_eq!(parse("/bibliography").unwrap(), Command::Bibliography);
        assert_eq!(parse("/bib").unwrap(), Command::Bibliography);
        assert_eq!(parse("/upgrade").unwrap(), Command::Upgrade(None));
        assert_eq!(parse("/stale 30").unwrap(), Command::Stale(Some("30".into())));
        assert_eq!(parse("/sources").unwrap(), Command::Sources);
        assert_eq!(parse("/import /docs/rome.md").unwrap(), Command::Import(Some("/docs/rome.md".into())));
        assert_eq!(parse("/import").unwrap(), Command::Import(None));
        assert_eq!(parse("/forget rome").unwrap(), Command::Forget("rome".into()));
        assert_eq!(
            parse("/web roman aqueduct capacity").unwrap(),
            Command::Web { ingest: None, query: "roman aqueduct capacity".into() }
        );
        assert_eq!(
            parse("/web --ingest roman history").unwrap(),
            Command::Web { ingest: Some(true), query: "roman history".into() }
        );
        assert_eq!(
            parse("/web --chat q").unwrap(),
            Command::Web { ingest: Some(false), query: "q".into() }
        );
        assert_eq!(parse("/calc 100 mi2km").unwrap(), Command::Calc("100 mi2km".into()));
        assert_eq!(parse("/world Astronomy").unwrap(), Command::World("Astronomy".into()));
        assert_eq!(parse("/world").unwrap(), Command::World(String::new()));
        assert_eq!(parse("/wikidata Rome").unwrap(), Command::Wikidata("Rome".into()));
        assert_eq!(parse("/geonames Rome").unwrap(), Command::Geonames("Rome".into()));
        assert_eq!(parse("/geo Rome").unwrap(), Command::Geonames("Rome".into()));
        assert_eq!(parse("/gutenberg pride and prejudice").unwrap(), Command::Gutenberg("pride and prejudice".into()));
        assert_eq!(parse("/pg dickens").unwrap(), Command::Gutenberg("dickens".into()));
        assert_eq!(parse("/openalex aqueduct").unwrap(), Command::OpenAlex("aqueduct".into()));
        assert_eq!(parse("/arxiv attention").unwrap(), Command::Arxiv("attention".into()));
        assert_eq!(parse("/triangulate the sky is blue").unwrap(), Command::Triangulate("the sky is blue".into()));
        assert_eq!(parse("/tri x").unwrap(), Command::Triangulate("x".into()));
        assert_eq!(parse("/whatswrong").unwrap(), Command::WhatsWrong(None));
        assert_eq!(
            parse("/whatswrong facts/rome/fall").unwrap(),
            Command::WhatsWrong(Some("facts/rome/fall".into()))
        );
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

    #[test]
    fn specs_all_parse_and_hint() {
        for s in SPECS {
            assert!(
                !matches!(parse(&format!("/{}", s.name)), Some(Command::Unknown(_))),
                "spec `{}` doesn't parse to a real command",
                s.name
            );
        }
        assert!(hint_for("/web ").unwrap().contains("query")); // completed → usage
        assert!(hint_for("/wik").unwrap().to_lowercase().contains("wikidata")); // one match
        assert!(hint_for("/w").unwrap().contains("commands")); // several matches
        assert!(hint_for("plain text").is_none());
    }
}
