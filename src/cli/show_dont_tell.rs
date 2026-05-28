//! 1.2.11+ — `inkhaven show-dont-tell …` subcommand
//! family.  Currently hosts `bootstrap`, which queries
//! the configured LLM for the four per-language word
//! lists the show-don't-tell overlay consumes and emits
//! a copy-paste-ready HJSON snippet on stdout.
//!
//! Design choices:
//!
//!   * **Stdout-only.**  Mirrors `doctor
//!     --filter-words-snippet`.  Never touches the
//!     user's `inkhaven.hjson` — the author reviews
//!     and pastes what they like.
//!   * **Strict JSON contract with the model.**  The
//!     prompt instructs the LLM to emit a single JSON
//!     object with four arrays.  We extract the JSON
//!     from the streamed response (tolerating
//!     surrounding chatter or a fenced code block) and
//!     deserialise; on any parse failure we dump the
//!     raw response on stderr and exit non-zero so the
//!     author can debug + retry.
//!   * **No detector at runtime.**  The overlay stays
//!     regex + wordlist (instant, deterministic,
//!     offline, free).  The LLM is used once-per-
//!     language as a vocabulary curator, not as an
//!     always-on classifier.  Rationale: an always-on
//!     LLM overlay would burn tokens on every
//!     keystroke and lose the offline guarantee.

use std::io::Write;
use std::path::Path;

use crate::ai::AiClient;
use crate::ai::stream::{StreamMsg, spawn_chat_stream};
use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;

use super::ShowDontTellCommand;

pub fn run(project: &Path, cmd: ShowDontTellCommand) -> Result<()> {
    match cmd {
        ShowDontTellCommand::Bootstrap {
            language,
            genre,
            provider,
        } => bootstrap(project, &language, genre.as_deref(), provider.as_deref()),
    }
}

fn bootstrap(
    project: &Path,
    language: &str,
    genre: Option<&str>,
    provider: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load(&layout.config_path())?;
    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env_var) = ai.resolve_provider(&cfg.llm, provider)?;

    let prompt = build_prompt(language, genre);
    eprintln!(
        "inkhaven show-dont-tell bootstrap · language: {language}{genre_tail} · model: {model}",
        genre_tail = genre
            .map(|g| format!(" · genre: {g}"))
            .unwrap_or_default(),
    );

    let mut rx = spawn_chat_stream(
        ai.client.clone(),
        model.to_string(),
        Some(SYSTEM_PROMPT.to_string()),
        Vec::new(),
        prompt,
    );

    let mut raw = String::new();
    while let Some(msg) = rx.blocking_recv() {
        match msg {
            StreamMsg::Token(t) => {
                raw.push_str(&t);
                let _ = std::io::stderr().write_all(b".");
                let _ = std::io::stderr().flush();
            }
            StreamMsg::Done => break,
            StreamMsg::Error(e) => {
                eprintln!();
                eprintln!("inference error: {e}");
                return Ok(());
            }
        }
    }
    eprintln!();

    let lists = match parse_lists(&raw) {
        Ok(l) => l,
        Err(why) => {
            eprintln!("could not parse model response: {why}");
            eprintln!("---- raw response ----");
            eprintln!("{raw}");
            eprintln!("---- end ----");
            return Ok(());
        }
    };

    print_snippet(language, &lists);
    Ok(())
}

const SYSTEM_PROMPT: &str = "\
You are a precise lexicographer assisting an author with their writing \
craft tooling.  Reply with a SINGLE JSON object and nothing else — no \
prose, no preamble, no markdown fences.  Every word you produce must be \
in the requested language (a real word native speakers use), lowercased, \
in its dictionary form (lemma).  Avoid genre-specific slang unless \
explicitly asked.";

fn build_prompt(language: &str, genre: Option<&str>) -> String {
    let genre_line = match genre {
        Some(g) => format!(
            "Bias the vocabulary toward the {g} register — pick words an \
             author working in that mode would actually reach for.\n"
        ),
        None => String::new(),
    };
    format!(
        "Produce vocabulary lists for a show-don't-tell style detector \
         in {language}.  The detector flags four kinds of \"telling\" \
         constructions:\n\n\
         1. `linking_verbs` — copula and quasi-copula verbs used to \
            assert internal state (English equivalents: be, seem, feel, \
            look, appear, become).  Provide ~10-20 lemmas.\n\
         2. `emotion_adjectives` — adjectives that name an emotion \
            outright (English equivalents: angry, sad, afraid, happy, \
            tired, surprised, embarrassed, proud, jealous, lonely, \
            bored, excited, hopeless).  Cover the major emotion \
            families.  Provide ~30-60 lemmas.\n\
         3. `manner_adverbs` — emotion-labelling adverbs (English \
            equivalents: angrily, sadly, nervously, happily).  Provide \
            ~15-30 lemmas.\n\
         4. `cognition_verbs` — verbs that narrate thought instead of \
            showing it (English equivalents: realised, knew, \
            understood, wondered, decided, believed).  Provide ~10-20 \
            lemmas.\n\n\
         {genre_line}\
         Reply with EXACTLY this JSON shape and no other text:\n\n\
         {{\n  \"linking_verbs\":       [\"…\", \"…\"],\n  \
         \"emotion_adjectives\":  [\"…\", \"…\"],\n  \
         \"manner_adverbs\":      [\"…\", \"…\"],\n  \
         \"cognition_verbs\":     [\"…\", \"…\"]\n}}\n"
    )
}

#[derive(Debug, serde::Deserialize)]
struct Lists {
    #[serde(default)]
    linking_verbs: Vec<String>,
    #[serde(default)]
    emotion_adjectives: Vec<String>,
    #[serde(default)]
    manner_adverbs: Vec<String>,
    #[serde(default)]
    cognition_verbs: Vec<String>,
}

fn parse_lists(raw: &str) -> std::result::Result<Lists, String> {
    let trimmed = raw.trim();
    // Strip a leading ``` or ```json fence if the model
    // wrapped the JSON in a code block despite the
    // instruction.  Strip the trailing fence too.
    let stripped = strip_code_fence(trimmed);
    // Pluck the first balanced { … } substring so any
    // stray prose around the JSON doesn't kill the
    // parse.  Brace-balanced rather than first-/last-
    // index so an example object embedded in the
    // model's explanation doesn't break us.
    let json = match extract_json_object(stripped) {
        Some(j) => j,
        None => {
            return Err("no JSON object found in response".to_string());
        }
    };
    serde_json::from_str::<Lists>(json).map_err(|e| format!("invalid JSON: {e}"))
}

fn strip_code_fence(s: &str) -> &str {
    let s = s.trim();
    let s = s.strip_prefix("```json").unwrap_or(s);
    let s = s.strip_prefix("```").unwrap_or(s);
    s.strip_suffix("```").unwrap_or(s).trim()
}

fn extract_json_object(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let start = bytes.iter().position(|&b| b == b'{')?;
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

fn print_snippet(language: &str, lists: &Lists) {
    let lang = language.to_lowercase();
    println!("// Paste under editor.style_warnings.show_dont_tell:");
    println!(
        "// (existing values for other languages stay; empty arrays use built-in defaults.)"
    );
    println!();
    println!("show_dont_tell: {{");
    println!("  enabled: true");
    println!("  use_stemming: true");
    println!();
    print_list(&format!("{lang}_linking_verbs"), &lists.linking_verbs);
    print_list(
        &format!("{lang}_emotion_adjectives"),
        &lists.emotion_adjectives,
    );
    print_list(&format!("{lang}_manner_adverbs"), &lists.manner_adverbs);
    print_list(&format!("{lang}_cognition_verbs"), &lists.cognition_verbs);
    println!("}}");
}

fn print_list(field: &str, words: &[String]) {
    println!("  // Lemmas — Snowball stemming catches inflections.");
    println!("  {field}: [");
    let mut buf = String::from("    ");
    for (i, w) in words.iter().enumerate() {
        let w = w.trim();
        if w.is_empty() {
            continue;
        }
        if i > 0 {
            buf.push(' ');
        }
        buf.push('"');
        buf.push_str(&w.replace('"', "'"));
        buf.push('"');
        if buf.chars().count() > 64 {
            println!("{buf}");
            buf = String::from("    ");
        }
    }
    if buf.trim() != "" {
        println!("{buf}");
    }
    println!("  ]");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_handles_chatter() {
        let raw = "Here is the JSON you asked for:\n\n{ \"linking_verbs\": [\"быть\"], \"emotion_adjectives\": [], \"manner_adverbs\": [], \"cognition_verbs\": [] }\n\nLet me know if you'd like more.";
        let json = extract_json_object(raw).unwrap();
        assert!(json.starts_with('{') && json.ends_with('}'));
        let lists: Lists = serde_json::from_str(json).unwrap();
        assert_eq!(lists.linking_verbs, vec!["быть".to_string()]);
    }

    #[test]
    fn extract_json_handles_nested_braces() {
        let raw = "{ \"linking_verbs\": [\"a\"], \"note\": \"object {x: 1}\", \"emotion_adjectives\": [], \"manner_adverbs\": [], \"cognition_verbs\": [] }";
        let json = extract_json_object(raw).unwrap();
        assert!(json.ends_with('}'));
    }

    #[test]
    fn strip_fence_removes_markdown_wrapping() {
        let raw = "```json\n{\"linking_verbs\": [], \"emotion_adjectives\": [], \"manner_adverbs\": [], \"cognition_verbs\": []}\n```";
        let stripped = strip_code_fence(raw);
        assert!(stripped.starts_with('{'));
        assert!(stripped.ends_with('}'));
    }

    #[test]
    fn parse_lists_rejects_garbage() {
        let err = parse_lists("not json at all").unwrap_err();
        assert!(err.contains("no JSON object"));
    }
}
