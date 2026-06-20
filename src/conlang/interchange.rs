//! LANG-1 P6 — interchange with external conlang/linguistics tooling.
//!
//! Pure, book-walk-free renderers (exporters) and parsers (importers) that
//! bridge Inkhaven's lexicon to the formats real conlangers and linguists
//! already use. The CLI `language export` / `language import` handlers do the
//! book I/O and delegate the actual format work here so it stays unit-testable.
//!
//! Exporters (this increment): XLIFF 1.2 (translation interchange),
//! linguex (LaTeX linguistic examples), and an IPA inventory chart.

use crate::conlang::types::{Phoneme, PhonemeKind, Phonology};
use crate::language_entry::DictionaryEntry;

/// Map a project working-language name (`english`, `russian`, …) to its
/// ISO-639-1 code for interchange headers. Unknown names fall through to the
/// trimmed lowercase input so a code passed verbatim still works.
pub fn iso_code(working_language: &str) -> String {
    match working_language.trim().to_ascii_lowercase().as_str() {
        "english" | "" => "en".into(),
        "russian" => "ru".into(),
        "french" => "fr".into(),
        "german" => "de".into(),
        "spanish" => "es".into(),
        other => other.to_string(),
    }
}

/// A BCP-47 private-use tag for the invented language, derived from its name.
/// Conlangs share the ISO 639 collective code `art` (artificial); we append a
/// private-use subtag so distinct languages stay distinguishable in a TM.
fn art_tag(language: &str) -> String {
    let slug: String = language
        .chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else {
                None
            }
        })
        .take(8)
        .collect();
    if slug.is_empty() {
        "art".into()
    } else {
        format!("art-x-{slug}")
    }
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Render the lexicon as an XLIFF 1.2 document — the OASIS translation
/// interchange standard that CAT tools (OmegaT, memoQ, Trados, Weblate) read.
/// Each entry becomes a `trans-unit` whose source is the working-language
/// translation and whose target is the invented word, so the dictionary
/// doubles as a translation memory.
pub fn xliff(
    language: &str,
    working_language: &str,
    entries: &[(String, DictionaryEntry)],
) -> String {
    let src = iso_code(working_language);
    let tgt = art_tag(language);
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(
        "<xliff version=\"1.2\" xmlns=\"urn:oasis:names:tc:xliff:document:1.2\">\n",
    );
    out.push_str(&format!(
        "  <file original=\"{}.dictionary\" source-language=\"{}\" \
         target-language=\"{}\" datatype=\"plaintext\">\n",
        xml_escape(language),
        xml_escape(&src),
        xml_escape(&tgt),
    ));
    out.push_str("    <body>\n");
    for (idx, (title, e)) in entries.iter().enumerate() {
        // The source side is the meaning (working language); the target is
        // the coined word. Entries with no translation still round-trip with
        // an empty source so nothing is silently dropped.
        let source = if e.translation.is_empty() {
            title.as_str()
        } else {
            e.translation.as_str()
        };
        out.push_str(&format!(
            "      <trans-unit id=\"{}\" resname=\"{}\">\n",
            idx + 1,
            xml_escape(title),
        ));
        out.push_str(&format!(
            "        <source>{}</source>\n",
            xml_escape(source)
        ));
        out.push_str(&format!(
            "        <target>{}</target>\n",
            xml_escape(&e.word)
        ));
        if !e.pos.is_empty() {
            out.push_str(&format!(
                "        <note>{}</note>\n",
                xml_escape(&e.pos)
            ));
        }
        out.push_str("      </trans-unit>\n");
    }
    out.push_str("    </body>\n");
    out.push_str("  </file>\n");
    out.push_str("</xliff>\n");
    out
}

fn latex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '\\' => out.push_str("\\textbackslash{}"),
            _ => out.push(c),
        }
    }
    out
}

/// Render the lexicon as a LaTeX document using the `linguex` package — the
/// standard way linguists typeset numbered, glossed examples. Each headword is
/// a bold lemma with its part of speech and gloss; entries that carry an
/// example sentence get it as a numbered `\ex.` with the translation beneath,
/// ready to paste into a paper or grammar sketch.
pub fn linguex(language: &str, entries: &[(String, DictionaryEntry)]) -> String {
    let mut out = String::new();
    out.push_str("% Generated by Inkhaven · language export --format linguex\n");
    out.push_str("\\documentclass[11pt]{article}\n");
    out.push_str("\\usepackage[utf8]{inputenc}\n");
    out.push_str("\\usepackage{linguex}\n");
    out.push_str("\\usepackage{tipa}\n");
    out.push_str(&format!(
        "\\title{{{} --- Lexicon}}\n",
        latex_escape(language)
    ));
    out.push_str("\\begin{document}\n");
    out.push_str("\\maketitle\n\n");
    for (title, e) in entries {
        let pos = if e.pos.is_empty() {
            String::new()
        } else {
            format!(" \\textit{{{}}}", latex_escape(&e.pos))
        };
        let gloss = if e.translation.is_empty() {
            String::new()
        } else {
            format!(" `{}'", latex_escape(&e.translation))
        };
        out.push_str(&format!(
            "\\noindent\\textbf{{{}}}{}{}\\\\\n",
            latex_escape(if e.word.is_empty() { title } else { &e.word }),
            pos,
            gloss,
        ));
        if !e.example.is_empty() {
            // A flat example string — render it as a numbered linguex example
            // so it carries an example number a reader can cite.
            out.push_str("\\ex. ");
            out.push_str(&latex_escape(&e.example));
            out.push_str("\n\n");
        } else {
            out.push('\n');
        }
    }
    out.push_str("\\end{document}\n");
    out
}

/// Render the phoneme inventory as a printable IPA chart in Markdown. The
/// data model carries IPA + romanization + a coarse vowel/consonant kind (a
/// full place×manner grid needs the articulatory features that land later), so
/// the chart groups consonants and vowels and lists each sound with its
/// romanization — the inventory snapshot a grammar appendix or a reader needs.
pub fn ipa_chart(language: &str, phon: &Phonology) -> String {
    let mut consonants: Vec<&Phoneme> = Vec::new();
    let mut vowels: Vec<&Phoneme> = Vec::new();
    for p in &phon.phonemes {
        match p.kind {
            PhonemeKind::Consonant => consonants.push(p),
            PhonemeKind::Vowel => vowels.push(p),
        }
    }
    // Order by sonority then IPA so the chart is stable and reads roughly
    // obstruent → sonorant for consonants, and consistently for vowels.
    let sort = |v: &mut Vec<&Phoneme>| {
        v.sort_by(|a, b| {
            crate::conlang::phonology::ipa::sonority_of(phon, &a.ipa)
                .cmp(&crate::conlang::phonology::ipa::sonority_of(phon, &b.ipa))
                .then(a.ipa.cmp(&b.ipa))
        });
    };
    sort(&mut consonants);
    sort(&mut vowels);

    let section = |title: &str, set: &[&Phoneme]| -> String {
        let mut s = format!("## {title} ({})\n\n", set.len());
        if set.is_empty() {
            s.push_str("_none declared_\n\n");
            return s;
        }
        s.push_str("| IPA | Romanization |\n|-----|--------------|\n");
        for p in set {
            let rom = p.romanize.as_deref().unwrap_or("—");
            s.push_str(&format!("| {} | {} |\n", p.ipa, rom));
        }
        s.push('\n');
        s
    };

    let mut out = format!("# {language} — IPA inventory\n\n");
    out.push_str(&format!(
        "{} phonemes — {} consonants, {} vowels.\n\n",
        phon.phonemes.len(),
        consonants.len(),
        vowels.len()
    ));
    out.push_str(&section("Consonants", &consonants));
    out.push_str(&section("Vowels", &vowels));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn entry(word: &str, pos: &str, tr: &str, ex: &str) -> (String, DictionaryEntry) {
        (
            tr.to_string(),
            DictionaryEntry {
                word: word.into(),
                pos: pos.into(),
                translation: tr.into(),
                example: ex.into(),
                inflection: BTreeMap::new(),
                ..Default::default()
            },
        )
    }

    #[test]
    fn xliff_is_well_formed_and_escapes() {
        let entries = vec![entry("kira", "noun", "bird & friend", "")];
        let out = xliff("Eldar", "english", &entries);
        assert!(out.contains("source-language=\"en\""));
        assert!(out.contains("target-language=\"art-x-eldar\""));
        assert!(out.contains("<source>bird &amp; friend</source>"));
        assert!(out.contains("<target>kira</target>"));
        assert!(out.contains("<note>noun</note>"));
        // one trans-unit per entry
        assert_eq!(out.matches("<trans-unit").count(), 1);
    }

    #[test]
    fn xliff_uses_working_language_code() {
        let entries = vec![entry("mira", "adj", "bright", "")];
        let out = xliff("Eldar", "russian", &entries);
        assert!(out.contains("source-language=\"ru\""));
    }

    #[test]
    fn linguex_emits_document_and_examples() {
        let entries = vec![
            entry("kira", "noun", "bird", "kira nami"),
            entry("pata", "noun", "stone", ""),
        ];
        let out = linguex("Eldar", &entries);
        assert!(out.contains("\\documentclass"));
        assert!(out.contains("\\usepackage{linguex}"));
        assert!(out.contains("\\textbf{kira}"));
        assert!(out.contains("\\textit{noun}"));
        assert!(out.contains("`bird'"));
        // only the entry with an example emits an \ex.
        assert_eq!(out.matches("\\ex.").count(), 1);
        assert!(out.contains("\\end{document}"));
    }

    #[test]
    fn linguex_escapes_latex_specials() {
        let entries = vec![entry("ka_n", "noun", "100% sure", "")];
        let out = linguex("Test", &entries);
        assert!(out.contains("ka\\_n"));
        assert!(out.contains("100\\% sure"));
    }

    #[test]
    fn ipa_chart_groups_consonants_and_vowels() {
        let phon = Phonology {
            phonemes: vec![
                Phoneme {
                    ipa: "k".into(),
                    romanize: None,
                    kind: PhonemeKind::Consonant,
                    sonority: None,
                },
                Phoneme {
                    ipa: "a".into(),
                    romanize: Some("ah".into()),
                    kind: PhonemeKind::Vowel,
                    sonority: None,
                },
            ],
            ..Default::default()
        };
        let out = ipa_chart("Eldar", &phon);
        assert!(out.contains("## Consonants (1)"));
        assert!(out.contains("## Vowels (1)"));
        assert!(out.contains("| k | — |"));
        assert!(out.contains("| a | ah |"));
        assert!(out.contains("2 phonemes — 1 consonants, 1 vowels."));
    }
}
