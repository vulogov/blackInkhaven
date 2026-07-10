//! SOURCES-1 — `inkhaven sources …` terminal commands.
//!
//! - `check`  — validate that every `@key` cited in prose is defined in the
//!   Sources book (honouring `sources.all` scoping). Exits non-zero when any
//!   key is undefined, so it slots into a pre-build CI step.
//! - `list`   — list defined citation entries.
//! - `import` — read a `.bib` file and materialise its entries as HJSON
//!   paragraphs under the Sources chapter for a book.

use std::collections::BTreeSet;
use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::sources::{self, BibEntry};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{InsertPosition, NodeKind, Store, SYSTEM_TAG_SOURCES};

use super::SourcesCommand;

pub fn run(project: &Path, cmd: SourcesCommand) -> Result<()> {
    match cmd {
        SourcesCommand::Check { book_name, json } => check(project, book_name.as_deref(), json),
        SourcesCommand::Coverage { book_name, json, ai, provider } => {
            if ai {
                coverage_ai(project, book_name.as_deref(), provider.as_deref(), json)
            } else {
                coverage(project, book_name.as_deref(), json)
            }
        }
        SourcesCommand::List { book_name, json } => list(project, book_name.as_deref(), json),
        SourcesCommand::Import { file, book_name } => {
            import(project, &file, book_name.as_deref())
        }
        SourcesCommand::Export { format, book_name, out } => {
            export(project, &format, book_name.as_deref(), out.as_deref())
        }
    }
}

/// SOURCES-2 — export the Sources book to `bibtex` or `csl-json`.
fn export(project: &Path, format: &str, book_name: Option<&str>, out: Option<&Path>) -> Result<()> {
    let (_cfg, store, h) = open(project)?;
    let sources = sources_book(&h)?;
    let mut entries = collect_entries(&store, &h, sources);
    if let Some(name) = book_name {
        let needle = name.trim();
        entries.retain(|(chapter, _)| chapter == needle);
    }
    entries.sort_by(|a, b| a.1.key.to_lowercase().cmp(&b.1.key.to_lowercase()));
    let bibs: Vec<BibEntry> = entries.into_iter().map(|(_, e)| e).collect();

    let (text, count) = match format.to_ascii_lowercase().as_str() {
        "csl-json" | "csl" | "json" => sources::compile_csl_json(&bibs),
        "bibtex" | "bib" => sources::compile_bibtex(&bibs),
        other => {
            return Err(Error::Config(format!(
                "sources export: unknown format `{other}` (use `bibtex` or `csl-json`)"
            )))
        }
    };
    match out {
        Some(p) => {
            std::fs::write(p, text.as_bytes()).map_err(Error::Io)?;
            println!("sources export: {count} entry(ies) → {}", p.display());
        }
        None => println!("{text}"),
    }
    Ok(())
}

/// Open the project + load the hierarchy. Shared boilerplate.
fn open(project: &Path) -> Result<(Config, Store, Hierarchy)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    Ok((cfg, store, hierarchy))
}

/// The Sources system book, or a clear error if it's somehow missing.
fn sources_book<'a>(h: &'a Hierarchy) -> Result<&'a Node> {
    h.iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_SOURCES))
        .ok_or_else(|| {
            Error::Store("Sources system book missing — re-open the project to seed it".into())
        })
}

/// Walk up from `node` to the chapter that sits directly under the Sources
/// book; returns its title (the scope key).
fn enclosing_sources_chapter<'a>(
    h: &'a Hierarchy,
    sources_id: uuid::Uuid,
    node: &'a Node,
) -> Option<&'a str> {
    let mut cur: Option<&Node> = Some(node);
    while let Some(n) = cur {
        if n.parent_id == Some(sources_id) {
            return Some(n.title.as_str());
        }
        cur = n.parent_id.and_then(|pid| h.get(pid));
    }
    None
}

/// Read a paragraph body from disk (the file `inkhaven build` reads), stripping
/// a leading `= Title` heading. Reading from disk — not bdslib — keeps
/// `sources check` honest about what assembly will actually compile.
fn read_body(store: &Store, node: &Node) -> Option<String> {
    let rel = node.file.as_ref()?;
    let raw = std::fs::read_to_string(store.project_root().join(rel)).ok()?;
    Some(strip_heading(&raw))
}

/// Collect every defined citation entry under the Sources book, paired with its
/// enclosing chapter title (for scope filtering).
fn collect_entries(store: &Store, h: &Hierarchy, sources: &Node) -> Vec<(String, BibEntry)> {
    let mut out = Vec::new();
    for id in h.collect_subtree(sources.id) {
        if id == sources.id {
            continue;
        }
        let Some(node) = h.get(id) else { continue };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(body) = read_body(store, node) else { continue };
        if let Some(e) = BibEntry::from_hjson(&body) {
            if e.is_valid() {
                let chapter = enclosing_sources_chapter(h, sources.id, node)
                    .unwrap_or("")
                    .to_string();
                out.push((chapter, e));
            }
        }
    }
    out
}

fn strip_heading(body: &str) -> String {
    let mut lines = body.lines().peekable();
    if lines.peek().is_some_and(|l| l.trim_start().starts_with("= ")) {
        lines.next();
        if lines.peek().is_some_and(|l| l.trim().is_empty()) {
            lines.next();
        }
    }
    lines.collect::<Vec<_>>().join("\n")
}

// ──────────────────────────────────────────────────────────────────────────
// check
// ──────────────────────────────────────────────────────────────────────────

fn check(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let (cfg, store, h) = open(project)?;
    let sources = sources_book(&h)?;
    let all_entries = collect_entries(&store, &h, sources);

    // Which user books to check.
    let user_books: Vec<&Node> = match book_name {
        Some(_) => vec![
            super::resolve_user_book(&h, book_name, "sources check").map_err(Error::Store)?,
        ],
        None => h
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .collect(),
    };

    let mut missing: Vec<MissingCite> = Vec::new();
    let mut total_cites = 0usize;
    for book in &user_books {
        // Keys in scope for THIS book = all entries when sources.all, else only
        // the entries whose chapter is named after the book.
        let defined: BTreeSet<&str> = all_entries
            .iter()
            .filter(|(chapter, _)| cfg.sources.all || chapter == &book.title)
            .map(|(_, e)| e.key.as_str())
            .collect();

        for id in h.collect_subtree(book.id) {
            let Some(node) = h.get(id) else { continue };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(body) = read_body(&store, node) else { continue };
            for key in sources::extract_cite_keys(&body) {
                total_cites += 1;
                if !defined.contains(key.as_str()) {
                    missing.push(MissingCite {
                        book: book.title.clone(),
                        paragraph: node.title.clone(),
                        key,
                    });
                }
            }
        }
    }

    if json {
        emit_check_json(total_cites, &missing);
    } else {
        emit_check_human(total_cites, &missing, cfg.sources.all);
    }

    if missing.is_empty() {
        Ok(())
    } else {
        // Non-zero exit for CI; the report has already been printed.
        std::process::exit(1);
    }
}

struct MissingCite {
    book: String,
    paragraph: String,
    key: String,
}

/// NF-CITE — the Sourcing pass: flag uncited factual claims across the manuscript.
fn coverage(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let (_cfg, store, h) = open(project)?;

    let user_books: Vec<&Node> = match book_name {
        Some(_) => vec![
            super::resolve_user_book(&h, book_name, "sources coverage").map_err(Error::Store)?,
        ],
        None => h
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .collect(),
    };

    let mut findings: Vec<CoverageFinding> = Vec::new();
    for book in &user_books {
        for id in h.collect_subtree(book.id) {
            let Some(node) = h.get(id) else { continue };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            // The author's "this section is common knowledge" override.
            if node.tags.iter().any(|t| t == "no-cite") {
                continue;
            }
            let Some(body) = read_body(&store, node) else { continue };
            for claim in crate::sources::coverage::scan(&body) {
                findings.push(CoverageFinding {
                    book: book.title.clone(),
                    loc: h.slug_path(node),
                    sentence: claim.sentence,
                    signals: claim.signals.join(", "),
                });
            }
        }
    }

    if json {
        let arr: Vec<_> = findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "book": f.book, "location": f.loc,
                    "sentence": f.sentence, "signals": f.signals,
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "uncited_claims": arr, "count": findings.len() }));
    } else if findings.is_empty() {
        println!("sources coverage: every checkable claim carries a citation.");
    } else {
        let mut last = String::new();
        for f in &findings {
            let head = format!("{} · {}", f.book, f.loc);
            if head != last {
                println!("\n{head}");
                last = head;
            }
            println!("  \u{201C}{}\u{201D}   [{}]", clip_sentence(&f.sentence, 100), f.signals);
        }
        println!(
            "\nsources coverage: {} uncited claim(s). Cite them, or tag a paragraph `no-cite`.",
            findings.len()
        );
    }

    if findings.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

struct CoverageFinding {
    book: String,
    loc: String,
    sentence: String,
    signals: String,
}

fn clip_sentence(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}\u{2026}", s.chars().take(n).collect::<String>())
    }
}

const SOURCING_SYSTEM: &str = "You are a meticulous fact-checking editor for a nonfiction \
manuscript. You are given a PASSAGE and a list of the author's RESEARCHED FACTS. Find \
sentences in the PASSAGE that make a CHECKABLE FACTUAL CLAIM — a statistic, a specific date \
or figure, an attributed finding, a historical or scientific assertion — and that do NOT \
already contain a citation token (an @-word such as @smith2020). For each such sentence decide \
whether one of the RESEARCHED FACTS supports it. Output ONE line per claim, exactly:\n\
  <sentence> ||| SUPPORTED ||| <the supporting fact>\n\
or\n\
  <sentence> ||| UNSUPPORTED |||\n\
Quote the sentence briefly (you may truncate with …). Do NOT invent facts, and do NOT flag \
opinions, definitions, or common knowledge. If there are no such claims, output exactly: NONE";

struct AiClaim {
    book: String,
    chapter: String,
    sentence: String,
    supported: bool,
    fact: String,
}

fn build_sourcing_prompt(chapter: &str, prose: &str, facts: &[String]) -> String {
    let facts_block = if facts.is_empty() {
        "(none provided)".to_string()
    } else {
        facts.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n")
    };
    format!(
        "PASSAGE (chapter \"{chapter}\"):\n{prose}\n\nRESEARCHED FACTS:\n{facts_block}\n\n\
         List the uncited checkable claims, one per line, in the required format."
    )
}

fn parse_sourcing(raw: &str, book: &str, chapter: &str) -> Vec<AiClaim> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim().trim_start_matches(['-', '*', ' ']);
        if line.is_empty() || line.eq_ignore_ascii_case("none") {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, "|||").map(str::trim).collect();
        if parts.len() < 2 {
            continue;
        }
        let sentence = parts[0].trim_matches('"').trim().to_string();
        if sentence.is_empty() {
            continue;
        }
        let verdict = parts[1].to_lowercase();
        let supported = verdict.contains("support") && !verdict.contains("unsupport");
        out.push(AiClaim {
            book: book.to_string(),
            chapter: chapter.to_string(),
            sentence,
            supported,
            fact: parts.get(2).map(|s| s.trim().to_string()).unwrap_or_default(),
        });
    }
    out
}

/// NF-CITE AI track — catch subtler uncited claims and check each against the Facts book.
fn coverage_ai(project: &Path, book_name: Option<&str>, provider: Option<&str>, json: bool) -> Result<()> {
    use std::collections::HashSet;
    use uuid::Uuid;

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    // Facts book is optional — support-checking degrades gracefully to claim-finding.
    let facts_ids: HashSet<Uuid> = h
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_FACTS)
        })
        .map(|n| h.collect_subtree(n.id).into_iter().collect())
        .unwrap_or_default();

    let ai = crate::ai::AiClient::from_config(&cfg.llm).map_err(|e| Error::Store(format!("{e:#}")))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, provider)
        .map_err(|e| Error::Store(format!("{e:#}")))?;

    let user_books: Vec<&Node> = match book_name {
        Some(_) => vec![
            super::resolve_user_book(&h, book_name, "sources coverage").map_err(Error::Store)?,
        ],
        None => h
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .collect(),
    };

    let mut findings: Vec<AiClaim> = Vec::new();
    for book in &user_books {
        for chapter in h.children_of(Some(book.id)).into_iter().filter(|n| n.kind == NodeKind::Chapter) {
            let prose = crate::cli::book_walk::chapter_raw_prose(&layout, &h, chapter.id);
            let plain = crate::audiobook::typst_to_plain(&prose);
            if plain.trim().is_empty() {
                continue;
            }
            let facts = crate::cli::facts_scan::relevant_facts(&store, &h, &facts_ids, &plain, 12);
            let prompt = build_sourcing_prompt(&chapter.title, &plain, &facts);
            let raw = crate::cli::facts_scan::run_blocking(&ai, model, SOURCING_SYSTEM, &prompt)
                .map_err(|e| Error::Store(format!("{e:#}")))?;
            let claims = parse_sourcing(&raw, &book.title, &chapter.title);
            eprintln!("cite coverage · {} → {} claim(s)", chapter.title, claims.len());
            findings.extend(claims);
        }
    }

    let unsupported = findings.iter().filter(|f| !f.supported).count();
    let backed = findings.len() - unsupported;

    if json {
        let arr: Vec<_> = findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "book": f.book, "chapter": f.chapter, "sentence": f.sentence,
                    "supported": f.supported, "supporting_fact": f.fact,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({ "claims": arr, "count": findings.len(), "unsupported": unsupported, "backed_by_facts": backed })
        );
    } else if findings.is_empty() {
        println!("sources coverage (AI): every checkable claim carries a citation.");
    } else {
        let unsup: Vec<&AiClaim> = findings.iter().filter(|f| !f.supported).collect();
        let sup: Vec<&AiClaim> = findings.iter().filter(|f| f.supported).collect();
        if !unsup.is_empty() {
            println!("\nUncited — your Facts don't cover these (research or cite):");
            for f in &unsup {
                println!("  {} · \u{201C}{}\u{201D}", f.chapter, clip_sentence(&f.sentence, 90));
            }
        }
        if !sup.is_empty() {
            println!("\nUncited — but your Facts support these (add the citation):");
            for f in &sup {
                println!("  {} · \u{201C}{}\u{201D}", f.chapter, clip_sentence(&f.sentence, 80));
                if !f.fact.is_empty() {
                    println!("      \u{2190} {}", clip_sentence(&f.fact, 90));
                }
            }
        }
        println!("\nsources coverage: {} uncited claim(s) — {unsupported} unsupported, {backed} backed by your Facts", findings.len());
    }

    if findings.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn emit_check_human(total: usize, missing: &[MissingCite], all_scope: bool) {
    let scope = if all_scope { "all entries" } else { "per-book chapters" };
    if missing.is_empty() {
        println!("sources check: OK — {total} citation(s), all keys defined (scope: {scope}).");
        return;
    }
    println!(
        "sources check: {} undefined citation key(s) of {total} (scope: {scope}):",
        missing.len()
    );
    for m in missing {
        println!("  @{}  — {} › {}", m.key, m.book, m.paragraph);
    }
    println!("\nDefine them in the Sources book (or fix the @key spelling).");
}

fn emit_check_json(total: usize, missing: &[MissingCite]) {
    // Hand-rolled JSON — no serde_json dependency on this path.
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"total_citations\": {total},\n"));
    s.push_str(&format!("  \"missing_count\": {},\n", missing.len()));
    s.push_str("  \"missing\": [");
    for (i, m) in missing.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "\n    {{ \"key\": {}, \"book\": {}, \"paragraph\": {} }}",
            json_str(&m.key),
            json_str(&m.book),
            json_str(&m.paragraph)
        ));
    }
    if !missing.is_empty() {
        s.push('\n');
        s.push_str("  ");
    }
    s.push_str("]\n}");
    println!("{s}");
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' | '\t' => out.push(' '),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

// ──────────────────────────────────────────────────────────────────────────
// list
// ──────────────────────────────────────────────────────────────────────────

fn list(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let (_cfg, store, h) = open(project)?;
    let sources = sources_book(&h)?;
    let mut entries = collect_entries(&store, &h, sources);
    if let Some(name) = book_name {
        // Filter to the chapter named after the book (or the raw name).
        let needle = name.trim();
        entries.retain(|(chapter, _)| chapter == needle);
    }
    entries.sort_by(|a, b| a.1.key.to_lowercase().cmp(&b.1.key.to_lowercase()));

    if json {
        let mut s = String::from("[");
        for (i, (chapter, e)) in entries.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                "\n  {{ \"key\": {}, \"type\": {}, \"author\": {}, \"title\": {}, \"year\": {}, \"chapter\": {} }}",
                json_str(&e.key),
                json_str(&e.entry_type),
                json_str(&e.author),
                json_str(&e.title),
                json_str(&e.year),
                json_str(chapter),
            ));
        }
        if !entries.is_empty() {
            s.push('\n');
        }
        s.push(']');
        println!("{s}");
        return Ok(());
    }

    if entries.is_empty() {
        println!("No citation entries defined in the Sources book.");
        return Ok(());
    }
    println!("{} citation entry(ies):", entries.len());
    for (chapter, e) in &entries {
        let year = if e.year.is_empty() { "----" } else { e.year.as_str() };
        let who = if e.author.is_empty() { "(no author)" } else { e.author.as_str() };
        println!("  @{:<22} {year}  {who} — {}  [{}]", e.key, e.title, chapter);
    }
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────
// import
// ──────────────────────────────────────────────────────────────────────────

fn import(project: &Path, file: &Path, book_name: Option<&str>) -> Result<()> {
    let (cfg, store, h) = open(project)?;
    let sources = sources_book(&h)?;
    let book =
        super::resolve_user_book(&h, book_name, "sources import").map_err(Error::Store)?;
    let book_title = book.title.clone();

    let raw = std::fs::read_to_string(file).map_err(Error::Io)?;
    let parsed = sources::parse_bibtex(&raw);
    if parsed.is_empty() {
        return Err(Error::Store(format!(
            "sources import: no BibTeX entries found in {}",
            file.display()
        )));
    }

    // Find (or create) the Sources chapter named after the target book.
    let chapter = match h.iter().find(|n| {
        n.kind == NodeKind::Chapter
            && n.parent_id == Some(sources.id)
            && n.title == book_title
    }) {
        Some(c) => c.clone(),
        None => store.create_node(
            &cfg,
            &h,
            NodeKind::Chapter,
            &book_title,
            Some(sources),
            None,
            InsertPosition::End,
        )?,
    };

    // Existing keys under this chapter — skip duplicates rather than clobber.
    let existing: BTreeSet<String> = collect_entries(&store, &h, sources)
        .into_iter()
        .filter(|(c, _)| c == &book_title)
        .map(|(_, e)| e.key.to_lowercase())
        .collect();

    let mut created = 0usize;
    let mut skipped = 0usize;
    for entry in &parsed {
        if !entry.is_valid() {
            continue;
        }
        if existing.contains(&entry.key.to_lowercase()) {
            skipped += 1;
            continue;
        }
        // Reload so each create sees the prior sibling.
        let hier = Hierarchy::load(&store)?;
        let mut node = store.create_node(
            &cfg,
            &hier,
            NodeKind::Paragraph,
            &entry.key,
            Some(&chapter),
            None,
            InsertPosition::End,
        )?;
        node.content_type = Some("hjson".to_string());
        let body = entry.to_hjson();
        if let Some(rel) = &node.file {
            let abs = store.project_root().join(rel);
            // This is the authoritative on-disk copy that `sources check/list/
            // export` and `inkhaven build` read — a swallowed failure here left
            // the citation silently absent while import reported success.
            crate::io_atomic::write(&abs, body.as_bytes()).map_err(Error::Io)?;
        }
        store.update_paragraph_content(&mut node, body.as_bytes())?;
        created += 1;
    }

    println!(
        "sources import: {created} entry(ies) imported into Sources › {book_title}{}.",
        if skipped > 0 {
            format!(" ({skipped} duplicate key(s) skipped)")
        } else {
            String::new()
        }
    );
    Ok(())
}

#[cfg(test)]
mod coverage_tests {
    use super::{parse_sourcing};

    #[test]
    fn parse_sourcing_reads_verdicts_and_facts() {
        let raw = "preamble without pipes to ignore\n\
                   \"Deaths fell by 42%.\" ||| UNSUPPORTED |||\n\
                   - \"Violence has declined.\" ||| SUPPORTED ||| Pinker (2011): homicide fell\n\
                   malformed line\nNONE";
        let claims = parse_sourcing(raw, "Bk", "Ch1");
        assert_eq!(claims.len(), 2, "got {}", claims.len());
        assert!(!claims[0].supported);
        assert_eq!(claims[0].sentence, "Deaths fell by 42%.");
        assert!(claims[1].supported);
        assert_eq!(claims[1].fact, "Pinker (2011): homicide fell");
    }
}
