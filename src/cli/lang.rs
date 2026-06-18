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
        LangCommand::Bootstrap { language, provider, yes } => {
            bootstrap(project, &language, provider.as_deref(), yes)
        }
    }
}

const BOOTSTRAP_SYSTEM: &str = "You are a precise lexicographer for an author's writing-craft \
tooling. Reply with a SINGLE JSON object and nothing else — no prose, no preamble, no markdown \
fences. Every word must be a real word native speakers actually use in the requested language, \
lowercased, in dictionary / lemma form (Snowball stemming handles inflections). Pronouns must be \
standalone words (not bound suffixes).";

#[derive(Debug, Default, serde::Deserialize)]
struct Bootstrapped {
    #[serde(default)]
    filter_words: Vec<String>,
    #[serde(default)]
    linking_verbs: Vec<String>,
    #[serde(default)]
    emotion_adjectives: Vec<String>,
    #[serde(default)]
    manner_adverbs: Vec<String>,
    #[serde(default)]
    cognition_verbs: Vec<String>,
    #[serde(default)]
    stop_words: Vec<String>,
    #[serde(default)]
    pronouns: BootPronouns,
}

#[derive(Debug, Default, serde::Deserialize)]
struct BootPronouns {
    #[serde(default)]
    character: Vec<String>,
    #[serde(default)]
    place: Vec<String>,
    #[serde(default)]
    artefact: Vec<String>,
}

fn bootstrap(project: &Path, language: &str, provider: Option<&str>, yes: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let lang = language.trim().to_lowercase();

    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    eprintln!("inkhaven lang bootstrap · language: {lang} · model: {model}");
    if config::parse_stemmer_language(&lang).is_none() {
        eprintln!("  note: no Snowball stemmer for {lang} — detectors will use exact-match.");
    }

    let raw = crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(BOOTSTRAP_SYSTEM.to_string()),
        build_bootstrap_prompt(&lang),
    )
    .map_err(|e| crate::error::Error::Store(format!("inference error: {e}")))?;

    let lists: Bootstrapped = match parse_json_object(&raw) {
        Ok(l) => l,
        Err(why) => {
            eprintln!("could not parse model response: {why}\n---- raw ----\n{raw}\n---- end ----");
            return Ok(());
        }
    };

    print_snippet(&lang, &lists);

    if yes {
        match crate::config_tui::apply_in_place_edits(project, &build_updates(&lang, &lists)) {
            Ok(out) => eprintln!(
                "\npatched {} (pre-patch backup: {})",
                out.config_path.display(),
                out.backup.display()
            ),
            Err(e) => eprintln!(
                "\nin-place patch failed: {e}\n(paste the snippet above into inkhaven.hjson by hand)"
            ),
        }
    } else {
        eprintln!("\n(dry run — re-run with --yes to patch inkhaven.hjson, or paste the snippet above)");
    }
    Ok(())
}

fn build_bootstrap_prompt(language: &str) -> String {
    format!(
        "Produce detector vocabulary for an author's craft tooling in {language}. Reply with \
         EXACTLY this JSON shape and no other text:\n\n\
         {{\n  \
         \"filter_words\": [],          // intensifier crutches + hedges (English: just, really, very, seemed, felt) — ~15-25\n  \
         \"linking_verbs\": [],         // copula / quasi-copula asserting inner state (be, seem, feel, become) — ~10-20\n  \
         \"emotion_adjectives\": [],    // adjectives naming an emotion (angry, sad, afraid, proud) — ~30-50\n  \
         \"manner_adverbs\": [],        // emotion-labelling adverbs (angrily, sadly, nervously) — ~15-25\n  \
         \"cognition_verbs\": [],       // verbs narrating thought (realised, knew, decided) — ~10-20\n  \
         \"stop_words\": [],            // closed-class function words to exclude from n-grams (the, and, in) — ~20-40\n  \
         \"pronouns\": {{ \"character\": [], \"place\": [], \"artefact\": [] }}  // 3rd-person + here/there; standalone words only\n\
         }}\n\n\
         All words in {language}, lowercased, lemma form. Omit articles from `pronouns`. Empty a \
         list only if the language genuinely lacks that category."
    )
}

/// Extract the first `{ … }` JSON object from a model reply (tolerating stray
/// prose / fences) and deserialize it.
fn parse_json_object(raw: &str) -> std::result::Result<Bootstrapped, String> {
    let start = raw.find('{').ok_or("no JSON object found")?;
    let end = raw.rfind('}').ok_or("no closing brace")?;
    if end < start {
        return Err("malformed braces".into());
    }
    serde_json::from_str(&raw[start..=end]).map_err(|e| e.to_string())
}

fn build_updates(lang: &str, l: &Bootstrapped) -> Vec<(String, serde_json::Value)> {
    use serde_json::json;
    let sw = "editor.style_warnings";
    vec![
        (format!("{sw}.filter_words.languages.{lang}"), json!(l.filter_words)),
        (
            format!("{sw}.show_dont_tell.languages.{lang}"),
            json!({
                "linking_verbs": l.linking_verbs,
                "emotion_adjectives": l.emotion_adjectives,
                "manner_adverbs": l.manner_adverbs,
                "cognition_verbs": l.cognition_verbs,
            }),
        ),
        (format!("{sw}.repeated_phrases.languages.{lang}"), json!(l.stop_words)),
        (
            format!("drift.pronouns.{lang}"),
            json!({
                "character": l.pronouns.character,
                "place": l.pronouns.place,
                "artefact": l.pronouns.artefact,
            }),
        ),
    ]
}

fn print_snippet(lang: &str, l: &Bootstrapped) {
    let arr = |ws: &[String]| {
        ws.iter().map(|w| format!("\"{w}\"")).collect::<Vec<_>>().join(", ")
    };
    println!("// --- paste into inkhaven.hjson (language: {lang}) ---");
    println!("editor: {{ style_warnings: {{");
    println!("  filter_words:    {{ languages: {{ {lang}: [ {} ] }} }}", arr(&l.filter_words));
    println!("  show_dont_tell:  {{ languages: {{ {lang}: {{");
    println!("    linking_verbs:      [ {} ]", arr(&l.linking_verbs));
    println!("    emotion_adjectives: [ {} ]", arr(&l.emotion_adjectives));
    println!("    manner_adverbs:     [ {} ]", arr(&l.manner_adverbs));
    println!("    cognition_verbs:    [ {} ]", arr(&l.cognition_verbs));
    println!("  }} }} }}");
    println!("  repeated_phrases: {{ languages: {{ {lang}: [ {} ] }} }}", arr(&l.stop_words));
    println!("}} }}");
    println!("drift: {{ pronouns: {{ {lang}: {{");
    println!("  character: [ {} ]", arr(&l.pronouns.character));
    println!("  place:     [ {} ]", arr(&l.pronouns.place));
    println!("  artefact:  [ {} ]", arr(&l.pronouns.artefact));
    println!("}} }} }}");
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

    let pron = if crate::drift::has_pronouns(&l, &cfg.drift.pronouns) {
        "✓ available".to_string()
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
        && !crate::drift::has_pronouns(&l, &cfg.drift.pronouns)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_object_tolerates_fences_and_prose() {
        let raw = "Here you go:\n```json\n{\"filter_words\": [\"molto\"], \
                   \"pronouns\": {\"character\": [\"egli\"]}}\n```\nHope that helps!";
        let l = parse_json_object(raw).expect("extracts the object");
        assert_eq!(l.filter_words, vec!["molto"]);
        assert_eq!(l.pronouns.character, vec!["egli"]);
    }

    #[test]
    fn build_updates_targets_the_per_language_maps() {
        let l = Bootstrapped { stop_words: vec!["e".into()], ..Default::default() };
        let ups = build_updates("italian", &l);
        let paths: Vec<&str> = ups.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"editor.style_warnings.filter_words.languages.italian"));
        assert!(paths.contains(&"editor.style_warnings.show_dont_tell.languages.italian"));
        assert!(paths.contains(&"editor.style_warnings.repeated_phrases.languages.italian"));
        assert!(paths.contains(&"drift.pronouns.italian"));
    }
}
