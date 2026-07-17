//! `inkhaven language` morphology & grammar surface: the typology/grammar-spec
//! block, the grammar questionnaire, paradigm generation, derivation, and glossing
//! (plus the chapter-paragraph upsert helper). Split out of the flat handler.

use std::path::Path;

use crate::error::{Error, Result};

use super::*;

/// LING-1 L-P6 — `inkhaven language check <lang> --word W`: the Oracle. Judge a
/// candidate word for well-formedness by level (phonotactics, morphology).
pub(crate) fn oracle_check(project: &Path, language: &str, word: &str, json: bool) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let report = crate::conlang::oracle::check_word(&phon, &morph, &entries, word);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| Error::Store(format!("serializing oracle: {e}")))?
        );
        return Ok(());
    }

    println!("oracle · {language} · {word}");
    if report.ok() {
        println!("  ✓ a well-formed word of the language.");
    } else {
        for f in &report.findings {
            println!("      ✗ [{}] {}", f.level, f.message);
        }
    }
    Ok(())
}

/// LING-1 L-P5 — `inkhaven language link <lang> --verb V --args "a,b,c"`: work
/// out a clause's argument structure — thematic roles, RRG macroroles, and
/// grammatical relations — from the verb's valence.
pub(crate) fn link_args(
    project: &Path,
    language: &str,
    verb: &str,
    args_csv: &str,
    valence: Option<&str>,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;

    // Resolve the valence: explicit flag, else the declared verb class, else "" so
    // the linker infers it from the argument count.
    let resolved = valence.map(str::to_string).unwrap_or_else(|| {
        spec.verb_classes
            .iter()
            .find(|vc| vc.name.eq_ignore_ascii_case(verb))
            .map(|vc| vc.valence.clone())
            .unwrap_or_default()
    });
    let args: Vec<String> =
        args_csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

    let report = crate::conlang::link::link(verb, &resolved, &args);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| Error::Store(format!("serializing link: {e}")))?
        );
        return Ok(());
    }

    let val = if report.valence.trim().is_empty() { "(inferred)".to_string() } else { report.valence.clone() };
    println!("argument linking · {language} · {verb} [{val}]");
    for a in &report.args {
        println!("      {:<10} {:<10} {:<13} {}", a.arg, a.theta_role, a.macrorole, a.relation);
    }
    for i in &report.issues {
        println!("  ⚠ {i}");
    }
    Ok(())
}

/// LING-1 L-P5 — `inkhaven language parse <lang> --word W`: analyse a surface
/// word into root + affixes by reversing the morphology (the morphological
/// parser).
pub(crate) fn parse_surface(project: &Path, language: &str, word: &str, json: bool) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let report = crate::conlang::parse::parse(&phon, &morph, &entries, word);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| Error::Store(format!("serializing parse: {e}")))?
        );
        return Ok(());
    }

    println!("parse · {language} · {word}");
    if report.parses.is_empty() {
        println!("  no analysis — no root + affix combination reaches a dictionary word.");
        return Ok(());
    }
    for p in &report.parses {
        if p.affixes.is_empty() {
            println!("      {} ‘{}’  (bare root)", p.root, p.gloss);
        } else {
            println!("      {} ‘{}’ + {}", p.root, p.gloss, p.affixes.join(" + "));
        }
    }
    Ok(())
}

/// Load the `{ grammar: { … } }` typology block from the Grammar chapter,
/// returning the spec + the paragraph node that holds it (for in-place edits).
pub(crate) fn load_grammar_spec(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<(crate::conlang::types::grammar::GrammarSpec, Option<crate::store::node::Node>)> {
    use crate::conlang::types::grammar::GrammarSpec;
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Grammar"))
        .cloned()
    else {
        return Ok((GrammarSpec::default(), None));
    };
    for para in hierarchy.children_of(Some(chapter.id)) {
        if para.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(para.id) else { continue };
        if let Ok(Some(spec)) = GrammarSpec::from_hjson(&String::from_utf8_lossy(&bytes)) {
            return Ok((spec, Some(para.clone())));
        }
    }
    Ok((GrammarSpec::default(), None))
}

/// LANG-1 P3.4 — the grammar typological questionnaire.
pub(crate) fn grammar_questionnaire(
    project: &Path,
    language: &str,
    set: Option<&str>,
    json: bool,
) -> Result<()> {
    use crate::conlang::grammar;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (mut spec, node) = load_grammar_spec(&store, &hierarchy, &lang_book)?;

    if let Some(kv) = set {
        let (feat, val) = kv
            .split_once('=')
            .ok_or_else(|| Error::Config("use --set <feature>=<value>".into()))?;
        let f = grammar::feature(feat.trim()).ok_or_else(|| {
            Error::Config(format!("unknown feature `{}` — run `language grammar` to list them", feat.trim()))
        })?;
        let val = val.trim();
        if !f.is_valid(val) {
            return Err(Error::Config(format!(
                "`{val}` is not a valid value for `{}` — options: {}",
                f.id,
                f.values()
            )));
        }
        spec.grammar.insert(f.id.to_string(), val.to_lowercase());
        let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
        let body = serde_json::to_string_pretty(&spec)
            .map_err(|e| Error::Store(format!("serializing grammar: {e}")))?;
        upsert_grammar_paragraph(&store, &cfg, &lang_book, "typology", node, &body)?;
        eprintln!("{language}: set {} = {}", f.id, val.to_lowercase());
        return Ok(());
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&spec.grammar)
                .map_err(|e| Error::Store(format!("serializing grammar: {e}")))?
        );
        return Ok(());
    }

    let total = grammar::catalog().len();
    let answered = grammar::catalog().iter().filter(|f| spec.grammar.contains_key(f.id)).count();
    println!("grammar · {language} · {answered}/{total} feature(s) set\n");
    for f in grammar::catalog() {
        match spec.grammar.get(f.id) {
            Some(v) => println!("  ✓ {:<16} {}", f.id, v),
            None => println!("  · {:<16} {}", f.id, f.question),
        }
    }
    eprintln!("\nset an answer: inkhaven language grammar {language} --set <feature>=<value>");
    eprintln!("(see the options for a feature in `Documentation/CONLANG.md` or `--help`)");
    Ok(())
}

/// Create-or-update a named pure-HJSON paragraph under the Grammar chapter
/// (the home for the typology + expressions blocks). Reused by the grammar
/// questionnaire and the idioms/metaphors commands.
pub(crate) fn upsert_grammar_paragraph(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    para_title: &str,
    node: Option<crate::store::node::Node>,
    body: &str,
) -> Result<()> {
    upsert_chapter_paragraph(store, cfg, lang_book, "Grammar", para_title, node, body)
}

/// Create-or-update an HJSON paragraph in a named chapter of a language book.
/// When `node` is `None`, a new paragraph is created at the end of `chapter`.
pub(crate) fn upsert_chapter_paragraph(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    chapter: &str,
    para_title: &str,
    node: Option<crate::store::node::Node>,
    body: &str,
) -> Result<()> {
    let mut target = match node {
        Some(n) => n,
        None => {
            let hierarchy = Hierarchy::load(store)?;
            let chapter = hierarchy
                .children_of(Some(lang_book.id))
                .into_iter()
                .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case(chapter))
                .cloned()
                .ok_or_else(|| {
                    Error::Config(format!("no {chapter} chapter to store the block in"))
                })?;
            store.create_node(
                cfg,
                &hierarchy,
                NodeKind::Paragraph,
                para_title,
                Some(&chapter),
                None,
                InsertPosition::End,
            )?
        }
    };
    target.content_type = Some("hjson".to_string());
    if let Some(rel) = &target.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes())
            .map_err(|e| Error::Store(format!("write {para_title}: {e}")))?;
    }
    store
        .update_paragraph_content(&mut target, body.as_bytes())
        .map_err(|e| Error::Store(format!("update {para_title}: {e}")))?;
    Ok(())
}

/// LANG-1 P3.3 — propose (and optionally commit) derived lexemes for a root.
pub(crate) fn derive(
    project: &Path,
    language: &str,
    root: &str,
    gloss: Option<&str>,
    pos: Option<&str>,
    yes: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no morphology — add `derivations` HJSON under its `Grammar` chapter"
        ))
    })?;
    if morph.derivations.is_empty() {
        return Err(Error::Config(format!(
            "language `{language}` declares no derivation rules"
        )));
    }

    let root_gloss = gloss.unwrap_or(root);
    let root_pos = pos.unwrap_or("");
    let derived =
        crate::conlang::morphology::derive::generate(&phon, &morph, root, root_gloss, root_pos);
    if derived.is_empty() {
        eprintln!(
            "no derivation rules apply to a `{}` root",
            if root_pos.is_empty() { "(unspecified pos)" } else { root_pos }
        );
        return Ok(());
    }

    println!("derivations of {root} ({root_gloss}):");
    for d in &derived {
        let pos = if d.pos.is_empty() { String::new() } else { format!("  {}", d.pos) };
        println!("  {:<18} {:<26} [{}]{}", d.form, d.gloss, d.rule, pos);
    }

    if yes {
        let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
        let mut added = 0usize;
        for d in &derived {
            let entry = ImportEntry {
                word: d.form.clone(),
                pos: d.pos.clone(),
                translation: d.gloss.clone(),
                etymology: format!("derived from {root} via {}", d.rule),
                ..Default::default()
            };
            match add_imported_dictionary_entry(&store, &cfg, &lang_book, &entry) {
                Ok(_) => added += 1,
                Err(e) => eprintln!("  skipped {}: {e}", d.form),
            }
        }
        eprintln!("\nadded {added} derived entr(y/ies) to {language}'s Dictionary");
    } else {
        eprintln!("\n(dry run — re-run with --yes to add the {} derived form(s))", derived.len());
    }
    Ok(())
}

/// LANG-1 P3.2 — interlinear auto-gloss of conlang text.
pub(crate) fn gloss_text(project: &Path, language: &str, text: &str) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    // Phonology + morphology are optional: without them only bare forms gloss.
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;

    let index = crate::conlang::morphology::gloss::build_index(&phon, &morph, &entries);
    let items = index.gloss_text(text);
    if items.is_empty() {
        return Ok(());
    }

    // Two aligned lines: the surface words over their glosses (Leipzig style).
    let mut top = String::new();
    let mut bot = String::new();
    let mut matched = 0usize;
    for item in &items {
        let g = item.gloss.clone().unwrap_or_else(|| "?".to_string());
        if item.gloss.is_some() {
            matched += 1;
        }
        let w = item.surface.chars().count();
        let gw = g.chars().count();
        let width = w.max(gw) + 2;
        top.push_str(&format!("{:<width$}", item.surface, width = width));
        bot.push_str(&format!("{:<width$}", g, width = width));
    }
    println!("{}", top.trim_end());
    println!("{}", bot.trim_end());
    eprintln!("\n{matched} / {} word(s) glossed", items.len());
    Ok(())
}

/// LANG-1 P3.1 — generate + print a root's paradigm.
pub(crate) fn paradigm(
    project: &Path,
    language: &str,
    root: &str,
    template: &str,
    gloss: Option<&str>,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phonology = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no phoneme block"))
    })?;
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no morphology yet — add a `morphemes` / `paradigms` HJSON \
             paragraph under its `Grammar` chapter"
        ))
    })?;
    let tmpl = morph.paradigm(template).ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no paradigm template `{template}` (have: {})",
            morph.paradigms.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")
        ))
    })?;

    let root_gloss = gloss.unwrap_or(root);
    let rows = crate::conlang::morphology::paradigm::generate(
        &phonology, &morph, tmpl, root, root_gloss,
    );

    println!("paradigm `{}` of {root} ({root_gloss}) · {} cell(s)", tmpl.name, rows.len());
    for r in &rows {
        let feats = r
            .features
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("  {:<18} {:<24} {}", r.form, r.gloss, feats);
    }
    Ok(())
}
