//! `inkhaven poetry` — the poetry toolset CLI (POEM-1 onward). This slice covers
//! `forms`: list the built-in forms, print a form's `poem:` block for a language,
//! or scaffold a custom form. Later phases add scan / syllabify / rhyme / metre /
//! status / translation subcommands.

use crate::error::{Error, Result};
use crate::poetry::form::{FormsLibrary, PoemForm};
use crate::poetry::syllabify::{StressLevel, syllabify as syllabify_word};
use crate::prose::ProseLanguage;

/// `poetry syllabify <word> | --line "…" [--language]` — show syllable
/// boundaries and the stressed syllable.
pub fn syllabify(word: Option<&str>, line: Option<&str>, language: Option<&str>) -> Result<()> {
    let lang = ProseLanguage::from_label(language.unwrap_or("en"));
    let show = |w: &str| {
        let clean = w.trim_matches(|c: char| !c.is_alphabetic());
        if clean.is_empty() {
            return;
        }
        let sylls = syllabify_word(clean, lang.clone());
        let rendered: String = sylls
            .iter()
            .map(|s| {
                let mark = if s.stress == StressLevel::Primary { "ˈ" } else { "" };
                format!("{mark}{}", s.text)
            })
            .collect::<Vec<_>>()
            .join("·");
        println!("  {:<18} {rendered}  ({} syl)", w, sylls.len());
    };

    if let Some(l) = line {
        for w in l.split_whitespace() {
            show(w);
        }
        Ok(())
    } else if let Some(w) = word {
        show(w);
        Ok(())
    } else {
        Err(Error::Config("syllabify needs a <WORD> or --line \"…\"".into()))
    }
}

/// `poetry scan --text "…" --form N [--language]` — the Inner Poet fast track.
pub fn scan(text: &str, form_name: &str, language: Option<&str>) -> Result<()> {
    use crate::inner_poet::fast::{Severity, scan_stanza};
    let lang = language.unwrap_or("en");
    let lib = FormsLibrary::builtin();
    let form = lib
        .localized(form_name, lang)
        .ok_or_else(|| Error::Config(format!("unknown form `{form_name}` — run `inkhaven poetry forms`")))?;
    let findings = scan_stanza(text, &form);
    if findings.is_empty() {
        println!("♪ no findings — the stanza matches its declared {} form.", form.form);
        return Ok(());
    }
    for f in &findings {
        let sev = match f.severity {
            Severity::Praise => "Praise",
            Severity::Note => "Note",
            Severity::Concern => "Concern",
        };
        println!("♪ {sev:<8} [{}]  {}", f.kind, f.message);
    }
    Ok(())
}

/// `poetry status --text "…" --form N` — completion + missing components.
pub fn status(text: &str, form_name: &str, language: Option<&str>) -> Result<()> {
    let lang = language.unwrap_or("en");
    let lib = FormsLibrary::builtin();
    let form = lib
        .localized(form_name, lang)
        .ok_or_else(|| Error::Config(format!("unknown form `{form_name}` — run `inkhaven poetry forms`")))?;
    let st = crate::poetry::form_check::check_form(text, &form);
    let ratio = match st.expected_lines {
        Some(e) => format!("{}/{}", st.lines_written, e),
        None => format!("{} lines (open form)", st.lines_written),
    };
    let state = if st.complete { "complete" } else { "drafting" };
    println!("♩ {} · {ratio} · {state}", form.form);
    if st.issues.is_empty() {
        println!("  no structural issues");
    } else {
        for i in &st.issues {
            println!("  ⚠ {i}");
        }
    }
    Ok(())
}

/// `poetry trilemma --source --translation [--form --language --to-language]`.
pub fn trilemma(
    source: &str,
    translation: &str,
    form_name: Option<&str>,
    language: Option<&str>,
    to_language: Option<&str>,
) -> Result<()> {
    let (from, to) = (language.unwrap_or("en"), to_language.unwrap_or("en"));
    let form = match form_name {
        Some(name) => FormsLibrary::builtin()
            .localized(name, from)
            .ok_or_else(|| Error::Config(format!("unknown form `{name}`")))?,
        None => PoemForm::default(),
    };
    let (src_l, trans_l) =
        (ProseLanguage::from_label(from), ProseLanguage::from_label(to));
    let tri = crate::poetry::translation::trilemma(source, &src_l, translation, &trans_l, &form);

    let bar = |score: f64| -> String {
        let n = (score * 10.0).round().clamp(0.0, 10.0) as usize;
        format!("{}{}", "█".repeat(n), "░".repeat(10 - n))
    };
    println!("Translation trilemma ({from} → {to}):\n");
    println!(
        "  Form     {}  {:>3.0}%   {} · {}",
        bar(tri.form_score),
        tri.form_score * 100.0,
        tri.metre_note,
        tri.rhyme_note
    );
    println!("  Meaning  ░░░░░░░░░░       (the AI axis — engage the Inner Poet in the editor)");
    println!(
        "  Sound    {}  {:>3.0}%   {}",
        bar(tri.sound_score),
        tri.sound_score * 100.0,
        tri.sound_note
    );
    Ok(())
}

/// `poetry rhyme <word1> <word2> [--language]` — classify a rhyme.
pub fn rhyme(w1: &str, w2: &str, language: Option<&str>) -> Result<()> {
    use crate::poetry::rhyme::{RhymeQuality, RhymeType, analyse_rhyme};
    let lang = ProseLanguage::from_label(language.unwrap_or("en"));
    let r = analyse_rhyme(w1, w2, lang);
    let quality = match r.quality {
        RhymeQuality::Perfect => "perfect",
        RhymeQuality::Near => "near",
        RhymeQuality::Eye => "eye",
        RhymeQuality::None => "no",
    };
    let rtype = match r.rhyme_type {
        RhymeType::Masculine => "masculine",
        RhymeType::Feminine => "feminine",
        RhymeType::Dactylic => "dactylic",
    };
    if matches!(r.quality, RhymeQuality::None) {
        println!("  {w1} / {w2}: no rhyme");
    } else {
        let shared = if r.shared.is_empty() { String::new() } else { format!(" on “-{}”", r.shared) };
        println!("  {w1} / {w2}: {quality} {rtype} rhyme{shared}");
    }
    if let Some(note) = r.note {
        println!("  ({note})");
    }
    Ok(())
}

/// `poetry metre --line "…" [--form N] [--language L]` — scan a verse line.
pub fn metre(line: &str, form: Option<&str>, language: Option<&str>) -> Result<()> {
    let lang = ProseLanguage::from_label(language.unwrap_or("en"));
    let beats = crate::poetry::metre::line_to_beats(line, lang);
    let pattern: String = beats.iter().map(|b| b.glyph().to_string()).collect::<Vec<_>>().join(" ");

    println!("  {line}");
    println!("  {pattern}   ({} syllables)", beats.len());

    match crate::poetry::metre::detect(&beats) {
        Some(m) => println!("  → detected: {} (fit {:.2})", m.name, m.conformance),
        None => println!("  → detected: irregular / free"),
    }

    if let Some(f) = form {
        let lib = FormsLibrary::builtin();
        let pf = lib
            .localized(f, language.unwrap_or("en"))
            .ok_or_else(|| Error::Config(format!("unknown form `{f}`")))?;
        match crate::poetry::metre::Foot::parse(&pf.metre) {
            Some(foot) if pf.feet > 0 => {
                let scan = crate::poetry::metre::scan_line(&beats, foot, pf.feet as usize);
                let extra = if scan.feminine_ending {
                    " · feminine ending"
                } else if scan.catalectic {
                    " · catalectic (one short)"
                } else {
                    ""
                };
                println!(
                    "  → declared {} ({} feet): {} of {} syllables, fit {:.2}{extra}",
                    pf.metre, pf.feet, scan.syllables, scan.expected_syllables, scan.conformance
                );
            }
            _ => println!("  → form `{f}` declares no accentual-syllabic metre to check against"),
        }
    }
    Ok(())
}

/// `poetry forms [--form N] [--language L] [--new --name M]`.
pub fn forms(form: Option<&str>, language: Option<&str>, new: bool, name: Option<&str>) -> Result<()> {
    let lib = FormsLibrary::builtin();

    if new {
        // Scaffold a `form: custom` block to paste into a poem: sidecar or into
        // .inkhaven/custom-forms.hjson. (An interactive editor + auto-save is a
        // later refinement.)
        let scaffold = PoemForm {
            form: "custom".into(),
            title: name.unwrap_or("my-form").to_string(),
            metre: "iambic".into(),
            feet: 5,
            metre_tradition: "accentual_syllabic".into(),
            rhyme_scheme: "ABAB".into(),
            language: language.unwrap_or("en").to_string(),
            ..Default::default()
        };
        println!("// A custom-form scaffold — edit the fields, then paste this into a");
        println!("// `poem:` sidecar, or into .inkhaven/custom-forms.hjson to reuse it.");
        print!("{}", scaffold.to_poem_block());
        return Ok(());
    }

    if let Some(f) = form {
        let lang = language.unwrap_or("en");
        let pf = lib.localized(f, lang).ok_or_else(|| {
            Error::Config(format!(
                "unknown form `{f}` — run `inkhaven poetry forms` to list the {} available",
                lib.all().len()
            ))
        })?;
        print!("{}", pf.to_poem_block());
        return Ok(());
    }

    println!(
        "poetry forms — `--form <name> [--language en|ru|fr|de|es]` prints a poem: block:\n"
    );
    for pf in lib.all() {
        println!("  {:<22}  {}", pf.form, pf.desc);
    }
    Ok(())
}
