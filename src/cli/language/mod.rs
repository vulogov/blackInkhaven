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
mod authoring;
mod books;
mod contact;
mod diachronic;
mod doctor;
mod expressions;
mod import;
mod lexicon;
mod metrics;
mod morphology;
mod phonology;
mod render;
mod scaffold;
mod syntax;
mod translate;
mod typology;
mod varieties;
mod writing;
pub(crate) use authoring::*;
pub(crate) use books::*;
pub(crate) use contact::*;
pub(crate) use diachronic::*;
pub(crate) use doctor::*;
pub(crate) use expressions::*;
pub(crate) use import::*;
pub(crate) use lexicon::*;
pub(crate) use metrics::*;
pub(crate) use morphology::*;
pub(crate) use phonology::*;
pub(crate) use render::*;
pub(crate) use scaffold::*;
pub(crate) use syntax::*;
pub(crate) use translate::*;
pub(crate) use typology::*;
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
        LanguageCommand::Metrics { language, json } => metrics(project, &language, json),
        LanguageCommand::Universals { language, json } => universals(project, &language, json),
        LanguageCommand::GrammarCheck { language, json } => {
            grammar_check(project, &language, json)
        }
        LanguageCommand::Pairs { language, limit, json } => {
            pairs(project, &language, limit, json)
        }
        LanguageCommand::Naturalness { language, json } => {
            naturalness(project, &language, json)
        }
        LanguageCommand::Harmony { language, json } => harmony(project, &language, json),
        LanguageCommand::Distribution { language, json } => {
            distribution(project, &language, json)
        }
        LanguageCommand::Sketch { language, out } => sketch(project, &language, out.as_deref()),
        LanguageCommand::SuggestPhonemes { language, json } => {
            suggest_phonemes(project, &language, json)
        }
        LanguageCommand::Scaffold { from, out, provider } => {
            scaffold_from(project, &from, out.as_deref(), provider.as_deref())
        }
        LanguageCommand::Parse { language, word, json } => {
            parse_surface(project, &language, &word, json)
        }
        LanguageCommand::Link { language, verb, args, valence, json } => {
            link_args(project, &language, &verb, &args, valence.as_deref(), json)
        }
        LanguageCommand::Check { language, word, json } => {
            oracle_check(project, &language, &word, json)
        }
        LanguageCommand::CheckClause { language, verb, args, verb_root, subject_features, valence, json } => {
            oracle_check_clause(
                project,
                &language,
                &verb,
                &args,
                verb_root.as_deref(),
                subject_features.as_deref(),
                valence.as_deref(),
                json,
            )
        }
        LanguageCommand::CheckAgreement { language, dependent, form, root, head_features, json } => {
            oracle_check_agreement(project, &language, &dependent, &form, &root, &head_features, json)
        }
        LanguageCommand::Tree { language, verb, args, word_order, json } => {
            build_tree(project, &language, &verb, &args, word_order.as_deref(), json)
        }
        LanguageCommand::Movement { language, verb, args, r#move, word_order, json } => {
            movement(project, &language, &verb, &args, &r#move, word_order.as_deref(), json)
        }
        LanguageCommand::Binding {
            language,
            verb,
            args,
            antecedent,
            anaphor,
            r#type,
            word_order,
            json,
        } => binding(
            project,
            &language,
            &verb,
            &args,
            &antecedent,
            &anaphor,
            &r#type,
            word_order.as_deref(),
            json,
        ),
        LanguageCommand::Trace { language, rule, limit, json } => {
            trace(project, &language, &rule, limit, json)
        }
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
        LanguageCommand::Igt { language, text, save, name, json } => {
            igt_text(project, &language, &text, save, name.as_deref(), json)
        }
        LanguageCommand::Texts { language, name, set_translation, format, json } => {
            list_texts(project, &language, name.as_deref(), set_translation.as_deref(), &format, json)
        }
        LanguageCommand::Frequency { language, lemma, top, json } => {
            corpus_report(project, &language, lemma, top, json)
        }
        LanguageCommand::Concordance { language, word, lemma, window, json } => {
            concordance(project, &language, &word, lemma, window, json)
        }
        LanguageCommand::Collocations { language, word, lemma, window, top, json } => {
            collocations(project, &language, &word, lemma, window, top, json)
        }
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

/// IGT-1 (Wave 4) — load every stored interlinear under a language's `Texts`
/// chapter, as `(title, Igt)` in tree order. Best-effort: unparseable paragraphs
/// are skipped.
pub(crate) fn load_texts(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Vec<(String, crate::conlang::igt::Igt)> {
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Texts"))
        .cloned()
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for id in hierarchy.collect_subtree(chapter.id) {
        let Some(node) = hierarchy.get(id) else { continue };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(node.id) else { continue };
        let body = String::from_utf8_lossy(&bytes);
        if let Ok(igt) = serde_json::from_str::<crate::conlang::igt::Igt>(&body) {
            out.push((node.title.clone(), igt));
        }
    }
    out
}

/// IGT-1 (Wave 4) — curate a stored text's free translation in place. Finds the
/// paragraph named `name` under the language's `Texts` chapter, replaces the IGT's
/// `translation`, and rewrites the paragraph. Returns `false` when no such text
/// exists. The gloss/segmentation are untouched — only the free translation.
pub(crate) fn set_text_translation(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
    name: &str,
    translation: &str,
) -> Result<bool> {
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Texts"))
        .cloned()
    else {
        return Ok(false);
    };
    let Some(mut node) = hierarchy
        .collect_subtree(chapter.id)
        .into_iter()
        .filter_map(|id| hierarchy.get(id))
        .find(|n| n.kind == NodeKind::Paragraph && n.title.eq_ignore_ascii_case(name))
        .cloned()
    else {
        return Ok(false);
    };
    let Ok(Some(bytes)) = store.get_content(node.id) else { return Ok(false) };
    let mut igt: crate::conlang::igt::Igt = serde_json::from_str(&String::from_utf8_lossy(&bytes))
        .map_err(|e| Error::Store(format!("parsing stored text `{name}`: {e}")))?;
    igt.translation = translation.to_string();

    let body = serde_json::to_string_pretty(&igt).map_err(|e| Error::Store(format!("serialize igt: {e}")))?;
    if let Some(rel) = &node.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes()).map_err(|e| Error::Store(format!("write igt file: {e}")))?;
    }
    store
        .update_paragraph_content(&mut node, body.as_bytes())
        .map_err(|e| Error::Store(format!("write igt: {e}")))?;
    Ok(true)
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



/// Minimal HJSON string escape — backslash-quote +
/// backslash-backslash.  Sufficient for the
/// dictionary-entry seed body, which never sees
/// control characters in practice.
fn escape_hjson(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

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
