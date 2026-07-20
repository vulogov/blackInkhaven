//! `inkhaven language` morphology & grammar surface: the typology/grammar-spec
//! block, the grammar questionnaire, paradigm generation, derivation, and glossing
//! (plus the chapter-paragraph upsert helper). Split out of the flat handler.

use std::path::Path;

use crate::error::{Error, Result};

use super::*;

/// Resolve the clause's word order (flag → declared feature → SVO) + split args.
fn clause_setup<'a>(
    spec: &crate::conlang::types::grammar::GrammarSpec,
    word_order: Option<&str>,
    args_csv: &'a str,
) -> (String, Vec<&'a str>) {
    let order = word_order
        .map(str::to_string)
        .or_else(|| spec.grammar.get("word_order").cloned())
        .unwrap_or_else(|| "svo".to_string());
    let args: Vec<&str> = args_csv.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    (order, args)
}

/// LING-1 L-P6b — `inkhaven language movement <lang> --verb V --args "…" --move R`:
/// front a constituent (wh-movement / topicalisation), leaving a coindexed trace.
pub(crate) fn movement(
    project: &Path,
    language: &str,
    verb: &str,
    args_csv: &str,
    role: &str,
    word_order: Option<&str>,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let (order, args) = clause_setup(&spec, word_order, args_csv);
    let subject = *args.first().unwrap_or(&"(subject)");
    let report =
        crate::conlang::movement::front(&order, verb, subject, args.get(1).copied(), args.get(2).copied(), role);

    let Some(report) = report else {
        return Err(Error::Config(format!(
            "cannot front `{role}` — the role is unfilled or unknown (use subject | object | indirect)"
        )));
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| Error::Store(format!("serializing movement: {e}")))?
        );
        return Ok(());
    }

    println!("movement · {language} · {order}");
    println!("  fronted `{}` ({}) to {}\n", report.moved, report.role, report.landing);
    print!("{}", report.tree.render());
    println!("\n{}", report.tree.bracketed());
    Ok(())
}

/// LING-1 L-P6b — `inkhaven language binding <lang> …`: decide whether one
/// argument may refer to another, by c-command + the binding principles.
#[allow(clippy::too_many_arguments)]
pub(crate) fn binding(
    project: &Path,
    language: &str,
    verb: &str,
    args_csv: &str,
    antecedent: &str,
    anaphor: &str,
    anaphor_type: &str,
    word_order: Option<&str>,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let (order, args) = clause_setup(&spec, word_order, args_csv);
    let subject = *args.first().unwrap_or(&"(subject)");
    let report = crate::conlang::binding::analyze(
        &order,
        verb,
        subject,
        args.get(1).copied(),
        args.get(2).copied(),
        antecedent,
        anaphor,
        anaphor_type,
    );

    let Some(report) = report else {
        return Err(Error::Config(
            "cannot bind — an argument role is unfilled or unknown (use subject | object | indirect)".into(),
        ));
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| Error::Store(format!("serializing binding: {e}")))?
        );
        return Ok(());
    }

    println!("binding · {language}");
    println!(
        "  can `{}` corefer with `{}` (as a {})?",
        report.anaphor, report.antecedent, report.anaphor_type
    );
    println!(
        "  c-command: {} · Principle {} · coreference {}",
        if report.c_commands { "yes" } else { "no" },
        report.principle,
        report.coreference,
    );
    println!("  {}", report.note);
    Ok(())
}

/// LING-1 L-P6b — `inkhaven language tree <lang> --verb V --args "subj,obj"`:
/// build the X-bar phrase-structure tree of a clause, using the language's word
/// order for head–complement placement.
pub(crate) fn build_tree(
    project: &Path,
    language: &str,
    verb: &str,
    args_csv: &str,
    word_order: Option<&str>,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;

    // Word order: explicit flag, else the declared feature, else SVO.
    let order = word_order
        .map(str::to_string)
        .or_else(|| spec.grammar.get("word_order").cloned())
        .unwrap_or_else(|| "svo".to_string());

    let args: Vec<&str> = args_csv.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    let subject = *args.first().unwrap_or(&"(subject)");
    let object = args.get(1).copied();
    let indirect = args.get(2).copied();

    let tree = crate::conlang::xbar::build(&order, verb, subject, object, indirect);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&tree)
                .map_err(|e| Error::Store(format!("serializing tree: {e}")))?
        );
        return Ok(());
    }

    println!("X-bar tree · {language} · {order}\n");
    print!("{}", tree.render());
    println!("\n{}", tree.bracketed());
    Ok(())
}

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

/// Parse a `key=value,key=value` feature string into a map (blank entries skipped).
fn parse_features(spec: &str) -> std::collections::BTreeMap<String, String> {
    spec.split(',')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            let (k, v) = (k.trim(), v.trim());
            (!k.is_empty() && !v.is_empty()).then(|| (k.to_string(), v.to_string()))
        })
        .collect()
}

/// LING-1 L-P6 — `inkhaven language check-clause <lang> --verb V --args "…"`: the
/// Oracle over a clause (levels 3–4) — subject–verb agreement and argument
/// structure.
#[allow(clippy::too_many_arguments)]
pub(crate) fn oracle_check_clause(
    project: &Path,
    language: &str,
    verb: &str,
    args_csv: &str,
    verb_root: Option<&str>,
    subject_features: Option<&str>,
    valence: Option<&str>,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let (spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;

    // Resolve the valence: explicit flag, else the declared verb class, else "".
    let resolved = valence.map(str::to_string).unwrap_or_else(|| {
        spec.verb_classes
            .iter()
            .find(|vc| vc.name.eq_ignore_ascii_case(verb))
            .map(|vc| vc.valence.clone())
            .unwrap_or_default()
    });
    let args: Vec<String> =
        args_csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let features = subject_features.map(parse_features).unwrap_or_default();

    let clause = crate::conlang::oracle::ClauseInput {
        verb,
        verb_root,
        valence: &resolved,
        args: &args,
        subject_features: &features,
    };
    let report = crate::conlang::oracle::check_clause(&phon, &morph, &clause);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| Error::Store(format!("serializing oracle: {e}")))?
        );
        return Ok(());
    }

    let val = if resolved.trim().is_empty() { "(inferred)".to_string() } else { resolved.clone() };
    println!("oracle · clause · {language} · {verb} [{val}]");
    if report.ok() {
        println!("  ✓ a well-formed clause of the language.");
    } else {
        for f in &report.findings {
            println!("      ✗ [{}] {}", f.level, f.message);
        }
    }
    Ok(())
}

/// LING-1 L-P6 — `inkhaven language check-agreement <lang> --dependent D --form W
/// --root R --head-features "…"`: the Oracle's agreement check over any
/// head–dependent pair (adjective–noun, determiner–noun, verb–subject).
pub(crate) fn oracle_check_agreement(
    project: &Path,
    language: &str,
    dependent: &str,
    form: &str,
    root: &str,
    head_features: &str,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let features = parse_features(head_features);

    let finding =
        crate::conlang::oracle::check_agreement(&phon, &morph, dependent, root, form, &features);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&finding)
                .map_err(|e| Error::Store(format!("serializing oracle: {e}")))?
        );
        return Ok(());
    }

    println!("oracle · agreement · {language} · {dependent} `{form}`");
    match finding {
        Some(f) => println!("      ✗ [{}] {}", f.level, f.message),
        None => println!("  ✓ agrees (or no agreement rule declared for `{dependent}`)."),
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

/// IGT-1 (Wave 4) — `inkhaven language igt <lang> --text "…" [--save --name N]`:
/// interlinear glossed text — the segmented sentence, its gloss, and a literal
/// translation, aligned as a Leipzig block; `--save` stores it in `Texts`.
pub(crate) fn igt_text(
    project: &Path,
    language: &str,
    text: &str,
    save: bool,
    name: Option<&str>,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;

    let igt = crate::conlang::igt::build(&phon, &morph, &entries, text);

    if save {
        let title = name.map(str::trim).filter(|s| !s.is_empty()).unwrap_or_else(|| text.trim());
        let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
        save_igt(&store, &cfg, &lang_book, title, &igt)?;
        eprintln!("saved to {language}/Texts as `{title}`");
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&igt)
                .map_err(|e| Error::Store(format!("serializing igt: {e}")))?
        );
        return Ok(());
    }

    if igt.words.is_empty() {
        return Ok(());
    }
    println!("{}", igt.render());
    eprintln!("\n{} / {} word(s) glossed", igt.recognised, igt.words.len());
    Ok(())
}

/// Store `igt` as a paragraph titled `name` under the language's `Texts` chapter,
/// creating the chapter on first use. Rejects a duplicate name.
fn save_igt(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    name: &str,
    igt: &crate::conlang::igt::Igt,
) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let texts = match hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Texts"))
        .cloned()
    {
        Some(existing) => existing,
        None => store.create_node(cfg, &hierarchy, NodeKind::Chapter, "Texts", Some(lang_book), None, InsertPosition::End)?,
    };

    let hierarchy = Hierarchy::load(store)?;
    if hierarchy.children_of(Some(texts.id)).iter().any(|n| n.title.eq_ignore_ascii_case(name)) {
        return Err(Error::Config(format!(
            "a text named `{name}` already exists in {}/Texts — choose another `--name`",
            lang_book.title
        )));
    }

    let hierarchy = Hierarchy::load(store)?;
    let mut node =
        store.create_node(cfg, &hierarchy, NodeKind::Paragraph, name, Some(&texts), None, InsertPosition::End)?;
    // Flip the fresh paragraph to HJSON and overwrite its `.typ` file with the
    // serialized IGT, then persist through the store (mirrors how the dictionary
    // chapter seeds structured paragraphs).
    node.content_type = Some("hjson".to_string());
    let body =
        serde_json::to_string_pretty(igt).map_err(|e| Error::Store(format!("serialize igt: {e}")))?;
    if let Some(rel) = &node.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes()).map_err(|e| Error::Store(format!("write igt file: {e}")))?;
    }
    store
        .update_paragraph_content(&mut node, body.as_bytes())
        .map_err(|e| Error::Store(format!("write igt: {e}")))?;
    Ok(())
}

/// IGT-1 (Wave 4) — `inkhaven language texts <lang> [--name N] [--format latex]`:
/// list the stored interlinear texts, print one, or export them as a linguex
/// LaTeX document.
#[allow(clippy::too_many_arguments)]
pub(crate) fn list_texts(
    project: &Path,
    language: &str,
    name: Option<&str>,
    set_translation: Option<&str>,
    format: &str,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;

    // Edit mode: curate the named text's free translation, then fall through to
    // printing it.
    if let Some(new_translation) = set_translation {
        let Some(n) = name else {
            return Err(Error::Config("--set-translation needs --name <text>".into()));
        };
        if !set_text_translation(&store, &hierarchy, &lang_book, n, new_translation)? {
            return Err(Error::Config(format!("no stored text named `{n}` in {language}/Texts")));
        }
        eprintln!("updated the translation of `{n}`");
    }

    let hierarchy = crate::store::hierarchy::Hierarchy::load(&store)?;
    let texts = load_texts(&store, &hierarchy, &lang_book);

    // The selection: the named text, or all of them.
    let selected: Vec<&(String, crate::conlang::igt::Igt)> = match name {
        Some(n) => vec![
            texts
                .iter()
                .find(|(t, _)| t.eq_ignore_ascii_case(n))
                .ok_or_else(|| Error::Config(format!("no stored text named `{n}` in {language}/Texts")))?,
        ],
        None => texts.iter().collect(),
    };

    // LaTeX export (the selected text, or all).
    if format.eq_ignore_ascii_case("latex") {
        let owned: Vec<(String, crate::conlang::igt::Igt)> =
            selected.iter().map(|(t, igt)| (t.clone(), igt.clone())).collect();
        println!("{}", crate::conlang::interchange::igt_linguex(language, &owned));
        return Ok(());
    }
    if !format.eq_ignore_ascii_case("text") {
        return Err(Error::Config(format!("unknown --format `{format}` (text | latex)")));
    }

    if json {
        if name.is_some() {
            println!(
                "{}",
                serde_json::to_string_pretty(&selected[0].1)
                    .map_err(|e| Error::Store(format!("serializing igt: {e}")))?
            );
        } else {
            let listing: Vec<_> = texts.iter().map(|(t, igt)| (t, &igt.text, &igt.translation)).collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&listing)
                    .map_err(|e| Error::Store(format!("serializing texts: {e}")))?
            );
        }
        return Ok(());
    }

    // Plain text: a named text renders its block; the bare list is a summary.
    if name.is_some() {
        println!("{}", selected[0].1.render());
        return Ok(());
    }
    if texts.is_empty() {
        println!("no stored texts yet — `inkhaven language igt {language} --text \"…\" --save`");
        return Ok(());
    }
    println!("texts · {language} ({})", texts.len());
    for (title, igt) in &texts {
        println!("  {title} — '{}'", igt.translation);
    }
    Ok(())
}

/// CORPUS-1 (Wave 4) — `inkhaven language corpus <lang>`: corpus statistics and a
/// word-frequency list over the stored interlinear texts.
pub(crate) fn corpus_report(project: &Path, language: &str, by_lemma: bool, top: usize, json: bool) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let texts = load_texts(&store, &hierarchy, &lang_book);
    let corpus = crate::conlang::corpus::Corpus::from_texts(&texts);
    let stats = corpus.stats();
    let freq = corpus.frequency(by_lemma);

    if json {
        let rows: Vec<_> = freq.iter().take(top).collect();
        let out = serde_json::json!({ "stats": stats, "frequency": rows });
        println!(
            "{}",
            serde_json::to_string_pretty(&out).map_err(|e| Error::Store(format!("serializing corpus: {e}")))?
        );
        return Ok(());
    }

    println!("corpus · {language}");
    println!(
        "  texts {} · tokens {} · types {} · lemmas {} · TTR {:.2}",
        stats.texts, stats.tokens, stats.types, stats.lemmas, stats.ttr
    );
    if stats.tokens >= 2 {
        println!("  Zipf slope {:.2} (R² {:.2})", stats.zipf_slope, stats.zipf_r2);
    }
    if freq.is_empty() {
        println!("\n  no texts yet — save some with `inkhaven language igt {language} --text \"…\" --save`");
        return Ok(());
    }
    println!("\n  frequency ({}, top {}):", if by_lemma { "by lemma" } else { "by surface" }, top);
    for (w, c) in freq.iter().take(top) {
        println!("    {c:>4}  {w}");
    }
    Ok(())
}

/// CORPUS-1 (Wave 4) — `inkhaven language concordance <lang> --word W`: a KWIC
/// concordance of a word (or, with `--lemma`, a root) across the stored texts.
pub(crate) fn concordance(
    project: &Path,
    language: &str,
    word: &str,
    by_lemma: bool,
    window: usize,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let texts = load_texts(&store, &hierarchy, &lang_book);
    let corpus = crate::conlang::corpus::Corpus::from_texts(&texts);
    let lines = corpus.concordance(word, by_lemma, window);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&lines).map_err(|e| Error::Store(format!("serializing concordance: {e}")))?
        );
        return Ok(());
    }

    println!("concordance · {language} · \"{word}\"{}", if by_lemma { " (lemma)" } else { "" });
    if lines.is_empty() {
        println!("  no occurrences.");
        return Ok(());
    }
    // Right-align the left context so the keywords line up in a column.
    let width = lines.iter().map(|k| k.left.chars().count()).max().unwrap_or(0);
    for k in &lines {
        let pad = " ".repeat(width.saturating_sub(k.left.chars().count()));
        println!("  {pad}{}  [{}]  {}", k.left, k.keyword, k.right);
    }
    println!("\n  {} occurrence(s)", lines.len());
    Ok(())
}

/// CORPUS-1 (Wave 4) — `inkhaven language collocations <lang> --word W`: the words
/// that keep company with a word across the stored texts, ranked by co-occurrence
/// and PMI.
#[allow(clippy::too_many_arguments)]
pub(crate) fn collocations(
    project: &Path,
    language: &str,
    word: &str,
    by_lemma: bool,
    window: usize,
    top: usize,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let texts = load_texts(&store, &hierarchy, &lang_book);
    let corpus = crate::conlang::corpus::Corpus::from_texts(&texts);
    let cols = corpus.collocates(word, by_lemma, window);

    if json {
        let rows: Vec<_> = cols.iter().take(top).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).map_err(|e| Error::Store(format!("serializing collocations: {e}")))?
        );
        return Ok(());
    }

    println!(
        "collocations · {language} · \"{word}\"{} (window {window})",
        if by_lemma { " (lemma)" } else { "" }
    );
    if cols.is_empty() {
        println!("  no collocates.");
        return Ok(());
    }
    println!("    co  tot    PMI  word");
    for c in cols.iter().take(top) {
        println!("    {:>3} {:>4}  {:>5.2}  {}", c.cooccur, c.total, c.pmi, c.word);
    }
    println!("\n  {} collocate(s)", cols.len());
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
