//! TERMS-1 — `inkhaven terms …` terminal commands.
//!
//! - `check` — scan prose for banned synonyms of Glossary canonical terms and
//!   report every occurrence with its location. Exits non-zero when any are
//!   found, so it slots into a pre-build CI step. `--book` scopes to one book;
//!   `--json` emits a machine-readable report.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{InsertPosition, NodeKind, Store, SYSTEM_TAG_GLOSSARY};
use crate::tui::concordance::{self, ConcordanceEntry, ParagraphInput};
use crate::tui::style_warnings::BannedSynonymDetector;

use super::TermsCommand;

pub fn run(project: &Path, cmd: TermsCommand) -> Result<()> {
    match cmd {
        TermsCommand::Check { book, json } => check(project, book.as_deref(), json),
        TermsCommand::Suggest { book, provider, max_cost, force, auto_create } => {
            suggest(project, book.as_deref(), provider.as_deref(), max_cost, force, auto_create)
        }
    }
}

fn open(project: &Path) -> Result<(Config, Store, Hierarchy)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    Ok((cfg, store, hierarchy))
}

/// Read a paragraph body from disk (what assembly compiles), stripping a leading
/// `= Title` heading.
fn read_body(store: &Store, node: &Node) -> Option<String> {
    let rel = node.file.as_ref()?;
    let raw = std::fs::read_to_string(store.project_root().join(rel)).ok()?;
    let body = if raw.trim_start().starts_with("= ") {
        raw.splitn(2, '\n').nth(1).unwrap_or("").to_string()
    } else {
        raw
    };
    Some(body)
}

struct TermFinding {
    path: String,
    line: usize,
    synonym: String,
    canonical: String,
}

fn check(project: &Path, book: Option<&str>, json: bool) -> Result<()> {
    let (_cfg, store, h) = open(project)?;

    // Target books: one named book, or every user book.
    let user_books: Vec<&Node> = match book {
        Some(_) => vec![super::resolve_user_book(&h, book, "terms check").map_err(Error::Store)?],
        None => h
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .collect(),
    };

    let mut findings: Vec<TermFinding> = Vec::new();
    let mut paragraphs_scanned = 0usize;
    for book in &user_books {
        // The detector is scoped to this book (global + this book's entries).
        // Suppression set is empty until T-P4 wires the intent ledger.
        let detector =
            BannedSynonymDetector::from_store(&store, &h, Some(&book.slug));
        if detector.is_empty() {
            continue; // no glossary entries apply — nothing to flag in this book
        }
        for id in h.collect_subtree(book.id) {
            let Some(node) = h.get(id) else { continue };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(body) = read_body(&store, node) else { continue };
            paragraphs_scanned += 1;
            let path = h.slug_path(node);
            for (i, line) in body.lines().enumerate() {
                for hit in detector.detect(line) {
                    if let Some((synonym, canonical)) = detector.hint_at(line, hit.col_start) {
                        findings.push(TermFinding {
                            path: path.clone(),
                            line: i + 1,
                            synonym,
                            canonical,
                        });
                    }
                }
            }
        }
    }

    if json {
        emit_json(paragraphs_scanned, &findings);
    } else {
        emit_human(paragraphs_scanned, &findings);
    }

    if findings.is_empty() {
        Ok(())
    } else {
        // Non-zero exit for CI; the report is already printed.
        std::process::exit(1);
    }
}

fn emit_human(scanned: usize, findings: &[TermFinding]) {
    if findings.is_empty() {
        println!("terms check: OK — no banned synonyms in {scanned} paragraph(s).");
        return;
    }
    println!(
        "terms check: {} banned-synonym occurrence(s) in {scanned} paragraph(s):",
        findings.len()
    );
    for f in findings {
        println!(
            "  {} line {}: \"{}\" → use \"{}\"",
            f.path, f.line, f.synonym, f.canonical
        );
    }
    println!("\nUse the canonical form, or declare the variant deliberate in the Glossary.");
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

fn emit_json(scanned: usize, findings: &[TermFinding]) {
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"paragraphs_scanned\": {scanned},\n"));
    s.push_str(&format!("  \"finding_count\": {},\n", findings.len()));
    s.push_str("  \"findings\": [");
    for (i, f) in findings.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "\n    {{ \"path\": {}, \"line\": {}, \"synonym\": {}, \"canonical\": {} }}",
            json_str(&f.path),
            f.line,
            json_str(&f.synonym),
            json_str(&f.canonical),
        ));
    }
    if !findings.is_empty() {
        s.push_str("\n  ");
    }
    s.push_str("]\n}");
    println!("{s}");
}

// ──────────────────────────────────────────────────────────────────────────
// suggest (LLM-assisted canonicalisation)
// ──────────────────────────────────────────────────────────────────────────

const SUGGEST_SYSTEM: &str = "\
You are a terminology editor for a long document. You are given clusters of words \
that appear in MULTIPLE surface forms in the text. Propose a canonical Glossary \
entry only for clusters that represent genuine TERMINOLOGY DRIFT — the same \
concept written inconsistently (e.g. \"frontend\" vs \"front end\"). \
SKIP clusters that are mere grammatical inflection (plural / tense), pronouns, \
proper names, or ordinary stylistic variation. For each real cluster, output ONE \
HJSON block. Use MULTI-LINE HJSON — one field per line, one synonym per line \
(unquoted values run to end of line, so never put two fields on one line):\n\
{\n  term: <the form to standardise on>\n  definition: <one short line>\n  synonyms: [\n    <other form>\n  ]\n}\n\
Output only the HJSON blocks (separated by a blank line), or the single word NONE \
if no cluster is genuine terminology drift. Do not explain.";

fn suggest(
    project: &Path,
    book: Option<&str>,
    provider: Option<&str>,
    max_cost: usize,
    force: bool,
    auto_create: bool,
) -> Result<()> {
    let (cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book, "terms suggest").map_err(Error::Store)?;

    // Collect the book's prose as concordance input.
    let mut bodies: Vec<(String, Vec<String>)> = Vec::new();
    for id in h.collect_subtree(book.id) {
        let Some(node) = h.get(id) else { continue };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(body) = read_body(&store, node) else { continue };
        bodies.push((h.slug_path(node), body.lines().map(String::from).collect()));
    }
    if bodies.is_empty() {
        println!("terms suggest: no prose in `{}`.", book.title);
        return Ok(());
    }
    let inputs: Vec<ParagraphInput<'_>> = bodies
        .iter()
        .map(|(slug, lines)| ParagraphInput { slug_path: slug.clone(), lines })
        .collect();
    let data = concordance::build(
        &cfg.editor.style_warnings.repeated_phrases,
        &cfg.language,
        &inputs,
    );
    let clusters: Vec<&ConcordanceEntry> =
        data.entries.iter().filter(|e| e.variants.len() > 1).collect();
    if clusters.is_empty() {
        println!(
            "terms suggest: no multi-form term clusters in `{}` ({} paragraphs).",
            book.title, data.paragraphs_scanned
        );
        return Ok(());
    }

    let prompt = build_suggest_prompt(&clusters);
    let raw = terms_llm_call(&cfg, provider, SUGGEST_SYSTEM, prompt, max_cost, force)?;

    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("none") || trimmed.is_empty() {
        println!("terms suggest: the model found no genuine terminology drift.");
        return Ok(());
    }
    println!("{trimmed}");

    if auto_create {
        let created = create_glossary_drafts(&store, &cfg, &h, &raw)?;
        println!(
            "\nterms suggest: created {created} draft entry(ies) in the Glossary book."
        );
    } else {
        println!(
            "\nPaste the entries you want into the Glossary book (or re-run with --auto-create)."
        );
    }
    Ok(())
}

/// The user prompt — one line per cluster with its surface forms + count.
fn build_suggest_prompt(clusters: &[&ConcordanceEntry]) -> String {
    let mut s = String::from(
        "Clusters of words sharing a stem but appearing in multiple surface forms:\n\n",
    );
    for e in clusters {
        s.push_str(&format!(
            "- \"{}\" ({} occurrences): {}\n",
            e.headword,
            e.count,
            e.variants.join(", ")
        ));
    }
    s.push_str("\nFor each cluster that is genuine terminology drift, output one HJSON entry.");
    s
}

/// Pull balanced `{ … }` HJSON blocks out of an LLM response.
fn extract_hjson_blocks(raw: &str) -> Vec<String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            let mut depth = 0usize;
            let mut j = i;
            while j < chars.len() {
                match chars[j] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            if depth == 0 && j < chars.len() {
                blocks.push(chars[i..=j].iter().collect());
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    blocks
}

/// Materialise valid proposed entries as draft paragraphs under the Glossary book.
fn create_glossary_drafts(
    store: &Store,
    cfg: &Config,
    h: &Hierarchy,
    raw: &str,
) -> Result<usize> {
    let Some(glossary) = h.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_GLOSSARY)
    }) else {
        return Err(Error::Store(
            "Glossary system book missing — re-open the project to seed it".into(),
        ));
    };
    let mut created = 0usize;
    for block in extract_hjson_blocks(raw) {
        let Some(entry) = crate::glossary::GlossaryEntry::from_hjson(&block) else { continue };
        if !entry.is_valid() {
            continue;
        }
        let hier = Hierarchy::load(store)?;
        let mut node = store.create_node(
            cfg,
            &hier,
            NodeKind::Paragraph,
            entry.term.trim(),
            Some(glossary),
            None,
            InsertPosition::End,
        )?;
        node.content_type = Some("hjson".to_string());
        let body = block.as_bytes();
        if let Some(rel) = &node.file {
            let abs = store.project_root().join(rel);
            let _ = crate::io_atomic::write(&abs, body);
        }
        store.update_paragraph_content(&mut node, body)?;
        created += 1;
    }
    Ok(created)
}

/// The LLM call for `terms suggest`: provider resolution + a soft-cap preflight
/// (informative; `--force` overrides) + retry-on-transient. Returns the raw
/// response.
fn terms_llm_call(
    cfg: &Config,
    provider: Option<&str>,
    system: &str,
    prompt: String,
    max_cost: usize,
    force: bool,
) -> Result<String> {
    use crate::world::fact_check_slow::{
        backoff_delay, is_transient, slow_preflight, PreflightVerdict,
    };
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| Error::Config(format!("no LLM provider for terms suggest: {e}")))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, provider)
        .map_err(|e| Error::Config(format!("resolving provider: {e}")))?;

    let soft = if force { 0 } else { max_cost };
    // No per-feature daily cap (this is a manual, infrequent command) — gate on
    // the per-call soft cap only.
    let (pf, verdict) = slow_preflight(system, &prompt, 0, i64::MAX, soft);
    if let PreflightVerdict::OverSoftCap { est_total_tokens, soft_cap } = verdict {
        return Err(Error::Config(format!(
            "terms suggest skipped: estimated ~{est_total_tokens} tokens exceeds soft cap \
             {soft_cap} — re-run with --force or raise --max-cost"
        )));
    }
    eprintln!("terms suggest · model: {model} · ~{} tokens · proposing…", pf.est_total_tokens);

    const MAX_ATTEMPTS: u32 = 3;
    let mut last_err = String::new();
    for attempt in 0..MAX_ATTEMPTS {
        match crate::ai::stream::collect_blocking(
            ai.client.clone(),
            model.to_string(),
            Some(system.to_string()),
            prompt.clone(),
        ) {
            Ok(raw) => return Ok(raw),
            Err(e) => {
                last_err = e.to_string();
                if attempt + 1 < MAX_ATTEMPTS && is_transient(&last_err) {
                    std::thread::sleep(backoff_delay(attempt));
                    continue;
                }
                break;
            }
        }
    }
    Err(Error::Config(format!("terms suggest LLM call failed: {last_err}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_hjson_blocks_pulls_balanced_braces() {
        // Multi-line HJSON (the format the prompt requests) — one field per line.
        let raw = "Here are two:\n\
                   {\n  term: frontend\n  synonyms: [\n    front end\n  ]\n}\n\n\
                   {\n  term: backend\n  definition: x\n  synonyms: [\n    back end\n  ]\n}\nDone.";
        let blocks = extract_hjson_blocks(raw);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("frontend"));
        assert!(blocks[1].contains("backend"));
        // Each block round-trips into a GlossaryEntry (term parses cleanly).
        let e = crate::glossary::GlossaryEntry::from_hjson(&blocks[0]).unwrap();
        assert_eq!(e.term, "frontend");
        let syn: Vec<String> = e.banned_synonyms().collect();
        assert_eq!(syn, vec!["front end"]);
    }

    #[test]
    fn extract_hjson_blocks_handles_none_and_empty() {
        assert!(extract_hjson_blocks("NONE").is_empty());
        assert!(extract_hjson_blocks("").is_empty());
        // An unbalanced brace is not a block.
        assert!(extract_hjson_blocks("{ term: x").is_empty());
    }
}
