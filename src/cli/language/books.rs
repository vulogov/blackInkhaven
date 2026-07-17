//! `inkhaven language` book-generation surface: the printable dictionary, the
//! grammar book, the tutorial/study companion, and the sample-text + example
//! helpers they share. Split out of the flat handler; the AI prompt consts and
//! loaders live in the parent module.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::store::Store;

use super::*;

/// Load `(title, body)` pairs from a language's `Sample texts` chapter.
pub(crate) fn load_samples(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<Vec<(String, String)>> {
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Sample texts"))
        .cloned()
    else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for para in hierarchy.children_of(Some(chapter.id)) {
        if para.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(para.id) else { continue };
        let body = String::from_utf8_lossy(&bytes).trim().to_string();
        if !body.is_empty() {
            out.push((para.title.clone(), body));
        }
    }
    Ok(out)
}

/// LANG-1 P6.3 — render the grammar as a Markdown or Typst document.
/// A facts-only brief of the grammatical features that need explaining, for the
/// AI study guide.
pub(crate) fn grammar_study_brief(
    language: &str,
    phon: &crate::conlang::Phonology,
    morph: &Option<crate::conlang::types::morphology::Morphology>,
    typology: &std::collections::BTreeMap<String, String>,
) -> String {
    use std::fmt::Write as _;
    let mut b = String::new();
    let _ = writeln!(b, "LANGUAGE: {language}");
    let _ = writeln!(
        b,
        "PHONEME INVENTORY: {} consonants, {} vowels",
        phon.phonemes.iter().filter(|p| matches!(p.kind, crate::conlang::types::phoneme::PhonemeKind::Consonant)).count(),
        phon.phonemes.iter().filter(|p| matches!(p.kind, crate::conlang::types::phoneme::PhonemeKind::Vowel)).count(),
    );
    if let Some(st) = &phon.stress {
        let _ = writeln!(b, "STRESS RULE: {:?}", st.primary);
    }
    if !phon.allophony.is_empty() {
        let _ = writeln!(b, "ALLOPHONY / SOUND-CHANGE RULES (SPE notation):");
        for r in &phon.allophony {
            let _ = writeln!(b, "  {}", r.source);
        }
    }
    if let Some(m) = morph {
        if !m.morphemes.is_empty() {
            let _ = writeln!(b, "MORPHEMES (these realise grammatical categories — gloss | kind | realization | category | value):");
            for mo in &m.morphemes {
                let _ = writeln!(
                    b,
                    "  {} | {} | {} | {} | {}",
                    mo.gloss,
                    crate::conlang::output::morpheme_kind(mo),
                    crate::conlang::output::morpheme_realization(mo),
                    mo.category,
                    mo.value
                );
            }
        }
        if !m.derivations.is_empty() {
            let _ = writeln!(b, "WORD-BUILDING (derivation) RULES (name | from POS | to POS):");
            for d in &m.derivations {
                let _ = writeln!(b, "  {} | {} | {}", d.name, d.from_pos.as_deref().unwrap_or("any"), d.to_pos);
            }
        }
        if !m.agreement.is_empty() {
            let _ = writeln!(b, "AGREEMENT RULES (dependent agrees with head in features):");
            for a in &m.agreement {
                let _ = writeln!(b, "  {} agrees with {} in {}", a.dependent, a.head, a.features.join(", "));
            }
        }
    }
    if !typology.is_empty() {
        let _ = writeln!(b, "TYPOLOGICAL FEATURES (WALS-style feature = value):");
        for (k, v) in typology {
            let _ = writeln!(b, "  {k} = {v}");
        }
    }
    b
}

#[allow(clippy::too_many_arguments)]
/// Build an example clause from the lexicon (first noun = subject, first verb,
/// second noun = object) via the syntax engine, returned as `(surface,
/// interlinear, literal)`. `None` when there isn't at least a noun and a verb.
pub(crate) fn build_example_sentence(
    phon: &crate::conlang::Phonology,
    morph: &crate::conlang::types::morphology::Morphology,
    typology: &std::collections::BTreeMap<String, String>,
    entries: &[crate::language_entry::DictionaryEntry],
) -> Option<(String, String, String)> {
    use crate::conlang::syntax::{self, Clause, NounPhrase, Word};
    let nouns: Vec<&crate::language_entry::DictionaryEntry> =
        entries.iter().filter(|e| e.pos.eq_ignore_ascii_case("noun")).collect();
    let verb = entries.iter().find(|e| e.pos.eq_ignore_ascii_case("verb"))?;
    let subject = nouns.first()?;
    let w = |e: &crate::language_entry::DictionaryEntry| Word {
        root: e.word.clone(),
        gloss: e.translation.clone(),
    };
    let clause = Clause {
        subject: Some(NounPhrase { head: w(subject), number: "sg".into(), adjective: None }),
        verb: Some(w(verb)),
        verb_person: "3".into(),
        object: nouns.get(1).map(|o| NounPhrase { head: w(o), number: "sg".into(), adjective: None }),
        noun_paradigm: "noun".into(),
        verb_paradigm: "verb".into(),
        ..Default::default()
    };
    let r = syntax::assemble(phon, morph, typology, &clause);
    if r.words.is_empty() {
        return None;
    }
    let widths: Vec<usize> =
        r.words.iter().map(|(w, g)| w.chars().count().max(g.chars().count()) + 2).collect();
    let mut l1 = String::new();
    let mut l2 = String::new();
    for (i, (w, g)) in r.words.iter().enumerate() {
        l1.push_str(&format!("{:<width$}", w, width = widths[i]));
        l2.push_str(&format!("{:<width$}", g, width = widths[i]));
    }
    let interlinear = format!("{}\n{}", l1.trim_end(), l2.trim_end());
    Some((r.surface, interlinear, r.literal))
}

pub(crate) fn grammar_book(
    project: &Path,
    language: &str,
    format: &str,
    out: Option<&Path>,
    font: Option<&str>,
    study: bool,
    provider: Option<&str>,
) -> Result<()> {
    use crate::conlang::output::{self, GrammarBook};
    use crate::conlang::analysis;

    let typst = match format.to_ascii_lowercase().as_str() {
        "md" | "markdown" => false,
        "typ" | "typst" => true,
        other => {
            return Err(Error::Config(format!("unknown --format `{other}` (expected md or typ)")))
        }
    };

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let morphology = load_morphology(&store, &hierarchy, &lang_book)?;
    let (grammar_spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let (expressions, _) = load_expressions(&store, &hierarchy, &lang_book)?;
    let samples = load_samples(&store, &hierarchy, &lang_book)?;
    let font_cfg = load_font_config(&store, &hierarchy, &lang_book)?;
    let profile = analysis::profile(&phon, &entries);

    let family = font
        .map(str::to_string)
        .or_else(|| font_cfg.as_ref().and_then(|c| c.family.clone()));
    let has_expr = !expressions.idioms.is_empty() || !expressions.metaphors.is_empty();

    // The optional AI study guide explains the linguistic terms the reference
    // uses. Raw Markdown for the md path; converted Typst for the typ path.
    let study_doc: Option<String> = if study {
        let brief = grammar_study_brief(&lang_book.title, &phon, &morphology, &grammar_spec.grammar);
        let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
        let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
        let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
        eprintln!("inkhaven language grammar-book · study guide · {} · model: {model}", lang_book.title);
        let raw = crate::ai::stream::collect_blocking(
            ai.client.clone(),
            model.to_string(),
            Some(GRAMMAR_STUDY_SYSTEM.to_string()),
            format!(
                "Write the study guide for this language, using ONLY the features in the \
                 brief below.\n\n{brief}\n\nOUTPUT FORMAT: GitHub-flavored Markdown — use `##` \
                 for sections and `###` for each term you define. Output the guide only."
            ),
        )
        .map_err(|e| Error::Store(format!("inference error: {e}")))?;
        let md = strip_code_fence(&raw);
        if md.trim().is_empty() {
            None
        } else if typst {
            Some(output::markdown_to_typst(&md))
        } else {
            Some(md)
        }
    } else {
        None
    };

    // An example sentence, assembled from the lexicon by the syntax engine —
    // a subject noun, a verb, and (if there is a second noun) an object.
    let example_sentence = morphology
        .as_ref()
        .and_then(|m| build_example_sentence(&phon, m, &grammar_spec.grammar, &entries));
    let variation = build_variation(&store, &hierarchy, &lang_book, &phon, &entries, 12)?;

    let book = GrammarBook {
        language: &lang_book.title,
        font_family: if typst { family.as_deref() } else { None },
        profile: &profile,
        phonology: &phon,
        morphology: morphology.as_ref(),
        typology: &grammar_spec.grammar,
        expressions: has_expr.then_some(&expressions),
        samples: &samples,
        study: study_doc.as_deref(),
        example_sentence,
        variation,
    };
    let doc = if typst {
        output::grammar_typst(&book)
    } else {
        output::grammar_markdown(&book)
    };

    if let Some(p) = out {
        crate::io_atomic::write(p, doc.as_bytes()).map_err(Error::Io)?;
        println!("{} grammar ({}) → {}", lang_book.title, format, p.display());
        if typst && book.font_family.is_some() {
            eprintln!(
                "(build the font with `font-build --language {language} --format ttf` and compile \
                 with `typst compile --font-path <dir> {}`)",
                p.display()
            );
        }
    } else {
        print!("{doc}");
    }
    Ok(())
}

/// Strip a leading/trailing markdown code fence (```lang … ```) if the model
/// wrapped its whole reply in one.
pub(crate) fn strip_code_fence(text: &str) -> String {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix("```") {
        // drop the rest of the opening fence line, then the trailing fence.
        if let Some(nl) = rest.find('\n') {
            let body = &rest[nl + 1..];
            if let Some(end) = body.rfind("```") {
                return body[..end].trim_end().to_string();
            }
        }
    }
    t.to_string()
}

/// LANG-1 P7 — an AI-authored learner tutorial. The model writes a complete
/// graded textbook from the language's own data (the prose is never hardcoded);
/// for Typst, a deterministic scaffold (page setup + font embedding + helpers)
/// is prepended so the result always compiles and embeds the conscript font.
pub(crate) fn tutorial(
    project: &Path,
    language: &str,
    format: &str,
    out: Option<&Path>,
    font: Option<&str>,
    provider: Option<&str>,
) -> Result<()> {
    use crate::conlang::output;
    use crate::conlang::types::phoneme::PhonemeKind;
    use crate::conlang::{morphology, writing::input};
    use std::fmt::Write as _;

    let typst = match format.to_ascii_lowercase().as_str() {
        "md" | "markdown" => false,
        "typ" | "typst" => true,
        other => {
            return Err(Error::Config(format!("unknown --format `{other}` (expected md or typ)")))
        }
    };

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let morph = load_morphology(&store, &hierarchy, &lang_book)?;
    let (grammar_spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let (expressions, _) = load_expressions(&store, &hierarchy, &lang_book)?;
    let samples = load_samples(&store, &hierarchy, &lang_book)?;
    let font_cfg = load_font_config(&store, &hierarchy, &lang_book)?;

    if entries.is_empty() {
        return Err(Error::Config(format!(
            "language `{language}` has no dictionary entries to teach"
        )));
    }

    let family = font
        .map(str::to_string)
        .or_else(|| font_cfg.as_ref().and_then(|c| c.family.clone()));

    // ── Build the language brief (facts only; the AI writes the prose) ──────
    let mut brief = String::new();
    let _ = writeln!(brief, "LANGUAGE: {}", lang_book.title);

    let consonants: Vec<String> =
        phon.phonemes.iter().filter(|p| p.kind == PhonemeKind::Consonant).map(|p| p.ipa.clone()).collect();
    let vowels: Vec<String> =
        phon.phonemes.iter().filter(|p| p.kind == PhonemeKind::Vowel).map(|p| p.ipa.clone()).collect();
    if !consonants.is_empty() {
        let _ = writeln!(brief, "CONSONANTS: {}", consonants.join(" "));
    }
    if !vowels.is_empty() {
        let _ = writeln!(brief, "VOWELS: {}", vowels.join(" "));
    }
    if let Some(st) = &phon.stress {
        let _ = writeln!(brief, "STRESS: {:?}", st.primary);
    }
    if !phon.allophony.is_empty() {
        let _ = writeln!(brief, "SOUND CHANGES (notation `X > Y / context`, _ = the changing sound):");
        for r in &phon.allophony {
            let _ = writeln!(brief, "  {}", r.source);
        }
    }

    let _ = writeln!(brief, "\nVOCABULARY (word | part-of-speech | meaning | pronunciation):");
    for e in &entries {
        let pron = pronounce(&phon, &e.word).unwrap_or_default();
        let _ = writeln!(
            brief,
            "  {} | {} | {} | {}",
            e.word,
            if e.pos.is_empty() { "?" } else { &e.pos },
            e.translation,
            pron
        );
    }

    if let Some(m) = &morph {
        if !m.morphemes.is_empty() {
            let _ = writeln!(brief, "\nMORPHEMES (gloss | kind | realization | meaning):");
            for mo in &m.morphemes {
                let _ = writeln!(
                    brief,
                    "  {} | {} | {} | {}",
                    mo.gloss,
                    crate::conlang::output::morpheme_kind(mo),
                    crate::conlang::output::morpheme_realization(mo),
                    mo.value
                );
            }
        }
        if !m.derivations.is_empty() {
            let _ = writeln!(brief, "\nWORD-BUILDING RULES (name | from part-of-speech | to part-of-speech | suffix | meaning):");
            for d in &m.derivations {
                let _ = writeln!(
                    brief,
                    "  {} | {} | {} | {} | {}",
                    d.name,
                    d.from_pos.as_deref().unwrap_or("any"),
                    d.to_pos,
                    d.form,
                    d.gloss
                );
            }
        }
        if !m.agreement.is_empty() {
            let _ = writeln!(brief, "\nAGREEMENT (dependent agrees with head in features):");
            for a in &m.agreement {
                let _ = writeln!(brief, "  {} agrees with {} in {}", a.dependent, a.head, a.features.join(", "));
            }
        }
    }

    if !grammar_spec.grammar.is_empty() {
        let _ = writeln!(brief, "\nGRAMMAR (typological features):");
        for (k, v) in &grammar_spec.grammar {
            let _ = writeln!(brief, "  {} = {}", k, v);
        }
    }

    if !expressions.idioms.is_empty() {
        let _ = writeln!(brief, "\nIDIOMS (phrase | literal | meaning):");
        for i in &expressions.idioms {
            let _ = writeln!(brief, "  {} | {} | {}", i.form, i.literal, i.meaning);
        }
    }

    if !samples.is_empty() {
        let _ = writeln!(brief, "\nSAMPLE TEXTS (use these for reading passages; word-by-word gloss follows each):");
        for (title, body) in &samples {
            let glossable: String = body
                .chars()
                .map(|c| if matches!(c, '.' | ',' | '!' | '?' | ';' | ':') { ' ' } else { c })
                .collect();
            let gloss = morph
                .as_ref()
                .map(|m| {
                    let index = morphology::gloss::build_index(&phon, m, &entries);
                    index
                        .gloss_text(&glossable)
                        .iter()
                        .map(|it| format!("{}={}", it.surface, it.gloss.clone().unwrap_or_else(|| "?".into())))
                        .collect::<Vec<_>>()
                        .join("  ")
                })
                .unwrap_or_default();
            let _ = writeln!(brief, "  [{title}] {}", body.trim());
            if !gloss.is_empty() {
                let _ = writeln!(brief, "    gloss: {gloss}");
            }
        }
    }
    // ── The AI authors the textbook ─────────────────────────────────────────
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    eprintln!("inkhaven language tutorial · {} · model: {model}", lang_book.title);

    // The AI always authors Markdown (which it does reliably); the Typst path
    // converts that deterministically, so the document always compiles.
    let format_rules = "OUTPUT FORMAT: GitHub-flavored Markdown. Use `#` for the book title, \
         `##` for each lesson, `###` for subsections, Markdown tables for vocabulary, \
         `-` for bullet lists, and `>` blockquotes for the practice exercises. Output the \
         textbook only — no commentary before or after.";

    let prompt = format!(
        "Write a complete beginner's textbook that teaches a newcomer to read this \
         constructed language, using ONLY the facts in the brief below.\n\n{brief}\n\n{format_rules}"
    );
    let raw = crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(TUTORIAL_SYSTEM.to_string()),
        prompt,
    )
    .map_err(|e| Error::Store(format!("inference error: {e}")))?;
    let body = strip_code_fence(&raw);
    if body.trim().is_empty() {
        return Err(Error::Store("the model returned an empty tutorial".into()));
    }

    // ── Assemble: Typst gets the Markdown converted + the scaffold prepended ─
    let doc = if typst {
        let cover = samples
            .first()
            .and_then(|(_, b)| font_cfg.as_ref().map(|c| input::to_script(c, b)))
            .filter(|o| o.mapped > 0)
            .map(|o| o.script);
        let scaffold = output::tutorial_typst_scaffold(&lang_book.title, family.as_deref(), cover.as_deref());
        let converted = output::markdown_to_typst(&body);
        format!("{scaffold}{converted}\n")
    } else {
        format!("{body}\n")
    };

    if let Some(p) = out {
        crate::io_atomic::write(p, doc.as_bytes()).map_err(Error::Io)?;
        println!("{} tutorial ({}) → {}", lang_book.title, format, p.display());
        if typst && family.is_some() {
            eprintln!(
                "(build the font with `font-build --language {language} --format ttf` and compile \
                 with `typst compile --font-path <dir> {}`)",
                p.display()
            );
        }
    } else {
        print!("{doc}");
    }
    Ok(())
}

/// LANG-1 P6.2 — render the dictionary as a Markdown or Typst document.
pub(crate) fn dictionary(
    project: &Path,
    language: &str,
    format: &str,
    out: Option<&Path>,
    font: Option<&str>,
) -> Result<()> {
    use crate::conlang::output::{self, DictMeta, RenderEntry};
    use crate::conlang::{analysis, writing::input};

    let typst = match format.to_ascii_lowercase().as_str() {
        "md" | "markdown" => false,
        "typ" | "typst" => true,
        other => {
            return Err(Error::Config(format!("unknown --format `{other}` (expected md or typ)")))
        }
    };

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let font_cfg = load_font_config(&store, &hierarchy, &lang_book)?;
    let profile = analysis::profile(&phon, &entries);

    // Font family: --font override > the `font` block's family. The conscript
    // form needs glyph bindings to transliterate against.
    let family = font
        .map(str::to_string)
        .or_else(|| font_cfg.as_ref().and_then(|c| c.family.clone()));
    let can_transliterate = font_cfg.as_ref().is_some_and(|c| !c.glyphs.is_empty());

    let rendered: Vec<RenderEntry> = entries
        .iter()
        .map(|e| {
            let conscript = match (&font_cfg, can_transliterate) {
                (Some(cfg), true) => {
                    let out = input::to_script(cfg, &e.word);
                    (out.mapped > 0).then_some(out.script)
                }
                _ => None,
            };
            RenderEntry {
                headword: e.word.clone(),
                conscript,
                pronunciation: pronounce(&phon, &e.word),
                pos: e.pos.clone(),
                gloss: e.translation.clone(),
                registers: e.registers.clone(),
                domain: e.domain.clone(),
                era: e.era.clone(),
                etymology: e.etymology.clone(),
                example: (!e.example.trim().is_empty()).then(|| e.example.clone()),
            }
        })
        .collect();

    let meta = DictMeta {
        language: &lang_book.title,
        font_family: if typst { family.as_deref() } else { None },
        profile: Some(&profile),
    };
    let doc = if typst {
        output::dictionary_typst(&meta, &rendered)
    } else {
        output::dictionary_markdown(&meta, &rendered)
    };

    if let Some(p) = out {
        crate::io_atomic::write(p, doc.as_bytes()).map_err(Error::Io)?;
        println!("{} dictionary ({}) → {}", lang_book.title, format, p.display());
        if typst && meta.font_family.is_some() {
            eprintln!(
                "(build the font with `font-build --language {language} --format ttf` and compile \
                 with `typst compile --font-path <dir> {}`)",
                p.display()
            );
        }
    } else {
        print!("{doc}");
    }
    Ok(())
}
