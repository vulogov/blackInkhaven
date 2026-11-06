//! `inkhaven language` phonology surface: syllabification, IPA realisation,
//! stress, tone sandhi, romanisation, and word generation. Split out of the flat
//! handler; the phonology loaders (`load_phonology`, `pronounce`) live in the
//! parent module.

use crate::error::{Error, Result};

use super::*;

/// LANG-1 P1.6 — apply tone sandhi to an explicit tone sequence.
pub(crate) fn tone_sandhi(project: &Path, language: &str, tones: &str, json: bool) -> Result<()> {
    let (_store, phonology) = open_phonology(project, language)?;
    let system = phonology.tone.as_ref().ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` declares no `tone` system in its Phonology block"
        ))
    })?;
    let input: Vec<String> = tones.split_whitespace().map(String::from).collect();
    let surface = crate::conlang::phonology::tone_eval::apply_sandhi(system, &input);
    emit(json, &shapes::ToneSandhi { input, surface }, |t| println!("{}", t.surface.join(" ")))
}

/// LANG-1 P1.5 — convert between IPA and a named romanization scheme.
pub(crate) fn romanize_text(
    project: &Path,
    language: &str,
    text: &str,
    scheme: Option<&str>,
    reverse: bool,
    json: bool,
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

    let (direction, output) = if reverse {
        let seq = romanize::deromanize(scheme_ref, &phonology, text);
        ("text_to_ipa", seq.join(""))
    } else {
        let seq: Vec<String> = text.split_whitespace().map(String::from).collect();
        ("ipa_to_text", romanize::romanize(scheme_ref, &phonology, &seq))
    };
    let shape = shapes::Romanized {
        input: text.to_string(),
        scheme: scheme_ref.name.clone(),
        direction: direction.to_string(),
        output,
    };
    emit(json, &shape, |r| {
        if reverse {
            println!("/{}/", r.output);
        } else {
            println!("{}", r.output);
        }
    })
}

/// LANG-1 P1.4 — place primary stress on a word per the language's stress
/// rule and print the syllabification with `ˈ` before the stressed syllable.
pub(crate) fn stress_word(project: &Path, language: &str, word: &str, json: bool) -> Result<()> {
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
    let syllables: Vec<String> = sylls
        .iter()
        .map(|s| s.onset.iter().chain(&s.nucleus).chain(&s.coda).map(&g).collect())
        .collect();
    let marked = syllables
        .iter()
        .enumerate()
        .map(|(i, body)| if Some(i) == stressed { format!("ˈ{body}") } else { body.clone() })
        .collect::<Vec<_>>()
        .join(".");
    let shape = shapes::StressedWord { word: word.to_string(), syllables, stressed, marked };
    emit(json, &shape, |s| println!("{}", s.marked))
}

/// 1.8.2 — scan a line (or stanza) of verse: mark each syllable's beat and name
/// the metre. Stress per word resolves through explicit mark → lexicon → rule.
pub(crate) fn scan_verse(project: &Path, language: &str, text: &str, json: bool) -> Result<()> {
    use crate::conlang::scansion;

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phonology = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no phoneme block yet — add a `phonemes` block \
             under its `Phonology` chapter before scanning verse"
        ))
    })?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;

    let lines = scansion::scan_text(&phonology, &entries, text);
    if lines.is_empty() {
        return Err(Error::Config("no verse to scan (the text is empty)".into()));
    }
    let dominant = if lines.len() > 1 { scansion::dominant_meter(&lines) } else { None };
    let shape = shapes::Scansion {
        lines: lines.iter().map(shapes::ScannedLine::from).collect(),
        dominant_meter: dominant.as_ref().map(shapes::MeterShape::from),
    };

    emit(json, &shape, |_| {
        println!("scan · {language}   (/ stressed  × unstressed  · flexible)\n");
        for line in &lines {
            // Two aligned rows: the syllables, and their beat glyphs beneath.
            let mut top: Vec<String> = Vec::new();
            let mut bot: Vec<String> = Vec::new();
            for (wi, w) in line.words.iter().enumerate() {
                if wi > 0 {
                    top.push("|".into());
                    bot.push("|".into());
                }
                for s in &w.syllables {
                    let width = s.text.chars().count().max(1);
                    top.push(format!("{:<width$}", s.text, width = width));
                    bot.push(format!("{:^width$}", s.beat.glyph(), width = width));
                }
            }
            println!("  {}", top.join(" "));
            println!("  {}", bot.join(" "));
            let n = line.beats().len();
            match &line.meter {
                Some(m) => println!("  → {} · {n} syllable(s) · fit {:.2}\n", m.name, m.conformance),
                None => println!("  → irregular / free · {n} syllable(s)\n"),
            }
        }
        if lines.len() > 1 {
            match &dominant {
                Some(m) => println!("dominant metre · {} (fit {:.2})", m.name, m.conformance),
                None => println!("dominant metre · irregular"),
            }
        }
    })
}

/// LANG-1 P1.3 — derive and print a word's surface pronunciation by applying
/// the language's allophony rules to its underlying form.
pub(crate) fn ipa_surface(project: &Path, language: &str, word: &str, json: bool) -> Result<()> {
    let (_store, phonology) = open_phonology(project, language)?;
    let underlying = phonology.segment(word);
    let surface = crate::conlang::phonology::allophony_eval::surface_form(&phonology, &underlying);

    let romanized: String = surface
        .iter()
        .map(|ipa| {
            phonology
                .phoneme(ipa)
                .map(|p| p.grapheme().to_string())
                .unwrap_or_else(|| ipa.clone())
        })
        .collect();
    let shape = shapes::IpaSurface {
        word: word.to_string(),
        underlying: underlying.join(""),
        surface: surface.join(""),
        romanized,
    };
    emit(json, &shape, |s| {
        println!("underlying  /{}/", s.underlying);
        println!("surface     [{}]", s.surface);
        println!("romanized    {}", s.romanized);
    })
}

/// LANG-1 P1.2 — syllabify a word against a language's phonology and print
/// the `CV.CVC`-style breakdown. Loads the Phonology block, segments the
/// word into phonemes (longest-grapheme match), and runs the sonority-aware
/// syllabifier.
pub(crate) fn syllabify_word(project: &Path, language: &str, word: &str, json: bool) -> Result<()> {
    let (_store, phonology) = open_phonology(project, language)?;
    let seq = phonology.segment(word);
    let sylls = crate::conlang::phonology::syllable::syllabify(&phonology, &seq);
    // The same list `ink.lang.syllabify` returns: one grapheme string per syllable.
    let g = |ipa: &String| {
        phonology.phoneme(ipa).map(|p| p.grapheme().to_string()).unwrap_or_else(|| ipa.clone())
    };
    let syllables: Vec<String> = sylls
        .iter()
        .map(|s| s.onset.iter().chain(&s.nucleus).chain(&s.coda).map(&g).collect())
        .collect();
    emit(json, &syllables, |_| {
        println!("{}", crate::conlang::phonology::syllable::render(&phonology, &sylls));
    })?;
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
/// declared constraint.  Empty / absent phonology, and a batch in which no
/// candidate passes, are clear, actionable errors (exit 1) rather than a
/// silent empty list.
/// Generate `count` candidates and fail when none passes (LI-7): the
/// constraints are the reason only when candidates were actually tried, and
/// clap guarantees `count >= 1` on the CLI.
pub(crate) fn generate_checked(
    phonology: &crate::conlang::Phonology,
    role: crate::conlang::TemplateRole,
    count: usize,
) -> Result<Vec<String>> {
    let words = crate::conlang::generate::word::generate_words(phonology, role, count);
    if words.is_empty() {
        return Err(Error::Config(format!(
            "0 of {count} candidates passed the constraints — loosen the phonotactic constraints \
             (`constraints`, `max_cluster_size`, `no_geminate`), widen the `{}` templates' classes, \
             or check `language doctor` for an empty class",
            role.as_str()
        )));
    }
    Ok(words)
}

pub(crate) fn generate_word(project: &Path, language: &str, role: &str, count: usize, json: bool) -> Result<()> {
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

    let words = generate_checked(&phonology, role, count)?;
    // The same list `ink.lang.generate_words` returns.
    emit(json, &words, |ws| {
        for w in ws {
            println!("{w}");
        }
    })?;
    eprintln!(
        "generated {} / {} requested `{}` word(s) for {}",
        words.len(),
        count,
        role.as_str(),
        language
    );
    Ok(())
}

#[cfg(test)]
mod generate_checked_tests {
    use super::*;
    use crate::conlang::{Phonology, TemplateRole};

    #[test]
    fn a_batch_with_no_passing_candidate_is_an_error_that_names_the_count() {
        // `V` is declared but empty, so `C V` can never be realised.
        let p = Phonology::from_hjson(r#"{
            phonemes: [ { ipa: "t", kind: "consonant" }, { ipa: "k", kind: "consonant" } ],
            classes: { C: ["t", "k"], V: [] },
            templates: { root: [ { pattern: "C V" } ] }
        }"#)
        .unwrap()
        .unwrap();
        let err = generate_checked(&p, TemplateRole::Root, 7).unwrap_err().to_string();
        assert!(err.contains("0 of 7 candidates passed the constraints"), "{err}");
        assert!(err.contains("`root` templates"), "{err}");

        // With a vowel the same request succeeds and respects `count`.
        let ok = Phonology::from_hjson(r#"{
            phonemes: [ { ipa: "t", kind: "consonant" }, { ipa: "a", kind: "vowel" } ],
            classes: { C: ["t"], V: ["a"] },
            templates: { root: [ { pattern: "C V" } ] }
        }"#)
        .unwrap()
        .unwrap();
        assert_eq!(generate_checked(&ok, TemplateRole::Root, 3).unwrap().len(), 3);
    }
}
