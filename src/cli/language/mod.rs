//! 1.2.13+ Phase A — `inkhaven language …`
//! subcommand family.  Currently hosts `init`,
//! which scaffolds a per-language sub-book under
//! the top-level `Language` system book with the
//! five standard chapters (`Meta`, `Dictionary`,
//! `Grammar`, `Phonology`, `Sample texts`) and a
//! seeded `Meta/overview.typ` HJSON paragraph.
//!
//! See `Documentation/PROPOSALS/LANGUAGE_BOOK.md`
//! for the full design including the dictionary
//! entry HJSON schema, grammar-rule schema, and the
//! AI text-to-text translation flow that Phases B-D
//! will add on top of this foundation.

use std::path::Path;

use crate::config::Config;
use crate::conlang::types::font::DEFAULT_UPM;
use crate::conlang::writing::font::GlyphSource;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{
    InsertPosition, NodeKind, Store, SYSTEM_TAG_CHARACTERS, SYSTEM_TAG_LANGUAGES,
    SYSTEM_TAG_PLACES,
};

use super::{LanguageCommand, LanguageExportFormat};

// Per-family submodules peeled off the original flat handler. Each does
// `use super::*` to see the shared core here; `pub(crate) use <sub>::*` below
// re-exports their items at `crate::cli::language::*` so both the `run` dispatch
// and external callers (`stdlib::lang`, `export::html::companions`) keep their
// paths unchanged.
mod import;
mod render;
mod syntax;
mod translate;
mod varieties;
mod writing;
pub(crate) use import::*;
pub(crate) use render::*;
pub(crate) use syntax::*;
pub(crate) use translate::*;
pub(crate) use varieties::*;
pub(crate) use writing::*;

pub fn run(project: &Path, cmd: LanguageCommand) -> Result<()> {
    match cmd {
        LanguageCommand::Init { name } => init(project, &name),
        LanguageCommand::AddWord {
            language,
            word,
            r#type,
            translation,
            example,
            import,
            new,
            force,
        } => {
            if let Some(csv_path) = import {
                import_dictionary_csv(project, &language, &csv_path, new, force)
            } else {
                // Single-add mode requires word + type +
                // translation positionals/flags.
                let word = word.ok_or_else(|| {
                    Error::Config(
                        "missing <WORD> — pass a word argument OR use --import <PATH>"
                            .into(),
                    )
                })?;
                let pos = r#type.ok_or_else(|| {
                    Error::Config(
                        "missing --type — pass a part-of-speech OR use --import".into(),
                    )
                })?;
                let translation = translation.ok_or_else(|| {
                    Error::Config(
                        "missing --translation — pass a working-language gloss OR use --import"
                            .into(),
                    )
                })?;
                add_word(
                    project,
                    &language,
                    &word,
                    &pos,
                    &translation,
                    example.as_deref(),
                )
            }
        }
        LanguageCommand::Import {
            language,
            file,
            format,
            r#yes,
        } => import_foreign(project, &language, &file, format, r#yes),
        LanguageCommand::Gaps {
            language,
            scope,
            json,
        } => gaps(project, &language, &scope, json),
        LanguageCommand::Compose {
            language,
            kind,
            count,
            seed,
            meter,
            provider,
        } => compose(
            project,
            &language,
            &kind,
            count,
            seed,
            &meter,
            provider.as_deref(),
        ),
        LanguageCommand::Doctor { language, json } => doctor(project, &language, json),
        LanguageCommand::Export {
            language,
            format,
            output,
        } => export(project, &language, format, output.as_deref()),
        LanguageCommand::List => list(project),
        LanguageCommand::RemoveWord { language, word } => {
            remove_word(project, &language, &word)
        }
        LanguageCommand::DefineRule {
            language,
            rule_id,
            category,
        } => define_rule(project, &language, &rule_id, &category),
        LanguageCommand::GenerateWord {
            language,
            role,
            count,
        } => generate_word(project, &language, &role, count),
        LanguageCommand::Syllabify { language, word } => {
            syllabify_word(project, &language, &word)
        }
        LanguageCommand::Ipa { language, word } => ipa_surface(project, &language, &word),
        LanguageCommand::Stress { language, word } => stress_word(project, &language, &word),
        LanguageCommand::Romanize {
            language,
            text,
            scheme,
            reverse,
        } => romanize_text(project, &language, &text, scheme.as_deref(), reverse),
        LanguageCommand::Tone { language, tones } => tone_sandhi(project, &language, &tones),
        LanguageCommand::Audit { language, json } => audit(project, &language, json),
        LanguageCommand::Stats { language, json } => stats(project, &language, json),
        LanguageCommand::Dictionary { language, format, out, font } => {
            dictionary(project, &language, &format, out.as_deref(), font.as_deref())
        }
        LanguageCommand::GrammarBook { language, format, out, font, study, provider } => {
            grammar_book(project, &language, &format, out.as_deref(), font.as_deref(), study, provider.as_deref())
        }
        LanguageCommand::Tutorial { language, format, out, font, provider } => {
            tutorial(project, &language, &format, out.as_deref(), font.as_deref(), provider.as_deref())
        }
        LanguageCommand::LinkPlace {
            place,
            language,
            secondary,
            variety,
        } => link_place(project, &place, &language, secondary, variety.as_deref()),
        LanguageCommand::LinkCharacter {
            character,
            language,
            proficiency,
            native_variety,
        } => link_character(
            project,
            &character,
            &language,
            &proficiency,
            native_variety.as_deref(),
        ),
        LanguageCommand::Speakers { language } => speakers(project, &language),
        LanguageCommand::Ecology { svg } => ecology(project, svg.as_deref()),
        LanguageCommand::Idiolect {
            character,
            word,
            text,
        } => idiolect(project, &character, word.as_deref(), text.as_deref()),
        LanguageCommand::ScanManuscript { language, json } => {
            scan_manuscript(project, &language, json)
        }
        LanguageCommand::Paradigm {
            language,
            root,
            template,
            gloss,
        } => paradigm(project, &language, &root, &template, gloss.as_deref()),
        LanguageCommand::Sentence {
            language,
            subject,
            subject_number,
            subject_person,
            subject_adj,
            verb,
            object,
            object_number,
            object_adj,
            noun_paradigm,
            verb_paradigm,
            negate,
            negator,
            question,
            q_particle,
        } => sentence(
            project,
            &language,
            subject.as_deref(),
            &subject_number,
            &subject_person,
            subject_adj.as_deref(),
            verb.as_deref(),
            object.as_deref(),
            &object_number,
            object_adj.as_deref(),
            &noun_paradigm,
            &verb_paradigm,
            negate,
            negator.as_deref(),
            question,
            q_particle.as_deref(),
        ),
        LanguageCommand::Translate { language, text, trace, json } => {
            translate(project, &language, &text, trace, json)
        }
        LanguageCommand::Reverse { language, text, json } => {
            reverse_translate(project, &language, &text, json)
        }
        LanguageCommand::Cross { from, to, text, json } => {
            cross_translate(project, &from, &to, &text, json)
        }
        LanguageCommand::Remember { language, english, conlang } => {
            remember(project, &language, &english, &conlang)
        }
        LanguageCommand::Memory { language } => memory_list(project, &language),
        LanguageCommand::Corpus { language, pool, limit, yes, json } => {
            corpus(project, &language, pool.as_deref(), limit, yes, json)
        }
        LanguageCommand::Eval { language, test_set, limit, json } => {
            eval(project, &language, test_set.as_deref(), limit, json)
        }
        LanguageCommand::ExportTranslation { language, out } => {
            export_translation(project, &language, out.as_deref())
        }
        LanguageCommand::Relative {
            language,
            head,
            role,
            verb,
            with,
            relativizer,
            noun_paradigm,
            verb_paradigm,
        } => relative(
            project,
            &language,
            &head,
            &role,
            &verb,
            with.as_deref(),
            relativizer.as_deref(),
            &noun_paradigm,
            &verb_paradigm,
        ),
        LanguageCommand::Coordinate {
            language,
            clauses,
            nps,
            conjunction,
            noun_paradigm,
            verb_paradigm,
        } => coordinate(
            project,
            &language,
            &clauses,
            &nps,
            conjunction.as_deref(),
            &noun_paradigm,
            &verb_paradigm,
        ),
        LanguageCommand::Complement {
            language,
            subject,
            subject_person,
            subject_number,
            verb,
            complementizer,
            comp_subject,
            comp_verb,
            comp_object,
            noun_paradigm,
            verb_paradigm,
        } => complement(
            project,
            &language,
            subject.as_deref(),
            &subject_person,
            &subject_number,
            &verb,
            complementizer.as_deref(),
            comp_subject.as_deref(),
            &comp_verb,
            comp_object.as_deref(),
            &noun_paradigm,
            &verb_paradigm,
        ),
        LanguageCommand::Agree {
            language,
            word,
            pos,
            features,
            gloss,
        } => agree(project, &language, &word, &pos, &features, gloss.as_deref()),
        LanguageCommand::Varieties { language } => varieties(project, &language),
        LanguageCommand::Lect {
            language,
            variety,
            word,
            text,
        } => lect(project, &language, &variety, word.as_deref(), text.as_deref()),
        LanguageCommand::Dialects { language, count } => dialects(project, &language, count),
        LanguageCommand::Borrow {
            language,
            form,
            from,
            gloss,
            r#type,
            r#yes,
        } => borrow(
            project,
            &language,
            &form,
            from.as_deref(),
            gloss.as_deref(),
            &r#type,
            r#yes,
        ),
        LanguageCommand::Areal { language } => areal(project, language.as_deref()),
        LanguageCommand::ProposeDialect {
            language,
            describe,
            id,
            provider,
            r#yes,
        } => propose_dialect(project, &language, &describe, id.as_deref(), provider.as_deref(), r#yes),
        LanguageCommand::ArealCheck { language, provider } => {
            areal_check(project, &language, provider.as_deref())
        }
        LanguageCommand::ProposeLoans {
            language,
            from,
            topic,
            count,
            provider,
        } => propose_loans(project, &language, &from, topic.as_deref(), count, provider.as_deref()),
        LanguageCommand::Gloss { language, text } => gloss_text(project, &language, &text),
        LanguageCommand::Grammar { language, set, json } => {
            grammar_questionnaire(project, &language, set.as_deref(), json)
        }
        LanguageCommand::IdiomAdd {
            language,
            form,
            literal,
            meaning,
            register,
        } => idiom_add(
            project,
            &language,
            &form,
            literal.as_deref(),
            &meaning,
            register.as_deref(),
        ),
        LanguageCommand::MetaphorAdd {
            language,
            source,
            target,
            example,
        } => metaphor_add(project, &language, &source, &target, example.as_deref()),
        LanguageCommand::Idioms { language } => idioms_list(project, &language),
        LanguageCommand::FontBuild {
            family,
            language,
            glyphs,
            out,
            upm,
            format,
        } => font_build(
            project,
            family.as_deref(),
            language.as_deref(),
            glyphs.as_deref(),
            out.as_deref(),
            upm,
            &format,
        ),
        LanguageCommand::FontImportGlyph {
            language,
            svg,
            phoneme,
            codepoint,
            name,
        } => font_import_glyph(
            project,
            &language,
            &svg,
            phoneme.as_deref(),
            codepoint.as_deref(),
            name.as_deref(),
        ),
        LanguageCommand::FontConfig { language, json } => {
            font_config_show(project, &language, json)
        }
        LanguageCommand::FontTemplates { language } => font_templates(project, &language),
        LanguageCommand::FontCompose {
            language,
            template,
            name,
            codepoint,
            phoneme,
            slots,
            out,
            yes,
        } => font_compose(
            project,
            &language,
            &template,
            &name,
            codepoint.as_deref(),
            phoneme.as_deref(),
            &slots,
            out.as_deref(),
            yes,
        ),
        LanguageCommand::SpatialTypst {
            language,
            template,
            name,
            slots,
            size,
            out,
        } => spatial_typst(project, &language, &template, &name, &slots, &size, out.as_deref()),
        LanguageCommand::Transliterate { language, text, json } => {
            transliterate(project, &language, &text, json)
        }
        LanguageCommand::GlyphDraft {
            language,
            describe,
            phoneme,
            codepoint,
            name,
            provider,
            out,
            yes,
        } => glyph_draft(
            project,
            &language,
            &describe,
            phoneme.as_deref(),
            codepoint.as_deref(),
            name.as_deref(),
            provider.as_deref(),
            out.as_deref(),
            yes,
        ),
        LanguageCommand::GlyphLint { svg } => glyph_lint(&svg),
        LanguageCommand::Reconstruct {
            forms,
            gloss,
            provider,
        } => reconstruct(project, &forms, gloss.as_deref(), provider.as_deref()),
        LanguageCommand::RealismCheck { language, provider } => {
            realism_check(project, &language, provider.as_deref())
        }
        LanguageCommand::FamilyTree => family_tree(project),
        LanguageCommand::Cognates { proto, form } => cognates(project, &proto, &form),
        LanguageCommand::SoundChange { language, form } => {
            sound_change(project, &language, &form)
        }
        LanguageCommand::DeriveLexicon { language, yes } => {
            derive_lexicon_cmd(project, &language, yes)
        }
        LanguageCommand::Derive {
            language,
            root,
            gloss,
            pos,
            yes,
        } => derive(project, &language, &root, gloss.as_deref(), pos.as_deref(), yes),
        LanguageCommand::Query {
            language,
            register,
            domain,
            era,
            pos,
            text,
            json,
        } => query(
            project,
            &language,
            register.as_deref(),
            domain.as_deref(),
            era.as_deref(),
            pos.as_deref(),
            text.as_deref(),
            json,
        ),
        LanguageCommand::GenerateLexicon {
            language,
            topic,
            count,
            era,
            register,
            provider,
            semantic,
            semantic_threshold,
            yes,
        } => generate_lexicon(
            project,
            &language,
            topic.as_deref(),
            count,
            era.as_deref(),
            register.as_deref(),
            provider.as_deref(),
            semantic,
            semantic_threshold,
            yes,
        ),
    }
}

pub(crate) const LEXGEN_SYSTEM: &str = "You are a meticulous lexicographer for a constructed language. \
Reply with a SINGLE JSON object and nothing else — no prose, no preamble, no markdown fences. \
Shape: {\"entries\":[{\"form\":\"…\",\"gloss\":\"…\",\"pos\":\"…\",\"example\":\"…\",\"register\":\"…\",\
\"domain\":[\"…\"]}]}. Choose each `form` ONLY from the provided candidate list (never invent a \
form). Never assign two entries the same meaning. Keep `pos` a short lowercase tag \
(noun/verb/adjective/…). `register` is one short tag (neutral/formal/vulgar/sacred/archaic); \
`domain` is one or two short semantic-domain tags.";

/// Resolve a name against a system book (Places / Characters), returning the
/// canonical node title. `None` when no node matches — the caller warns but
/// still records the link (the entry may be added later).
fn resolve_system_node(hierarchy: &Hierarchy, system_tag: &str, name: &str) -> Option<String> {
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
fn link_place(
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
fn link_character(
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
fn ecology(project: &Path, svg: Option<&Path>) -> Result<()> {
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
fn ecology_svg(
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
fn idiolect(project: &Path, character: &str, word: Option<&str>, text: Option<&str>) -> Result<()> {
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
fn propose_dialect(
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
fn areal_check(project: &Path, language: &str, provider: Option<&str>) -> Result<()> {
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
fn propose_loans(
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

/// Find + parse the `Morphology`-chapter HJSON block for a language sub-book.
pub(crate) fn load_morphology(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<Option<crate::conlang::types::morphology::Morphology>> {
    // The 1.2.13 scaffold has no Morphology chapter, so the block lives in
    // the Grammar chapter (or a hand-added Morphology chapter).
    let chapters: Vec<_> = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .filter(|n| {
            n.kind == NodeKind::Chapter
                && (n.title.eq_ignore_ascii_case("Morphology")
                    || n.title.eq_ignore_ascii_case("Grammar"))
        })
        .cloned()
        .collect();
    for chapter in chapters {
        for para in hierarchy.children_of(Some(chapter.id)) {
            if para.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(bytes) = store.get_content(para.id)? else { continue };
            let body = String::from_utf8_lossy(&bytes);
            match crate::conlang::types::morphology::Morphology::from_hjson(&body) {
                Ok(Some(m))
                    if !m.morphemes.is_empty()
                        || !m.paradigms.is_empty()
                        || !m.derivations.is_empty() =>
                {
                    return Ok(Some(m));
                }
                // A Grammar paragraph that isn't a morphology block (a
                // define-rule rule) just won't match the shape — skip it.
                Ok(_) | Err(_) => continue,
            }
        }
    }
    Ok(None)
}

/// LANG-2 P1 — load the `{ varieties: [ … ] }` block from the Grammar chapter.
pub(crate) fn load_varieties(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<crate::conlang::types::variety::Varieties> {
    use crate::conlang::types::variety::Varieties;
    let chapters: Vec<_> = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Grammar"))
        .cloned()
        .collect();
    // Merge every `{ varieties: [ … ] }` block across the Grammar chapter, so a
    // hand-authored set and AI-proposed dialects (written as separate paragraphs)
    // are all seen together.
    let mut merged = Varieties::default();
    for chapter in chapters {
        for para in hierarchy.children_of(Some(chapter.id)) {
            if para.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(bytes) = store.get_content(para.id)? else { continue };
            let body = String::from_utf8_lossy(&bytes);
            if let Ok(Some(v)) = Varieties::from_hjson(&body) {
                for var in v.varieties {
                    if !merged.varieties.iter().any(|x| x.id.eq_ignore_ascii_case(&var.id)) {
                        merged.varieties.push(var);
                    }
                }
            }
        }
    }
    Ok(merged)
}

/// LANG-2 P3 — load the `{ contact: { … } }` block from the Grammar chapter
/// (a language's areal / Sprachbund membership). `None` when absent.
pub(crate) fn load_contact(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<Option<crate::conlang::types::contact::Contact>> {
    use crate::conlang::types::contact::Contact;
    let chapters: Vec<_> = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Grammar"))
        .cloned()
        .collect();
    for chapter in chapters {
        for para in hierarchy.children_of(Some(chapter.id)) {
            if para.kind != NodeKind::Paragraph {
                continue;
            }
            let Ok(Some(bytes)) = store.get_content(para.id) else { continue };
            if let Ok(Some(c)) = Contact::from_hjson(&String::from_utf8_lossy(&bytes)) {
                return Ok(Some(c));
            }
        }
    }
    Ok(None)
}

/// LANG-2 P2 — load the `{ loan_phonology: { … } }` block from the Phonology
/// chapter (how this language nativises borrowings). Defaults when absent.
pub(crate) fn load_loan_phonology(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<crate::conlang::types::contact::LoanPhonology> {
    use crate::conlang::types::contact::LoanPhonology;
    let chapters: Vec<_> = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Phonology"))
        .cloned()
        .collect();
    for chapter in chapters {
        for para in hierarchy.children_of(Some(chapter.id)) {
            if para.kind != NodeKind::Paragraph {
                continue;
            }
            let Ok(Some(bytes)) = store.get_content(para.id) else { continue };
            if let Ok(Some(lp)) = LoanPhonology::from_hjson(&String::from_utf8_lossy(&bytes)) {
                return Ok(lp);
            }
        }
    }
    Ok(LoanPhonology::default())
}

/// Load the `{ diachronics: { proto, rules } }` block from the Phonology
/// chapter.
pub(crate) fn load_diachronics(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<Option<crate::conlang::types::diachronic::Diachronics>> {
    use crate::conlang::types::diachronic::Diachronics;
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Phonology"))
        .cloned()
    else {
        return Ok(None);
    };
    for para in hierarchy.children_of(Some(chapter.id)) {
        if para.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(para.id) else { continue };
        if let Ok(Some(d)) = Diachronics::from_hjson(&String::from_utf8_lossy(&bytes)) {
            return Ok(Some(d));
        }
    }
    Ok(None)
}


pub(crate) const GLYPH_DRAFT_SYSTEM: &str = "You are a type designer drafting a single glyph for a constructed \
writing system. Output ONE self-contained SVG and NOTHING else — no prose, no explanation, no \
markdown fences. Hard requirements (the glyph is rejected otherwise): the root element is <svg> with \
viewBox=\"0 0 1000 1000\"; the shape is one or more FILLED black <path> elements \
(fill=\"black\" or fill=\"#000\"); outline every stroke into a filled shape — NO stroke-only paths, \
NO stroke attribute; NO <image> or embedded raster data; NO gradients; NO <text>. \
A font is MONOCHROME: the fill colour is discarded and only the outline survives, so NEVER use a \
white or light fill to carve out a hole/counter (the inside of an O, the eye of an e) — a white \
shape just becomes solid ink. Instead cut counters the TrueType way: draw the inner contour as a \
subpath wound in the OPPOSITE direction to the outer contour, both in the SAME black <path> (e.g. \
outer ring clockwise, inner hole counter-clockwise); the opposing winding makes the hole. Use ONE \
<path> with multiple subpaths so the windings combine. Design the glyph to read clearly at small \
sizes: bold, centered, with margins inside the viewBox.";

const GRAMMAR_STUDY_SYSTEM: &str = "You are a linguistics tutor writing the study-guide companion \
to a reference grammar of a constructed language. Your job is to make the grammar approachable to a \
reader who is NOT a trained linguist: introduce and clearly DEFINE every linguistic term the grammar \
relies on (phoneme, consonant/vowel, syllable, stress and where it falls, allophony / conditioned \
sound change, affix and the difference between inflection and derivation, grammatical case, the \
specific cases present, word order such as SOV, morphosyntactic alignment such as \
nominative–accusative, adpositions, agent nouns, and any others the brief implies), and explain in \
plain language what each feature MEANS and how THIS language uses it, with short examples grounded in \
the brief. Define the term first, then show how it applies here. Be accurate and concise; use only \
the features in the brief (never invent data). Warm, clear, textbook voice. Output the guide only.";

const TUTORIAL_SYSTEM: &str = "You are an experienced language teacher writing a beginner's \
textbook for a constructed (invented) language. From the language brief you are given — and using \
ONLY the sounds, words, and grammar it lists (never invent vocabulary, sounds, or rules) — write a \
complete graded course that takes an absolute beginner to reading the language. Cover, in order: a \
short warm introduction; a pronunciation guide (the consonants and vowels, where stress falls, and \
any sound-changes explained in plain language with examples); graded lessons that introduce \
vocabulary in small sets and EXPLAIN the grammar — word order, the affixes/cases, word-building — \
each with worked examples built from the provided words; a reading lesson that walks through a \
provided sample text with an interlinear gloss and invites the learner to translate it; and a \
short practice exercise at the end of every lesson. Teach and explain; do not merely tabulate. Keep \
a clear, encouraging textbook voice. Write the document and nothing else (no preamble about what \
you are doing).";

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
fn reconstruct(
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
fn realism_check(project: &Path, language: &str, provider: Option<&str>) -> Result<()> {
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
fn all_language_books(hierarchy: &Hierarchy) -> Vec<crate::store::node::Node> {
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
fn family_tree(project: &Path) -> Result<()> {
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
fn cognates(project: &Path, proto: &str, form: &str) -> Result<()> {
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
fn resolve_proto(
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
fn sound_change(project: &Path, language: &str, form: &str) -> Result<()> {
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

/// LANG-1 P4.1 — derive a daughter lexicon from its proto.
fn derive_lexicon_cmd(project: &Path, language: &str, yes: bool) -> Result<()> {
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

/// Load the `{ idioms: [...], metaphors: [...] }` block from the Grammar
/// chapter + the paragraph node that holds it.
pub(crate) fn load_expressions(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<(crate::conlang::types::expression::Expressions, Option<crate::store::node::Node>)> {
    use crate::conlang::types::expression::Expressions;
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Grammar"))
        .cloned()
    else {
        return Ok((Expressions::default(), None));
    };
    for para in hierarchy.children_of(Some(chapter.id)) {
        if para.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(para.id) else { continue };
        if let Ok(Some(e)) = Expressions::from_hjson(&String::from_utf8_lossy(&bytes)) {
            return Ok((e, Some(para.clone())));
        }
    }
    Ok((Expressions::default(), None))
}

fn save_expressions(
    project: &Path,
    store: &Store,
    lang_book: &crate::store::node::Node,
    node: Option<crate::store::node::Node>,
    expr: &crate::conlang::types::expression::Expressions,
) -> Result<()> {
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let body = serde_json::to_string_pretty(expr)
        .map_err(|e| Error::Store(format!("serializing expressions: {e}")))?;
    upsert_grammar_paragraph(store, &cfg, lang_book, "expressions", node, &body)
}

/// LANG-1 P3.5 — add an idiom.
fn idiom_add(
    project: &Path,
    language: &str,
    form: &str,
    literal: Option<&str>,
    meaning: &str,
    register: Option<&str>,
) -> Result<()> {
    use crate::conlang::types::expression::Idiom;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (mut expr, node) = load_expressions(&store, &hierarchy, &lang_book)?;
    expr.idioms.push(Idiom {
        form: form.trim().to_string(),
        literal: literal.unwrap_or("").trim().to_string(),
        meaning: meaning.trim().to_string(),
        register: register.map(|r| vec![r.trim().to_string()]).unwrap_or_default(),
    });
    save_expressions(project, &store, &lang_book, node, &expr)?;
    eprintln!("{language}: added idiom `{}` ({} total)", form.trim(), expr.idioms.len());
    Ok(())
}

/// LANG-1 P3.5 — declare a conceptual metaphor.
fn metaphor_add(
    project: &Path,
    language: &str,
    source: &str,
    target: &str,
    example: Option<&str>,
) -> Result<()> {
    use crate::conlang::types::expression::Metaphor;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (mut expr, node) = load_expressions(&store, &hierarchy, &lang_book)?;
    expr.metaphors.push(Metaphor {
        source: source.trim().to_string(),
        target: target.trim().to_string(),
        examples: example.map(|e| vec![e.trim().to_string()]).unwrap_or_default(),
        note: String::new(),
    });
    save_expressions(project, &store, &lang_book, node, &expr)?;
    eprintln!(
        "{language}: declared metaphor {} → {} ({} total)",
        source.trim(),
        target.trim(),
        expr.metaphors.len()
    );
    Ok(())
}

/// LANG-1 P3.5 — list idioms + metaphors.
fn idioms_list(project: &Path, language: &str) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (expr, _) = load_expressions(&store, &hierarchy, &lang_book)?;
    if expr.idioms.is_empty() && expr.metaphors.is_empty() {
        println!("{language}: no idioms or metaphors yet");
        return Ok(());
    }
    if !expr.idioms.is_empty() {
        println!("idioms ({}):", expr.idioms.len());
        for i in &expr.idioms {
            let reg = if i.register.is_empty() { String::new() } else { format!("  [{}]", i.register.join(",")) };
            println!("  {}  —  {}{}", i.form, i.meaning, reg);
            if !i.literal.trim().is_empty() {
                println!("      (lit. {})", i.literal);
            }
        }
    }
    if !expr.metaphors.is_empty() {
        println!("\nmetaphors ({}):", expr.metaphors.len());
        for m in &expr.metaphors {
            let ex = if m.examples.is_empty() { String::new() } else { format!("  e.g. {}", m.examples.join("; ")) };
            println!("  {} → {}{}", m.source, m.target, ex);
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
fn grammar_questionnaire(
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
fn upsert_grammar_paragraph(
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
fn upsert_chapter_paragraph(
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
fn derive(
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
fn gloss_text(project: &Path, language: &str, text: &str) -> Result<()> {
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
fn paradigm(
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




/// LANG-1 P2.7 — scan the manuscript for candidate undefined conlang words.
fn scan_manuscript(project: &Path, language: &str, json: bool) -> Result<()> {
    use std::collections::HashSet;
    use unicode_segmentation::UnicodeSegmentation;

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phonology = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no phoneme block — the scan needs the inventory to tell \
             conlang words from prose"
        ))
    })?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let known: HashSet<String> = entries
        .iter()
        .flat_map(|e| e.surface_forms().into_iter().map(|s| s.to_lowercase()))
        .collect();
    if known.is_empty() {
        eprintln!("note: {language} has no dictionary entries yet — nothing anchors the scan");
    }

    // Every user-book paragraph as a word list (system books are reference
    // material, not manuscript prose).
    let mut paragraphs: Vec<Vec<String>> = Vec::new();
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let mut cursor = Some(node.id);
        let mut is_system = false;
        while let Some(id) = cursor {
            match hierarchy.get(id) {
                Some(n) if n.system_tag.is_some() => {
                    is_system = true;
                    break;
                }
                Some(n) => cursor = n.parent_id,
                None => break,
            }
        }
        if is_system {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(node.id) else { continue };
        let Ok(body) = std::str::from_utf8(&bytes) else { continue };
        paragraphs.push(body.unicode_words().map(String::from).collect());
    }

    let report = crate::conlang::lexicon::scan_undefined(&phonology, &known, &paragraphs);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| Error::Store(format!("serializing scan: {e}")))?
        );
        return Ok(());
    }

    println!(
        "scan {language} · {} paragraph(s), {} in a conlang context",
        report.paragraphs_scanned, report.conlang_paragraphs
    );
    if report.candidates.is_empty() {
        println!("  ✓ no undefined conlang words found");
        return Ok(());
    }
    println!("\n  candidate undefined words ({}):", report.candidates.len());
    for c in &report.candidates {
        println!("    {:<16} ×{}", c.word, c.count);
    }
    eprintln!("\n(heuristic — `add-word` the real ones, fix the typos)");
    Ok(())
}

/// LANG-1 P2.6 — list Places + Characters linked to a language.
fn speakers(project: &Path, language: &str) -> Result<()> {
    use crate::conlang::links::ConlangLinks;
    let (store, _hierarchy, lang_book) = open_lang_book(project, language)?;
    let links = ConlangLinks::load(store.project_root()).map_err(Error::Io)?;
    let (places, characters) = links.speakers_of(&lang_book.title);

    println!("speakers of {}", lang_book.title);
    if places.is_empty() && characters.is_empty() {
        println!("  (none linked yet — see `inkhaven language link-place` / `link-character`)");
        return Ok(());
    }
    if !places.is_empty() {
        println!("\n  places ({}):", places.len());
        for p in &places {
            println!("    {p}");
        }
    }
    if !characters.is_empty() {
        println!("\n  characters ({}):", characters.len());
        for (name, level) in &characters {
            println!("    {name:<20} {level}");
        }
    }
    Ok(())
}

/// LANG-1 P2.4 — query the dictionary by the rich entry fields.
#[allow(clippy::too_many_arguments)]
fn query(
    project: &Path,
    language: &str,
    register: Option<&str>,
    domain: Option<&str>,
    era: Option<&str>,
    pos: Option<&str>,
    text: Option<&str>,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let f = crate::conlang::lexicon::Filter { register, domain, era, pos, text };
    let matches = crate::conlang::lexicon::filter(&entries, &f);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&matches)
                .map_err(|e| Error::Store(format!("serializing query: {e}")))?
        );
        return Ok(());
    }

    println!("{} / {} entr(y/ies) match", matches.len(), entries.len());
    for e in &matches {
        let mut tags = Vec::new();
        if !e.registers.is_empty() {
            tags.push(format!("[{}]", e.registers.join(",")));
        }
        if !e.domain.is_empty() {
            tags.push(format!("{{{}}}", e.domain.join(",")));
        }
        if let Some(era) = &e.era {
            tags.push(format!("<{era}>"));
        }
        let pos = if e.pos.trim().is_empty() { String::new() } else { format!(" ({})", e.pos) };
        println!(
            "  {:<16} {}{}{}",
            e.word,
            e.translation,
            pos,
            if tags.is_empty() { String::new() } else { format!("  {}", tags.join(" ")) }
        );
    }
    Ok(())
}

/// LANG-1 P2.2 — AI-assisted dictionary generation behind the dedup gate.
#[allow(clippy::too_many_arguments)]
fn generate_lexicon(
    project: &Path,
    language: &str,
    topic: Option<&str>,
    count: usize,
    era: Option<&str>,
    register: Option<&str>,
    provider: Option<&str>,
    semantic: bool,
    semantic_threshold: f32,
    yes: bool,
) -> Result<()> {
    use crate::conlang::generate::lexicon as lexgen;

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let phonology = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no phoneme block — add `phonemes` / `classes` / `templates` \
             HJSON under its `Phonology` chapter first"
        ))
    })?;
    if phonology.templates_for(crate::conlang::TemplateRole::Root).is_empty() {
        return Err(Error::Config(format!(
            "language `{language}` declares no `root` templates — needed to generate forms"
        )));
    }
    let existing = load_dictionary(&store, &hierarchy, &lang_book)?;

    let pool = lexgen::build_pool(&phonology, &existing, count);
    if pool.is_empty() {
        return Err(Error::Config(
            "could not generate any valid candidate forms — loosen the phonotactic constraints".into(),
        ));
    }

    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    let work_lang = if cfg.language.trim().is_empty() { "english" } else { cfg.language.trim() };
    eprintln!(
        "inkhaven language generate-lexicon · {language} · model: {model} · glosses in {work_lang}"
    );

    let prompt = build_lexgen_prompt(language, topic, count, era, register, work_lang, &pool);
    let raw = crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(LEXGEN_SYSTEM.to_string()),
        prompt,
    )
    .map_err(|e| Error::Store(format!("inference error: {e}")))?;

    let proposals = match lexgen::parse_proposals(&raw) {
        Ok(p) => p,
        Err(why) => {
            eprintln!("could not parse model reply: {why}\n---- raw ----\n{raw}\n---- end ----");
            return Ok(());
        }
    };
    let (mut kept, rejected) = lexgen::dedup(&phonology, &existing, proposals);

    // Semantic half of the dedup gate: reject near-synonyms by gloss
    // embedding (catches "stone" vs "rock" the string check misses).
    let mut near_synonyms: Vec<(lexgen::LexProposal, f32)> = Vec::new();
    if semantic && !kept.is_empty() {
        let existing_glosses: Vec<&str> = existing
            .iter()
            .map(|e| e.translation.trim())
            .filter(|g| !g.is_empty())
            .collect();
        let kept_glosses: Vec<&str> = kept.iter().map(|p| p.gloss.trim()).collect();
        let existing_vecs = if existing_glosses.is_empty() {
            Vec::new()
        } else {
            store.embed_batch(&existing_glosses)?
        };
        let kept_vecs = store.embed_batch(&kept_glosses)?;
        let (sem_kept, sem_rejected) =
            lexgen::semantic_filter(kept, &existing_vecs, &kept_vecs, semantic_threshold);
        kept = sem_kept;
        near_synonyms = sem_rejected;
    }

    println!(
        "proposed {} entr(y/ies) for {language}{} ({} rejected by the dedup gate):",
        kept.len(),
        topic.map(|t| format!(" · topic: {t}")).unwrap_or_default(),
        rejected.len()
    );
    for p in &kept {
        let pos = if p.pos.trim().is_empty() { "?" } else { p.pos.trim() };
        println!("  {:<16} {} ({})", p.form, p.gloss, pos);
    }
    if !rejected.is_empty() {
        eprintln!("\nrejected:");
        for (p, reason) in &rejected {
            eprintln!("  {:<16} {} — {}", p.form, p.gloss, reason.as_str());
        }
    }
    if !near_synonyms.is_empty() {
        eprintln!("\nrejected (near-synonyms, cosine > {semantic_threshold:.2}):");
        for (p, sim) in &near_synonyms {
            eprintln!("  {:<16} {} — too close ({sim:.2})", p.form, p.gloss);
        }
    }

    if yes {
        let mut added = 0usize;
        for p in &kept {
            // Commit through the rich-import path so the AI's register /
            // domain tags + the batch era land on the entry (P2.5).
            let entry = ImportEntry {
                word: p.form.trim().to_string(),
                pos: if p.pos.trim().is_empty() { "noun".into() } else { p.pos.trim().to_string() },
                translation: p.gloss.trim().to_string(),
                example: p.example.trim().to_string(),
                register: p.register.trim().to_string(),
                domain: p.domain.iter().map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).collect(),
                era: era.unwrap_or("").trim().to_string(),
                ..Default::default()
            };
            match add_imported_dictionary_entry(&store, &cfg, &lang_book, &entry) {
                Ok(_) => added += 1,
                Err(e) => eprintln!("  skipped {}: {e}", p.form),
            }
        }
        eprintln!("\nadded {added} entr(y/ies) to {language}'s Dictionary");
    } else {
        eprintln!(
            "\n(dry run — re-run with --yes to add the {} kept entr(y/ies))",
            kept.len()
        );
        // PANE-1 P3 — surface the advisory proposals in the Output pane with a
        // promote-to-Dictionary action (no-op outside the TUI / active pane).
        emit_lexicon_proposal(language, topic, era, &kept);
    }
    Ok(())
}

pub(crate) fn build_lexgen_prompt(
    language: &str,
    topic: Option<&str>,
    count: usize,
    era: Option<&str>,
    register: Option<&str>,
    work_lang: &str,
    pool: &[String],
) -> String {
    let domain = topic.unwrap_or("core everyday life");
    let candidates = pool
        .iter()
        .map(|f| format!("\"{f}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let mut constraints = format!(
        "Language: {language}. Produce {count} dictionary entries for the semantic domain: {domain}."
    );
    if let Some(e) = era {
        constraints.push_str(&format!(" In-world era: {e}."));
    }
    if let Some(r) = register {
        constraints.push_str(&format!(" Register: {r}."));
    }
    format!(
        "{constraints}\n\n\
         Pick a coherent set of {count} concepts a culture needs for this domain, then assign each \
         a distinct `form` chosen ONLY from the candidate list below. Write every `gloss` and \
         `example` in {work_lang}. Do not repeat a meaning. Keep `pos` a short lowercase tag. Tag \
         each entry with a `register` and one or two `domain` tags appropriate to its concept.\n\n\
         Candidate forms (choose from these): [{candidates}]\n\n\
         Reply with the JSON object only."
    )
}

/// LANG-1 P1.6 — apply tone sandhi to an explicit tone sequence.
fn tone_sandhi(project: &Path, language: &str, tones: &str) -> Result<()> {
    let (_store, phonology) = open_phonology(project, language)?;
    let system = phonology.tone.as_ref().ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` declares no `tone` system in its Phonology block"
        ))
    })?;
    let input: Vec<String> = tones.split_whitespace().map(String::from).collect();
    let surface = crate::conlang::phonology::tone_eval::apply_sandhi(system, &input);
    println!("{}", surface.join(" "));
    Ok(())
}

/// LANG-1 P1.5 — convert between IPA and a named romanization scheme.
fn romanize_text(
    project: &Path,
    language: &str,
    text: &str,
    scheme: Option<&str>,
    reverse: bool,
) -> Result<()> {
    use crate::conlang::phonology::romanize;

    let (_store, phonology) = open_phonology(project, language)?;
    let scheme_ref = phonology.scheme(scheme).ok_or_else(|| {
        Error::Config(match scheme {
            Some(s) => format!("language `{language}` has no romanization scheme `{s}`"),
            None => format!(
                "language `{language}` declares no romanization schemes — add a `romanizations` \
                 block to its Phonology, or rely on the per-phoneme `romanize` field"
            ),
        })
    })?;

    if reverse {
        let seq = romanize::deromanize(scheme_ref, &phonology, text);
        println!("/{}/", seq.join(""));
    } else {
        let seq: Vec<String> = text.split_whitespace().map(String::from).collect();
        println!("{}", romanize::romanize(scheme_ref, &phonology, &seq));
    }
    Ok(())
}

/// LANG-1 P1.4 — place primary stress on a word per the language's stress
/// rule and print the syllabification with `ˈ` before the stressed syllable.
fn stress_word(project: &Path, language: &str, word: &str) -> Result<()> {
    use crate::conlang::phonology::{stress_eval, syllable};

    let (_store, phonology) = open_phonology(project, language)?;
    let rule = phonology.stress.clone().ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` declares no `stress` rule in its Phonology block \
             (e.g. `stress: \"penultimate\"`)"
        ))
    })?;

    let seq = phonology.segment(word);
    let sylls = syllable::syllabify(&phonology, &seq);
    let stressed = stress_eval::primary_stress(&rule, &sylls);

    let g = |ipa: &String| {
        phonology
            .phoneme(ipa)
            .map(|p| p.grapheme().to_string())
            .unwrap_or_else(|| ipa.clone())
    };
    let out = sylls
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let body: String = s.onset.iter().chain(&s.nucleus).chain(&s.coda).map(&g).collect();
            if Some(i) == stressed {
                format!("ˈ{body}")
            } else {
                body
            }
        })
        .collect::<Vec<_>>()
        .join(".");
    println!("{out}");
    Ok(())
}

/// LANG-1 P1.3 — derive and print a word's surface pronunciation by applying
/// the language's allophony rules to its underlying form.
fn ipa_surface(project: &Path, language: &str, word: &str) -> Result<()> {
    let (_store, phonology) = open_phonology(project, language)?;
    let underlying = phonology.segment(word);
    let surface = crate::conlang::phonology::allophony_eval::surface_form(&phonology, &underlying);

    let render_ipa = |seq: &[String]| seq.join("");
    let render_roman = |seq: &[String]| -> String {
        seq.iter()
            .map(|ipa| {
                phonology
                    .phoneme(ipa)
                    .map(|p| p.grapheme().to_string())
                    .unwrap_or_else(|| ipa.clone())
            })
            .collect()
    };

    println!("underlying  /{}/", render_ipa(&underlying));
    println!("surface     [{}]", render_ipa(&surface));
    println!("romanized    {}", render_roman(&surface));
    Ok(())
}

/// LANG-1 P1.2 — syllabify a word against a language's phonology and print
/// the `CV.CVC`-style breakdown. Loads the Phonology block, segments the
/// word into phonemes (longest-grapheme match), and runs the sonority-aware
/// syllabifier.
fn syllabify_word(project: &Path, language: &str, word: &str) -> Result<()> {
    let (_store, phonology) = open_phonology(project, language)?;
    let seq = phonology.segment(word);
    let sylls = crate::conlang::phonology::syllable::syllabify(&phonology, &seq);
    println!("{}", crate::conlang::phonology::syllable::render(&phonology, &sylls));
    eprintln!(
        "{} → {} syllable(s), {} phoneme(s)",
        word,
        sylls.len(),
        seq.len()
    );
    Ok(())
}

/// LANG-1 P1.1 — generate deterministic candidate words from a language's
/// phonotactic templates.  Loads the typed phoneme block from the language's
/// `Phonology` chapter (whichever paragraph holds the HJSON), samples
/// `count` words for the requested role, and prints those that satisfy every
/// declared constraint.  Empty / absent phonology is a clear, actionable
/// error rather than a silent empty list.
fn generate_word(project: &Path, language: &str, role: &str, count: usize) -> Result<()> {
    let role = crate::conlang::TemplateRole::parse(role).ok_or_else(|| {
        Error::Config(format!(
            "unknown role `{role}` — use root | prefix | suffix | infix | circumfix | compound"
        ))
    })?;

    let (_store, phonology) = open_phonology(project, language)?;

    if phonology.templates_for(role).is_empty() {
        return Err(Error::Config(format!(
            "language `{language}` declares no `{}` templates in its Phonology block",
            role.as_str()
        )));
    }

    let words = crate::conlang::generate::word::generate_words(&phonology, role, count);
    if words.is_empty() {
        eprintln!(
            "no words satisfied the constraints in {} attempts — loosen the phonotactic constraints",
            count
        );
        return Ok(());
    }
    for w in &words {
        println!("{w}");
    }
    eprintln!(
        "generated {} / {} requested `{}` word(s) for {}",
        words.len(),
        count,
        role.as_str(),
        language
    );
    Ok(())
}

/// Open a project and resolve a language sub-book under the `Language`
/// system book. The shared front-half of every conlang command — returns the
/// open `Store` (kept alive for the DuckDB lock), the loaded `Hierarchy`, and
/// the language's `Book` node.
fn open_lang_book(
    project: &Path,
    language: &str,
) -> Result<(Store, Hierarchy, crate::store::node::Node)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .ok_or_else(|| {
            Error::Store("Language system book missing — re-open the project to seed it".into())
        })?
        .clone();
    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language))
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` not found — run `inkhaven language init {language}` first"
            ))
        })?;
    Ok((store, hierarchy, lang_book))
}

// ── Store-based API for the Bund `ink.lang.*` stdlib (1.3.21) ─────────────
//
// The CLI handlers above re-open the store from a `&Path`; a Bund script
// already holds the active project store, so these variants take `&Store`
// (and an already-loaded `&Hierarchy`) instead, reusing the same engine
// loaders and node-creation paths.

/// Find a language sub-book by name in an already-loaded hierarchy.
pub(crate) fn find_language_book(
    hierarchy: &Hierarchy,
    name: &str,
) -> Result<crate::store::node::Node> {
    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .ok_or_else(|| {
            Error::Store("Language system book missing — re-open the project to seed it".into())
        })?;
    hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(name))
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{name}` not found — create it first (e.g. `ink.lang.init`)"
            ))
        })
}

/// Create a language sub-book plus its scaffold chapters, using the active
/// store. Errors if the name already exists. Returns the new book node.
pub(crate) fn init_language(
    store: &Store,
    cfg: &Config,
    name: &str,
) -> Result<crate::store::node::Node> {
    let hierarchy = Hierarchy::load(store)?;
    let lang_book = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store("Language system book missing — re-open the project to seed it".into())
        })?;
    if hierarchy
        .children_of(Some(lang_book.id))
        .iter()
        .any(|n| n.title.eq_ignore_ascii_case(name))
    {
        return Err(Error::Config(format!(
            "language `{name}` already exists under Language"
        )));
    }
    let hierarchy = Hierarchy::load(store)?;
    let per_lang = store.create_node(
        cfg,
        &hierarchy,
        NodeKind::Book,
        name,
        Some(&lang_book),
        None,
        InsertPosition::End,
    )?;
    scaffold_language_chapters(store, cfg, &per_lang, |_| {})?;
    Ok(per_lang)
}

/// Create a paragraph with an HJSON `body` under a named chapter of a language
/// sub-book — the primitive behind `ink.lang.define`, which lets a Bund script
/// write a phonology / grammar / morphology / typology / sample block exactly as
/// it would appear in the book. Returns the created paragraph node.
pub(crate) fn create_chapter_paragraph(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    chapter_title: &str,
    paragraph_title: &str,
    body: &str,
) -> Result<crate::store::node::Node> {
    let hierarchy = Hierarchy::load(store)?;
    let chapter = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case(chapter_title))
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{}` has no `{chapter_title}` chapter (try Phonology, Grammar, \
                 Sample texts, or Meta)",
                lang_book.title
            ))
        })?;
    let hierarchy = Hierarchy::load(store)?;
    let mut entry = store.create_node(
        cfg,
        &hierarchy,
        NodeKind::Paragraph,
        paragraph_title,
        Some(&chapter),
        None,
        InsertPosition::End,
    )?;
    entry.content_type = Some("hjson".to_string());
    if let Some(rel) = &entry.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes())
            .map_err(|e| Error::Store(format!("write block: {e}")))?;
    }
    store
        .update_paragraph_content(&mut entry, body.as_bytes())
        .map_err(|e| Error::Store(format!("seed block: {e}")))?;
    Ok(entry)
}

/// Set one typology answer (validated against the WALS catalogue) and persist
/// the `{ grammar: { … } }` block. Store-based; for `ink.lang.grammar_set`.
pub(crate) fn set_grammar_feature(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    feature: &str,
    value: &str,
) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let (mut spec, node) = load_grammar_spec(store, &hierarchy, lang_book)?;
    let f = crate::conlang::grammar::feature(feature)
        .ok_or_else(|| Error::Config(format!("unknown typology feature `{feature}`")))?;
    if !f.is_valid(value) {
        return Err(Error::Config(format!(
            "`{value}` is not a valid value for `{}` — options: {}",
            f.id,
            f.values()
        )));
    }
    spec.grammar.insert(f.id.to_string(), value.to_lowercase());
    let body = serde_json::to_string_pretty(&spec)
        .map_err(|e| Error::Store(format!("serializing grammar: {e}")))?;
    upsert_grammar_paragraph(store, cfg, lang_book, "typology", node, &body)
}

/// Append an idiom to the language's expressions block (store-based).
pub(crate) fn add_idiom(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    form: &str,
    literal: &str,
    meaning: &str,
) -> Result<()> {
    use crate::conlang::types::expression::Idiom;
    let hierarchy = Hierarchy::load(store)?;
    let (mut expr, node) = load_expressions(store, &hierarchy, lang_book)?;
    expr.idioms.push(Idiom {
        form: form.trim().to_string(),
        literal: literal.trim().to_string(),
        meaning: meaning.trim().to_string(),
        register: Vec::new(),
    });
    let body = serde_json::to_string_pretty(&expr)
        .map_err(|e| Error::Store(format!("serializing expressions: {e}")))?;
    upsert_grammar_paragraph(store, cfg, lang_book, "expressions", node, &body)
}

/// Append a conceptual metaphor to the language's expressions block.
pub(crate) fn add_metaphor(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    source: &str,
    target: &str,
) -> Result<()> {
    use crate::conlang::types::expression::Metaphor;
    let hierarchy = Hierarchy::load(store)?;
    let (mut expr, node) = load_expressions(store, &hierarchy, lang_book)?;
    expr.metaphors.push(Metaphor {
        source: source.trim().to_string(),
        target: target.trim().to_string(),
        examples: Vec::new(),
        note: String::new(),
    });
    let body = serde_json::to_string_pretty(&expr)
        .map_err(|e| Error::Store(format!("serializing expressions: {e}")))?;
    upsert_grammar_paragraph(store, cfg, lang_book, "expressions", node, &body)
}

/// Delete a dictionary entry by headword (store-based; for `ink.lang.remove_word`).
pub(crate) fn remove_dictionary_entry(
    store: &Store,
    lang_book: &crate::store::node::Node,
    word: &str,
) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let dictionary = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary"))
        .cloned()
        .ok_or_else(|| Error::Config("language has no Dictionary chapter".into()))?;
    let bucket = derive_alphabet_bucket(store, &hierarchy, lang_book, word)?
        .or_else(|| alphabet_bucket(word))
        .ok_or_else(|| Error::Config(format!("could not derive alphabet bucket from `{word}`")))?;
    let subchapter = hierarchy
        .children_of(Some(dictionary.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Subchapter && n.title.eq_ignore_ascii_case(&bucket))
        .cloned()
        .ok_or_else(|| Error::Config(format!("`{word}` isn't defined (no `{bucket}` bucket)")))?;
    let entry = hierarchy
        .children_of(Some(subchapter.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Paragraph && n.title.eq_ignore_ascii_case(word))
        .cloned()
        .ok_or_else(|| Error::Config(format!("word `{word}` not found")))?;
    let ids = hierarchy.collect_subtree(entry.id);
    let fs_rel = entry
        .file
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    store
        .delete_subtree(&fs_rel, &ids)
        .map_err(|e| Error::Store(format!("delete entry: {e}")))?;
    Ok(())
}

/// Open a project and load a language's `Phonology` value — the shared
/// front-half of every P1 phonology inspector / generator.
fn open_phonology(project: &Path, language: &str) -> Result<(Store, crate::conlang::Phonology)> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phonology = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no phoneme block yet — add `phonemes` / `classes` / \
             `templates` HJSON under its `Phonology` chapter (see Documentation/PROPOSALS/LANG-1_PLAN.md)"
        ))
    })?;
    Ok((store, phonology))
}

/// Load every parseable `DictionaryEntry` under a language's `Dictionary`
/// chapter (across all alphabet subchapters).
pub(crate) fn load_dictionary(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<Vec<crate::language_entry::DictionaryEntry>> {
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary"))
        .cloned()
    else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for id in hierarchy.collect_subtree(chapter.id) {
        let Some(node) = hierarchy.get(id) else { continue };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(node.id) else { continue };
        let body = String::from_utf8_lossy(&bytes);
        if let Ok(Some(entry)) = crate::language_entry::parse(&body) {
            out.push(entry);
        }
    }
    Ok(out)
}

/// 1.3.19 LANG-1 P6 — semantic-gap finder. Diff the lexicon's glosses against a
/// reference concept scope (built-in Swadesh-100, keyed to the project working
/// language, or an HJSON file) and report the missing concepts, frequency-ranked.
fn gaps(project: &Path, language: &str, scope: &str, json: bool) -> Result<()> {
    use crate::conlang::gaps as gapmod;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let glosses: Vec<String> = entries
        .iter()
        .map(|e| e.translation.clone())
        .filter(|g| !g.trim().is_empty())
        .collect();

    // Resolve the scope: a built-in name → bundled list in the working
    // language; otherwise treat `scope` as a path to an HJSON concept list.
    let (scope_name, concepts) = if gapmod::is_builtin(scope) {
        (
            format!("Swadesh-100 ({})", cfg.language),
            gapmod::swadesh_100(&cfg.language),
        )
    } else {
        let path = Path::new(scope);
        let body = std::fs::read_to_string(path).map_err(|e| {
            Error::Config(format!(
                "scope `{scope}` is neither a built-in (swadesh_100) nor a readable file: {e}"
            ))
        })?;
        let parsed = gapmod::ScopeFile::from_hjson(&body).map_err(Error::Config)?;
        let name = parsed
            .name
            .clone()
            .unwrap_or_else(|| path.display().to_string());
        (name, parsed.into_concepts())
    };

    let report = gapmod::find_gaps(&scope_name, &concepts, &glosses);

    if json {
        let out = serde_json::json!({
            "scope": report.scope_name,
            "total": report.total,
            "covered": report.covered,
            "missing": report.missing,
            "coverage_pct": report.coverage_pct(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&out)
                .map_err(|e| Error::Store(format!("serializing gap report: {e}")))?
        );
        return Ok(());
    }

    println!(
        "{} — {}/{} concepts covered ({:.0}%)",
        report.scope_name,
        report.covered.len(),
        report.total,
        report.coverage_pct()
    );
    if report.missing.is_empty() {
        println!("\n✓ full coverage — nothing missing in this scope.");
        return Ok(());
    }
    println!("\nMissing ({}), most-core first:", report.missing.len());
    for chunk in report.missing.chunks(6) {
        println!("  {}", chunk.join(", "));
    }
    println!(
        "\nFeed these to the generator, e.g.:\n  \
         inkhaven language generate-lexicon {language} --topic \"{}\" --count {}",
        report.missing.iter().take(3).cloned().collect::<Vec<_>>().join(", "),
        report.missing.len().min(20),
    );
    Ok(())
}

/// Format a rendered clause as `surface` + a two-line interlinear gloss + a
/// literal rendering — the same shape the grammar book uses.
fn format_clause(r: &crate::conlang::syntax::RenderedClause) -> String {
    let widths: Vec<usize> = r
        .words
        .iter()
        .map(|(w, g)| w.chars().count().max(g.chars().count()) + 2)
        .collect();
    let (mut l1, mut l2) = (String::new(), String::new());
    for (i, (w, g)) in r.words.iter().enumerate() {
        l1.push_str(&format!("{:<width$}", w, width = widths[i]));
        l2.push_str(&format!("{:<width$}", g, width = widths[i]));
    }
    format!(
        "{}\n  {}\n  {}\n  ‘{}’",
        r.surface,
        l1.trim_end(),
        l2.trim_end(),
        r.literal
    )
}

/// 1.3.19 LANG-1 P6 — creative text generators. Deterministic names / prose /
/// verse from the phonology + lexicon + syntax engine; AI-composed but
/// lexicon-constrained blessing / curse / incantation. Prints only — never
/// writes to the book.
fn compose(
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

/// A compact human summary of the typology answers for an AI prompt.
pub(crate) fn summarize_typology(typology: &std::collections::BTreeMap<String, String>) -> String {
    if typology.is_empty() {
        return "word order: SVO; alignment: nominative–accusative (defaults)".into();
    }
    typology
        .iter()
        .map(|(k, v)| format!("{}: {}", k.replace('_', " "), v))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Print a warning listing any whitespace token on a `NATIVE:` line that is not
/// a known lexicon surface form (bare headword or an inflected form). Purely
/// advisory — the constraint is enforced by the prompt, this just surfaces drift.
fn warn_unknown_tokens(text: &str, entries: &[crate::language_entry::DictionaryEntry]) {
    let mut known: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for e in entries {
        if !e.word.trim().is_empty() {
            known.insert(e.word.to_lowercase());
        }
        for v in e.inflection.values() {
            known.insert(v.to_lowercase());
        }
    }
    let mut unknown: Vec<String> = Vec::new();
    for line in text.lines() {
        let lower = line.trim_start().to_lowercase();
        if let Some(rest) = lower.strip_prefix("native:") {
            for tok in rest.split(|c: char| !c.is_alphanumeric()) {
                let t = tok.trim();
                if t.len() > 1 && !known.contains(t) && !unknown.contains(&t.to_string()) {
                    unknown.push(t.to_string());
                }
            }
        }
    }
    if !unknown.is_empty() {
        eprintln!(
            "\n⚠ {} token(s) not in the lexicon (model may have drifted): {}",
            unknown.len(),
            unknown.join(", ")
        );
    }
}

/// LANG-1 P6.1 — descriptive language profile: inventory balance, phoneme
/// frequency, syllable-length distribution, onset/coda usage, POS spread.
fn stats(project: &Path, language: &str, json: bool) -> Result<()> {
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

/// Syllabified surface pronunciation of a headword (e.g. `ka.ta`), or `None`
/// when it doesn't read as the language's phonemes.
pub(crate) fn pronounce(phon: &crate::conlang::Phonology, word: &str) -> Option<String> {
    let seq = phon.segment(&word.to_lowercase());
    if seq.is_empty() || !seq.iter().all(|s| phon.phoneme(s).is_some()) {
        return None;
    }
    let surface = crate::conlang::phonology::allophony_eval::surface_form(phon, &seq);
    let sylls = crate::conlang::phonology::syllable::syllabify(phon, &surface);
    if sylls.is_empty() {
        return None;
    }
    Some(
        sylls
            .iter()
            .map(|s| format!("{}{}{}", s.onset.join(""), s.nucleus.join(""), s.coda.join("")))
            .collect::<Vec<_>>()
            .join("."),
    )
}

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
fn grammar_study_brief(
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

fn grammar_book(
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
fn strip_code_fence(text: &str) -> String {
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
fn tutorial(
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
fn dictionary(
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

/// LANG-1 P2.1 — deterministic lexicon audit: phonotactic violations,
/// homophones (surface-form collisions), and duplicate meanings.
fn audit(project: &Path, language: &str, json: bool) -> Result<()> {
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

/// Find and parse the `Phonology`-chapter HJSON block for a language
/// sub-book.  Scans every paragraph under the `Phonology` chapter and
/// returns the first that parses as a phonology block (so the author can keep
/// it in `overview`, a dedicated `inventory` paragraph, or wherever).
pub(crate) fn load_phonology(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<Option<crate::conlang::Phonology>> {
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Phonology"))
        .cloned()
    else {
        return Ok(None);
    };
    for para in hierarchy.children_of(Some(chapter.id)) {
        if para.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(bytes) = store.get_content(para.id)? else {
            continue;
        };
        let body = String::from_utf8_lossy(&bytes);
        match crate::conlang::Phonology::from_hjson(&body) {
            Ok(Some(p)) if !p.phonemes.is_empty() => return Ok(Some(p)),
            Ok(_) => continue,
            // A malformed block under Phonology is worth surfacing.
            Err(e) => return Err(Error::Config(e)),
        }
    }
    Ok(None)
}

/// The five standard chapters every language book
/// gets at scaffold time.  Order matches the order
/// authors are most likely to fill them in:
///
///   * Meta — language metadata HJSON (alphabet,
///     language_kind, family, reading_direction).
///     Author fills first because every other
///     chapter depends on the alphabet.
///   * Dictionary — entries land here under
///     alphabet subchapters (auto-created on first
///     entry in each letter).
///   * Grammar — HJSON rules the AI translator
///     consumes.
///   * Phonology — sound rules kept separate so
///     they don't bloat every translation prompt.
///   * Sample texts — few-shot examples for the AI
///     plus author re-immersion material.
const STANDARD_CHAPTERS: &[&str] = &[
    "Meta",
    "Dictionary",
    "Grammar",
    "Phonology",
    "Sample texts",
];

/// Seed body for `Meta/overview` — pure HJSON so the
/// editor renders with HJSON syntax highlighting.
/// The paragraph's `content_type` is set to `"hjson"`
/// at create time; the body is just the metadata
/// object (no Typst headings, no markdown fences).
///
/// switched FROM the Typst-
/// with-fenced-HJSON format to pure HJSON because the
/// Typst editor mode rendered the body as a heading +
/// opaque code fence instead of as structured config.
/// The translation prompt composer + parser handle
/// both formats; new entries use pure HJSON, legacy
/// Typst-wrapped entries still parse via the fence
/// extractor.
const META_OVERVIEW_BODY: &str = "{
  // ──────────────────────────────────────────────────
  // IDENTITY
  // ──────────────────────────────────────────────────

  // Display name for the language.
  name: \"\"

  // Sibling languages (e.g. Elvish, Romance, Slavic).
  // Phase D.2 will use this for cross-language family
  // browsing in the sidebar.
  family: \"\"

  // \"constructed\" | \"natural\" — drives default
  // assumptions in the AI translator.  Constructed
  // languages get stricter adherence to the explicit
  // rules below; natural languages let the LLM lean
  // more on its pretraining.
  language_kind: constructed

  // Optional ISO 639-3 code (e.g. \"qya\" for Quenya).
  // Used by the multilingual prompt resolver when
  // mixing this language with the project's working
  // language flow.
  iso_code: \"\"

  // ──────────────────────────────────────────────────
  // ORTHOGRAPHY
  // ──────────────────────────────────────────────────

  // Alphabet entries in canonical order.  For non-
  // Latin orthographies, override with the author's
  // declared groupings:
  //   * paired-case Latin: [\"Aa\", \"Bb\", \"Cc\"]
  //   * Hebrew letter names: [\"Aleph\", \"Beth\", \"Gimel\"]
  //   * Greek: [\"Α\", \"Β\", \"Γ\"]
  //   * Cyrillic: [\"А\", \"Б\", \"В\"]
  //   * Polish digraphs: [\"A\", \"Cz\", \"Dz\", \"Sz\"]
  // Drives Dictionary bucket auto-creation in
  // `inkhaven language add-word` and the in-TUI `+`
  // chord.
  alphabet: [\"A\", \"B\", \"C\", \"D\", \"E\", \"F\", \"G\", \"H\", \"I\",
             \"J\", \"K\", \"L\", \"M\", \"N\", \"O\", \"P\", \"Q\", \"R\",
             \"S\", \"T\", \"U\", \"V\", \"W\", \"X\", \"Y\", \"Z\"]

  // \"ltr\" (default) | \"rtl\" | \"ttb\" (top-to-bottom)
  reading_direction: ltr

  // Script / writing system name (Latin, Cyrillic,
  // Tengwar, Devanagari, …).  Free-form; informational.
  script: \"\"

  // ──────────────────────────────────────────────────
  // LINGUISTIC SHAPE — quick-reference summary the
  // AI translator reads before composing prompts.
  // ──────────────────────────────────────────────────

  // Word order: SVO | SOV | VSO | VOS | OSV | OVS | free
  word_order: \"\"

  // Morphological type: isolating | agglutinative |
  // fusional | polysynthetic | mixed
  morphology: \"\"

  // Tonal: true | false (informational only).
  tonal: false

  // Has grammatical case (declension)?
  has_cases: false

  // Has grammatical gender?
  has_gender: false

  // ──────────────────────────────────────────────────
  // RUNTIME / TOOLING
  // ──────────────────────────────────────────────────

  // Optional Snowball stemmer algo name (\"english\",
  // \"russian\", \"french\", \"spanish\", \"german\").
  // Rare for conlangs — leave empty to let the
  // lexicon overlay rely on the dictionary
  // `inflection` paradigm fields instead.
  stemmer: \"\"

  // Free-form citation for the canonical sample
  // corpus the LLM should treat as authoritative
  // (Tolkien's Etymologies, Klingon Dictionary, etc.).
  example_corpus_ref: \"\"

  // ──────────────────────────────────────────────────
  // NOTES
  // ──────────────────────────────────────────────────

  // Worldbuilding context — who speaks the language,
  // where, in what era, what register.  Read by the
  // human author; the LLM only consumes the
  // structured fields above when composing
  // translation prompts.
  notes: \"\"
}
";

fn init(project: &Path, name: &str) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    let lang_book = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it"
                    .into(),
            )
        })?;

    // Reject duplicate before the create so the
    // failure mode is a friendly error, not a
    // silent `-2` slug suffix on the second
    // attempt.
    if hierarchy
        .children_of(Some(lang_book.id))
        .iter()
        .any(|n| n.title.eq_ignore_ascii_case(name))
    {
        return Err(Error::Config(format!(
            "language `{name}` already exists under Language"
        )));
    }

    let hierarchy = Hierarchy::load(&store)?;
    let per_lang = store.create_node(
        &cfg,
        &hierarchy,
        NodeKind::Book,
        name,
        Some(&lang_book),
        None,
        InsertPosition::End,
    )?;
    eprintln!(
        "created language book `{name}` at {}",
        hierarchy.slug_path(&per_lang),
    );

    scaffold_language_chapters(&store, &cfg, &per_lang, |chapter_title| {
        eprintln!("  · {chapter_title}");
    })?;

    eprintln!("\nNext steps:");
    eprintln!(
        "  · edit `Language/{name}/Meta/overview` to set the alphabet + metadata"
    );
    eprintln!(
        "  · add dictionary entries under `Language/{name}/Dictionary` (`inkhaven language add-word`)"
    );
    eprintln!(
        "  · add grammar rules under `Language/{name}/Grammar` for the AI translation flow"
    );

    Ok(())
}

/// shared scaffold helper.
/// Creates the 5 standard chapters under an already-
/// existing per-language book + seeds
/// `Meta/overview` with the starter HJSON.  Used by
/// both the CLI `init` path and the in-TUI tree-pane
/// commit path (see `App::provision_language_book`)
/// so the two entry points produce identical
/// scaffolds.
///
/// `on_chapter` is called for each chapter at create
/// time so the caller can emit progress (CLI prints
/// `· Meta`; the TUI updates the status bar).
pub(crate) fn scaffold_language_chapters(
    store: &Store,
    cfg: &Config,
    per_lang: &crate::store::node::Node,
    mut on_chapter: impl FnMut(&str),
) -> Result<()> {
    for title in STANDARD_CHAPTERS {
        // Reload between creates so each subsequent
        // create sees the previous create's slug +
        // order.
        let hierarchy = Hierarchy::load(store)?;
        let chapter = store.create_node(
            cfg,
            &hierarchy,
            NodeKind::Chapter,
            title,
            Some(per_lang),
            None,
            InsertPosition::End,
        )?;
        on_chapter(title);
        if *title == "Meta" {
            let hierarchy = Hierarchy::load(store)?;
            let mut overview = store.create_node(
                cfg,
                &hierarchy,
                NodeKind::Paragraph,
                "overview",
                Some(&chapter),
                None,
                InsertPosition::End,
            )?;
            // Switch to HJSON content type so the editor
            // renders with syntax highlighting + the
            // paragraph status bar shows `[hjson]` to
            // match the rest of the project's HJSON
            // configuration paragraphs.  Mutating
            // `node.content_type` before
            // `update_paragraph_content` lets the
            // metadata write inside that call persist
            // the change.
            overview.content_type = Some("hjson".to_string());
            // `update_paragraph_content` only writes
            // to bdslib — the on-disk `.typ` file
            // (already created with the default
            // `= overview\n\n` template by
            // `create_node`) needs an explicit
            // overwrite so the editor (which reads
            // from disk) sees the seeded body.  Same
            // pattern `ensure_system_books` uses for
            // its seeded paragraphs.
            if let Some(rel) = &overview.file {
                let abs = store.project_root().join(rel);
                std::fs::write(&abs, META_OVERVIEW_BODY.as_bytes())
                    .map_err(|e| Error::Store(format!("write overview: {e}")))?;
            }
            store
                .update_paragraph_content(&mut overview, META_OVERVIEW_BODY.as_bytes())
                .map_err(|e| Error::Store(format!("seed overview: {e}")))?;
        }
    }
    Ok(())
}

/// `inkhaven language add-word`.
/// Resolves the target language sub-book by case-
/// insensitive title; finds its Dictionary chapter;
/// derives the alphabet bucket for the new word from
/// the first character (auto-creates the subchapter
/// when missing); rejects duplicate words.
fn add_word(
    project: &Path,
    language: &str,
    word: &str,
    pos: &str,
    translation: &str,
    example: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;

    let hierarchy = Hierarchy::load(&store)?;
    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it"
                    .into(),
            )
        })?
        .clone();

    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` not found — run `inkhaven language init {language}` first"
            ))
        })?;

    let (entry, bucket) = add_dictionary_entry_impl(
        &store,
        &cfg,
        &lang_book,
        word,
        pos,
        translation,
        example,
    )?;
    let _ = entry;
    eprintln!(
        "added `{word}` to `{language}/Dictionary/{bucket}` ({pos} · {translation})"
    );
    Ok(())
}

/// shared "add dictionary
/// entry" implementation used by:
///   * the CLI `add-word` subcommand (above);
///   * the in-TUI tree-pane Add Paragraph (`+`) commit
///     handler when the cursor sits anywhere under
///     `Language/<lang>/Dictionary`.
///
/// Caller supplies the per-language Book; we look up
/// the Dictionary chapter, derive the alphabet bucket
/// (consulting Meta/overview first, first-char
/// uppercase as fallback), find-or-create the bucket
/// subchapter, reject duplicates, create the entry
/// paragraph, and seed its body with the HJSON
/// template (POS / translation / example fields are
/// left empty in the TUI flow — the author fills them
/// in by editing the paragraph).
///
/// Returns `(entry_node, bucket_name)` so callers can
/// surface a status message or move the tree cursor.
pub(crate) fn add_dictionary_entry_impl(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    word: &str,
    pos: &str,
    translation: &str,
    example: Option<&str>,
) -> Result<(crate::store::node::Node, String)> {
    let body = seed_dictionary_entry_body(word, pos, translation, example);
    create_dictionary_entry(store, cfg, lang_book, word, &body)
}

/// fully-populated entry record
/// used by the CSV import path.  Distinct from the
/// `language_entry::DictionaryEntry` parser type
/// because we own this one (mutable builder) and the
/// parser one is immutable (deserialised view).
#[derive(Debug, Default, Clone)]
pub(crate) struct ImportEntry {
    pub word: String,
    pub pos: String,
    pub translation: String,
    pub example: String,
    pub pronunciation: String,
    pub etymology: String,
    pub related: Vec<String>,
    pub inflection: std::collections::BTreeMap<String, String>,
    pub examples: Vec<String>,
    pub register: String,
    pub era: String,
    pub notes: String,
    /// LANG-1 P2.4/P2.5 — semantic-domain tags.
    pub domain: Vec<String>,
}

/// Add a fully-populated dictionary entry from an
/// import row.  Bypasses the verbose commented seed
/// template and writes compact HJSON with only the
/// populated fields.  Shares the bucket-derivation +
/// duplicate-check + persistence machinery with the
/// interactive `add_dictionary_entry_impl`.
pub(crate) fn add_imported_dictionary_entry(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    entry: &ImportEntry,
) -> Result<(crate::store::node::Node, String)> {
    let body = build_imported_entry_body(entry);
    create_dictionary_entry(store, cfg, lang_book, &entry.word, &body)
}

/// Shared bucket-derivation + duplicate-check +
/// node-creation + disk/bdslib persistence for both
/// the interactive and bulk-import paths.  Body is
/// passed verbatim — callers pick whether they want
/// the verbose commented template or a compact
/// concrete entry.
fn create_dictionary_entry(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    word: &str,
    body: &str,
) -> Result<(crate::store::node::Node, String)> {
    let hierarchy = Hierarchy::load(store)?;
    let dictionary = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary")
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{}` has no `Dictionary` chapter — likely scaffolded with a pre-Phase-A inkhaven",
                lang_book.title
            ))
        })?;
    let bucket = derive_alphabet_bucket(store, &hierarchy, lang_book, word)?
        .or_else(|| alphabet_bucket(word))
        .ok_or_else(|| {
            Error::Config(format!("could not derive alphabet bucket from `{word}`"))
        })?;
    let dictionary_kids = hierarchy.children_of(Some(dictionary.id));
    let subchapter = match dictionary_kids
        .iter()
        .find(|n| n.kind == NodeKind::Subchapter && n.title == bucket)
        .cloned()
    {
        Some(existing) => existing.clone(),
        None => {
            let hierarchy = Hierarchy::load(store)?;
            store.create_node(
                cfg,
                &hierarchy,
                NodeKind::Subchapter,
                &bucket,
                Some(&dictionary),
                None,
                InsertPosition::End,
            )?
        }
    };
    let hierarchy = Hierarchy::load(store)?;
    if hierarchy
        .children_of(Some(subchapter.id))
        .iter()
        .any(|n| n.title.eq_ignore_ascii_case(word))
    {
        return Err(Error::Config(format!(
            "word `{word}` already defined under `{}/Dictionary/{bucket}`",
            lang_book.title
        )));
    }
    let hierarchy = Hierarchy::load(store)?;
    let mut entry = store.create_node(
        cfg,
        &hierarchy,
        NodeKind::Paragraph,
        word,
        Some(&subchapter),
        None,
        InsertPosition::End,
    )?;
    entry.content_type = Some("hjson".to_string());
    if let Some(rel) = &entry.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes())
            .map_err(|e| Error::Store(format!("write entry: {e}")))?;
    }
    store
        .update_paragraph_content(&mut entry, body.as_bytes())
        .map_err(|e| Error::Store(format!("seed entry: {e}")))?;
    Ok((entry, bucket))
}

/// compact concrete HJSON for an
/// imported entry.  Emits ONLY the fields the import
/// row actually populated; skips empty optional
/// fields entirely so the resulting paragraph reads
/// cleanly when the author opens it.
fn build_imported_entry_body(entry: &ImportEntry) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!("  word:         \"{}\"\n", escape_hjson(&entry.word)));
    out.push_str(&format!("  type:         \"{}\"\n", escape_hjson(&entry.pos)));
    out.push_str(&format!(
        "  translation:  \"{}\"\n",
        escape_hjson(&entry.translation)
    ));
    if !entry.example.is_empty() {
        out.push_str(&format!(
            "  example:      \"{}\"\n",
            escape_hjson(&entry.example)
        ));
    }
    if !entry.examples.is_empty() {
        out.push_str("  examples: [\n");
        for ex in &entry.examples {
            out.push_str(&format!("    \"{}\"\n", escape_hjson(ex)));
        }
        out.push_str("  ]\n");
    }
    if !entry.pronunciation.is_empty() {
        out.push_str(&format!(
            "  pronunciation: \"{}\"\n",
            escape_hjson(&entry.pronunciation)
        ));
    }
    if !entry.etymology.is_empty() {
        out.push_str(&format!(
            "  etymology:    \"{}\"\n",
            escape_hjson(&entry.etymology)
        ));
    }
    if !entry.related.is_empty() {
        let items: Vec<String> = entry
            .related
            .iter()
            .map(|r| format!("\"{}\"", escape_hjson(r)))
            .collect();
        out.push_str(&format!("  related:      [{}]\n", items.join(", ")));
    }
    if !entry.inflection.is_empty() {
        out.push_str("  inflection: {\n");
        for (k, v) in &entry.inflection {
            out.push_str(&format!(
                "    {}: \"{}\"\n",
                k,
                escape_hjson(v)
            ));
        }
        out.push_str("  }\n");
    }
    if !entry.register.is_empty() {
        out.push_str(&format!(
            "  register:     \"{}\"\n",
            escape_hjson(&entry.register)
        ));
    }
    if !entry.era.is_empty() {
        out.push_str(&format!("  era:          \"{}\"\n", escape_hjson(&entry.era)));
    }
    if !entry.notes.is_empty() {
        out.push_str(&format!(
            "  notes:        \"{}\"\n",
            escape_hjson(&entry.notes)
        ));
    }
    if !entry.domain.is_empty() {
        let items = entry
            .domain
            .iter()
            .map(|d| format!("\"{}\"", escape_hjson(d)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("  domain:       [{items}]\n"));
    }
    out.push_str("}\n");
    out
}

/// seed body for a grammar
/// rule paragraph created in the TUI.  Mirrors the
/// proposal §4 schema so future Phase D.2 work
/// (`--format grammar` exporter, `language define-rule`
/// CLI) can parse it the same way the dictionary entry
/// parser handles entries today.  Authors edit the
/// HJSON to fill in `category`, `applies_when`, etc.
pub(crate) const GRAMMAR_RULE_SEED_BODY: &str = "{
  // ──────────────────────────────────────────────────
  // IDENTITY
  // ──────────────────────────────────────────────────

  // Identifier the AI translation prompt references
  // in applied-rules lists.  Lowercase + hyphens.
  // Example: \"noun-case-system\",
  // \"verb-tense-aspect\", \"reduplication\".
  rule_id:      \"\"

  // Human-readable title for the rule card renderer.
  title:        \"\"

  // Category — drives Phase D.2 grammar export
  // sectioning AND the in-prompt grouping.
  //   morphology   — word-formation, inflection
  //   syntax       — clause structure, word order
  //   phonology    — sound rules
  //   orthography  — spelling conventions
  //   semantics    — meaning relationships
  //   pragmatics   — usage / discourse rules
  category:     \"\"

  // ──────────────────────────────────────────────────
  // RULE BODY — read by both the LLM and the human.
  // Plain text inside an HJSON multi-line string;
  // tabular layouts work fine.
  // ──────────────────────────────────────────────────

  rule:         '''
    Describe the rule here.  This text is fed
    verbatim to the AI translator at translation
    time, so be explicit:

      * State the input → output transformation.
      * Show the morpheme boundaries (- or .).
      * Show ALL exceptions inline so the LLM
        doesn't have to guess.

    Example layout for a case system:

      NOM: zero suffix.   aran     (king)
      ACC: -n.             aran → aranin
      DAT: -en.            aran → aranen
      GEN: -o.             aran → arano
  '''

  // ──────────────────────────────────────────────────
  // FEW-SHOT EXAMPLES — bundled into the translation
  // prompt envelope so the LLM sees the rule applied.
  // ──────────────────────────────────────────────────

  examples: [
    // { source: \"\",  target: \"\",  gloss: \"\" }
    // { source: \"\",  target: \"\",  gloss: \"\" }
  ]

  // ──────────────────────────────────────────────────
  // RAG TRIGGERING — when this rule should be
  // included in the translation prompt envelope.
  // ──────────────────────────────────────────────────

  // Plain-language condition the LLM evaluates
  // against the source sentence.  Tight applies_when
  // keeps the prompt focused (Phase C envelope
  // includes only matching rules; default cap is 6).
  applies_when: \"\"

  // Sibling rules this one builds on, by rule_id.
  // The RAG layer pulls dependent rules
  // automatically.  Example: a verb-conjugation
  // rule depends on the stem-formation rule.
  depends_on:   []

  // Rules that conflict with this one — only one
  // should fire per translation pass.  Phase D.2
  // `language doctor` will surface conflicting
  // pairs that lack an `applies_when` disambiguator.
  conflicts_with: []

  // ──────────────────────────────────────────────────
  // METADATA / NOTES
  // ──────────────────────────────────────────────────

  // Productivity — how broadly the rule applies.
  // \"core\"        — fires on most sentences
  // \"common\"      — fires on a recognisable
  //                  subset of constructions
  // \"specialised\" — narrow / register-bound
  // \"vestigial\"   — historical residue only
  productivity: \"\"

  // Register / style restrictions, if any:
  // formal | informal | literary | sacred | archaic.
  register:     \"\"

  // Author's notes — historical motivation,
  // worldbuilding rationale, comparison to natural-
  // language analogues.  Not read by the LLM.
  notes:        \"\"
}
";

/// seed body for a
/// phonology rule paragraph.  Lighter than the
/// grammar template because phonology rules tend to
/// be more declarative (allowed onsets, vowel
/// harmony patterns) than triggered.
pub(crate) const PHONOLOGY_RULE_SEED_BODY: &str = "{
  // ──────────────────────────────────────────────────
  // IDENTITY
  // ──────────────────────────────────────────────────

  // Identifier — lowercase + hyphens.  Referenced by
  // grammar rules' `depends_on` field and by the
  // phonotactic generator (`Ctrl+B Shift+W` in the
  // Language book — Phase D.2).
  // Examples: \"consonant-inventory\",
  // \"vowel-harmony\", \"syllable-template\",
  // \"intervocalic-voicing\".
  rule_id:      \"\"

  // Human-readable title for the rule card renderer.
  title:        \"\"

  // Category — drives Phase D.2 phonology export
  // sectioning AND the phonotactic generator's
  // weighting.
  //   consonants     — IPA inventory of consonants
  //   vowels         — IPA inventory of vowels
  //   phonotactics   — allowed onset / nucleus / coda
  //   syllable       — syllable template (CV, CVC, …)
  //   stress         — stress placement rule
  //   tone           — tonal system / pitch rules
  //   sound-changes  — historical or allophonic shifts
  //   prosody        — intonation / rhythm patterns
  category:     \"\"

  // ──────────────────────────────────────────────────
  // RULE BODY — read by both the LLM and the human.
  // ──────────────────────────────────────────────────

  rule:         '''
    Describe the rule here.  Use IPA inside
    /slashes/ for phonemic and [brackets] for
    phonetic.

    Example layouts:

      Phonotactic template:
        ONSET: zero | C | CC (only stop+liquid)
        NUCLEUS: V | VV (long vowels)
        CODA: zero | C | CC (limited to /s, n, r, l/)

      Sound change:
        /s/ → [z] / V_V (intervocalic voicing)

      Vowel harmony:
        Front vowels {i, e} co-occur in roots;
        back vowels {a, o, u} co-occur in roots;
        suffixes harmonise with the root.
  '''

  // ──────────────────────────────────────────────────
  // INVENTORIES — for consonants / vowels categories.
  // ──────────────────────────────────────────────────

  // List of phonemes (IPA strings).  Optional; used
  // by the phonotactic generator to constrain output.
  // phonemes:     []

  // Allophonic variants by environment.  Map of
  // phoneme → list of (environment, realisation).
  // allophones:   {}

  // ──────────────────────────────────────────────────
  // ENVIRONMENT — for sound-changes / allophony.
  // ──────────────────────────────────────────────────

  // Where the rule applies (LLM evaluates against the
  // source's phonetic context).
  // environment:  \"\"

  // ──────────────────────────────────────────────────
  // EXAMPLES — IPA pairs showing the rule in action.
  // ──────────────────────────────────────────────────

  examples: [
    // { input: \"\", output: \"\", gloss: \"\" }
  ]

  // Known exceptions — words / morphemes where the
  // rule does NOT apply.
  exceptions: []

  // ──────────────────────────────────────────────────
  // NOTES
  // ──────────────────────────────────────────────────

  // Register / style restrictions, if any.
  register:     \"\"

  // Author's notes — historical motivation, source
  // dialect, comparison to natural-language analogues.
  notes:        \"\"
}
";

/// Derive the alphabet-bucket subchapter name for a
/// word.  Uses the first non-whitespace character,
/// uppercased.  Returns `None` only if the input is
/// entirely whitespace — alphanumeric, Cyrillic,
/// Greek, hyphen / apostrophe-prefix all map to
/// their leading letter or symbol.
fn alphabet_bucket(word: &str) -> Option<String> {
    let ch = word.chars().find(|c| !c.is_whitespace())?;
    Some(ch.to_uppercase().to_string())
}

/// Consult the language sub-book's `Meta/overview`
/// HJSON for the alphabet-bucket name.  The author's
/// declared groupings override the naive first-char
/// uppercase (Phase B's fallback).  Returns:
///   * `Ok(Some(bucket))` — declared alphabet covers
///     the word's first character.
///   * `Ok(None)` — Meta chapter missing, overview
///     paragraph missing, HJSON block absent, alphabet
///     list empty, or first char not in any declared
///     entry.  Caller falls back to `alphabet_bucket`.
///   * `Err` — HJSON parse failure or store IO error.
///     Surfaced rather than swallowed so a malformed
///     overview is noisy enough to fix.
fn derive_alphabet_bucket(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
    word: &str,
) -> Result<Option<String>> {
    let Some(meta_chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Meta")
        })
        .cloned()
    else {
        return Ok(None);
    };
    let Some(overview) = hierarchy
        .children_of(Some(meta_chapter.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Paragraph && n.title.eq_ignore_ascii_case("overview")
        })
        .cloned()
    else {
        return Ok(None);
    };
    let Some(bytes) = store.get_content(overview.id)? else {
        return Ok(None);
    };
    let body = std::str::from_utf8(&bytes).map_err(|e| {
        Error::Config(format!("Meta/overview body is not UTF-8: {e}"))
    })?;
    let meta = match crate::language_entry::parse_meta_overview(body)
        .map_err(Error::Config)?
    {
        Some(m) => m,
        None => return Ok(None),
    };
    Ok(meta.bucket_for_word(word).map(|s| s.to_string()))
}

/// Build the seeded body for a freshly-added
/// dictionary entry.  Pure HJSON — no Typst wrappers
/// — so the editor renders with HJSON syntax
/// highlighting.  The paragraph's `content_type` is
/// set to `"hjson"` at create time.
///
/// switched FROM Typst-
/// with-fenced-HJSON to pure HJSON.  The translation
/// prompt composer + parser handle both formats; new
/// entries use pure HJSON.
fn seed_dictionary_entry_body(
    word: &str,
    pos: &str,
    translation: &str,
    example: Option<&str>,
) -> String {
    let example_value = example.unwrap_or("").trim();
    format!(
        "{{\n  \
         // ──────────────────────────────────────────────────\n  \
         // CORE — required for the entry to function as a\n  \
         // lexicon-overlay target + translation-prompt source.\n  \
         // ──────────────────────────────────────────────────\n  \
         \n  \
         word:         \"{word}\"\n  \
         \n  \
         // Part of speech.  Free-form string; the\n  \
         // proposal suggests: noun | verb | adjective |\n  \
         // adverb | pronoun | preposition | conjunction |\n  \
         // interjection | particle.  Language-specific\n  \
         // categories (\"classifier\", \"evidential\",\n  \
         // \"applicative\") are fine.\n  \
         type:         \"{pos}\"\n  \
         \n  \
         // Working-language gloss — what this word\n  \
         // means in the project's `language` (the value\n  \
         // the AI translator maps to/from).\n  \
         translation:  \"{translation}\"\n  \
         \n  \
         // Canonical sample sentence the author wants\n  \
         // frozen into the entry.  Becomes few-shot\n  \
         // anchor data in the translation prompt.\n  \
         example:      \"{example}\"\n  \
         \n  \
         // ──────────────────────────────────────────────────\n  \
         // OPTIONAL — uncomment and fill the ones you need.\n  \
         // Each is consumed by either the translation\n  \
         // prompt envelope (Phase C) or the future\n  \
         // dictionary card renderer (Phase D.2).\n  \
         // ──────────────────────────────────────────────────\n  \
         \n  \
         // Additional example sentences beyond the\n  \
         // canonical one.  Phase C translation flow\n  \
         // uses every example as few-shot data.\n  \
         // examples:     [\n  \
         //   \"\"\n  \
         //   \"\"\n  \
         // ]\n  \
         \n  \
         // IPA transcription (between slashes for\n  \
         // phonemic, brackets for phonetic).\n  \
         // pronunciation: \"\"\n  \
         \n  \
         // Etymology / derivation.  Plain text or\n  \
         // [[wikilink]] style cross-reference to a\n  \
         // proto-form entry.\n  \
         // etymology:    \"\"\n  \
         \n  \
         // Cross-references to sibling entries — other\n  \
         // words in this language that share roots,\n  \
         // contrast in register, or commonly co-occur.\n  \
         // related:      []\n  \
         \n  \
         // Paradigm forms.  Every VALUE here gets\n  \
         // added to the lexicon overlay so inflected\n  \
         // words light up in prose alongside the\n  \
         // lemma.  KEY names are free-form and feed\n  \
         // the translation prompt as paradigm hints.\n  \
         // inflection:   {{\n  \
         //   plural:     \"\"\n  \
         //   genitive:   \"\"\n  \
         //   accusative: \"\"\n  \
         //   dative:     \"\"\n  \
         //   ablative:   \"\"\n  \
         // }}\n  \
         \n  \
         // Register / style: formal | informal |\n  \
         // archaic | literary | colloquial | sacred.\n  \
         // register:     \"\"\n  \
         \n  \
         // Era — when the word entered the language.\n  \
         // Useful for historical-fiction projects.\n  \
         // era:          \"\"\n  \
         \n  \
         // Auto-tracked count of mentions in the\n  \
         // manuscript.  Phase D.2 `language doctor`\n  \
         // updates this; leave 0 for now.\n  \
         // frequency:    0\n  \
         \n  \
         // Free-form usage notes — register cues,\n  \
         // taboos, mnemonic etymology, whatever\n  \
         // helps you remember the word.\n  \
         notes:        \"\"\n\
         }}\n",
        word = escape_hjson(word),
        pos = escape_hjson(pos),
        translation = escape_hjson(translation),
        example = escape_hjson(example_value),
    )
}

/// Minimal HJSON string escape — backslash-quote +
/// backslash-backslash.  Sufficient for the
/// dictionary-entry seed body, which never sees
/// control characters in practice.
fn escape_hjson(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// health report for a language
/// sub-book.  Walks every chapter, counts entries +
/// rules + samples, computes coverage metrics, and
/// emits a human-readable summary on stdout.  Exit
/// code 0 always — informational, not a gate.
///
/// Coverage gap analysis (§13 of the proposal):
///   * count manuscript words (working language) that
///     don't appear as translations in this language's
///     dictionary.  Surfaces vocabulary the author has
///     written in prose but hasn't yet defined a
///     translation for.
///   * count dictionary entries that lack examples —
///     half-finished work.
///   * count entries that lack inflection paradigms —
///     hint that the lexicon overlay won't catch
///     inflected forms for those words.
fn doctor(project: &Path, language: &str, json: bool) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

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
            Error::Config(format!(
                "language `{language}` not found — run `inkhaven language init {language}` first"
            ))
        })?;

    // Walk each chapter's paragraphs.  We don't reach
    // for the in-memory TUI helpers because doctor /
    // export need to run from a headless CLI process.
    let chapters = hierarchy.children_of(Some(lang_book.id));
    let mut dict_entries: Vec<(String, crate::language_entry::DictionaryEntry)> =
        Vec::new();
    let mut dict_unparseable = 0usize;
    let mut grammar_count = 0usize;
    let mut phonology_count = 0usize;
    let mut sample_count = 0usize;
    let mut meta: Option<crate::language_entry::MetaOverview> = None;
    for chapter in &chapters {
        let title_lc = chapter.title.to_lowercase();
        let paragraphs: Vec<_> = hierarchy
            .collect_subtree(chapter.id)
            .into_iter()
            .filter_map(|id| hierarchy.get(id))
            .filter(|n| n.kind == NodeKind::Paragraph)
            .cloned()
            .collect();
        match title_lc.as_str() {
            "dictionary" => {
                for p in &paragraphs {
                    let Ok(Some(bytes)) = store.get_content(p.id) else {
                        continue;
                    };
                    let Ok(body) = std::str::from_utf8(&bytes) else {
                        continue;
                    };
                    match crate::language_entry::parse(body) {
                        Ok(Some(e)) => dict_entries.push((p.title.clone(), e)),
                        Ok(None) => dict_unparseable += 1,
                        Err(_) => dict_unparseable += 1,
                    }
                }
            }
            "grammar" => grammar_count = paragraphs.len(),
            "phonology" => phonology_count = paragraphs.len(),
            "sample texts" => sample_count = paragraphs.len(),
            "meta" => {
                for p in &paragraphs {
                    if p.title.eq_ignore_ascii_case("overview") {
                        let Ok(Some(bytes)) = store.get_content(p.id) else {
                            continue;
                        };
                        if let Ok(body) = std::str::from_utf8(&bytes) {
                            if let Ok(Some(m)) =
                                crate::language_entry::parse_meta_overview(body)
                            {
                                meta = Some(m);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let total_entries = dict_entries.len();
    let with_examples = dict_entries
        .iter()
        .filter(|(_, e)| !e.example.trim().is_empty())
        .count();
    let with_inflection = dict_entries
        .iter()
        .filter(|(_, e)| !e.inflection.is_empty())
        .count();
    let missing_examples = total_entries.saturating_sub(with_examples);
    let missing_inflection = total_entries.saturating_sub(with_inflection);

    // Coverage-gap analysis: which working-language
    // words in the manuscript have no dictionary
    // translation?  Walk every paragraph in user
    // books (skip system books — Notes / Places /
    // Characters / Artefacts / Prompts / Language /
    // Typst are reference material, not manuscript
    // prose) and collect their words.
    use unicode_segmentation::UnicodeSegmentation;
    let dictionary_translations: std::collections::HashSet<String> = dict_entries
        .iter()
        .filter_map(|(_, e)| {
            let t = e.translation.trim().to_lowercase();
            if t.is_empty() { None } else { Some(t) }
        })
        .collect();
    let mut manuscript_words: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        // Skip system-book content.
        let mut cursor = Some(node.id);
        let mut is_system = false;
        while let Some(id) = cursor {
            if let Some(n) = hierarchy.get(id) {
                if n.system_tag.is_some() {
                    is_system = true;
                    break;
                }
                cursor = n.parent_id;
            } else {
                break;
            }
        }
        if is_system {
            continue;
        }
        if let Ok(Some(bytes)) = store.get_content(node.id) {
            if let Ok(body) = std::str::from_utf8(&bytes) {
                for w in UnicodeSegmentation::unicode_words(body) {
                    let lc = w.to_lowercase();
                    // Stop-word-ish filter: drop
                    // 1-letter "words" (a, I) — most
                    // are noise; the rest are too
                    // common to be worth flagging.
                    if lc.chars().count() < 2 {
                        continue;
                    }
                    manuscript_words.insert(lc);
                }
            }
        }
    }
    let manuscript_word_count = manuscript_words.len();
    let undefined_words: Vec<String> = manuscript_words
        .difference(&dictionary_translations)
        .cloned()
        .collect();

    // 1.2.13+ Phase D.1 — JSON mode emits the same
    // numbers in a structured form so CI pipelines
    // can gate on `coverage.with_example_pct < 80`
    // etc.  Returns early; the text render below
    // stays unchanged.
    if json {
        use serde_json::{json, Map, Value};
        let mut sorted_undefined: Vec<String> =
            undefined_words.iter().take(50).cloned().collect();
        sorted_undefined.sort();
        let example_pct = if total_entries > 0 {
            with_examples * 100 / total_entries
        } else {
            0
        };
        let inflection_pct = if total_entries > 0 {
            with_inflection * 100 / total_entries
        } else {
            0
        };
        let coverage_pct = if manuscript_word_count > 0 {
            manuscript_word_count.saturating_sub(undefined_words.len()) * 100
                / manuscript_word_count
        } else {
            0
        };
        let mut report = Map::new();
        report.insert("language".into(), Value::String(lang_book.title.clone()));
        report.insert(
            "meta".into(),
            meta.as_ref()
                .map(|m| json!({
                    "name": m.name,
                    "language_kind": m.language_kind,
                    "family": m.family,
                    "iso_code": m.iso_code,
                    "alphabet_count": m.alphabet.len(),
                    "reading_direction": m.reading_direction,
                }))
                .unwrap_or(Value::Null),
        );
        report.insert(
            "chapters".into(),
            json!({
                "dictionary_parseable": total_entries,
                "dictionary_unparseable": dict_unparseable,
                "grammar": grammar_count,
                "phonology": phonology_count,
                "sample_texts": sample_count,
            }),
        );
        report.insert(
            "coverage".into(),
            json!({
                "with_example": with_examples,
                "with_example_pct": example_pct,
                "with_paradigm": with_inflection,
                "with_paradigm_pct": inflection_pct,
                "missing_example": missing_examples,
                "missing_paradigm": missing_inflection,
            }),
        );
        report.insert(
            "manuscript_gap".into(),
            json!({
                "unique_words": manuscript_word_count,
                "uncovered_count": undefined_words.len(),
                "coverage_pct": coverage_pct,
                "uncovered_sample": sorted_undefined,
            }),
        );
        let s = serde_json::to_string_pretty(&Value::Object(report))
            .map_err(|e| Error::Config(format!("json serialise: {e}")))?;
        println!("{s}");
        return Ok(());
    }

    // Emit the human-readable report.
    println!("Language doctor — `{}`", lang_book.title);
    println!();
    if let Some(m) = meta.as_ref() {
        if !m.name.is_empty() {
            println!("  name           : {}", m.name);
        }
        if !m.language_kind.is_empty() {
            println!("  kind           : {}", m.language_kind);
        }
        if !m.family.is_empty() {
            println!("  family         : {}", m.family);
        }
        if !m.iso_code.is_empty() {
            println!("  iso_code       : {}", m.iso_code);
        }
        if !m.alphabet.is_empty() {
            println!("  alphabet       : {} entries", m.alphabet.len());
        }
        if !m.reading_direction.is_empty() {
            println!("  direction      : {}", m.reading_direction);
        }
        println!();
    } else {
        println!("  Meta/overview  : MISSING or unparseable");
        println!();
    }
    println!("Chapters");
    println!("  Dictionary     : {total_entries} parseable entries");
    if dict_unparseable > 0 {
        println!(
            "                   {dict_unparseable} unparseable (no HJSON block — pre-Phase-B authoring)"
        );
    }
    println!("  Grammar        : {grammar_count} rules");
    println!("  Phonology      : {phonology_count} rules");
    println!("  Sample texts   : {sample_count} samples");
    println!();
    println!("Dictionary coverage");
    if total_entries > 0 {
        let example_pct = with_examples * 100 / total_entries;
        let inflection_pct = with_inflection * 100 / total_entries;
        println!(
            "  with example   : {with_examples}/{total_entries} ({example_pct}%)"
        );
        println!(
            "  with paradigm  : {with_inflection}/{total_entries} ({inflection_pct}%)"
        );
        if missing_examples > 0 {
            println!("  missing example: {missing_examples}");
        }
        if missing_inflection > 0 {
            println!(
                "  missing paradigm: {missing_inflection} (overlay won't catch inflected forms)"
            );
        }
    } else {
        println!("  no dictionary entries yet — try `inkhaven language add-word`");
    }
    println!();
    println!("Manuscript gap analysis");
    println!("  unique words (≥2 chars) in manuscript prose: {manuscript_word_count}");
    let undefined_count = undefined_words.len();
    if total_entries > 0 {
        let covered = manuscript_word_count.saturating_sub(undefined_count);
        let pct = if manuscript_word_count > 0 {
            covered * 100 / manuscript_word_count
        } else {
            0
        };
        println!("  covered by dictionary: {covered}/{manuscript_word_count} ({pct}%)");
        if undefined_count > 0 {
            println!("  uncovered words (sample, max 15):");
            let mut sample: Vec<&String> = undefined_words.iter().take(15).collect();
            sample.sort();
            for w in sample {
                println!("    · {w}");
            }
            if undefined_count > 15 {
                println!("    ... and {} more", undefined_count - 15);
            }
        }
    } else {
        println!("  (skipping — no dictionary entries to compare against)");
    }
    Ok(())
}

/// export a language's content
/// to a portable artefact.  Three formats land in
/// Phase D; `grammar` and `phrasebook` from the
/// proposal §12 are deferred to D.2.

/// Extract a single string field from an HJSON
/// body via a forgiving line-based scan.  Avoids
/// pulling in a full HJSON parse here — the bodies
/// are author-written and we only want one field
/// per call.  Returns the trimmed value when found.
fn extract_hjson_string_field(body: &str, field: &str) -> Option<String> {
    let needle = format!("{field}:");
    for line in body.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(&needle) {
            continue;
        }
        let rest = trimmed[needle.len()..].trim();
        // Strip surrounding quotes if present.
        let v = rest.trim_matches('"').trim_matches('\'').trim();
        if v.is_empty() {
            return None;
        }
        return Some(v.to_string());
    }
    None
}

/// Extract the `examples:` array from an HJSON
/// body.  Handles both single-line array form
/// (`examples: ["a", "b"]`) and multi-line block
/// form.  Light-touch parsing — same rationale as
/// `extract_hjson_string_field`.
fn extract_hjson_examples(body: &str) -> Option<Vec<String>> {
    let mut found = false;
    let mut single_line: Option<String> = None;
    let mut block_lines: Vec<String> = Vec::new();
    let mut in_block = false;

    for line in body.lines() {
        let trimmed = line.trim_start();
        if !found && trimmed.starts_with("examples:") {
            found = true;
            let rest = trimmed["examples:".len()..].trim();
            if rest.starts_with('[') && rest.ends_with(']') {
                single_line = Some(rest[1..rest.len() - 1].to_string());
                break;
            }
            if rest.starts_with('[') {
                in_block = true;
            }
            continue;
        }
        if in_block {
            if trimmed.starts_with(']') {
                break;
            }
            block_lines.push(trimmed.trim_end_matches(',').to_string());
        }
    }
    if !found {
        return None;
    }
    if let Some(sl) = single_line {
        return Some(
            sl.split(',')
                .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        );
    }
    Some(
        block_lines
            .into_iter()
            .map(|s| s.trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

/// 1.2.16+ Phase P.5 — `inkhaven language
/// define-rule <language> <rule_id> [--category
/// grammar|phonology]`.  Opens the rule's HJSON
/// template in `$EDITOR` (fallback `vi`); on the
/// editor's exit, writes the saved content into
/// a new or existing rule paragraph under the
/// chosen category.
fn define_rule(
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

fn rule_template(rule_id: &str, category: &str) -> String {
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
fn open_in_editor(seed: &str, label: &str) -> Result<String> {
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

/// `inkhaven language list`.
/// Walks the `Language` system book and emits one
/// row per language with summary counts.  Quick
/// at-a-glance complement to `language doctor`.
/// `inkhaven language add-word
/// <lang> --import <path.csv>`.  Bulk-load a CSV
/// dictionary.  Format described in the CLI variant
/// docstring; mechanically:
///   * RFC 4180 quoting (`"…"` for fields with
///     commas / quotes / newlines; `""` for embedded
///     quotes).
///   * Header row maps column NAMES to row positions
///     so the CSV's columns can appear in any order
///     and any subset.
///   * Complex fields parsed inside the row:
///       - `inflection`: `;`-separated `key=value` pairs
///       - `examples`:   `|`-separated sentences
///       - `related`:    `;`-separated word slugs
///   * Skip rules: empty `word` cell + `word` starting
///     with `#` both treated as skip-this-row; duplicate
///     `word` (already in the dictionary) skipped with
///     warning.
///   * Tally printed at end (imported / skipped /
///     failed counts).
/// 1.3.19 LANG-1 P6 — import a dictionary from a foreign conlang/linguistics
/// tool (Toolbox/MDF SFM, PolyGlot). Parses the file into neutral lexemes
/// (`conlang::interchange`), previews them by default, and writes them into the
/// Dictionary only with `--yes`. Deterministic format conversion — no AI — but
/// non-committal by default so an author reviews before the book changes.

fn list(project: &Path) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

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
    let languages = hierarchy.children_of(Some(lang_root.id));
    if languages.is_empty() {
        eprintln!("no languages defined — run `inkhaven language init <name>`");
        return Ok(());
    }
    // Compute counts up-front so the column widths
    // can size to the data.  Tuple shape:
    // (name, entries, grammar, phonology, samples).
    let mut rows: Vec<(String, usize, usize, usize, usize)> =
        Vec::with_capacity(languages.len());
    for lang in &languages {
        let chapters = hierarchy.children_of(Some(lang.id));
        let mut entries = 0usize;
        let mut grammar = 0usize;
        let mut phonology = 0usize;
        let mut samples = 0usize;
        for chapter in &chapters {
            let title_lc = chapter.title.to_lowercase();
            let paragraph_count = hierarchy
                .collect_subtree(chapter.id)
                .into_iter()
                .filter_map(|id| hierarchy.get(id))
                .filter(|n| n.kind == NodeKind::Paragraph)
                .count();
            match title_lc.as_str() {
                "dictionary" => entries = paragraph_count,
                "grammar" => grammar = paragraph_count,
                "phonology" => phonology = paragraph_count,
                "sample texts" => samples = paragraph_count,
                _ => {}
            }
        }
        rows.push((lang.title.clone(), entries, grammar, phonology, samples));
    }
    let max_name = rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(8);
    let name_w = max_name.max(8);
    println!(
        "  {:<width$}  {:>6}  {:>7}  {:>9}  {:>7}",
        "name", "words", "grammar", "phonology", "samples",
        width = name_w,
    );
    println!(
        "  {}",
        "-".repeat(name_w + 36)
    );
    for (name, entries, grammar, phonology, samples) in &rows {
        println!(
            "  {:<width$}  {:>6}  {:>7}  {:>9}  {:>7}",
            name, entries, grammar, phonology, samples,
            width = name_w,
        );
    }
    Ok(())
}

/// `inkhaven language
/// remove-word <language> <word>`.  Mirror of
/// `add-word`: resolves the language sub-book by
/// case-insensitive title; finds the Dictionary
/// chapter; locates the bucket subchapter via the
/// same alphabet-bucket derivation
/// (`Meta/overview.alphabet` consultation first,
/// first-char fallback); deletes the entry
/// paragraph.  Errors when the entry doesn't
/// exist rather than silently no-op-ing so the
/// caller knows their `remove-word foo` against
/// an already-removed entry needs no follow-up
/// action.
fn remove_word(project: &Path, language: &str, word: &str) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it".into(),
            )
        })?
        .clone();
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
    let dictionary = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Chapter
                && n.title.eq_ignore_ascii_case("Dictionary")
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` has no Dictionary chapter"
            ))
        })?;
    // Same bucket derivation as add-word.
    let bucket = derive_alphabet_bucket(&store, &hierarchy, &lang_book, word)?
        .or_else(|| alphabet_bucket(word))
        .ok_or_else(|| {
            Error::Config(format!("could not derive alphabet bucket from `{word}`"))
        })?;
    let subchapter = hierarchy
        .children_of(Some(dictionary.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Subchapter
                && n.title.eq_ignore_ascii_case(&bucket)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "no bucket subchapter `{bucket}` under `{language}/Dictionary` — `{word}` isn't defined"
            ))
        })?;
    let entry = hierarchy
        .children_of(Some(subchapter.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Paragraph
                && n.title.eq_ignore_ascii_case(word)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "word `{word}` not found under `{language}/Dictionary/{bucket}`"
            ))
        })?;
    let ids = hierarchy.collect_subtree(entry.id);
    // Entry is a Paragraph — its on-disk path lives
    // in `entry.file` (no children to walk for the
    // fs path).
    let fs_rel = entry
        .file
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    store
        .delete_subtree(&fs_rel, &ids)
        .map_err(|e| Error::Store(format!("delete entry: {e}")))?;
    eprintln!(
        "removed `{word}` from `{language}/Dictionary/{bucket}`"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_chapters_match_proposal() {
        // Locks the chapter shape against the
        // proposal §1 hierarchy diagram — adding
        // or renaming a chapter requires updating
        // both the constant + the proposal.
        assert_eq!(
            STANDARD_CHAPTERS,
            &["Meta", "Dictionary", "Grammar", "Phonology", "Sample texts"]
        );
    }

    /// the verbose seed
    /// templates use HJSON multi-line strings (`'''`)
    /// and a generous amount of commented-out
    /// optional fields.  A typo or unbalanced bracket
    /// in any of them would silently break every new
    /// language sub-book the user scaffolds.  Parse
    /// each template through serde_hjson directly to
    /// catch syntax regressions at test time, not at
    /// the user's first `+` press.
    #[test]
    fn first_unknown_letter_passes_when_all_chars_in_inventory() {
        let inv = vec!["A".into(), "B".into(), "C".into()];
        assert_eq!(first_unknown_letter("abc", &inv), None);
        // Case-insensitive.
        assert_eq!(first_unknown_letter("ABC", &inv), None);
        // Punctuation always passes.
        assert_eq!(first_unknown_letter("a-b'c", &inv), None);
        // Whitespace always passes.
        assert_eq!(first_unknown_letter("a b c", &inv), None);
    }

    #[test]
    fn first_unknown_letter_returns_first_violation() {
        let inv = vec!["A".into(), "B".into()];
        assert_eq!(first_unknown_letter("abz", &inv), Some('z'));
        // First violation wins.
        assert_eq!(first_unknown_letter("xyz", &inv), Some('x'));
    }

    #[test]
    fn first_unknown_letter_handles_multichar_inventory_entries() {
        // Paired-case Latin: each alphabet entry is
        // a two-char string but we look for the char
        // as substring.
        let inv = vec!["Aa".into(), "Bb".into(), "Cc".into()];
        assert_eq!(first_unknown_letter("aBc", &inv), None);
        assert_eq!(first_unknown_letter("aBz", &inv), Some('z'));
    }

    #[test]
    fn first_unknown_letter_handles_non_latin() {
        let inv = vec!["А".into(), "Б".into()];
        assert_eq!(first_unknown_letter("аб", &inv), None);
        assert_eq!(first_unknown_letter("абя", &inv), Some('я'));
    }

    #[test]
    fn csv_parser_handles_quoted_fields() {
        let csv = "word,type,translation\n\
                   atal,noun,river\n\
                   sora,verb,\"to flow, swiftly\"\n\
                   nan,pronoun,\"\"\"you\"\"\"\n";
        let rows = parse_csv(csv).unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0], vec!["word", "type", "translation"]);
        assert_eq!(rows[1], vec!["atal", "noun", "river"]);
        assert_eq!(rows[2], vec!["sora", "verb", "to flow, swiftly"]);
        // Embedded "" doubles to one literal quote.
        assert_eq!(rows[3], vec!["nan", "pronoun", "\"you\""]);
    }

    #[test]
    fn csv_parser_handles_newlines_in_quoted_fields() {
        let csv = "word,notes\natal,\"line1\nline2\"\n";
        let rows = parse_csv(csv).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1], vec!["atal", "line1\nline2"]);
    }

    #[test]
    fn csv_parser_handles_crlf_and_missing_trailing_newline() {
        let csv = "a,b\r\nc,d";
        let rows = parse_csv(csv).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["a", "b"]);
        assert_eq!(rows[1], vec!["c", "d"]);
    }

    #[test]
    fn csv_parser_errors_on_unclosed_quote() {
        assert!(parse_csv("word\n\"unclosed").is_err());
    }

    #[test]
    fn inflection_parser_extracts_pairs() {
        let m = parse_inflection_field("nominative=atal;genitive=atale;plural=atatal");
        assert_eq!(m.len(), 3);
        assert_eq!(m.get("nominative"), Some(&"atal".to_string()));
        assert_eq!(m.get("genitive"), Some(&"atale".to_string()));
        assert_eq!(m.get("plural"), Some(&"atatal".to_string()));
    }

    #[test]
    fn inflection_parser_tolerates_whitespace_and_skips_malformed() {
        let m = parse_inflection_field(" plural = atatal ; bad-no-equals ; genitive=atale ");
        assert_eq!(m.len(), 2);
        assert!(m.contains_key("plural"));
        assert!(m.contains_key("genitive"));
    }

    #[test]
    fn split_helpers_filter_empty_tokens() {
        assert_eq!(
            split_pipe("a|b||c"),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(
            split_semicolon("a;b;;c"),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn resolve_csv_columns_requires_word_type_translation() {
        let header = vec!["word".into(), "type".into(), "translation".into()];
        let cols = resolve_csv_columns(&header).unwrap();
        assert_eq!(cols.word, 0);
        assert_eq!(cols.pos, 1);
        assert_eq!(cols.translation, 2);
        assert!(cols.example.is_none());
    }

    #[test]
    fn resolve_csv_columns_errors_on_missing_required() {
        let header = vec!["word".into(), "type".into()];
        assert!(resolve_csv_columns(&header).is_err());
    }

    #[test]
    fn resolve_csv_columns_is_case_insensitive_and_order_independent() {
        let header = vec![
            "Notes".into(),
            "Translation".into(),
            "TYPE".into(),
            "Word".into(),
            "inflection".into(),
        ];
        let cols = resolve_csv_columns(&header).unwrap();
        assert_eq!(cols.word, 3);
        assert_eq!(cols.pos, 2);
        assert_eq!(cols.translation, 1);
        assert_eq!(cols.notes, Some(0));
        assert_eq!(cols.inflection, Some(4));
    }

    #[test]
    fn imported_entry_body_skips_empty_optionals() {
        let entry = ImportEntry {
            word: "atal".into(),
            pos: "noun".into(),
            translation: "river".into(),
            ..Default::default()
        };
        let body = build_imported_entry_body(&entry);
        assert!(body.contains("word:"));
        assert!(body.contains("type:"));
        assert!(body.contains("translation:"));
        // Empty optionals must be absent — no `example:`,
        // `pronunciation:`, `notes:` etc. in the body
        // when the import didn't populate them.
        assert!(!body.contains("example:"));
        assert!(!body.contains("pronunciation:"));
        assert!(!body.contains("notes:"));
        assert!(!body.contains("inflection:"));
    }

    #[test]
    fn imported_entry_body_emits_inflection_and_examples() {
        let mut entry = ImportEntry {
            word: "atal".into(),
            pos: "noun".into(),
            translation: "river".into(),
            ..Default::default()
        };
        entry.inflection.insert("plural".into(), "atatal".into());
        entry.inflection.insert("genitive".into(), "atale".into());
        entry.examples = vec!["Atal sora-mi.".into(), "Atal kima.".into()];
        let body = build_imported_entry_body(&entry);
        assert!(body.contains("inflection: {"));
        assert!(body.contains("plural: \"atatal\""));
        assert!(body.contains("genitive: \"atale\""));
        assert!(body.contains("examples: ["));
        assert!(body.contains("\"Atal sora-mi.\""));
        // Round-trips through the parser.
        let parsed: serde_hjson::Value =
            serde_hjson::from_str(&body).expect("imported entry body must parse");
        let _ = parsed;
    }

    #[test]
    fn meta_overview_seed_parses() {
        let _: serde_hjson::Value = serde_hjson::from_str(META_OVERVIEW_BODY)
            .expect("META_OVERVIEW_BODY must be valid HJSON");
    }

    #[test]
    fn dictionary_entry_seed_parses() {
        let body = seed_dictionary_entry_body(
            "aiya", "interjection", "hail", Some("Aiya!"),
        );
        let _: serde_hjson::Value = serde_hjson::from_str(&body)
            .expect("dictionary entry seed must be valid HJSON");
    }

    #[test]
    fn grammar_rule_seed_parses() {
        let _: serde_hjson::Value = serde_hjson::from_str(GRAMMAR_RULE_SEED_BODY)
            .expect("GRAMMAR_RULE_SEED_BODY must be valid HJSON");
    }

    #[test]
    fn phonology_rule_seed_parses() {
        let _: serde_hjson::Value = serde_hjson::from_str(PHONOLOGY_RULE_SEED_BODY)
            .expect("PHONOLOGY_RULE_SEED_BODY must be valid HJSON");
    }

    #[test]
    fn meta_overview_body_contains_alphabet_field() {
        // The `alphabet` field is the load-bearing
        // metadata key — drives Dictionary
        // subchapter auto-creation in Phase B.
        // Lock its presence in the seeded body so
        // a future seed-body edit can't silently
        // drop it.
        assert!(META_OVERVIEW_BODY.contains("alphabet:"));
        assert!(META_OVERVIEW_BODY.contains("language_kind:"));
    }

    #[test]
    fn alphabet_bucket_uppercases_first_char() {
        assert_eq!(alphabet_bucket("aiya"), Some("A".to_string()));
        assert_eq!(alphabet_bucket("Bran"), Some("B".to_string()));
        assert_eq!(alphabet_bucket("  zeta"), Some("Z".to_string()));
    }

    #[test]
    fn alphabet_bucket_handles_non_latin() {
        // Cyrillic 'я' uppercases to 'Я'.
        assert_eq!(alphabet_bucket("ярости"), Some("Я".to_string()));
        // Greek 'α' uppercases to 'Α'.
        assert_eq!(alphabet_bucket("αυτός"), Some("Α".to_string()));
    }

    #[test]
    fn alphabet_bucket_returns_none_for_whitespace() {
        assert_eq!(alphabet_bucket(""), None);
        assert_eq!(alphabet_bucket("   "), None);
    }

    #[test]
    fn seed_dictionary_entry_includes_core_fields() {
        let body = seed_dictionary_entry_body(
            "aiya",
            "interjection",
            "hail",
            Some("Aiya Eärendil!"),
        );
        // The four core HJSON fields land in the
        // body.  Locking presence stops a future
        // schema rename from silently breaking the
        // seed.
        assert!(body.contains("word:"));
        assert!(body.contains("type:"));
        assert!(body.contains("translation:"));
        assert!(body.contains("example:"));
        assert!(body.contains("aiya"));
        assert!(body.contains("interjection"));
        assert!(body.contains("hail"));
        assert!(body.contains("Aiya Eärendil!"));
    }

    #[test]
    fn csv_field_quotes_when_needed() {
        // Plain field — emit verbatim.
        assert_eq!(csv_field("aiya"), "aiya");
        // Comma triggers quoting.
        assert_eq!(csv_field("hail, friend"), "\"hail, friend\"");
        // Embedded quote doubles + wraps.
        assert_eq!(csv_field("he said \"hi\""), "\"he said \"\"hi\"\"\"");
        // Newline triggers quoting too.
        assert_eq!(csv_field("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn typst_escape_handles_markup_chars() {
        // Markup-bearing characters get backslashed
        // so the renderer doesn't apply emphasis /
        // code / link semantics to dictionary
        // content.
        assert_eq!(typst_escape("plain"), "plain");
        assert_eq!(typst_escape("a*b"), "a\\*b");
        assert_eq!(typst_escape("[bracket]"), "\\[bracket\\]");
        assert_eq!(typst_escape("#hash"), "\\#hash");
        assert_eq!(typst_escape("with_under"), "with\\_under");
        // Non-Latin / Unicode passes through.
        assert_eq!(typst_escape("ñ'olor"), "ñ'olor");
    }

    #[test]
    fn render_anki_emits_header_row() {
        let out = render_anki(&[]).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("word,translation,type,example,inflection\n"));
    }

    #[test]
    fn render_anki_renders_entry_row() {
        let mut entry = crate::language_entry::DictionaryEntry::default();
        entry.word = "aiya".into();
        entry.translation = "hail".into();
        entry.pos = "interjection".into();
        entry.example = "Aiya Eärendil!".into();
        let out = render_anki(&[("aiya".into(), entry)]).unwrap();
        let s = String::from_utf8(out).unwrap();
        // Header on line 1, entry on line 2.
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 2, "got: {s:?}");
        assert!(lines[1].contains("aiya"));
        assert!(lines[1].contains("hail"));
        assert!(lines[1].contains("interjection"));
        assert!(lines[1].contains("Aiya Eärendil!"));
    }

    // 1.2.16+ Phase P.5 — render_csv tests.

    #[test]
    fn render_csv_emits_header_row() {
        let out = render_csv(&[]);
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("word,type,translation,example,inflection\n"));
    }

    #[test]
    fn render_csv_round_trip_columns_match_in_memory_struct() {
        // The whole point of the CSV format is
        // that the `--import` path can re-ingest
        // it.  Pin the column order against the
        // documented in-memory struct shape.
        let mut entry = crate::language_entry::DictionaryEntry::default();
        entry.word = "stelle".into();
        entry.pos = "noun".into();
        entry.translation = "star".into();
        entry.example = "Le stelle brillano.".into();
        entry.inflection.insert("plural".into(), "stelle".into());
        entry
            .inflection
            .insert("singular".into(), "stella".into());
        let out = render_csv(&[("stelle".into(), entry)]);
        let s = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 2);
        // Inflection serialises sorted by key:
        // plural=stelle;singular=stella.
        assert!(
            lines[1].contains("plural=stelle;singular=stella"),
            "unexpected inflection serialisation: {}",
            lines[1]
        );
        assert!(lines[1].contains("stelle,noun,star,Le stelle brillano."));
    }

    #[test]
    fn render_csv_quotes_fields_with_commas_and_quotes() {
        let mut entry = crate::language_entry::DictionaryEntry::default();
        entry.word = "salve".into();
        entry.pos = "interjection".into();
        entry.translation = "hello, hi".into(); // contains comma
        entry.example = "She said \"salve\".".into(); // contains quote
        let out = render_csv(&[("salve".into(), entry)]);
        let s = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = s.lines().collect();
        assert!(
            lines[1].contains("\"hello, hi\""),
            "comma field should be quoted: {}",
            lines[1]
        );
        assert!(
            lines[1].contains("\"She said \"\"salve\"\".\""),
            "quote field should escape inner quotes: {}",
            lines[1]
        );
    }

    // 1.2.16+ Phase P.5 — extract_hjson_string_field tests.

    #[test]
    fn extract_hjson_finds_simple_string_field() {
        let body = "{\n  rule: \"i becomes y before vowel\"\n  category: \"phonology\"\n}";
        assert_eq!(
            extract_hjson_string_field(body, "rule"),
            Some("i becomes y before vowel".into())
        );
        assert_eq!(
            extract_hjson_string_field(body, "category"),
            Some("phonology".into())
        );
        assert_eq!(extract_hjson_string_field(body, "missing"), None);
    }

    #[test]
    fn extract_hjson_skips_empty_fields() {
        let body = "{\n  rule: \"\"\n  category: \"grammar\"\n}";
        assert_eq!(extract_hjson_string_field(body, "rule"), None);
        assert_eq!(
            extract_hjson_string_field(body, "category"),
            Some("grammar".into())
        );
    }

    #[test]
    fn extract_hjson_examples_inline_array() {
        let body = "{\n  examples: [\"one\", \"two\", \"three\"]\n}";
        let got = extract_hjson_examples(body).unwrap();
        assert_eq!(got, vec!["one", "two", "three"]);
    }

    #[test]
    fn extract_hjson_examples_block_form() {
        let body = "{\n  examples: [\n    \"alpha\",\n    \"beta\"\n  ]\n}";
        let got = extract_hjson_examples(body).unwrap();
        assert_eq!(got, vec!["alpha", "beta"]);
    }

    #[test]
    fn rule_template_includes_id_and_grammar_examples() {
        let t = rule_template("noun-cases", "grammar");
        assert!(t.contains("rule_id: \"noun-cases\""));
        assert!(t.contains("invented language"));
    }

    #[test]
    fn rule_template_uses_phonology_examples_when_category_phonology() {
        let t = rule_template("vowel-shift", "phonology");
        assert!(t.contains("rule_id: \"vowel-shift\""));
        assert!(t.contains("phoneme example"));
    }

    #[test]
    fn render_dictionary_twocol_groups_by_alphabet() {
        let mut a_entry = crate::language_entry::DictionaryEntry::default();
        a_entry.word = "aiya".into();
        a_entry.pos = "interj.".into();
        a_entry.translation = "hail".into();
        let mut b_entry = crate::language_entry::DictionaryEntry::default();
        b_entry.word = "bara".into();
        b_entry.pos = "noun".into();
        b_entry.translation = "fire".into();
        let out = render_dictionary_twocol(
            "Quenya",
            None,
            &[("aiya".into(), a_entry), ("bara".into(), b_entry)],
        );
        let s = String::from_utf8(out).unwrap();
        // Bucket headers for both A and B sections.
        assert!(s.contains("— A —"), "got: {s}");
        assert!(s.contains("— B —"), "got: {s}");
        // Page setup + entries appear.
        assert!(s.contains("#set page(paper: \"a4\", columns: 2)"));
        assert!(s.contains("*aiya*"));
        assert!(s.contains("*bara*"));
        // Title shows the language name.
        assert!(s.contains("Quenya dictionary"));
    }

    #[test]
    fn escape_hjson_handles_quotes_and_backslashes() {
        assert_eq!(escape_hjson(r#"he said "hi""#), r#"he said \"hi\""#);
        assert_eq!(escape_hjson(r"a\b"), r"a\\b");
    }
}
