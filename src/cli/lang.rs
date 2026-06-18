//! 1.3.13 BREADTH-1 — `inkhaven lang status`: an honest coverage matrix for the
//! project (or `--language`) language — what's automatic (stemming / prompts /
//! embeddings), what's curated, and what's off until you bootstrap or
//! configure it.

use std::path::Path;

use crate::config::{self, Config};
use crate::error::Result;
use crate::project::ProjectLayout;

use super::LangCommand;

pub fn run(project: &Path, cmd: LangCommand) -> Result<()> {
    match cmd {
        LangCommand::Status { language } => status(project, language.as_deref()),
    }
}

fn status(project: &Path, language: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    let cfg = Config::load_layered(&layout.config_path()).unwrap_or_default();
    let lang = match language {
        Some(l) => l.to_string(),
        None if cfg.language.trim().is_empty() => "english".to_string(),
        None => cfg.language.clone(),
    };
    let l = lang.to_lowercase();

    println!("inkhaven lang status · language: {lang}\n");

    let stem = match config::parse_stemmer_language(&l) {
        Some(_) => format!("✓ Snowball ({l})"),
        None => "✗ exact-match only (no Snowball algorithm)".to_string(),
    };
    row("stemming", &stem);

    row("filter words", &coverage(config::built_in_filter_words(&l).len()));

    let sdt = config::built_in_linking_verbs(&l).len()
        + config::built_in_emotion_adjectives(&l).len()
        + config::built_in_manner_adverbs(&l).len()
        + config::built_in_cognition_verbs(&l).len();
    row("show-don't-tell", &coverage(sdt));

    row(
        "repeated-phrase stop-words",
        &coverage(config::built_in_stop_words(&l).len()),
    );

    let pron = if crate::drift::has_builtin_pronouns(&l) {
        "✓ built-in".to_string()
    } else {
        "none — coref off".to_string()
    };
    row("drift pronouns (coref)", &pron);

    row(
        "anachronism lexicon",
        "English built-ins + your `terms` (language-neutral)",
    );
    row("embeddings", &format!("multilingual · {}", cfg.embeddings.model));
    row("AI world-check output", &format!("forced in {lang}"));
    let (_, prompt_fb) = crate::cli::world_prompts::world_system_prompt("facts-check", &l);
    row(
        "AI world-check prompts",
        if prompt_fb {
            "English (no localized prompt — fallback with a warning)"
        } else {
            "✓ localized (facts check / scan · drift · continuity)"
        },
    );

    if config::built_in_filter_words(&l).is_empty()
        && !crate::drift::has_builtin_pronouns(&l)
    {
        println!(
            "\n  ▶ no curated detector lists for {l} — run `inkhaven lang bootstrap {l}` \
             or add lists to inkhaven.hjson (stemming, prompts, and embeddings already work)."
        );
    }
    Ok(())
}

fn row(label: &str, val: &str) {
    println!("  {label:<28} {val}");
}

fn coverage(n: usize) -> String {
    if n == 0 {
        "none".to_string()
    } else {
        format!("built-in {n}")
    }
}
