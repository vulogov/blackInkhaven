//! `inkhaven language` clause-building surface: relative / complement /
//! coordinate clause assembly, agreement, and interlinear rendering over the
//! conlang syntax engine. Split out of the flat handler.

use crate::error::{Error, Result};

use super::*;

/// Print a rendered clause/phrase as a two-line interlinear gloss + literal.
pub(crate) fn print_interlinear(rendered: &crate::conlang::syntax::RenderedClause) {
    let widths: Vec<usize> = rendered
        .words
        .iter()
        .map(|(w, g)| w.chars().count().max(g.chars().count()) + 2)
        .collect();
    let mut line1 = String::from("  ");
    let mut line2 = String::from("  ");
    for (i, (w, g)) in rendered.words.iter().enumerate() {
        line1.push_str(&format!("{:<width$}", w, width = widths[i]));
        line2.push_str(&format!("{:<width$}", g, width = widths[i]));
    }
    println!("{line1}");
    println!("{line2}");
    println!("  '{}'", rendered.literal);
}

/// 1.3.19 LANG-1 P9 — render a head noun modified by a relative clause.
#[allow(clippy::too_many_arguments)]
pub(crate) fn relative(
    project: &Path,
    language: &str,
    head: &str,
    role: &str,
    verb: &str,
    with: Option<&str>,
    relativizer: Option<&str>,
    noun_paradigm: &str,
    verb_paradigm: &str,
) -> Result<()> {
    use crate::conlang::syntax::{self, RelativeClause};
    let head_is_subject = match role.to_ascii_lowercase().as_str() {
        "subject" | "subj" | "s" => true,
        "object" | "obj" | "o" => false,
        other => {
            return Err(Error::Config(format!(
                "unknown --role `{other}` (expected subject or object)"
            )))
        }
    };
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no phoneme block"))
    })?;
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let (grammar_spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;

    let rc = RelativeClause {
        head_is_subject,
        verb: parse_word(verb),
        other: with.map(parse_word),
        relativizer: relativizer.map(parse_word),
        noun_paradigm: noun_paradigm.to_string(),
        verb_paradigm: verb_paradigm.to_string(),
    };
    let head_word = parse_word(head);
    let rendered = syntax::relative_np(&phon, &morph, &grammar_spec.grammar, &head_word, &rc);

    let strategy = grammar_spec
        .grammar
        .get("relative_clause")
        .map(String::as_str)
        .unwrap_or("postnominal");
    println!("{} ({strategy})", rendered.surface);
    print_interlinear(&rendered);
    Ok(())
}

/// 1.3.19 LANG-1 P9 — coordinate noun phrases or clauses with a conjunction.
pub(crate) fn coordinate(
    project: &Path,
    language: &str,
    clauses: &[String],
    nps: &[String],
    conjunction: Option<&str>,
    noun_paradigm: &str,
    verb_paradigm: &str,
) -> Result<()> {
    use crate::conlang::syntax::{self, Clause, NounPhrase};
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no phoneme block"))
    })?;
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let (grammar_spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let conj = conjunction.map(parse_word);

    let rendered = if nps.len() >= 2 {
        let parts: Vec<_> = nps
            .iter()
            .map(|n| syntax::bare_np(&parse_word(n)))
            .collect();
        syntax::coordinate(&parts, conj.as_ref())
    } else if clauses.len() >= 2 {
        // Each --clause is "subj verb [obj]", space-separated root:gloss words.
        let mut parts = Vec::new();
        for spec in clauses {
            let toks: Vec<&str> = spec.split_whitespace().collect();
            if toks.len() < 2 {
                return Err(Error::Config(format!(
                    "--clause `{spec}` needs at least a subject and a verb"
                )));
            }
            let np = |t: &str| NounPhrase {
                head: parse_word(t),
                number: "sg".into(),
                adjective: None,
            };
            let c = Clause {
                subject: Some(np(toks[0])),
                verb: Some(parse_word(toks[1])),
                verb_person: "3".into(),
                object: toks.get(2).map(|t| np(t)),
                noun_paradigm: noun_paradigm.to_string(),
                verb_paradigm: verb_paradigm.to_string(),
                ..Default::default()
            };
            parts.push(syntax::assemble(&phon, &morph, &grammar_spec.grammar, &c));
        }
        syntax::coordinate(&parts, conj.as_ref())
    } else {
        return Err(Error::Config(
            "give at least two --np nouns or two --clause clauses to coordinate".into(),
        ));
    };

    println!("{}", rendered.surface);
    print_interlinear(&rendered);
    Ok(())
}

/// 1.3.19 LANG-1 P9 — assemble a sentence with a complement clause.
#[allow(clippy::too_many_arguments)]
pub(crate) fn complement(
    project: &Path,
    language: &str,
    subject: Option<&str>,
    subject_person: &str,
    subject_number: &str,
    verb: &str,
    complementizer: Option<&str>,
    comp_subject: Option<&str>,
    comp_verb: &str,
    comp_object: Option<&str>,
    noun_paradigm: &str,
    verb_paradigm: &str,
) -> Result<()> {
    use crate::conlang::syntax::{self, Clause, ComplementSentence, NounPhrase};
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no phoneme block"))
    })?;
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let (grammar_spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;

    let np = |w: &str| NounPhrase {
        head: parse_word(w),
        number: "sg".into(),
        adjective: None,
    };
    let complement_clause = Clause {
        subject: comp_subject.map(np),
        verb: Some(parse_word(comp_verb)),
        verb_person: "3".into(),
        object: comp_object.map(np),
        noun_paradigm: noun_paradigm.to_string(),
        verb_paradigm: verb_paradigm.to_string(),
        ..Default::default()
    };
    let cs = ComplementSentence {
        matrix_subject: subject.map(parse_word),
        matrix_verb: parse_word(verb),
        matrix_person: subject_person.to_string(),
        matrix_number: subject_number.to_string(),
        complementizer: complementizer.map(parse_word),
        complement: complement_clause,
        noun_paradigm: noun_paradigm.to_string(),
        verb_paradigm: verb_paradigm.to_string(),
    };
    let rendered = syntax::complement_sentence(&phon, &morph, &grammar_spec.grammar, &cs);

    let order = grammar_spec.grammar.get("word_order").map(String::as_str).unwrap_or("svo");
    println!("{} ({} order)", rendered.surface, order.to_uppercase());
    print_interlinear(&rendered);
    Ok(())
}

/// LANG-1 P3.x — make a dependent word agree with its head's features.
pub(crate) fn agree(
    project: &Path,
    language: &str,
    word: &str,
    pos: &str,
    features: &str,
    gloss: Option<&str>,
) -> Result<()> {
    use std::collections::BTreeMap;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phonology = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no phoneme block"))
    })?;
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no morphology yet — add a `morphemes` / `paradigms` / \
             `agreement` HJSON paragraph under its `Grammar` chapter"
        ))
    })?;
    let rule = morph.agreement_for(pos).ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no agreement rule for `{pos}` (dependents: {})",
            morph.agreement.iter().map(|a| a.dependent.as_str()).collect::<Vec<_>>().join(", ")
        ))
    })?;

    // Parse `number=pl,case=dat` into a feature map.
    let head_features: BTreeMap<String, String> = features
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();

    let root_gloss = gloss.unwrap_or(word);
    let result = crate::conlang::morphology::agreement::agree(
        &phonology, &morph, rule, word, root_gloss, &head_features,
    )
    .ok_or_else(|| {
        Error::Config(format!(
            "no form of `{word}` agrees with those features — check the `{}` paradigm has a \
             matching cell, and that --features uses the rule's features ({})",
            rule.paradigm,
            rule.features.join(", ")
        ))
    })?;

    let matched = result
        .matched
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" ");
    let head = if rule.head.is_empty() { "head".to_string() } else { rule.head.clone() };
    println!("{word} ({pos}) agreeing with its {head} [{matched}]:");
    println!("  {} — {}", result.form, result.gloss);
    Ok(())
}
