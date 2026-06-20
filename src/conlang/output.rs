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
    // Cover line: the first headword in the native script, if a font exists.
    let cover = entries
        .iter()
        .find_map(|e| e.conscript.as_ref().filter(|c| !c.is_empty()).cloned());
    let mut s = book_scaffold(
        &format!("{} — Dictionary", meta.language),
        &format!("{} Dictionary", meta.language),
        "A lexicon",
        meta.font_family,
        cover.as_deref(),
    );
    // The dictionary's conscript helper renders a touch larger than the body
    // native() default.
    if let Some(f) = meta.font_family {
        s.push_str(&format!(
            "#let conscript(cp) = text(font: \"{}\", size: 1.5em)[#cp]\n\n",
            typst_text(f)
        ));
    }

    if let Some(p) = meta.profile {
        s.push_str("= Overview\n");
        s.push_str("#table(columns: 2, stroke: none, inset: (x: 0pt, y: 3pt),\n");
        s.push_str(&format!(
            "  [Phonemes], [{} ({} consonants / {} vowels)],\n",
            p.phoneme_inventory, p.consonants, p.vowels
        ));
        s.push_str(&format!("  [Entries], [{}],\n", entries.len()));
        if p.analyzable_words > 0 {
            s.push_str(&format!(
                "  [Average word], [{:.1} phonemes, {:.1} syllables],\n",
                p.avg_phonemes, p.avg_syllables
            ));
        }
        s.push_str(")\n#pagebreak()\n\n");
    }

    s.push_str("= The Lexicon\n");
    s.push_str("#columns(2, gutter: 1.2em)[\n");
    let mut current = String::new();
    for e in sorted(entries) {
        let key = section_key(&e.headword);
        if key != current {
            s.push_str(&format!("== {}\n", typst_text(&key)));
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
            s.push_str(&format!(" #text(fill: luma(110))[/{}/]", typst_text(pron)));
        }
        if !e.pos.is_empty() {
            s.push_str(&format!(" #text(style: \"italic\", fill: luma(110))[{}]", typst_text(&e.pos)));
        }
        // Definition body.
        s.push_str(&format!(": {}", typst_text(&e.gloss)));
        let tagstr = tags(e);
        if !tagstr.is_empty() {
            s.push_str(&format!(" #text(size: 0.85em, fill: luma(120))[({})]", typst_text(&tagstr)));
        }
        if let Some(et) = &e.etymology {
            s.push_str(&format!(" #text(size: 0.85em, fill: luma(120))[← {}]", typst_text(et)));
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
    /// Optional AI-authored study guide, ready to drop in — raw Markdown for the
    /// Markdown renderer, converted Typst for the Typst renderer. When present,
    /// it leads the book (as a study companion) ahead of the reference sections.
    pub study: Option<&'a str>,
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

    // AI-authored study guide leads, when present.
    if let Some(study) = book.study {
        s.push_str("## Study Guide\n\n");
        s.push_str(study.trim());
        s.push_str("\n\n---\n\n");
    }

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
    let cover = book
        .samples
        .first()
        .and_then(|_| None::<String>); // grammar has no per-word conscript handy
    let mut s = book_scaffold(
        &format!("{} — A Grammar", book.language),
        &format!("A Grammar of {}", book.language),
        "Phonology · Morphology · Syntax",
        book.font_family,
        cover.as_deref(),
    );

    // AI-authored study guide leads, when present.
    if let Some(study) = book.study {
        s.push_str("= Study Guide\n");
        s.push_str(study);
        s.push_str("\n#pagebreak()\n\n");
    }

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

// ─────────────────────────── tutorial ───────────────────────────

/// The shared manual-style book chrome for every ConLang document: document /
/// page / type setup (B5, serif body, weighted+ruled headings), the `#native`
/// (conscript) and `#practice` / `#term` callout helpers, a title page with a
/// `title` + `subtitle` and an optional native-script cover line, and a table
/// of contents. The body — deterministic or AI-authored — is appended after.
pub fn book_scaffold(
    doc_title: &str,
    title: &str,
    subtitle: &str,
    font_family: Option<&str>,
    cover: Option<&str>,
) -> String {
    let mut s = String::new();
    s.push_str(&format!("#set document(title: \"{}\")\n", typst_text(doc_title)));
    s.push_str("#set page(paper: \"iso-b5\", margin: (x: 2.2cm, y: 2.4cm), numbering: \"1\")\n");
    // Fonts: rely on families Typst bundles ("Libertinus Serif", "New Computer
    // Modern") so the book compiles warning-free anywhere; headings get contrast
    // from weight + size + a rule rather than a separate (unbundled) sans face.
    s.push_str("#set text(size: 11pt, font: (\"Libertinus Serif\", \"New Computer Modern\"))\n");
    s.push_str("#set par(justify: true, leading: 0.7em, first-line-indent: 1em)\n");
    s.push_str("#set heading(numbering: none)\n");
    s.push_str("#show heading.where(level: 1): it => block(below: 1em)[\n");
    s.push_str("  #set text(size: 18pt, weight: \"bold\")\n");
    s.push_str("  #it.body #v(-0.3em) #line(length: 100%, stroke: 0.5pt + luma(180))\n]\n");
    s.push_str("#show heading.where(level: 2): set text(size: 12pt, weight: \"bold\")\n");
    s.push_str("#let practice(body) = block(width: 100%, fill: luma(244), stroke: (left: 2pt + rgb(\"#7a4a2f\")), inset: 8pt, radius: 2pt)[\n");
    s.push_str("  #text(size: 8pt, weight: \"bold\", fill: rgb(\"#7a4a2f\"), tracking: 1pt)[PRACTICE] #parbreak() #body\n]\n");
    // A definitional callout for explaining a linguistic term.
    s.push_str("#let term(name, body) = block(width: 100%, fill: rgb(\"#f2f6f9\"), stroke: (left: 2pt + rgb(\"#2f5d7a\")), inset: 8pt, radius: 2pt)[\n");
    s.push_str("  #text(weight: \"bold\", fill: rgb(\"#2f5d7a\"))[#name] #parbreak() #body\n]\n");
    match font_family {
        Some(f) => s.push_str(&format!(
            "#let native(cp) = text(font: \"{}\", size: 1.3em)[#cp]\n",
            typst_text(f)
        )),
        // Degrade gracefully if no font: render the escapes as plain text.
        None => s.push_str("#let native(cp) = text(size: 1.3em)[#cp]\n"),
    }
    s.push('\n');
    s.push_str("#align(center + horizon)[\n");
    s.push_str(&format!("  #text(size: 32pt, weight: \"bold\")[{}] \\\n", typst_text(title)));
    if !subtitle.is_empty() {
        s.push_str(&format!(
            "  #v(4mm) #text(size: 13pt, style: \"italic\", fill: luma(90))[{}] \\\n",
            typst_text(subtitle)
        ));
    }
    if let (Some(cp), Some(_)) = (cover, font_family) {
        if !cp.is_empty() {
            s.push_str(&format!("  #v(12mm) #native({})\n", typst_escapes(cp)));
        }
    }
    s.push_str("]\n#pagebreak()\n\n");
    s.push_str("#outline(title: \"Contents\", depth: 2)\n#pagebreak()\n\n");
    s
}

/// The tutorial's book scaffold (a thin wrapper over [`book_scaffold`]).
pub fn tutorial_typst_scaffold(language: &str, font_family: Option<&str>, cover: Option<&str>) -> String {
    book_scaffold(
        &format!("Learn {language}"),
        &format!("Learn {language}"),
        "A first course",
        font_family,
        cover,
    )
}


/// Convert a constrained subset of Markdown (what the tutorial AI emits) into
/// Typst markup, to be dropped into [`tutorial_typst_scaffold`]. Handles
/// headings, bold/italic, lists, numbered lists, blockquotes (→ a `#practice`
/// box), tables, horizontal rules, fenced + inline code, and links — escaping
/// Typst-special characters in prose. Deterministic.
pub fn markdown_to_typst(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut dropped_title = false;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_end();

        // Fenced code block — pass straight through (Typst supports ``` raw).
        if trimmed.trim_start().starts_with("```") {
            out.push_str(trimmed);
            out.push('\n');
            i += 1;
            while i < lines.len() {
                out.push_str(lines[i]);
                out.push('\n');
                let done = lines[i].trim_start().starts_with("```");
                i += 1;
                if done {
                    break;
                }
            }
            continue;
        }

        // Table: a `|`-row followed by a `|---|` separator.
        if trimmed.starts_with('|')
            && i + 1 < lines.len()
            && is_table_separator(lines[i + 1])
        {
            let header = split_row(trimmed);
            let ncols = header.len().max(1);
            out.push_str(&format!("#table(columns: {ncols},\n  table.header("));
            out.push_str(&header.iter().map(|c| format!("[{}]", md_inline(c))).collect::<Vec<_>>().join(", "));
            out.push_str("),\n");
            i += 2; // header + separator
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                let cells = split_row(lines[i].trim_end());
                out.push_str("  ");
                for c in &cells {
                    out.push_str(&format!("[{}], ", md_inline(c)));
                }
                out.push('\n');
                i += 1;
            }
            out.push_str(")\n\n");
            continue;
        }

        // Blockquote → a practice/aside box.
        if trimmed.trim_start().starts_with('>') {
            let mut body = String::new();
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                let content = lines[i].trim_start().trim_start_matches('>').trim();
                body.push_str(&md_inline(content));
                body.push(' ');
                i += 1;
            }
            out.push_str(&format!("#practice[{}]\n\n", body.trim()));
            continue;
        }

        // Horizontal rule.
        if matches!(trimmed.trim(), "---" | "***" | "___") {
            out.push_str("#line(length: 100%, stroke: 0.5pt + luma(200))\n\n");
            i += 1;
            continue;
        }

        // Heading.
        if let Some(level) = heading_level(trimmed) {
            let text = trimmed[level..].trim();
            // Drop the very first H1 — the scaffold already has a title page.
            if level == 1 && !dropped_title {
                dropped_title = true;
                i += 1;
                continue;
            }
            let eq = "=".repeat(level.max(1));
            out.push_str(&format!("{eq} {}\n", md_inline(text)));
            i += 1;
            continue;
        }

        // Blank line.
        if trimmed.trim().is_empty() {
            out.push('\n');
            i += 1;
            continue;
        }

        // List item (bullet or numbered).
        let ls = trimmed.trim_start();
        if let Some(rest) = ls.strip_prefix("- ").or_else(|| ls.strip_prefix("* ")) {
            out.push_str(&format!("- {}\n", md_inline(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = strip_numbered(ls) {
            out.push_str(&format!("+ {}\n", md_inline(rest)));
            i += 1;
            continue;
        }

        // Ordinary paragraph line.
        out.push_str(&md_inline(trimmed));
        out.push('\n');
        i += 1;
    }
    out
}

fn heading_level(line: &str) -> Option<usize> {
    let t = line.trim_start();
    if !t.starts_with('#') {
        return None;
    }
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if hashes >= 1 && hashes <= 6 && t[hashes..].starts_with(' ') {
        Some(hashes)
    } else {
        None
    }
}

fn strip_numbered(s: &str) -> Option<&str> {
    let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && s[digits..].starts_with(". ") {
        Some(&s[digits + 2..])
    } else {
        None
    }
}

fn is_table_separator(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|')
        && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
        && t.contains('-')
}

fn split_row(line: &str) -> Vec<String> {
    let t = line.trim().trim_start_matches('|').trim_end_matches('|');
    t.split('|').map(|c| c.trim().to_string()).collect()
}

/// Inline Markdown → Typst, with prose escaping. Code spans pass through.
fn md_inline(s: &str) -> String {
    // 1. Strip links `[text](url)` → text.
    let s = strip_links(s);
    // 2. Protect inline code spans.
    let mut protected: Vec<String> = Vec::new();
    let mut work = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '`' {
            let mut code = String::from("`");
            for c2 in chars.by_ref() {
                code.push(c2);
                if c2 == '`' {
                    break;
                }
            }
            work.push('\u{0}');
            work.push_str(&((protected.len()).to_string()));
            work.push('\u{0}');
            protected.push(code);
        } else {
            work.push(c);
        }
    }
    // 3. Escape Typst-special chars in the prose.
    let mut esc = String::new();
    for c in work.chars() {
        match c {
            '\\' => esc.push_str("\\\\"),
            '#' => esc.push_str("\\#"),
            '@' => esc.push_str("\\@"),
            '$' => esc.push_str("\\$"),
            '<' => esc.push_str("\\<"),
            '>' => esc.push_str("\\>"),
            '[' => esc.push_str("\\["),
            ']' => esc.push_str("\\]"),
            _ => esc.push(c),
        }
    }
    // 4. Emphasis → Typst FUNCTION calls (`#strong[…]` / `#emph[…]`), matched
    //    pairs only. Functions work regardless of surrounding characters, unlike
    //    `*`/`_` markup which Typst only treats as emphasis at word boundaries —
    //    Markdown allows intra-word emphasis (`*pa*ta`), which as `_pa_ta` would
    //    be an unclosed Typst delimiter. Any leftover lone `*`/`_` is escaped.
    use std::sync::OnceLock;
    static BOLD: OnceLock<regex::Regex> = OnceLock::new();
    static ITAL_STAR: OnceLock<regex::Regex> = OnceLock::new();
    static ITAL_US: OnceLock<regex::Regex> = OnceLock::new();
    let bold = BOLD.get_or_init(|| regex::Regex::new(r"\*\*([^*]+)\*\*").unwrap());
    let ital_star = ITAL_STAR.get_or_init(|| regex::Regex::new(r"\*([^*]+)\*").unwrap());
    let ital_us = ITAL_US.get_or_init(|| regex::Regex::new(r"_([^_]+)_").unwrap());
    let s1 = bold.replace_all(&esc, "#strong[${1}]").into_owned();
    let s2 = ital_star.replace_all(&s1, "#emph[${1}]").into_owned();
    let s3 = ital_us.replace_all(&s2, "#emph[${1}]").into_owned();
    let s4 = s3.replace('*', "\\*").replace('_', "\\_");
    // 5. Restore code spans.
    let mut result = s4;
    for (idx, code) in protected.iter().enumerate() {
        result = result.replace(&format!("\u{0}{idx}\u{0}"), code);
    }
    result
}

fn strip_links(s: &str) -> String {
    let bytes: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '[' {
            if let Some(close) = bytes[i + 1..].iter().position(|&c| c == ']') {
                let close = i + 1 + close;
                if close + 1 < bytes.len() && bytes[close + 1] == '(' {
                    if let Some(paren) = bytes[close + 2..].iter().position(|&c| c == ')') {
                        let text: String = bytes[i + 1..close].iter().collect();
                        out.push_str(&text);
                        i = close + 2 + paren + 1;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
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
        assert!(typ.contains("#columns(2"));
        assert!(typ.contains("/ *kata*"));
        // Manual-style book chrome.
        assert!(typ.contains("iso-b5"));
        assert!(typ.contains("= The Lexicon"));
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
            study: None,
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
        assert!(typ.contains("#outline(title: \"Contents\""));
        assert!(typ.contains("= Phonology"));
        assert!(typ.contains("#table(columns: 2"));
        assert!(typ.contains("== Greeting"));
        // Manual-style book chrome from the shared scaffold.
        assert!(typ.contains("iso-b5"));
        assert!(typ.contains("#let term(name, body)"));
    }

    #[test]
    fn grammar_study_guide_leads_when_present() {
        let profile = LanguageProfile::default();
        let phon = grammar_phon();
        let typology = std::collections::BTreeMap::new();
        let samples: Vec<(String, String)> = Vec::new();
        let book = GrammarBook {
            language: "Avesha",
            font_family: None,
            profile: &profile,
            phonology: &phon,
            morphology: None,
            typology: &typology,
            expressions: None,
            samples: &samples,
            study: Some("## What is a case?\n\nA grammatical case marks a noun's role."),
        };
        let md = grammar_markdown(&book);
        let study_pos = md.find("## Study Guide").unwrap();
        let phon_pos = md.find("## Phonology").unwrap();
        assert!(study_pos < phon_pos, "study guide should lead");
        assert!(md.contains("What is a case?"));

        let typ = grammar_typst(&book);
        assert!(typ.contains("= Study Guide"));
    }

    #[test]
    fn tutorial_scaffold_sets_up_a_book() {
        let s = tutorial_typst_scaffold("Avesha", Some("Avesha"), Some("\u{E000}\u{E001}"));
        assert!(s.contains("#set document(title: \"Learn Avesha\")"));
        assert!(s.contains("iso-b5"));
        assert!(s.contains("#let native(cp) = text(font: \"Avesha\""));
        assert!(s.contains("#let practice(body)"));
        // The cover line renders the native escapes.
        assert!(s.contains("#native(\"\\u{E000}\\u{E001}\")"));
        assert!(s.contains("#outline(title: \"Contents\""));
    }

    #[test]
    fn tutorial_scaffold_degrades_without_font() {
        let s = tutorial_typst_scaffold("Avesha", None, None);
        assert!(s.contains("#let native(cp) = text(size: 1.3em)"));
        assert!(!s.contains("font: \"Avesha\""));
    }

    #[test]
    fn markdown_to_typst_headings_and_emphasis() {
        let md = "# Learn It\n\n## Lesson 1\n\nThis is **bold** and *italic* text.\n";
        let typ = markdown_to_typst(md);
        // The first H1 (book title) is dropped (scaffold has a title page).
        assert!(!typ.contains("Learn It"));
        assert!(typ.contains("== Lesson 1"));
        assert!(typ.contains("#strong[bold]"));
        assert!(typ.contains("#emph[italic]"));
        // No markdown heading marks survive.
        assert!(!typ.contains("## "));
    }

    #[test]
    fn markdown_to_typst_intraword_emphasis_is_safe() {
        // Markdown allows `*pa*ta`; emitted as a function call it stays valid
        // Typst (mid-word `_pa_ta` would be an unclosed delimiter).
        let md = "- *pa*ta and ki*ra* here\n";
        let typ = markdown_to_typst(md);
        assert!(typ.contains("#emph[pa]ta"), "got: {typ}");
        assert!(typ.contains("ki#emph[ra]"), "got: {typ}");
        // No bare emphasis markup left to trip the boundary rule.
        assert!(!typ.contains("_pa_"));
    }

    #[test]
    fn markdown_to_typst_table_and_quote() {
        let md = "## Words\n\n| Word | Meaning |\n|---|---|\n| pata | stone |\n\n> Practice this.\n";
        let typ = markdown_to_typst(md);
        assert!(typ.contains("#table(columns: 2"));
        assert!(typ.contains("table.header([Word], [Meaning])"));
        assert!(typ.contains("[pata], [stone],"));
        assert!(typ.contains("#practice[Practice this.]"));
    }

    #[test]
    fn markdown_to_typst_escapes_prose_specials() {
        let md = "Use the # sign and the @ at-sign and a < b.\n";
        let typ = markdown_to_typst(md);
        assert!(typ.contains("\\#"));
        assert!(typ.contains("\\@"));
        assert!(typ.contains("\\<"));
    }

    #[test]
    fn markdown_to_typst_lists_and_code() {
        let md = "- one\n- two\n\n1. first\n2. second\n\nInline `code` stays.\n";
        let typ = markdown_to_typst(md);
        assert!(typ.contains("- one"));
        assert!(typ.contains("+ first"));
        assert!(typ.contains("`code`"));
    }

    #[test]
    fn markdown_to_typst_list_item_emphasis() {
        let md = "- pa*ta* (stress on *PA*-ta.)\n";
        let typ = markdown_to_typst(md);
        assert!(typ.contains("pa#emph[ta]"), "got: {typ}");
        assert!(typ.contains("#emph[PA]"), "got: {typ}");
        // No bare `*` emphasis markup survives.
        assert!(!typ.contains('*'), "stray asterisk: {typ}");
    }

    #[test]
    fn markdown_to_typst_balances_stray_emphasis() {
        // An unbalanced `*` must not produce an unclosed Typst delimiter.
        let md = "a lone * star and 5*3 math\n";
        let typ = markdown_to_typst(md);
        let stars = typ.matches('*').count();
        let unescaped = typ.matches("\\*").count();
        assert_eq!(stars, unescaped, "unescaped lone star: {typ}");
    }
}
