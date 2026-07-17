//! `inkhaven language` phonology surface: syllabification, IPA realisation,
//! stress, tone sandhi, romanisation, and word generation. Split out of the flat
//! handler; the phonology loaders (`load_phonology`, `pronounce`) live in the
//! parent module.

use crate::error::{Error, Result};

use super::*;

/// LANG-1 P1.6 — apply tone sandhi to an explicit tone sequence.
pub(crate) fn tone_sandhi(project: &Path, language: &str, tones: &str) -> Result<()> {
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
pub(crate) fn romanize_text(
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
pub(crate) fn stress_word(project: &Path, language: &str, word: &str) -> Result<()> {
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
pub(crate) fn ipa_surface(project: &Path, language: &str, word: &str) -> Result<()> {
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
pub(crate) fn syllabify_word(project: &Path, language: &str, word: &str) -> Result<()> {
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
pub(crate) fn generate_word(project: &Path, language: &str, role: &str, count: usize) -> Result<()> {
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
