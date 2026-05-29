//! 1.2.12+ Phase B — `inkhaven prompts …` subcommand
//! family.  Currently hosts `bootstrap`, which asks the
//! configured LLM to produce per-language variants of
//! the seven embedded inkhaven prompts and emits a
//! copy-paste-ready HJSON snippet on stdout (default),
//! or merges into `prompts.hjson` in place with
//! `--update`.
//!
//! Design choices:
//!
//!   * **Stdout-only by default.**  Mirrors
//!     `inkhaven show-dont-tell bootstrap` and the
//!     long-standing `doctor --filter-words-snippet`
//!     pattern.  Never writes to the user's project
//!     without explicit `--update`.
//!   * **Strict JSON contract with the model.**  Same
//!     parser machinery as SDT bootstrap — the prompt
//!     instructs the LLM to emit one JSON object with
//!     seven string fields; we extract via brace-
//!     balanced JSON walk so stray chatter or fenced
//!     code blocks don't kill the parse.
//!   * **Surgical merge for `--update`.**  Existing
//!     `prompts.hjson` entries whose `(name,
//!     language)` already match a generated variant
//!     are spliced in place; new entries are appended
//!     under the parent `prompts` array.  Other
//!     entries (`tighten`, `darker`, user-curated
//!     prompts) are left untouched.  No clobbering.
//!   * **No runtime translation.**  The bootstrap is a
//!     one-shot curator.  Once written, every AI flow
//!     uses the persisted entries via the Phase A
//!     three-pass resolver — no LLM calls on the hot
//!     path.

use std::io::Write;
use std::path::Path;

use crate::ai::AiClient;
use crate::ai::prompts::{Prompt, PromptLibrary, iso_from_long};
use crate::ai::stream::{StreamMsg, spawn_chat_stream};
use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;

use super::PromptsCommand;

pub fn run(project: &Path, cmd: PromptsCommand) -> Result<()> {
    match cmd {
        PromptsCommand::Bootstrap {
            language,
            genre,
            provider,
            update,
        } => bootstrap(
            project,
            &language,
            genre.as_deref(),
            provider.as_deref(),
            update,
        ),
    }
}

/// The seven embedded prompt names + descriptions the
/// bootstrap asks the LLM to translate.  Kept in sync
/// with the `*_default_prompt` functions in
/// `src/tui/app.rs` — adding a new embedded prompt
/// means adding a row here.
const EMBEDDED_PROMPTS: &[(&str, &str)] = &[
    (
        "grammar-check",
        "F7 — copy-edit pass over the open paragraph.",
    ),
    (
        "explain-diagnostic",
        "Ctrl+F12 — explain the Typst compiler diagnostic at the cursor.",
    ),
    (
        "critique-edit",
        "F12 (editor mode) — identify the two or three weakest elements in the open paragraph.",
    ),
    (
        "critique-changes",
        "F12 (split-edit mode) — evaluate what the latest revision changed.",
    ),
    (
        "critique-compare",
        "F12 (split-view mode, 1.2.12+) — compare two distinct paragraphs (translation source vs translation, draft vs draft); identify convergence, divergence, which one lands the beat better.",
    ),
    (
        "show-dont-tell",
        "Ctrl+B Shift+T — find every place the writer is telling instead of showing.",
    ),
    (
        "sentence-rhythm-rewrite",
        "Ctrl+B Shift+M — break monotonous sentence rhythm while preserving voice.",
    ),
    (
        "timeline-health",
        "Timeline modal — review the story timeline for internal consistency.",
    ),
];

fn bootstrap(
    project: &Path,
    language: &str,
    genre: Option<&str>,
    provider: Option<&str>,
    update: bool,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load(&layout.config_path())?;
    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env_var) = ai.resolve_provider(&cfg.llm, provider)?;

    // Normalise to ISO 639-1 once — `iso_from_long`
    // accepts both the long form ("russian") and the
    // ISO code ("ru") gracefully (anything not in
    // the supported five maps to "en").
    let lang_iso = iso_from_long(language).to_string();

    let prompt = build_prompt(language, &lang_iso, genre);
    eprintln!(
        "inkhaven prompts bootstrap · language: {language} ({lang_iso}){genre_tail} · model: {model}",
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

    let parsed = match parse_prompts(&raw) {
        Ok(p) => p,
        Err(why) => {
            eprintln!("could not parse model response: {why}");
            eprintln!("---- raw response ----");
            eprintln!("{raw}");
            eprintln!("---- end ----");
            return Ok(());
        }
    };

    // Verify every embedded prompt got a translation;
    // print a warning for any that didn't.  We still
    // emit the snippet for whatever the model produced
    // so a partial result is salvageable.
    for (name, _) in EMBEDDED_PROMPTS {
        if parsed.get(*name).map(String::is_empty).unwrap_or(true) {
            eprintln!("warning: model returned no `{name}` body");
        }
    }

    if update {
        match apply_update(project, &lang_iso, &parsed) {
            Ok((written, backup)) => {
                eprintln!(
                    "patched {} (pre-patch backup: {})",
                    written.display(),
                    backup.display(),
                );
            }
            Err(e) => {
                eprintln!("in-place update failed: {e}");
                eprintln!(
                    "(nothing was written; pasting the snippet below by hand still works)"
                );
            }
        }
    }

    print_snippet(&lang_iso, &parsed);
    Ok(())
}

/// 1.2.12+ Phase B — `--update` writer.  Loads
/// `<project>/prompts.hjson` (or starts from the
/// embedded `assets/default_prompts.hjson` when the
/// project file is missing — same defaulting
/// behaviour as the prompts-editor TUI), merges the
/// generated prompts, backs up the pre-patch file under
/// `<project>/.config-backups/`, and writes back via
/// `serde_hjson::to_string` + atomic `.tmp` + rename.
///
/// Merge semantics:
///   * For each generated `(name, lang_iso)` pair,
///     find any existing `prompts.hjson` entry with
///     the same `name` AND the same `language` —
///     overwrite its template.
///   * No match → append a new entry.
///   * Existing entries with a DIFFERENT language for
///     the same `name` (e.g. English `grammar-check`)
///     are left untouched — they're how the resolver's
///     Pass 1 cascade works.
///
/// Returns `(written_path, backup_path)` on success.
fn apply_update(
    project: &Path,
    lang_iso: &str,
    parsed: &std::collections::HashMap<String, String>,
) -> std::result::Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let layout = ProjectLayout::new(project);
    let prompts_path = layout.root.join("prompts.hjson");

    // Pre-patch snapshot: even if prompts.hjson doesn't
    // exist yet, we still write the backup of whatever
    // content we used as the baseline so the user can
    // compare diffs.
    let baseline = if prompts_path.exists() {
        std::fs::read_to_string(&prompts_path)
            .map_err(|e| format!("read {}: {e}", prompts_path.display()))?
    } else {
        crate::config::DEFAULT_PROMPTS.to_string()
    };

    let backup_dir = layout.root.join(".config-backups");
    std::fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("create {}: {e}", backup_dir.display()))?;
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_path = backup_dir.join(format!("prompts_{ts}.hjson"));
    std::fs::write(&backup_path, &baseline)
        .map_err(|e| format!("write {}: {e}", backup_path.display()))?;

    // Parse, merge, serialise.
    let mut library: PromptLibrary = serde_hjson::from_str(&baseline)
        .map_err(|e| format!("parse baseline HJSON: {e}"))?;
    merge_into_library(&mut library, lang_iso, parsed);
    let body = serde_hjson::to_string(&library)
        .map_err(|e| format!("serialise PromptLibrary: {e}"))?;

    // Atomic write.
    let mut tmp = prompts_path.clone();
    let mut name = prompts_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.push_str(".tmp");
    tmp.set_file_name(&name);
    std::fs::write(&tmp, &body)
        .map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &prompts_path)
        .map_err(|e| format!("rename {} → {}: {e}", tmp.display(), prompts_path.display()))?;
    Ok((prompts_path, backup_path))
}

/// 1.2.12+ Phase B — splice the generated `(name,
/// language)` prompts into `library`.  Same-`(name,
/// language)` entries are overwritten; otherwise the
/// new prompt is appended.
fn merge_into_library(
    library: &mut PromptLibrary,
    lang_iso: &str,
    parsed: &std::collections::HashMap<String, String>,
) {
    for (name, _) in EMBEDDED_PROMPTS {
        let Some(body) = parsed.get(*name) else {
            continue;
        };
        if body.trim().is_empty() {
            continue;
        }
        let template = body.to_string();
        let lang_tag = lang_iso.to_string();
        let existing = library.prompts.iter_mut().find(|p| {
            p.name == *name
                && p.language
                    .as_deref()
                    .map(|l| l.eq_ignore_ascii_case(&lang_tag))
                    .unwrap_or(false)
        });
        match existing {
            Some(prompt) => {
                prompt.template = template;
            }
            None => {
                library.prompts.push(Prompt {
                    name: name.to_string(),
                    description: format!("{name} ({lang_tag})"),
                    template,
                    language: Some(lang_tag),
                });
            }
        }
    }
}

const SYSTEM_PROMPT: &str = "\
You are a precise literary-craft translator helping an author localise \
the prompt library of their writing tool.  Reply with a SINGLE JSON \
object and nothing else — no prose, no preamble, no markdown fences.  \
Every prompt body must be in the requested target language (a native \
speaker would write it that way).  Preserve technical references to \
Typst markup verbatim — they're API tokens, not prose.  Preserve any \
double-curly placeholders like {{selection}} and {{context}} verbatim — \
they're substituted at runtime.  Keep the tight, directive tone of the \
English originals; do not soften the imperative voice.";

fn build_prompt(language: &str, lang_iso: &str, genre: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Translate / adapt the seven inkhaven embedded prompts below into \
         {language}.  Each prompt is a system message inkhaven sends to an \
         LLM to perform one specific writing-craft task on the open \
         paragraph.  Match the directive tone, technical references, and \
         output-discipline clauses (e.g. \"Return ONLY the rewritten \
         paragraph\") exactly — those clauses matter for the downstream \
         pipeline.\n\n"
    ));
    if let Some(g) = genre {
        out.push_str(&format!(
            "Where vocabulary choice has wiggle room, bias toward the \
             {g} register (the author works in that mode).\n\n"
        ));
    }
    out.push_str("--- The seven prompts ---\n\n");
    for (name, description) in EMBEDDED_PROMPTS {
        out.push_str(&format!("Name: {name}\nDescription: {description}\n\n"));
    }
    out.push_str(&format!(
        "--- Output format ---\n\n\
         Reply with EXACTLY this JSON shape (target language: {language}, \
         ISO 639-1 code: {lang_iso}) and NO other text:\n\n\
         {{\n\
         \x20 \"grammar-check\":            \"…\",\n\
         \x20 \"explain-diagnostic\":       \"…\",\n\
         \x20 \"critique-edit\":            \"…\",\n\
         \x20 \"critique-changes\":         \"…\",\n\
         \x20 \"show-dont-tell\":           \"…\",\n\
         \x20 \"sentence-rhythm-rewrite\":  \"…\",\n\
         \x20 \"timeline-health\":          \"…\"\n\
         }}\n\n\
         Each value is the full prompt body the model will see.  Do NOT \
         translate the JSON keys — those are stable identifiers.  Do NOT \
         add any other top-level keys; do NOT include English originals \
         alongside the translations.\n"
    ));
    out
}

fn parse_prompts(raw: &str) -> std::result::Result<std::collections::HashMap<String, String>, String> {
    let trimmed = raw.trim();
    let stripped = strip_code_fence(trimmed);
    let json = extract_json_object(stripped)
        .ok_or_else(|| "no JSON object found in response".to_string())?;
    serde_json::from_str::<std::collections::HashMap<String, String>>(json)
        .map_err(|e| format!("invalid JSON: {e}"))
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

fn print_snippet(
    lang_iso: &str,
    parsed: &std::collections::HashMap<String, String>,
) {
    println!("// Paste under the `prompts` array in prompts.hjson.");
    println!(
        "// Each block carries `language: {lang_iso}` so the Phase A resolver"
    );
    println!("// picks it over the embedded English fallback when the project");
    println!("// language (or paragraph-detected language) matches.");
    println!();
    for (name, _description) in EMBEDDED_PROMPTS {
        let Some(body) = parsed.get(*name) else {
            continue;
        };
        if body.trim().is_empty() {
            continue;
        }
        println!("{{");
        println!("  name: {name}");
        println!("  language: {lang_iso}");
        println!("  description: {name} ({lang_iso})");
        println!("  template: '''");
        // Indent every line of the body 4 spaces so it
        // sits inside the HJSON triple-quoted block
        // without the user having to re-indent.
        for line in body.lines() {
            println!("    {line}");
        }
        println!("  '''");
        println!("}}");
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_handles_chatter() {
        let raw = "Here you go:\n\n{ \"grammar-check\": \"проверь грамматику\", \"explain-diagnostic\": \"объясни\", \"critique-edit\": \"критика\", \"critique-changes\": \"оцени\", \"show-dont-tell\": \"показывай\", \"sentence-rhythm-rewrite\": \"перепиши\", \"timeline-health\": \"проверь хронологию\" }\n\nLet me know.";
        let json = extract_json_object(raw).unwrap();
        let parsed: std::collections::HashMap<String, String> =
            serde_json::from_str(json).unwrap();
        assert_eq!(parsed.get("grammar-check").unwrap(), "проверь грамматику");
        assert_eq!(parsed.len(), 7);
    }

    #[test]
    fn parse_prompts_handles_fenced_block() {
        let raw = "```json\n{\"grammar-check\":\"x\",\"explain-diagnostic\":\"x\",\"critique-edit\":\"x\",\"critique-changes\":\"x\",\"show-dont-tell\":\"x\",\"sentence-rhythm-rewrite\":\"x\",\"timeline-health\":\"x\"}\n```";
        let parsed = parse_prompts(raw).unwrap();
        assert_eq!(parsed.len(), 7);
    }

    #[test]
    fn parse_prompts_rejects_garbage() {
        let err = parse_prompts("not json").unwrap_err();
        assert!(err.contains("no JSON object"));
    }

    #[test]
    fn embedded_prompts_table_covers_every_named_resolver_path() {
        // If a future commit adds a new named embedded
        // prompt without adding it here, the bootstrap
        // won't translate it.  This test is a forcing
        // function — bump the count and add the entry.
        let names: Vec<&str> = EMBEDDED_PROMPTS.iter().map(|(n, _)| *n).collect();
        for expected in [
            "grammar-check",
            "explain-diagnostic",
            "critique-edit",
            "critique-changes",
            "critique-compare",
            "show-dont-tell",
            "sentence-rhythm-rewrite",
            "timeline-health",
        ] {
            assert!(
                names.contains(&expected),
                "EMBEDDED_PROMPTS missing entry for `{expected}`",
            );
        }
        assert_eq!(EMBEDDED_PROMPTS.len(), 8);
    }

    #[test]
    fn merge_replaces_same_name_same_language() {
        let mut library = PromptLibrary::default();
        library.prompts.push(Prompt {
            name: "grammar-check".into(),
            description: "old".into(),
            template: "OLD RU".into(),
            language: Some("ru".into()),
        });
        let mut parsed = std::collections::HashMap::new();
        parsed.insert("grammar-check".into(), "NEW RU".into());
        merge_into_library(&mut library, "ru", &parsed);
        // Same name + same language → overwrite.  Single
        // entry — no append.
        assert_eq!(library.prompts.len(), 1);
        assert_eq!(library.prompts[0].template, "NEW RU");
    }

    #[test]
    fn merge_appends_when_no_same_language_match() {
        let mut library = PromptLibrary::default();
        library.prompts.push(Prompt {
            name: "grammar-check".into(),
            description: "english".into(),
            template: "EN".into(),
            language: Some("en".into()),
        });
        let mut parsed = std::collections::HashMap::new();
        parsed.insert("grammar-check".into(), "RU".into());
        merge_into_library(&mut library, "ru", &parsed);
        // English entry preserved; Russian appended.
        assert_eq!(library.prompts.len(), 2);
        assert!(
            library
                .prompts
                .iter()
                .any(|p| p.language.as_deref() == Some("en") && p.template == "EN")
        );
        assert!(
            library
                .prompts
                .iter()
                .any(|p| p.language.as_deref() == Some("ru") && p.template == "RU")
        );
    }

    #[test]
    fn merge_skips_empty_bodies() {
        let mut library = PromptLibrary::default();
        let mut parsed = std::collections::HashMap::new();
        parsed.insert("grammar-check".into(), "   ".into());
        merge_into_library(&mut library, "ru", &parsed);
        assert_eq!(library.prompts.len(), 0);
    }
}
