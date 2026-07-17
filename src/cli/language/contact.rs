//! `inkhaven language` sociolinguistic & world-link surface: linking a
//! language to Places/Characters, language ecology (+ SVG map), idiolects, and
//! the AI dialect/loanword proposers. Split out of the flat handler; the
//! variety/contact loaders live in the parent module.

use std::path::Path;

use crate::error::{Error, Result};
use crate::store::hierarchy::Hierarchy;

use super::*;

/// Resolve a name against a system book (Places / Characters), returning the
/// canonical node title. `None` when no node matches — the caller warns but
/// still records the link (the entry may be added later).
pub(crate) fn resolve_system_node(hierarchy: &Hierarchy, system_tag: &str, name: &str) -> Option<String> {
    let root = hierarchy
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(system_tag))?;
    hierarchy
        .collect_subtree(root.id)
        .into_iter()
        .filter_map(|id| hierarchy.get(id))
        .find(|n| n.title.eq_ignore_ascii_case(name))
        .map(|n| n.title.clone())
}

/// LANG-1 P2.6 — link a Place to a (primary or secondary) language.
pub(crate) fn link_place(
    project: &Path,
    place: &str,
    language: &str,
    secondary: bool,
    variety: Option<&str>,
) -> Result<()> {
    use crate::conlang::links::ConlangLinks;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let place_name = match resolve_system_node(&hierarchy, SYSTEM_TAG_PLACES, place) {
        Some(canonical) => canonical,
        None => {
            eprintln!("note: no Place named `{place}` found — recording the link anyway");
            place.to_string()
        }
    };
    let root = store.project_root();
    let mut links = ConlangLinks::load(root).map_err(Error::Io)?;
    if secondary {
        links.add_place_secondary(&place_name, &lang_book.title);
        eprintln!("{place_name} → secondary language {}", lang_book.title);
    } else {
        links.set_place_primary(&place_name, &lang_book.title);
        eprintln!("{place_name} → primary language {}", lang_book.title);
    }
    if let Some(v) = variety {
        links.set_place_variety(&place_name, v);
        eprintln!("{place_name} speaks the {v} variety");
    }
    links.save(root).map_err(Error::Io)?;
    Ok(())
}

/// LANG-1 P2.6 — declare a Character's proficiency in a language.
pub(crate) fn link_character(
    project: &Path,
    character: &str,
    language: &str,
    proficiency: &str,
    native_variety: Option<&str>,
) -> Result<()> {
    use crate::conlang::links::{ConlangLinks, Level};
    let level = Level::parse(proficiency).ok_or_else(|| {
        Error::Config(format!(
            "unknown proficiency `{proficiency}` — use native | fluent | conversational | broken | reading_only"
        ))
    })?;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let char_name = match resolve_system_node(&hierarchy, SYSTEM_TAG_CHARACTERS, character) {
        Some(canonical) => canonical,
        None => {
            eprintln!("note: no Character named `{character}` found — recording the link anyway");
            character.to_string()
        }
    };
    let root = store.project_root();
    let mut links = ConlangLinks::load(root).map_err(Error::Io)?;
    links.set_character_proficiency(&char_name, &lang_book.title, level);
    if let Some(v) = native_variety {
        links.set_character_native_variety(&char_name, v);
    }
    links.save(root).map_err(Error::Io)?;
    let nv = native_variety.map(|v| format!(", native variety {v}")).unwrap_or_default();
    eprintln!("{char_name} → {} ({}){nv}", lang_book.title, level.as_str());
    Ok(())
}

/// LANG-2 P4 — the language ecology: who speaks what (and which variety) where.
pub(crate) fn ecology(project: &Path, svg: Option<&Path>) -> Result<()> {
    use crate::conlang::links::ConlangLinks;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    let links = ConlangLinks::load(store.project_root()).map_err(Error::Io)?;

    // Contact areas (P3), gathered across every language.
    let mut areas: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for book in all_language_books(&hierarchy) {
        if let Some(c) = load_contact(&store, &hierarchy, &book)? {
            let key = if c.region.is_empty() { "(unnamed area)".into() } else { c.region.clone() };
            areas.entry(key).or_default().push(book.title.clone());
        }
    }

    if let Some(path) = svg {
        let doc = ecology_svg(&links, &areas);
        crate::io_atomic::write(path, doc.as_bytes()).map_err(Error::Io)?;
        eprintln!("ecology atlas → {}", path.display());
        return Ok(());
    }

    if links.places.is_empty() && links.characters.is_empty() && areas.is_empty() {
        println!("no speech-community links yet — use `link-place` / `link-character` (with --variety).");
        return Ok(());
    }
    if !links.places.is_empty() {
        println!("Places:");
        for (place, l) in &links.places {
            let lang = l.primary.as_deref().unwrap_or("—");
            let var = l.variety.as_deref().map(|v| format!(" · {v}")).unwrap_or_default();
            let sec = if l.secondary.is_empty() {
                String::new()
            } else {
                format!("  (also {})", l.secondary.join(", "))
            };
            println!("  {:<16} {lang}{var}{sec}", place);
        }
    }
    if !links.characters.is_empty() {
        println!("\nCharacters:");
        for (ch, c) in &links.characters {
            let langs: Vec<String> = c
                .languages
                .iter()
                .map(|p| format!("{} ({})", p.language, p.level))
                .collect();
            let nv = c.native_variety.as_deref().map(|v| format!(" · native {v}")).unwrap_or_default();
            println!("  {:<16} {}{nv}", ch, langs.join(", "));
        }
    }
    if !areas.is_empty() {
        println!("\nContact areas:");
        for (region, members) in &areas {
            println!("  {region} — {}", members.join(", "));
        }
    }
    Ok(())
}

/// A simple node-link atlas: each contact area is a labelled box listing its
/// member languages; standalone languages (places' primaries) sit below.
pub(crate) fn ecology_svg(
    links: &crate::conlang::links::ConlangLinks,
    areas: &std::collections::BTreeMap<String, Vec<String>>,
) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let w = 720;
    let mut y = 40;
    let mut body = String::new();
    body.push_str(&format!(
        "<text x='{}' y='{}' font-family='sans-serif' font-size='22' font-weight='bold' text-anchor='middle'>Language Ecology</text>\n",
        w / 2,
        y
    ));
    y += 30;
    for (region, members) in areas {
        let h = 30 + members.len() as i32 * 22;
        body.push_str(&format!(
            "<rect x='30' y='{y}' width='{}' height='{h}' rx='8' fill='#eef3f7' stroke='#2f5d7a'/>\n",
            w - 60
        ));
        body.push_str(&format!(
            "<text x='44' y='{}' font-family='sans-serif' font-size='14' font-weight='bold' fill='#2f5d7a'>{}</text>\n",
            y + 20,
            esc(region)
        ));
        let mut ly = y + 42;
        for m in members {
            // count places speaking m
            let where_: Vec<String> = links
                .places
                .iter()
                .filter(|(_, l)| l.primary.as_deref().is_some_and(|p| p.eq_ignore_ascii_case(m)))
                .map(|(p, l)| match &l.variety {
                    Some(v) => format!("{p} ({v})"),
                    None => p.clone(),
                })
                .collect();
            let suffix = if where_.is_empty() { String::new() } else { format!("  —  {}", where_.join(", ")) };
            body.push_str(&format!(
                "<text x='58' y='{ly}' font-family='sans-serif' font-size='13'>• {}{}</text>\n",
                esc(m),
                esc(&suffix)
            ));
            ly += 22;
        }
        y += h + 20;
    }
    let total_h = y + 20;
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {w} {total_h}' width='{w}' height='{total_h}'>\n\
         <rect width='{w}' height='{total_h}' fill='#fdfaf3'/>\n{body}</svg>\n"
    )
}

/// LANG-2 P4 — render a form/text in a character's idiolect (their native
/// variety of their primary language).
pub(crate) fn idiolect(project: &Path, character: &str, word: Option<&str>, text: Option<&str>) -> Result<()> {
    use crate::conlang::links::ConlangLinks;
    use crate::conlang::variety as varengine;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    let links = ConlangLinks::load(store.project_root()).map_err(Error::Io)?;

    let link = links
        .characters
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(character))
        .map(|(_, v)| v)
        .ok_or_else(|| Error::Config(format!("no language links recorded for character `{character}`")))?;
    let lang = link
        .languages
        .first()
        .map(|p| p.language.clone())
        .ok_or_else(|| Error::Config(format!("`{character}` commands no language")))?;
    let var_id = link.native_variety.clone().ok_or_else(|| {
        Error::Config(format!(
            "`{character}` has no native variety — set one with `link-character … --native-variety <id>`"
        ))
    })?;

    let lang_book = find_language_book(&hierarchy, &lang)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let vs = load_varieties(&store, &hierarchy, &lang_book)?;
    let v = vs.get(&var_id).ok_or_else(|| {
        Error::Config(format!("language `{lang}` has no variety `{var_id}`"))
    })?;

    println!("{character} · {lang} ({})", v.id);
    if let Some(w) = word {
        println!("  {w}  →  {}", varengine::render_form(&phon, v, w));
    }
    if let Some(t) = text {
        println!("  base       {t}");
        println!("  idiolect   {}", varengine::render_text(&phon, v, t));
    }
    if word.is_none() && text.is_none() {
        return Err(Error::Config("give --word <form> or --text \"…\"".into()));
    }
    Ok(())
}

pub(crate) const PROPOSE_DIALECT_SYSTEM: &str = "You are a dialectologist designing a variety of a \
constructed language. Propose a COHERENT, naturalistic set of sound changes plus a few suppletive \
lexical swaps that give the variety the requested character. HARD CONSTRAINTS: every sound change uses \
ONLY the language's listed phonemes; write each in SPE notation `target > result / left _ right` (use \
`_` for the target slot, `#` for a word boundary, and a phoneme-class name such as `V` for context — \
omit the `/ …` context for an unconditioned change); 3 to 6 sound changes; 0 to 4 lexical swaps using \
only the inventory. Output EXACTLY two labelled blocks and NOTHING else:\nSOUND_CHANGES:\n<one rule per \
line>\nLEXICON:\n<gloss = newform, one per line>";

/// LANG-2 P6 — AI-propose a dialect/register; the deterministic engine validates.
pub(crate) fn propose_dialect(
    project: &Path,
    language: &str,
    describe: &str,
    id: Option<&str>,
    provider: Option<&str>,
    commit: bool,
) -> Result<()> {
    use crate::conlang::types::allophony::AllophonyRule;
    use crate::conlang::types::variety::Variety;
    use crate::conlang::variety as varengine;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no phoneme block"))
    })?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;

    let inventory = phon.phonemes.iter().map(|p| p.ipa.clone()).collect::<Vec<_>>().join(" ");
    let vowels = phon
        .phonemes
        .iter()
        .filter(|p| p.kind == crate::conlang::types::PhonemeKind::Vowel)
        .map(|p| p.ipa.clone())
        .collect::<Vec<_>>()
        .join(" ");
    let some_glosses = entries
        .iter()
        .take(8)
        .map(|e| e.translation.clone())
        .filter(|g| !g.trim().is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    let user = format!(
        "Language phonemes: {inventory}. Vowel class V = {vowels}. Concepts you may give a \
         dialect-specific word (gloss → form): {some_glosses}. Design a variety that is: {describe}."
    );

    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    eprintln!("inkhaven language propose-dialect · {language} · model: {model}");
    let raw = crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(PROPOSE_DIALECT_SYSTEM.to_string()),
        user,
    )
    .map_err(|e| Error::Store(format!("inference error: {e}")))?;

    // Parse the two labelled blocks.
    let (mut rules, mut lexicon): (Vec<String>, Vec<(String, String)>) = (Vec::new(), Vec::new());
    let mut section = "";
    for line in raw.lines() {
        let t = line.trim();
        let up = t.to_ascii_uppercase();
        if up.starts_with("SOUND_CHANGES") {
            section = "rules";
            continue;
        } else if up.starts_with("LEXICON") {
            section = "lex";
            continue;
        }
        if t.is_empty() {
            continue;
        }
        match section {
            "rules" if t.contains('>') => rules.push(t.trim_start_matches(['-', '*', ' ']).to_string()),
            "lex" => {
                if let Some((g, f)) = t.split_once('=') {
                    let (g, f) = (g.trim().to_string(), f.trim().to_string());
                    if !g.is_empty() && !f.is_empty() {
                        lexicon.push((g, f));
                    }
                }
            }
            _ => {}
        }
    }
    // Validate each rule parses as an AllophonyRule; drop the ones that don't.
    let valid: Vec<(String, AllophonyRule)> = rules
        .drain(..)
        .filter_map(|r| {
            serde_hjson::from_str::<AllophonyRule>(&format!("{{ rule: \"{r}\" }}"))
                .ok()
                .map(|ar| (r, ar))
        })
        .collect();
    if valid.is_empty() {
        eprintln!("the model proposed no usable sound changes\n--- raw ---\n{raw}");
        return Ok(());
    }
    let var_id = id
        .map(str::to_string)
        .unwrap_or_else(|| describe.split_whitespace().last().unwrap_or("variety").to_lowercase());
    let variety = Variety {
        id: var_id.clone(),
        kind: "dialect".into(),
        axis: String::new(),
        prestige: None,
        sound_changes: valid.iter().map(|(_, ar)| ar.clone()).collect(),
        lexicon: lexicon.iter().cloned().collect(),
        note: Some(describe.to_string()),
    };

    println!("proposed variety `{var_id}` ({describe}):");
    println!("  sound changes:");
    for (r, _) in &valid {
        println!("    {r}");
    }
    for (g, f) in &lexicon {
        println!("    word: {g} → {f}");
    }
    println!("  preview (base → {var_id}):");
    for e in entries.iter().take(5) {
        let (form, _) = varengine::render_concept(&phon, &variety, &e.translation, &e.word);
        if form != e.word || !lexicon.is_empty() {
            println!("    {:<12} {} → {form}", e.translation, e.word);
        }
    }

    if commit {
        let rules_hjson = valid
            .iter()
            .map(|(r, _)| format!("{{rule:\"{r}\"}}"))
            .collect::<Vec<_>>()
            .join(" ");
        let lex_hjson = lexicon
            .iter()
            .map(|(g, f)| format!("\"{g}\":\"{f}\""))
            .collect::<Vec<_>>()
            .join(" ");
        let body = format!(
            "{{ varieties:[{{ id:\"{var_id}\", kind:\"dialect\", note:\"{}\", \
             sound_changes:[{rules_hjson}], lexicon:{{{lex_hjson}}} }}] }}",
            describe.replace('"', "'")
        );
        create_chapter_paragraph(&store, &cfg, &lang_book, "Grammar", &format!("variety-{var_id}"), &body)?;
        eprintln!("\nadded variety `{var_id}` to {language}'s Grammar");
    } else {
        eprintln!("\n(advisory — re-run with --yes to add it to {language})");
    }
    Ok(())
}

/// LANG-2 P6 — AI areal-plausibility check.
pub(crate) fn areal_check(project: &Path, language: &str, provider: Option<&str>) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let contact = load_contact(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` declares no `contact` block to check"))
    })?;
    let (spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;

    let features = contact
        .areal_features
        .iter()
        .map(|(f, v)| format!("{f} = {v}"))
        .collect::<Vec<_>>()
        .join("; ");
    let own = spec
        .grammar
        .iter()
        .map(|(f, v)| format!("{f}={v}"))
        .collect::<Vec<_>>()
        .join("; ");
    let system = "You are a contact linguist. Assess whether a declared linguistic area (Sprachbund) \
        is typologically plausible — whether these features realistically spread across languages by \
        contact. Comment feature by feature, then give an overall verdict. Be concise.";
    let user = format!(
        "Language: {language}. It belongs to the contact area `{}` with neighbours: {}. The areal \
         features said to have converged: {features}. {language}'s own typology: {own}. Assess the \
         plausibility.",
        contact.region,
        contact.with.join(", ")
    );
    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    eprintln!("inkhaven language areal-check · {language} · model: {model}");
    let raw = crate::ai::stream::collect_blocking(ai.client.clone(), model.to_string(), Some(system.to_string()), user)
        .map_err(|e| Error::Store(format!("inference error: {e}")))?;
    println!("{}", raw.trim());
    Ok(())
}

/// LANG-2 P6 — AI-propose realistic loanwords, nativised by the P2 adapter.
pub(crate) fn propose_loans(
    project: &Path,
    language: &str,
    from: &str,
    topic: Option<&str>,
    count: usize,
    provider: Option<&str>,
) -> Result<()> {
    use crate::conlang::contact;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no phoneme block to borrow into"))
    })?;
    let loan = load_loan_phonology(&store, &hierarchy, &lang_book)?;
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let work_lang = if cfg.language.trim().is_empty() { "english" } else { cfg.language.trim() };
    let topic_clause = topic.map(|t| format!(" in the domain of {t}")).unwrap_or_default();

    let system = format!(
        "You are a contact linguist. Propose concepts a language would realistically BORROW from a \
         donor language{topic_clause} (trade goods, technology, religion, institutions — things a \
         culture adopts from neighbours). For each, give a plausible PHONEMIC donor word (one symbol \
         per sound, IPA-ish). Output exactly `concept = donorform`, one per line, the concept glossed \
         in {work_lang}, and NOTHING else."
    );
    let user = format!("Donor language: {from}. Propose {count} loanwords{topic_clause}.");
    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    eprintln!("inkhaven language propose-loans · {language} ← {from} · model: {model}");
    let raw = crate::ai::stream::collect_blocking(ai.client.clone(), model.to_string(), Some(system), user)
        .map_err(|e| Error::Store(format!("inference error: {e}")))?;

    println!("{language} could borrow from {from}{topic_clause}:");
    println!("  {:<20} {:<14} {}", "concept", "donor", "nativised");
    for line in raw.lines() {
        let t = line.trim();
        if let Some((concept, donor)) = t.split_once('=') {
            let (concept, donor) = (concept.trim(), donor.trim());
            if concept.is_empty() || donor.is_empty() {
                continue;
            }
            let a = contact::adapt(&phon, &loan, donor);
            println!("  {:<20} {:<14} {}", concept, donor, a.adapted);
        }
    }
    eprintln!("\n(advisory — add one with `inkhaven language borrow {language} --from {from} --form <donor> --gloss <concept> --yes`)");
    Ok(())
}

/// LANG-2 P5 — assemble the variation + contact data the grammar book renders
/// (varieties, a dialect-comparison table, areal/contact info). `None` when the
/// language declares no varieties, contact, or loan phonology.
pub(crate) fn build_variation(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
    phon: &crate::conlang::Phonology,
    entries: &[crate::language_entry::DictionaryEntry],
    count: usize,
) -> Result<Option<crate::conlang::output::Variation>> {
    use crate::conlang::output::Variation;
    use crate::conlang::variety as varengine;
    let vs = load_varieties(store, hierarchy, lang_book)?;
    let contact = load_contact(store, hierarchy, lang_book)?;
    let loan = load_loan_phonology(store, hierarchy, lang_book)?;

    let mut v = Variation::default();
    for var in &vs.varieties {
        v.varieties.push((var.id.clone(), var.summary()));
    }
    if !vs.varieties.is_empty() {
        v.dialect_header = vec!["gloss".to_string(), "base".to_string()];
        v.dialect_header.extend(vs.varieties.iter().map(|x| x.id.clone()));
        for e in entries.iter().take(count) {
            let mut row = vec![e.translation.clone(), e.word.clone()];
            for var in &vs.varieties {
                let (form, overridden) =
                    varengine::render_concept(phon, var, &e.translation, &e.word);
                row.push(if overridden { format!("{form}*") } else { form });
            }
            v.dialect_rows.push(row);
        }
    }
    if let Some(c) = contact {
        v.region = (!c.region.is_empty()).then(|| c.region.clone());
        v.neighbours = c.with.clone();
        v.areal = c.areal_features.iter().map(|(f, val)| (f.clone(), val.clone())).collect();
    }
    // A loan-phonology summary only when a real block was declared (the default
    // carries no substitutions and no epenthetic vowel).
    if !loan.substitutions.is_empty() || !loan.epenthetic_vowel.is_empty() {
        let subs = if loan.substitutions.is_empty() {
            String::new()
        } else {
            format!(
                ", substituting {}",
                loan.substitutions
                    .iter()
                    .map(|(k, vv)| format!("{k}→{vv}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        v.loan_summary = Some(format!("nativised by {}{}", loan.repair, subs));
    }
    Ok((!v.is_empty()).then_some(v))
}
