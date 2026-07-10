//! ARG-1 — `inkhaven argue`. Per chapter, asks the model for the central claims and
//! their support (an argument outline), then flags the two cheapest structural gaps —
//! an unsupported central claim and an orphan citation. Reuses the NF-CITE / facts-scan
//! LLM pipeline; the pure parser + quote-guard live in `crate::argue`.

use std::collections::BTreeMap;
use std::path::Path;

use crate::argue::{parse_argument, ArgClaim, OrphanCite};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::Store;

const ARGUMENT_SYSTEM: &str = "You are an argument analyst for a nonfiction manuscript. You are \
given a CHAPTER and the CITATIONS it uses. Identify the chapter's CENTRAL CLAIMS — the \
load-bearing assertions it argues for, NOT every sentence; a handful at most. For each claim, \
state its SUPPORT as it appears in the text: a cited @key, a short reasoning phrase, or NONE. \
Then list any CITATION that supports none of the claims. Quote each claim BRIEFLY and VERBATIM \
from the chapter — do not paraphrase or invent. Output ONLY lines of these two forms, nothing \
else:\n\
  CLAIM ||| <claim quoted from the chapter> ||| <@key or a short reason or NONE>\n\
  ORPHAN ||| <@key> ||| <cited but supports no claim>\n\
If the chapter advances no real argument, output exactly: NONE";

struct ChapterArg {
    chapter: String,
    claims: Vec<ArgClaim>,
    orphans: Vec<OrphanCite>,
}

pub fn run(project: &Path, book_name: Option<&str>, provider: Option<&str>, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let sources = source_labels(&store, &h);

    let ai = crate::ai::AiClient::from_config(&cfg.llm).map_err(|e| Error::Store(format!("{e:#}")))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, provider)
        .map_err(|e| Error::Store(format!("{e:#}")))?;

    let books: Vec<&Node> = match book_name {
        Some(_) => vec![super::resolve_user_book(&h, book_name, "argue").map_err(Error::Store)?],
        None => h
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .collect(),
    };

    let mut report: Vec<ChapterArg> = Vec::new();
    for book in &books {
        for chapter in h.children_of(Some(book.id)).into_iter().filter(|n| n.kind == NodeKind::Chapter) {
            let raw_prose = crate::cli::book_walk::chapter_raw_prose(&layout, &h, chapter.id);
            let plain = crate::audiobook::typst_to_plain(&raw_prose);
            if plain.trim().is_empty() {
                continue;
            }
            let cited = crate::sources::extract_cite_keys(&raw_prose);
            let cites_block = cited
                .iter()
                .map(|k| {
                    let label = sources.get(k).map(String::as_str).unwrap_or("(not in Sources)");
                    format!("@{k} — {label}")
                })
                .collect::<Vec<_>>()
                .join("\n");
            let prompt = format!(
                "CHAPTER \"{}\":\n{plain}\n\nCITATIONS USED IN THIS CHAPTER:\n{}\n\n\
                 List the central claims with their support, and any orphan citations, \
                 in the required format.",
                chapter.title,
                if cites_block.is_empty() { "(none)".into() } else { cites_block }
            );
            let out = crate::cli::facts_scan::run_blocking(&ai, model, ARGUMENT_SYSTEM, &prompt)
                .map_err(|e| Error::Store(format!("{e:#}")))?;
            let (claims, orphans) = parse_argument(&out, &plain);
            let gaps = claims.iter().filter(|c| !c.supported).count() + orphans.len();
            eprintln!("argue · {} → {} claim(s), {gaps} gap(s)", chapter.title, claims.len());
            report.push(ChapterArg { chapter: chapter.title.clone(), claims, orphans });
        }
    }

    let gap_count: usize = report
        .iter()
        .map(|c| c.claims.iter().filter(|x| !x.supported).count() + c.orphans.len())
        .sum();

    if json {
        emit_json(&report, gap_count);
    } else {
        render_text(&report, gap_count);
    }

    if gap_count == 0 {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

/// key → "Author (Year) — Title" for every entry in the Sources book.
fn source_labels(store: &Store, h: &Hierarchy) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(sources) = h.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_SOURCES)
    }) else {
        return out;
    };
    for id in h.collect_subtree(sources.id) {
        let Some(node) = h.get(id) else { continue };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(id) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let body = if text.trim_start().starts_with("= ") {
            text.splitn(2, '\n').nth(1).unwrap_or("")
        } else {
            &text
        };
        if let Some(e) = crate::sources::BibEntry::from_hjson(body) {
            if e.is_valid() {
                let who = if e.author.is_empty() { "?".into() } else { e.author.clone() };
                let year = if e.year.is_empty() { "n.d.".into() } else { e.year.clone() };
                out.insert(e.key, format!("{who} ({year}) — {}", e.title));
            }
        }
    }
    out
}

fn render_text(report: &[ChapterArg], gap_count: usize) {
    for c in report {
        if c.claims.is_empty() && c.orphans.is_empty() {
            continue;
        }
        println!("\nargue — chapter \u{201C}{}\u{201D}", c.chapter);
        for claim in &c.claims {
            let support = if claim.supported {
                format!("\u{2190} {}", claim.support)
            } else {
                "(no support)   \u{26A0} unsupported".into()
            };
            println!("  \u{2022} {}   {support}", claim.claim);
        }
        for o in &c.orphans {
            println!("  orphan citation: @{} \u{2014} {}", o.key, o.detail);
        }
    }
    if gap_count == 0 {
        println!("\nargue: every central claim is supported; no orphan citations.");
    } else {
        println!("\nargue: {gap_count} gap(s) — unsupported claims or orphan citations.");
    }
}

fn emit_json(report: &[ChapterArg], gap_count: usize) {
    let chapters: Vec<_> = report
        .iter()
        .map(|c| {
            serde_json::json!({
                "chapter": c.chapter,
                "claims": c.claims.iter().map(|x| serde_json::json!({
                    "claim": x.claim, "support": x.support, "supported": x.supported,
                })).collect::<Vec<_>>(),
                "orphan_citations": c.orphans.iter().map(|o| serde_json::json!({
                    "key": o.key, "detail": o.detail,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    println!(
        "{}",
        serde_json::json!({ "chapters": chapters, "gaps": gap_count })
    );
}
