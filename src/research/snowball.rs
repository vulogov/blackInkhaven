//! RESRCH-6 (snowball) — citation-following. Given a seed paper (a title, DOI, or
//! topic), it resolves the seed on OpenAlex and follows its citations both ways:
//! **backward** to the works it references, and **forward** to the works that
//! cite it — the seed's citation neighborhood. The neighbourhood is reported (not
//! auto-ingested), each paper with its OpenAlex id, so the author chooses which to
//! bring in as a Source (`research --openalex "<id>"`) — keeping the research
//! corpus curated rather than flooded, the same discipline the agentic loop uses.
//!
//! Built on `scholarly.rs` (OpenAlex, keyless). Gated by `research.scholarly`.

use anyhow::{Result, anyhow};

use crate::config::Config;

use super::scholarly::{self, Paper};

/// How many papers to follow per direction (backward references / forward citers).
const LIMIT: usize = 10;

/// Run a snowball pass from `seed_query`. `out` is the report path (stdout when
/// `None`).
pub(crate) fn run(cfg: &Config, seed_query: &str, out: Option<&str>) -> Result<()> {
    if !scholarly::available(&cfg.research.scholarly) {
        return Err(anyhow!(
            "scholarly sources are disabled — set `research.scholarly.enabled: true` in inkhaven.hjson"
        ));
    }
    let seed_query = seed_query.trim();
    if seed_query.is_empty() {
        return Err(anyhow!("give a seed paper to snowball from (a title, DOI, or topic)"));
    }
    let sc = cfg.research.scholarly.clone();

    eprintln!("⟳ snowball: resolving seed \"{seed_query}\"…");
    let seed = block_on_async(scholarly::openalex(sc.clone(), seed_query.to_string()))
        .map_err(|e| anyhow!("could not resolve a seed paper for `{seed_query}`: {e}"))?;
    eprintln!("· seed: {} ({})", seed.title, seed.id);

    // Backward (works the seed references) + forward (works that cite the seed).
    let backward =
        block_on_async(scholarly::works_by_ids(sc.clone(), seed.referenced_works.clone(), LIMIT))
            .unwrap_or_default();
    let forward =
        block_on_async(scholarly::cited_by(sc.clone(), seed.id.clone(), LIMIT)).unwrap_or_default();

    let report = render(&seed, &backward, &forward);
    match out {
        Some(p) => {
            std::fs::write(p, &report).map_err(|e| anyhow!("write {p}: {e}"))?;
            eprintln!("report → {p}");
        }
        None => print!("{report}"),
    }
    eprintln!(
        "✓ snowball complete — {} reference(s), {} citer(s). Bring any into your Sources \
         with `/openalex <query>` in the research TUI.",
        backward.len(),
        forward.len()
    );
    Ok(())
}

/// Drive one owned, `Send + 'static` future to completion from this (in-runtime,
/// multi-thread) sync context: spawn it and block on the result — the same
/// mechanism `ai::stream::collect_blocking` relies on.
fn block_on_async<F, T>(fut: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = tx.send(fut.await);
    });
    rx.blocking_recv().expect("snowball async task was dropped")
}

fn render(seed: &Paper, backward: &[Paper], forward: &[Paper]) -> String {
    let line = |p: &Paper| {
        let who = if p.authors.is_empty() {
            String::new()
        } else {
            let mut a = p.authors.first().cloned().unwrap_or_default();
            if p.authors.len() > 1 {
                a.push_str(" et al.");
            }
            format!(" — {a}")
        };
        let yr = if p.year.is_empty() { String::new() } else { format!(", {}", p.year) };
        format!("- {}{who}{yr}  `openalex:{}`\n", p.title, p.id)
    };

    let mut s = String::from("# Snowball — citation neighborhood\n\n");
    s.push_str(&format!("**Seed:** {} `openalex:{}`\n\n", seed.title, seed.id));

    s.push_str(&format!("## References (backward) — {} works the seed cites\n\n", backward.len()));
    if backward.is_empty() {
        s.push_str("_none found (OpenAlex lists no references for this work)._\n\n");
    } else {
        for p in backward {
            s.push_str(&line(p));
        }
        s.push('\n');
    }

    s.push_str(&format!("## Citations (forward) — {} works that cite the seed\n\n", forward.len()));
    if forward.is_empty() {
        s.push_str("_none found (the seed has no indexed citers yet)._\n\n");
    } else {
        for p in forward {
            s.push_str(&line(p));
        }
        s.push('\n');
    }

    s.push_str("_Bring any of these into your Sources with `/openalex <title or DOI>` in the research TUI._\n");
    s
}
