//! `inkhaven language` authoring & analysis commands: AI composition
//! (`compose`), lexicon/typology stats + deterministic audit, and grammar-rule
//! authoring (`define-rule` + its template/editor helpers). Split out of the
//! flat handler.

use std::path::Path;

use crate::error::{Error, Result};

use super::*;

/// 1.3.19 LANG-1 P6 — creative text generators. Deterministic names / prose /
/// verse from the phonology + lexicon + syntax engine; AI-composed but
/// lexicon-constrained blessing / curse / incantation. Prints only — never
/// writes to the book.
pub(crate) fn compose(
    project: &Path,
    language: &str,
    kind: &str,
    count: usize,
    seed: u64,
    meter: &str,
    provider: Option<&str>,
) -> Result<()> {
    use crate::conlang::creative;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let (grammar_spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let typology = &grammar_spec.grammar;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;

    match kind.to_ascii_lowercase().as_str() {
        "names" | "name" => {
            let names = creative::names(&phon, count, seed);
            if names.is_empty() {
                return Err(Error::Config(
                    "no names could be generated — does the language declare a `root` template?"
                        .into(),
                ));
            }
            println!("{language} — {} names:\n", names.len());
            for n in &names {
                println!("  {n}");
            }
        }
        "prose" | "sample" | "sample-text" => {
            let lines = creative::prose(&phon, &morph, typology, &entries, count, seed);
            if lines.is_empty() {
                return Err(Error::Config(
                    "need at least one noun and one verb in the lexicon to compose prose".into(),
                ));
            }
            println!("{language} — sample sentences:\n");
            for r in &lines {
                println!("• {}\n", format_clause(r));
            }
        }
        "poem" | "poetry" | "verse" => {
            // Clamp each line's syllable count to a sane range — an unbounded
            // value (a dropped digit → `--meter 5000000000`) would spin the poem
            // generator for billions of tries and overflow `target * 4`.
            let meter: Vec<usize> = meter
                .split(',')
                .filter_map(|s| s.trim().parse::<usize>().ok())
                .filter(|n| (1..=64).contains(n))
                .collect();
            if meter.is_empty() {
                return Err(Error::Config(
                    "invalid --meter — give comma-separated syllable counts, e.g. 5,7,5".into(),
                ));
            }
            let lines = creative::poem(&phon, &entries, &meter, seed);
            if lines.is_empty() {
                return Err(Error::Config("could not generate verse (empty inventory)".into()));
            }
            println!("{language} — verse ({}):\n", meter
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join("-"));
            for l in &lines {
                println!("  {:<32} ({}/{})", l.text, l.syllables, l.target);
            }
        }
        "blessing" | "curse" | "incantation" | "ceremony" => {
            if entries.is_empty() {
                return Err(Error::Config(
                    "the lexicon is empty — add some words before composing themed text".into(),
                ));
            }
            let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
            let typ_summary = summarize_typology(typology);
            let (system, user) = creative::themed_prompt(
                &lang_book.title,
                kind,
                &cfg.language,
                &typ_summary,
                &entries,
            );
            let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
            let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
            eprintln!("inkhaven language compose · {kind} · {} · model: {model}", lang_book.title);
            let raw = crate::ai::stream::collect_blocking(
                ai.client.clone(),
                model.to_string(),
                Some(system),
                user,
            )
            .map_err(|e| Error::Store(format!("inference error: {e}")))?;
            let text = strip_code_fence(&raw);
            println!("{}", text.trim());
            // Advisory check: flag any native token not found in the lexicon's
            // surface forms, so the author can see if the model drifted.
            warn_unknown_tokens(&text, &entries);
            eprintln!(
                "\n(advisory — generated text, not saved; review before use)"
            );
        }
        other => {
            return Err(Error::Config(format!(
                "unknown --kind `{other}` (expected names | prose | poem | blessing | curse | incantation)"
            )));
        }
    }
    Ok(())
}
/// LANG-1 P6.1 — descriptive language profile: inventory balance, phoneme
/// frequency, syllable-length distribution, onset/coda usage, POS spread.
pub(crate) fn stats(project: &Path, language: &str, json: bool) -> Result<()> {
    use crate::conlang::analysis;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let prof = analysis::profile(&phon, &entries);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&prof)
                .map_err(|e| Error::Store(format!("serializing profile: {e}")))?
        );
        return Ok(());
    }

    // "k×12 a×9 …" for the first `n` ranked entries.
    let top = |freq: &[(String, usize)], n: usize| {
        freq.iter()
            .take(n)
            .map(|(k, c)| format!("{k}×{c}"))
            .collect::<Vec<_>>()
            .join("  ")
    };

    println!("language profile · {language}");
    println!(
        "  inventory · {} phonemes ({} C / {} V)",
        prof.phoneme_inventory, prof.consonants, prof.vowels
    );
    println!(
        "  lexicon   · {} entr(y/ies), {} analyzable",
        prof.word_count, prof.analyzable_words
    );
    if prof.analyzable_words > 0 {
        println!(
            "  shape     · avg {:.1} phonemes, {:.1} syllables per word",
            prof.avg_phonemes, prof.avg_syllables
        );
        if !prof.syllable_hist.is_empty() {
            let max = prof.syllable_hist.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
            println!("  syllables ·");
            for (n, c) in &prof.syllable_hist {
                let bar = "█".repeat(((*c * 24) / max).max(1));
                println!("      {n}σ {bar} {c}");
            }
        }
        println!("  phonemes  · {}", top(&prof.phoneme_freq, 10));
        if !prof.onset_freq.is_empty() {
            println!("  onsets    · {}", top(&prof.onset_freq, 8));
        }
        if !prof.coda_freq.is_empty() {
            println!("  codas     · {}", top(&prof.coda_freq, 8));
        }
    }
    if !prof.pos_freq.is_empty() {
        println!("  parts of speech · {}", top(&prof.pos_freq, 8));
    }
    Ok(())
}
/// LANG-1 P2.1 — deterministic lexicon audit: phonotactic violations,
/// homophones (surface-form collisions), and duplicate meanings.
pub(crate) fn audit(project: &Path, language: &str, json: bool) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    // Phonology is optional — a dictionary-only language still audits for
    // homophones + duplicate meanings, just without the phonotactic check.
    let phonology = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let report = crate::conlang::lexicon::analyze(&phonology, &entries);

    if json {
        println!("{}", serde_json::to_string_pretty(&report).map_err(|e| {
            Error::Store(format!("serializing lexicon report: {e}"))
        })?);
        return Ok(());
    }

    println!("lexicon audit · {language} · {} entr(y/ies)", report.total);
    if report.issue_count() == 0 {
        println!("  ✓ no issues");
        return Ok(());
    }
    if !report.phonotactic_violations.is_empty() {
        println!("\n  ⚠ phonotactic violations ({}):", report.phonotactic_violations.len());
        for v in &report.phonotactic_violations {
            println!("      {} (/{}/) breaks the language's constraints", v.headword, v.underlying);
        }
    }
    if !report.homophones.is_empty() {
        println!("\n  ⚠ homophones ({} group(s)):", report.homophones.len());
        for c in &report.homophones {
            let m = c.members.iter().map(|m| format!("{} ({})", m.headword, m.gloss)).collect::<Vec<_>>();
            println!("      [{}] {}", c.key, m.join(", "));
        }
    }
    if !report.duplicate_meanings.is_empty() {
        println!("\n  ⚠ duplicate meanings ({} group(s)):", report.duplicate_meanings.len());
        for c in &report.duplicate_meanings {
            let m = c.members.iter().map(|m| m.headword.clone()).collect::<Vec<_>>();
            println!("      \"{}\" — {}", c.key, m.join(", "));
        }
    }
    Ok(())
}
/// 1.2.16+ Phase P.5 — `inkhaven language
/// define-rule <language> <rule_id> [--category
/// grammar|phonology]`.  Opens the rule's HJSON
/// template in `$EDITOR` (fallback `vi`); on the
/// editor's exit, writes the saved content into
/// a new or existing rule paragraph under the
/// chosen category.
pub(crate) fn define_rule(
    project: &Path,
    language: &str,
    rule_id: &str,
    category: &str,
) -> Result<()> {
    let category_norm = category.to_lowercase();
    if category_norm != "grammar" && category_norm != "phonology" {
        return Err(Error::Config(format!(
            "--category must be `grammar` or `phonology` (got `{category}`)"
        )));
    }
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    use crate::store::node::NodeKind;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it".into(),
            )
        })?;
    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!("language `{language}` not found"))
        })?;
    let category_chapter = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.title.eq_ignore_ascii_case(&category_norm))
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "`{category_norm}` chapter not found under language `{language}` — \
                 was it scaffolded? Try `inkhaven language init {language}`"
            ))
        })?;

    // Find existing paragraph by slug match, OR
    // build the seed template.
    let existing = hierarchy
        .collect_subtree(category_chapter.id)
        .into_iter()
        .filter_map(|id| hierarchy.get(id).cloned())
        .find(|n| {
            n.kind == NodeKind::Paragraph
                && n.slug.eq_ignore_ascii_case(rule_id)
        });

    let seed = if let Some(node) = &existing {
        match store.get_content(node.id) {
            Ok(Some(b)) => String::from_utf8_lossy(&b).into_owned(),
            _ => String::new(),
        }
    } else {
        rule_template(rule_id, &category_norm)
    };

    // Open in $EDITOR.
    let edited = open_in_editor(&seed, &format!("{rule_id}-{category_norm}"))?;

    // Roundtrip: persist back into the paragraph.
    if let Some(node) = existing {
        let mut n = node;
        store
            .update_paragraph_content(&mut n, edited.as_bytes())
            .map_err(|e| Error::Store(format!("save rule: {e}")))?;
        if let Some(rel) = &n.file {
            crate::io_atomic::write(&store.project_root().join(rel), edited.as_bytes())
                .map_err(Error::Io)?;
        }
        eprintln!("updated rule `{rule_id}` under {category_norm}");
    } else {
        let mut created = store
            .create_node(
                &cfg,
                &hierarchy,
                NodeKind::Paragraph,
                rule_id,
                Some(&category_chapter),
                None,
                crate::store::InsertPosition::End,
            )
            .map_err(|e| Error::Store(format!("create rule paragraph: {e}")))?;
        if let Some(rel) = &created.file {
            crate::io_atomic::write(
                &store.project_root().join(rel),
                edited.as_bytes(),
            )
            .map_err(Error::Io)?;
            store
                .update_paragraph_content(&mut created, edited.as_bytes())
                .map_err(|e| Error::Store(format!("save rule: {e}")))?;
        }
        eprintln!("created rule `{rule_id}` under {category_norm}");
    }

    Ok(())
}

pub(crate) fn rule_template(rule_id: &str, category: &str) -> String {
    // Mirrors the seed template used by the
    // tree-pane scaffolders in
    // `src/tui/app/threads_impl.rs` for the
    // Grammar / Phonology categories.
    let cat_examples = if category == "grammar" {
        "[\n    \"example 1 in invented language — translation\",\n    \"example 2 — translation\"\n  ]"
    } else {
        "[\n    \"phoneme example 1\",\n    \"phoneme example 2\"\n  ]"
    };
    format!(
        "{{\n  rule_id: \"{rule_id}\"\n  category: \"\"\n  rule: \"\"\n  examples: {cat_examples}\n  applies_when: \"\"\n  depends_on: []\n}}\n"
    )
}

/// Open `seed` in `$EDITOR`; return the saved
/// content.  Falls back to `vi` on Linux/macOS or
/// `notepad` on Windows.  Errors when the editor
/// process exits non-zero.
pub(crate) fn open_in_editor(seed: &str, label: &str) -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
        if cfg!(windows) {
            "notepad".into()
        } else {
            "vi".into()
        }
    });
    // Write seed to a temp file the editor edits
    // in place.  The temp file path is just under
    // the OS temp dir + a process-id prefix; the
    // editor handles its own atomic save on exit.
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!(
        "inkhaven-define-rule-{}-{}.hjson",
        std::process::id(),
        label
    ));
    std::fs::write(&tmp_path, seed.as_bytes()).map_err(Error::Io)?;
    let status = std::process::Command::new(&editor)
        .arg(&tmp_path)
        .status()
        .map_err(|e| Error::Config(format!("spawn `{editor}`: {e}")))?;
    if !status.success() {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(Error::Config(format!(
            "editor `{editor}` exited with status {status}"
        )));
    }
    let body = std::fs::read_to_string(&tmp_path).map_err(Error::Io)?;
    let _ = std::fs::remove_file(&tmp_path);
    Ok(body)
}
