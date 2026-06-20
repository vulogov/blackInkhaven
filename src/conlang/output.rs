//! Book output — dictionary & grammar rendering (LANG-1 P6.2).
//!
//! Render a language's data into a real document, in Markdown or Typst. The
//! Typst path is the showpiece: a paginated, two-column dictionary that embeds
//! the generated conscript font and shows each headword in the native script
//! (transliterated by the P5.6c input method) beside its romanization. Pure +
//! deterministic — the CLI prepares `RenderEntry`s (pronunciation, conscript)
//! and these functions lay them out.

use crate::conlang::analysis::LanguageProfile;

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
}
