//! Book output — dictionary & grammar rendering (LANG-1 P6.2 / P6.3).
//!
//! Render a language's data into a real document, in Markdown or Typst. The
//! Typst paths are the showpiece: a paginated, two-column dictionary that embeds
//! the generated conscript font and shows each headword in the native script
//! (transliterated by the P5.6c input method) beside its romanization (P6.2),
//! and a paginated reference grammar with an outline + numbered sections drawn
//! from the language's phonology / morphology / typology / expressions / sample
//! texts (P6.3). Pure + deterministic — the CLI prepares the inputs
//! (`RenderEntry`s, a `GrammarBook`) and these functions lay them out.

use std::collections::BTreeMap;

use crate::conlang::analysis::LanguageProfile;
use crate::conlang::types::constraint::PhonotacticConstraint;
use crate::conlang::types::expression::Expressions;
use crate::conlang::types::morphology::Morphology;
use crate::conlang::types::stress::{StressPlacement, StressRule};
use crate::conlang::types::template::SyllableTemplate;
use crate::conlang::Phonology;

/// One dictionary entry, fully prepared for rendering.
#[derive(Debug, Clone, Default)]
pub struct RenderEntry {
    pub headword: String,
    /// Native-script form (a string of glyph codepoints), when a font exists.
    pub conscript: Option<String>,
    /// Syllabified surface pronunciation, e.g. `ka.ta`.
    pub pronunciation: Option<String>,
    pub pos: String,
    pub gloss: String,
    pub registers: Vec<String>,
    pub domain: Vec<String>,
    pub era: Option<String>,
    pub etymology: Option<String>,
    pub example: Option<String>,
}

pub struct DictMeta<'a> {
    pub language: &'a str,
    /// Font family to render the conscript in (Typst only).
    pub font_family: Option<&'a str>,
    pub profile: Option<&'a LanguageProfile>,
}

/// The uppercase first character of a headword, used for sectioning.
fn section_key(word: &str) -> String {
    word.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()
}

/// Entries sorted case-insensitively by headword.
fn sorted(entries: &[RenderEntry]) -> Vec<&RenderEntry> {
    let mut v: Vec<&RenderEntry> = entries.iter().collect();
    v.sort_by(|a, b| a.headword.to_lowercase().cmp(&b.headword.to_lowercase()));
    v
}

fn tags(e: &RenderEntry) -> String {
    let mut t: Vec<String> = Vec::new();
    t.extend(e.registers.iter().cloned());
    t.extend(e.domain.iter().cloned());
    if let Some(era) = &e.era {
        t.push(era.clone());
    }
    t.join("; ")
}

/// Render a Markdown dictionary.
pub fn dictionary_markdown(meta: &DictMeta, entries: &[RenderEntry]) -> String {
    let mut s = String::new();
    s.push_str(&format!("# {} — Dictionary\n\n", meta.language));
    s.push_str(&format!("*{} entries*\n\n", entries.len()));

    if let Some(p) = meta.profile {
        s.push_str("## Overview\n\n");
        s.push_str(&format!(
            "- Inventory: {} phonemes ({} consonants / {} vowels)\n",
            p.phoneme_inventory, p.consonants, p.vowels
        ));
        if p.analyzable_words > 0 {
            s.push_str(&format!(
                "- Word shape: {:.1} phonemes, {:.1} syllables on average\n",
                p.avg_phonemes, p.avg_syllables
            ));
        }
        s.push('\n');
    }

    let mut current = String::new();
    for e in sorted(entries) {
        let key = section_key(&e.headword);
        if key != current {
            s.push_str(&format!("## {key}\n\n"));
            current = key;
        }
        let pron = e.pronunciation.as_deref().map(|p| format!(" /{p}/")).unwrap_or_default();
        let pos = if e.pos.is_empty() { String::new() } else { format!(" · *{}*", e.pos) };
        s.push_str(&format!("**{}**{pron}{pos}  \n", e.headword));
        let tagstr = tags(e);
        let tagsuffix = if tagstr.is_empty() { String::new() } else { format!(" — {tagstr}") };
        s.push_str(&format!("{}{tagsuffix}  \n", e.gloss));
        if let Some(et) = &e.etymology {
            s.push_str(&format!("*Etymology:* {et}  \n"));
        }
        if let Some(ex) = &e.example {
            s.push_str(&format!("*Example:* {ex}  \n"));
        }
        s.push('\n');
    }
    s
}

/// A Typst string literal of `s` as Unicode escapes (safe for any codepoint).
fn typst_escapes(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        out.push_str(&format!("\\u{{{:X}}}", c as u32));
    }
    out.push('"');
    out
}

/// Escape Typst markup-special characters in plain text.
fn typst_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if matches!(c, '#' | '*' | '_' | '`' | '$' | '\\' | '<' | '>' | '@' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Render a paginated, two-column Typst dictionary. When `font_family` is set,
/// each headword is also shown in the native script.
pub fn dictionary_typst(meta: &DictMeta, entries: &[RenderEntry]) -> String {
    let lang = typst_text(meta.language);
    let mut s = String::new();
    s.push_str(&format!("#set document(title: \"{lang} — Dictionary\")\n"));
    s.push_str("#set page(paper: \"a5\", margin: 1.6cm, numbering: \"1\")\n");
    s.push_str("#set text(size: 10pt)\n");
    s.push_str("#set par(justify: true)\n");
    if let Some(f) = meta.font_family {
        s.push_str(&format!(
            "#let conscript(cp) = text(font: \"{}\", size: 1.4em)[#cp]\n",
            typst_text(f)
        ));
    }
    s.push('\n');

    // Title block.
    s.push_str("#align(center)[\n");
    s.push_str(&format!("  #text(size: 26pt, weight: \"bold\")[{lang}] \\\n"));
    s.push_str("  #text(size: 14pt, fill: gray)[Dictionary]\n");
    s.push_str("]\n#v(1cm)\n\n");

    // Overview table.
    if let Some(p) = meta.profile {
        s.push_str("#heading(level: 1, numbering: none)[Overview]\n");
        s.push_str("#table(columns: 2, stroke: none,\n");
        s.push_str(&format!(
            "  [Phonemes], [{} ({} C / {} V)],\n",
            p.phoneme_inventory, p.consonants, p.vowels
        ));
        s.push_str(&format!("  [Entries], [{}],\n", entries.len()));
        if p.analyzable_words > 0 {
            s.push_str(&format!(
                "  [Word shape], [{:.1} phonemes, {:.1} syllables avg],\n",
                p.avg_phonemes, p.avg_syllables
            ));
        }
        s.push_str(")\n#v(0.5cm)\n\n");
    }

    s.push_str("#columns(2)[\n");
    let mut current = String::new();
    for e in sorted(entries) {
        let key = section_key(&e.headword);
        if key != current {
            s.push_str(&format!(
                "#heading(level: 2, numbering: none)[{}]\n",
                typst_text(&key)
            ));
            current = key;
        }
        // Headword (bold) + native script + pronunciation + POS.
        s.push_str(&format!("/ *{}*", typst_text(&e.headword)));
        if let (Some(cp), Some(_)) = (&e.conscript, meta.font_family) {
            if !cp.is_empty() {
                s.push_str(&format!(" #conscript({})", typst_escapes(cp)));
            }
        }
        if let Some(pron) = &e.pronunciation {
            s.push_str(&format!(" #text(fill: gray)[/{}/]", typst_text(pron)));
        }
        if !e.pos.is_empty() {
            s.push_str(&format!(" #emph[{}]", typst_text(&e.pos)));
        }
        // Definition body.
        s.push_str(&format!(": {}", typst_text(&e.gloss)));
        let tagstr = tags(e);
        if !tagstr.is_empty() {
            s.push_str(&format!(" #text(size: 0.85em, fill: gray)[({})]", typst_text(&tagstr)));
        }
        if let Some(et) = &e.etymology {
            s.push_str(&format!(" #text(size: 0.85em)[← {}]", typst_text(et)));
        }
        s.push('\n');
    }
    s.push_str("]\n");
    s
}

// ─────────────────────────── grammar book ───────────────────────────

/// Everything the grammar book draws on. Sections render only when present.
pub struct GrammarBook<'a> {
    pub language: &'a str,
    pub font_family: Option<&'a str>,
    pub profile: &'a LanguageProfile,
    pub phonology: &'a Phonology,
    pub morphology: Option<&'a Morphology>,
    /// Typology answers (feature id → value).
    pub typology: &'a BTreeMap<String, String>,
    pub expressions: Option<&'a Expressions>,
    /// Sample texts: `(title, body)`.
    pub samples: &'a [(String, String)],
}

/// A syllable template's pattern, e.g. `CV(C)`.
fn render_template(t: &SyllableTemplate) -> String {
    t.pattern
        .iter()
        .map(|a| {
            if a.is_optional() {
                format!("({})", a.class_name())
            } else {
                a.class_name().to_string()
            }
        })
        .collect()
}

fn describe_constraint(c: &PhonotacticConstraint) -> String {
    match c {
        PhonotacticConstraint::MaxClusterSize(n) => format!("clusters at most {n} segment(s) long"),
        PhonotacticConstraint::NoGeminate => "no geminate (doubled) consonants".into(),
        PhonotacticConstraint::ForbidBigram(a, b) => format!("the sequence /{a}{b}/ is forbidden"),
        PhonotacticConstraint::ForbidInOnset(cs) => format!("forbidden in onsets: {}", cs.join(", ")),
        PhonotacticConstraint::ForbidInCoda(cs) => format!("forbidden in codas: {}", cs.join(", ")),
        PhonotacticConstraint::SonoritySequencing => {
            "syllables obey the sonority-sequencing principle".into()
        }
    }
}

fn describe_stress(s: &StressRule) -> &'static str {
    match s.primary {
        StressPlacement::Initial => "initial — the first syllable",
        StressPlacement::Final => "final — the last syllable",
        StressPlacement::Penultimate => "penultimate — the second-to-last syllable",
        StressPlacement::Antepenultimate => "antepenultimate — the third-to-last syllable",
        StressPlacement::LatinRule => "weight-sensitive (the Latin rule)",
    }
}

/// Distinct syllable patterns across all template sets, in first-seen order.
fn syllable_patterns(phon: &Phonology) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for set in phon.templates.values() {
        for t in set {
            let p = render_template(t);
            if !p.is_empty() && seen.insert(p.clone()) {
                out.push(p);
            }
        }
    }
    out
}

/// Phonemes of a given kind, romanized/IPA, space-joined.
fn inventory(phon: &Phonology, kind: crate::conlang::types::phoneme::PhonemeKind) -> Vec<String> {
    phon.phonemes
        .iter()
        .filter(|p| p.kind == kind)
        .map(|p| p.ipa.clone())
        .collect()
}

/// Typology answer lines: `Word order: SOV — <consequence>`.
fn typology_lines(typology: &BTreeMap<String, String>) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for (id, value) in typology {
        let (label, consequence) = match crate::conlang::grammar::feature(id) {
            Some(f) => {
                let cons = f
                    .options
                    .iter()
                    .find(|(v, _)| v.eq_ignore_ascii_case(value))
                    .map(|(_, c)| c.to_string())
                    .unwrap_or_default();
                (f.id.replace('_', " "), cons)
            }
            None => (id.replace('_', " "), String::new()),
        };
        out.push((label, value.clone(), consequence));
    }
    out
}

/// Render the grammar as a Markdown reference.
pub fn grammar_markdown(book: &GrammarBook) -> String {
    use crate::conlang::types::phoneme::PhonemeKind;
    let mut s = String::new();
    s.push_str(&format!("# {} — A Grammar\n\n", book.language));
    let p = book.profile;
    s.push_str(&format!(
        "*{} phonemes ({} consonants / {} vowels) · {} lexicon entries*\n\n",
        p.phoneme_inventory, p.consonants, p.vowels, p.word_count
    ));

    s.push_str("## Phonology\n\n");
    let cons = inventory(book.phonology, PhonemeKind::Consonant);
    let vowels = inventory(book.phonology, PhonemeKind::Vowel);
    if !cons.is_empty() {
        s.push_str(&format!("**Consonants** ({}): {}\n\n", cons.len(), cons.join(" · ")));
    }
    if !vowels.is_empty() {
        s.push_str(&format!("**Vowels** ({}): {}\n\n", vowels.len(), vowels.join(" · ")));
    }
    let pats = syllable_patterns(book.phonology);
    if !pats.is_empty() {
        s.push_str(&format!("**Syllable structure:** {}\n\n", pats.join(", ")));
    }
    if !book.phonology.constraints.is_empty() {
        s.push_str("**Phonotactics:**\n\n");
        for c in &book.phonology.constraints {
            s.push_str(&format!("- {}\n", describe_constraint(c)));
        }
        s.push('\n');
    }
    if !book.phonology.allophony.is_empty() {
        s.push_str("**Allophony:**\n\n");
        for r in &book.phonology.allophony {
            s.push_str(&format!("- `{}`\n", r.source));
        }
        s.push('\n');
    }
    if let Some(st) = &book.phonology.stress {
        s.push_str(&format!("**Stress:** {}\n\n", describe_stress(st)));
    }
    if let Some(tone) = &book.phonology.tone {
        s.push_str(&format!("**Tone:** {} tone(s)\n\n", tone.tones.len()));
    }

    if let Some(m) = book.morphology {
        if !m.morphemes.is_empty() || !m.derivations.is_empty() {
            s.push_str("## Morphology\n\n");
            if !m.morphemes.is_empty() {
                s.push_str("**Affixes:**\n\n");
                for mo in &m.morphemes {
                    s.push_str(&format!(
                        "- **{}** `{}` — {} ({:?})\n",
                        mo.gloss, mo.form, mo.id, mo.position
                    ));
                }
                s.push('\n');
            }
            if !m.derivations.is_empty() {
                s.push_str("**Derivation:**\n\n");
                for d in &m.derivations {
                    let from = d.from_pos.as_deref().unwrap_or("any");
                    s.push_str(&format!("- **{}**: {} → {} via `{}`\n", d.name, from, d.to_pos, d.form));
                }
                s.push('\n');
            }
        }
    }

    let tl = typology_lines(book.typology);
    if !tl.is_empty() {
        s.push_str("## Grammar\n\n");
        for (label, value, cons) in &tl {
            let tail = if cons.is_empty() { String::new() } else { format!(" — {cons}") };
            s.push_str(&format!("- **{label}:** {value}{tail}\n"));
        }
        s.push('\n');
    }

    if let Some(ex) = book.expressions {
        if !ex.idioms.is_empty() || !ex.metaphors.is_empty() {
            s.push_str("## Expressions\n\n");
            for i in &ex.idioms {
                s.push_str(&format!("- *{}* — {} (lit. {})\n", i.form, i.meaning, i.literal));
            }
            for m in &ex.metaphors {
                s.push_str(&format!("- {} **is** {}\n", m.source, m.target));
            }
            s.push('\n');
        }
    }

    if !book.samples.is_empty() {
        s.push_str("## Sample texts\n\n");
        for (title, body) in book.samples {
            s.push_str(&format!("### {title}\n\n{}\n\n", body.trim()));
        }
    }
    s
}

/// Render the grammar as a Typst document (embeds the conscript font when set).
pub fn grammar_typst(book: &GrammarBook) -> String {
    use crate::conlang::types::phoneme::PhonemeKind;
    let lang = typst_text(book.language);
    let mut s = String::new();
    s.push_str(&format!("#set document(title: \"{lang} — A Grammar\")\n"));
    s.push_str("#set page(paper: \"a5\", margin: 1.6cm, numbering: \"1\")\n");
    s.push_str("#set text(size: 10pt)\n");
    s.push_str("#set par(justify: true)\n");
    s.push_str("#set heading(numbering: \"1.1\")\n");
    if let Some(f) = book.font_family {
        s.push_str(&format!(
            "#let native(cp) = text(font: \"{}\", size: 1.3em)[#cp]\n",
            typst_text(f)
        ));
    }
    s.push('\n');
    s.push_str("#align(center)[\n");
    s.push_str(&format!("  #text(size: 26pt, weight: \"bold\")[{lang}] \\\n"));
    s.push_str("  #text(size: 14pt, fill: gray)[A Grammar]\n");
    s.push_str("]\n#v(0.8cm)\n#outline()\n#pagebreak()\n\n");

    let para = |s: &mut String, label: &str, body: &str| {
        s.push_str(&format!("*{label}.* {body}\n\n", label = typst_text(label)));
    };

    s.push_str("= Phonology\n");
    let cons = inventory(book.phonology, PhonemeKind::Consonant);
    let vowels = inventory(book.phonology, PhonemeKind::Vowel);
    if !cons.is_empty() {
        para(&mut s, "Consonants", &typst_text(&cons.join(" · ")));
    }
    if !vowels.is_empty() {
        para(&mut s, "Vowels", &typst_text(&vowels.join(" · ")));
    }
    let pats = syllable_patterns(book.phonology);
    if !pats.is_empty() {
        para(&mut s, "Syllable structure", &typst_text(&pats.join(", ")));
    }
    if !book.phonology.constraints.is_empty() {
        s.push_str("*Phonotactics.*\n");
        for c in &book.phonology.constraints {
            s.push_str(&format!("- {}\n", typst_text(&describe_constraint(c))));
        }
        s.push('\n');
    }
    if !book.phonology.allophony.is_empty() {
        s.push_str("*Allophony.*\n");
        for r in &book.phonology.allophony {
            s.push_str(&format!("- `{}`\n", r.source));
        }
        s.push('\n');
    }
    if let Some(st) = &book.phonology.stress {
        para(&mut s, "Stress", describe_stress(st));
    }
    if let Some(tone) = &book.phonology.tone {
        para(&mut s, "Tone", &format!("{} tone(s)", tone.tones.len()));
    }

    if let Some(m) = book.morphology {
        if !m.morphemes.is_empty() || !m.derivations.is_empty() {
            s.push_str("= Morphology\n");
            if !m.morphemes.is_empty() {
                s.push_str("*Affixes.*\n");
                for mo in &m.morphemes {
                    s.push_str(&format!(
                        "/ *{}*: `{}` #emph[{}]\n",
                        typst_text(&mo.gloss),
                        typst_text(&mo.form),
                        typst_text(&format!("{:?}", mo.position))
                    ));
                }
                s.push('\n');
            }
            if !m.derivations.is_empty() {
                s.push_str("*Derivation.*\n");
                for d in &m.derivations {
                    let from = d.from_pos.as_deref().unwrap_or("any");
                    s.push_str(&format!(
                        "- *{}*: {} → {} via `{}`\n",
                        typst_text(&d.name),
                        typst_text(from),
                        typst_text(&d.to_pos),
                        typst_text(&d.form)
                    ));
                }
                s.push('\n');
            }
        }
    }

    let tl = typology_lines(book.typology);
    if !tl.is_empty() {
        s.push_str("= Grammar\n#table(columns: 2, stroke: none,\n");
        for (label, value, cons) in &tl {
            let v = if cons.is_empty() {
                typst_text(value)
            } else {
                format!("{} #text(fill: gray)[— {}]", typst_text(value), typst_text(cons))
            };
            s.push_str(&format!("  [{}], [{v}],\n", typst_text(label)));
        }
        s.push_str(")\n\n");
    }

    if let Some(ex) = book.expressions {
        if !ex.idioms.is_empty() || !ex.metaphors.is_empty() {
            s.push_str("= Expressions\n");
            for i in &ex.idioms {
                s.push_str(&format!(
                    "/ #emph[{}]: {} #text(fill: gray)[(lit. {})]\n",
                    typst_text(&i.form),
                    typst_text(&i.meaning),
                    typst_text(&i.literal)
                ));
            }
            for m in &ex.metaphors {
                s.push_str(&format!("- {} *is* {}\n", typst_text(&m.source), typst_text(&m.target)));
            }
            s.push('\n');
        }
    }

    if !book.samples.is_empty() {
        s.push_str("= Sample texts\n");
        for (title, body) in book.samples {
            s.push_str(&format!("== {}\n{}\n\n", typst_text(title), typst_text(body.trim())));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<RenderEntry> {
        vec![
            RenderEntry {
                headword: "kata".into(),
                conscript: Some("\u{E000}\u{E001}".into()),
                pronunciation: Some("ka.ta".into()),
                pos: "noun".into(),
                gloss: "stone".into(),
                registers: vec!["formal".into()],
                etymology: Some("proto *kapa".into()),
                ..Default::default()
            },
            RenderEntry {
                headword: "ami".into(),
                pronunciation: Some("a.mi".into()),
                pos: "verb".into(),
                gloss: "to see".into(),
                ..Default::default()
            },
        ]
    }

    #[test]
    fn markdown_sorts_and_sections() {
        let meta = DictMeta { language: "Avesha", font_family: None, profile: None };
        let md = dictionary_markdown(&meta, &entries());
        // `ami` sorts before `kata`; sections A then K.
        let a = md.find("## A").unwrap();
        let k = md.find("## K").unwrap();
        assert!(a < k);
        assert!(md.contains("**ami**"));
        assert!(md.contains("/ka.ta/"));
        assert!(md.contains("*Etymology:* proto *kapa"));
        assert!(md.contains("stone — formal"));
    }

    #[test]
    fn typst_embeds_font_and_conscript() {
        let meta = DictMeta { language: "Avesha", font_family: Some("Eldar"), profile: None };
        let typ = dictionary_typst(&meta, &entries());
        assert!(typ.contains("#set document(title: \"Avesha — Dictionary\")"));
        assert!(typ.contains("text(font: \"Eldar\""));
        // kata's conscript codepoints as Typst escapes.
        assert!(typ.contains("\\u{E000}\\u{E001}"));
        assert!(typ.contains("#columns(2)"));
        assert!(typ.contains("/ *kata*"));
    }

    #[test]
    fn typst_without_font_omits_conscript() {
        let meta = DictMeta { language: "Avesha", font_family: None, profile: None };
        let typ = dictionary_typst(&meta, &entries());
        assert!(!typ.contains("conscript("));
        assert!(!typ.contains("\\u{E000}"));
    }

    fn grammar_phon() -> Phonology {
        use crate::conlang::types::constraint::PhonotacticConstraint;
        use crate::conlang::types::phoneme::{Phoneme, PhonemeKind};
        let mk = |ipa: &str, kind| Phoneme {
            ipa: ipa.to_string(),
            romanize: None,
            kind,
            sonority: None,
        };
        Phonology {
            phonemes: vec![
                mk("k", PhonemeKind::Consonant),
                mk("t", PhonemeKind::Consonant),
                mk("a", PhonemeKind::Vowel),
            ],
            constraints: vec![PhonotacticConstraint::NoGeminate],
            ..Default::default()
        }
    }

    #[test]
    fn grammar_book_renders_sections() {
        let profile = LanguageProfile {
            phoneme_inventory: 3,
            consonants: 2,
            vowels: 1,
            word_count: 4,
            ..Default::default()
        };
        let phon = grammar_phon();
        let mut typology = std::collections::BTreeMap::new();
        typology.insert("word_order".to_string(), "sov".to_string());
        let samples = vec![("Greeting".to_string(), "kata ami".to_string())];
        let book = GrammarBook {
            language: "Avesha",
            font_family: None,
            profile: &profile,
            phonology: &phon,
            morphology: None,
            typology: &typology,
            expressions: None,
            samples: &samples,
        };

        let md = grammar_markdown(&book);
        assert!(md.contains("# Avesha — A Grammar"));
        assert!(md.contains("**Consonants** (2): k · t"));
        assert!(md.contains("no geminate"));
        assert!(md.contains("## Grammar"));
        assert!(md.contains("**word order:** sov"));
        assert!(md.contains("### Greeting"));

        let typ = grammar_typst(&book);
        assert!(typ.contains("#set document(title: \"Avesha — A Grammar\")"));
        assert!(typ.contains("#outline()"));
        assert!(typ.contains("= Phonology"));
        assert!(typ.contains("#table(columns: 2"));
        assert!(typ.contains("== Greeting"));
    }
}
