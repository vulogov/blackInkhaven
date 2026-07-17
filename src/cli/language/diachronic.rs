//! `inkhaven language` diachronic surface: family tree, cognates, comparative
//! reconstruction, sound-change application, and lexicon derivation across a
//! language family. Split out of the flat handler.

use std::path::Path;

use crate::error::{Error, Result};
use crate::store::hierarchy::Hierarchy;

use super::*;

pub(crate) const RECONSTRUCT_SYSTEM: &str = "You are a historical linguist applying the comparative method. \
Given cognate forms from related daughter languages, propose the single most plausible proto-form. \
Mark the proto-form with a leading asterisk. Then list the key regular sound correspondences you \
relied on, and justify the reconstruction in 2–3 sentences. Be concise; output plain text.";

pub(crate) const REALISM_SYSTEM: &str = "You are a historical phonologist. Assess whether a chain of diachronic \
sound changes is typologically plausible — i.e. whether each change is a naturally attested type \
(lenition, assimilation, final devoicing, palatalization, epenthesis, …) and whether the ordering \
is reasonable. Flag any rule that is unnatural or unattested, and give an overall verdict \
(plausible / mixed / implausible). Be concise; output plain text.";

/// LANG-1 P4.3 — AI comparative reconstruction of a proto-form from cognates.
pub(crate) fn reconstruct(
    project: &Path,
    forms: &str,
    gloss: Option<&str>,
    provider: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    eprintln!("inkhaven language reconstruct · model: {model}");

    let gloss_clause = gloss.map(|g| format!(" meaning '{g}'")).unwrap_or_default();
    let prompt = format!(
        "Cognate daughter forms{gloss_clause}: {forms}.\n\nReconstruct the proto-form."
    );
    let raw = crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(RECONSTRUCT_SYSTEM.to_string()),
        prompt,
    )
    .map_err(|e| Error::Store(format!("inference error: {e}")))?;
    println!("{}", raw.trim());
    Ok(())
}

/// LANG-1 P4.3 — AI genealogical-realism check of a language's sound-change chain.
pub(crate) fn realism_check(project: &Path, language: &str, provider: Option<&str>) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let dia = load_diachronics(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no diachronics chain to check"))
    })?;

    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    eprintln!("inkhaven language realism-check · {language} · model: {model}");

    let rules_text = dia
        .rules
        .iter()
        .enumerate()
        .map(|(i, r)| format!("{}. {}", i + 1, r.source))
        .collect::<Vec<_>>()
        .join("\n");
    let proto = dia.proto.as_deref().unwrap_or("the proto-language");
    let prompt = format!(
        "Sound-change chain deriving {language} from {proto} (applied in order):\n{rules_text}\n\n\
         Assess the plausibility, rule by rule, then give an overall verdict."
    );
    let raw = crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(REALISM_SYSTEM.to_string()),
        prompt,
    )
    .map_err(|e| Error::Store(format!("inference error: {e}")))?;
    println!("{}", raw.trim());
    Ok(())
}

/// All per-language `Book` nodes under the `Language` system book.
pub(crate) fn all_language_books(hierarchy: &Hierarchy) -> Vec<crate::store::node::Node> {
    let Some(lang_root) = hierarchy
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES))
    else {
        return Vec::new();
    };
    hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book)
        .cloned()
        .collect()
}

/// LANG-1 P4.2 — print the language-family tree.
pub(crate) fn family_tree(project: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let langs = all_language_books(&hierarchy);
    if langs.is_empty() {
        println!("no languages yet — `inkhaven language init <name>`");
        return Ok(());
    }
    let mut pairs: Vec<(String, Option<String>)> = Vec::new();
    for l in &langs {
        let proto = load_diachronics(&store, &hierarchy, l)?.and_then(|d| d.proto);
        pairs.push((l.title.clone(), proto));
    }
    print!("{}", crate::conlang::diachronic::family::render_tree(&pairs));

    // LANG-2 P3 — horizontal contact edges alongside the vertical inheritance.
    let mut edges: Vec<(String, String, String)> = Vec::new(); // (a, b, region)
    for l in &langs {
        if let Some(c) = load_contact(&store, &hierarchy, l)? {
            for n in &c.with {
                let (mut a, mut b) = (l.title.clone(), n.clone());
                if a.to_lowercase() > b.to_lowercase() {
                    std::mem::swap(&mut a, &mut b);
                }
                edges.push((a, b, c.region.clone()));
            }
        }
    }
    edges.sort();
    edges.dedup_by(|x, y| x.0.eq_ignore_ascii_case(&y.0) && x.1.eq_ignore_ascii_case(&y.1));
    if !edges.is_empty() {
        println!("\ncontact (horizontal):");
        for (a, b, region) in &edges {
            let where_ = if region.is_empty() { String::new() } else { format!("  ({region})") };
            println!("  {a} ⇄ {b}{where_}");
        }
    }
    Ok(())
}

/// LANG-1 P4.2 — the cognate set of a proto-form across its daughters.
pub(crate) fn cognates(project: &Path, proto: &str, form: &str) -> Result<()> {
    let (store, hierarchy, proto_book) = open_lang_book(project, proto)?;
    let proto_phon = load_phonology(&store, &hierarchy, &proto_book)?.unwrap_or_default();

    // Daughters: languages whose diachronics `proto` names this language.
    let mut reflexes: Vec<(String, String)> = Vec::new();
    for l in all_language_books(&hierarchy) {
        if l.id == proto_book.id {
            continue;
        }
        let Some(dia) = load_diachronics(&store, &hierarchy, &l)? else { continue };
        if dia.proto.as_deref().is_some_and(|p| p.eq_ignore_ascii_case(&proto_book.title)) {
            let reflex = crate::conlang::diachronic::apply::derive_form(&proto_phon, &dia.rules, form);
            reflexes.push((l.title.clone(), reflex));
        }
    }
    reflexes.sort();

    println!("cognate set · *{form} ({})", proto_book.title);
    if reflexes.is_empty() {
        println!("  (no daughter languages declare {} as their proto)", proto_book.title);
        return Ok(());
    }
    for (name, reflex) in &reflexes {
        println!("  {:<16} {reflex}", name);
    }
    Ok(())
}

/// Resolve a daughter's proto-language: its `Book` node + phonology (the
/// sound changes are defined on proto sounds, so segmentation + classes come
/// from the proto's inventory).
pub(crate) fn resolve_proto(
    store: &Store,
    hierarchy: &Hierarchy,
    dia: &crate::conlang::types::diachronic::Diachronics,
    daughter: &str,
) -> Result<(crate::store::node::Node, crate::conlang::Phonology, String)> {
    let proto_name = dia.proto.clone().ok_or_else(|| {
        Error::Config(format!(
            "language `{daughter}`'s diachronics block has no `proto` — name the parent language"
        ))
    })?;
    let lang_root = hierarchy
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES))
        .ok_or_else(|| Error::Store("Language system book missing".into()))?;
    let proto_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(&proto_name))
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "proto-language `{proto_name}` not found — `inkhaven language init {proto_name}` first"
            ))
        })?;
    let proto_phon = load_phonology(store, hierarchy, &proto_book)?.unwrap_or_default();
    Ok((proto_book, proto_phon, proto_name))
}

/// LANG-1 P4.1 — evolve a single proto-form through a language's sound-change
/// chain (segmented with the proto's inventory).
pub(crate) fn sound_change(project: &Path, language: &str, form: &str) -> Result<()> {
    let (store, hierarchy, daughter_book) = open_lang_book(project, language)?;
    let dia = load_diachronics(&store, &hierarchy, &daughter_book)?.ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no diachronics — add a `{{ diachronics: {{ proto, rules }} }}` \
             block to its Phonology chapter"
        ))
    })?;
    let (_proto_book, proto_phon, proto_name) = resolve_proto(&store, &hierarchy, &dia, language)?;
    let daughter = crate::conlang::diachronic::apply::derive_form(&proto_phon, &dia.rules, form);
    println!("{form}  >  {daughter}   (from {proto_name}, {} rule(s))", dia.rules.len());
    Ok(())
}

/// LING-1 L-P1 — the Consequence Tracer. Preview a pending sound change across
/// the current lexicon (which words shift, which distinctions merge) without
/// committing it.
pub(crate) fn trace(project: &Path, language: &str, rule: &str, limit: usize, json: bool) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let report = crate::conlang::trace::trace_sound_change(&phon, &entries, rule, limit.max(1));

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| Error::Store(format!("serializing trace: {e}")))?
        );
        return Ok(());
    }

    println!("consequence trace · {language} · rule  {rule}");
    if !report.rule_valid {
        println!(
            "  ✗ could not parse the rule. Use the form `X > Y / A _ B`, e.g. `s > ʃ / _ i` \
             or `d > t / _ #`."
        );
        return Ok(());
    }
    if report.analyzable_words == 0 {
        println!("  (no analyzable words — define a phoneme inventory and add dictionary entries)");
        return Ok(());
    }
    println!(
        "  affects {} of {} analyzable word(s)",
        report.affected, report.analyzable_words
    );
    if !report.changes.is_empty() {
        println!("  changes:");
        for c in &report.changes {
            let gloss = if c.gloss.is_empty() { String::new() } else { format!("  ‘{}’", c.gloss) };
            println!("      {}  >  {}{gloss}", c.from, c.to);
        }
    }
    if report.new_homophones.is_empty() {
        println!("  no new homophones — the change merges nothing.");
    } else {
        println!(
            "  ⚠ {} new homophone set(s) ({} words merged):",
            report.new_homophones.len(),
            report.merged_words
        );
        for h in &report.new_homophones {
            println!("      {} ← {}", h.form, h.words.join(", "));
        }
    }
    Ok(())
}

/// LANG-1 P4.1 — derive a daughter lexicon from its proto.
pub(crate) fn derive_lexicon_cmd(project: &Path, language: &str, yes: bool) -> Result<()> {
    let (store, hierarchy, daughter_book) = open_lang_book(project, language)?;
    let dia = load_diachronics(&store, &hierarchy, &daughter_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no diachronics block"))
    })?;
    let (proto_book, proto_phon, proto_name) = resolve_proto(&store, &hierarchy, &dia, language)?;
    let proto_entries = load_dictionary(&store, &hierarchy, &proto_book)?;
    if proto_entries.is_empty() {
        eprintln!("note: proto `{proto_name}` has no dictionary entries to derive from");
    }
    let derived =
        crate::conlang::diachronic::apply::derive_lexicon(&proto_phon, &dia.rules, &proto_entries);

    println!(
        "derive {language} from {proto_name} · {} rule(s) · {} entr(y/ies):",
        dia.rules.len(),
        derived.len()
    );
    for d in &derived {
        println!("  {:<14} > {:<14} {}", d.proto_form, d.form, d.gloss);
    }

    if yes {
        let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
        let mut added = 0usize;
        for d in &derived {
            let entry = ImportEntry {
                word: d.form.clone(),
                pos: d.pos.clone(),
                translation: d.gloss.clone(),
                etymology: format!("from {proto_name} {} via sound change", d.proto_form),
                ..Default::default()
            };
            match add_imported_dictionary_entry(&store, &cfg, &daughter_book, &entry) {
                Ok(_) => added += 1,
                Err(e) => eprintln!("  skipped {}: {e}", d.form),
            }
        }
        eprintln!("\nadded {added} derived entr(y/ies) to {language}'s Dictionary");
    } else {
        eprintln!("\n(dry run — re-run with --yes to add the {} derived entr(y/ies))", derived.len());
    }
    Ok(())
}
