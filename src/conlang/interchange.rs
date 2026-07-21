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

/// IGT-1 (Wave 4) — render stored interlinear texts as a LaTeX `linguex`
/// document: each text is a numbered `\ex.` with a `\gll` (the segmented sentence
/// over its gloss, word-aligned) and a `\glt` free translation — the form a
/// grammar sketch or paper pastes in. A space inside a token (a multi-word gloss)
/// becomes `~` so `\gll` keeps the two lines aligned.
pub fn igt_linguex(language: &str, texts: &[(String, crate::conlang::igt::Igt)]) -> String {
    let cell = |s: &str| latex_escape(s).replace(' ', "~");

    let mut out = String::new();
    out.push_str("% Generated by Inkhaven · language texts --format latex\n");
    out.push_str("\\documentclass[11pt]{article}\n");
    out.push_str("\\usepackage[utf8]{inputenc}\n");
    out.push_str("\\usepackage{linguex}\n");
    out.push_str("\\usepackage{tipa}\n");
    out.push_str(&format!("\\title{{{} --- Texts}}\n", latex_escape(language)));
    out.push_str("\\begin{document}\n\\maketitle\n\n");
    for (name, igt) in texts {
        out.push_str(&format!("% {}\n", latex_escape(name)));
        let cols = igt.columns();
        let seg = cols.iter().map(|(s, _)| cell(s)).collect::<Vec<_>>().join(" ");
        let gloss = cols.iter().map(|(_, g)| cell(g)).collect::<Vec<_>>().join(" ");
        out.push_str("\\ex.\n");
        out.push_str(&format!("\\gll {seg}\\\\\n"));
        out.push_str(&format!("{gloss}\\\\\n"));
        out.push_str(&format!("\\glt `{}'\n\n", latex_escape(&igt.translation)));
    }
    out.push_str("\\end{document}\n");
    out
}

/// UD (1.8.1) — export stored interlinear texts as CoNLL-U, the Universal
/// Dependencies interchange format. Emits morphological annotation: FORM,
/// LEMMA, UPOS (mapped from the lexicon's part of speech), XPOS (the raw POS),
/// and FEATS (the Leipzig grammatical glosses mapped to UD features). HEAD and
/// DEPREL are left unspecified (`_`) — this is a valid morphology-only treebank
/// that UD tooling accepts; a full dependency parse is a later increment. The
/// tag→feature map is language-independent (Leipzig abbreviations are the same
/// across languages), so a Russian gloss `stone-PL-GEN` yields
/// `Case=Gen|Number=Plur` exactly as an invented language would. Pure.
pub fn igt_conllu(
    texts: &[(String, crate::conlang::igt::Igt)],
    entries: &[DictionaryEntry],
) -> String {
    use std::collections::BTreeMap;
    let pos_of: BTreeMap<&str, &str> =
        entries.iter().map(|e| (e.word.as_str(), e.pos.as_str())).collect();

    let mut out = String::new();
    out.push_str("# generator = Inkhaven · language texts --format conllu\n");
    for (name, igt) in texts {
        out.push_str(&format!("# sent_id = {}\n", conllu_sent_id(name)));
        out.push_str(&format!("# text = {}\n", conllu_meta(&igt.text)));
        if !igt.translation.trim().is_empty() {
            out.push_str(&format!("# text_translation = {}\n", conllu_meta(&igt.translation)));
        }
        for (i, w) in igt.words.iter().enumerate() {
            let id = i + 1;
            let form = conllu_field(&w.surface);
            let root = w.root.as_deref().filter(|s| !s.trim().is_empty());
            let lemma = conllu_field(root.unwrap_or(&w.surface));
            let (upos, xpos) = match root.and_then(|r| pos_of.get(r)) {
                Some(pos) => (conllu_upos(pos), conllu_field(pos)),
                None => ("X", "_".to_string()),
            };
            let feats = conllu_feats(w.gloss.as_deref(), &w.segments);
            out.push_str(&format!("{id}\t{form}\t{lemma}\t{upos}\t{xpos}\t{feats}\t_\t_\t_\t_\n"));
        }
        out.push('\n');
    }
    out
}

/// A CoNLL-U comment value: no tabs or newlines.
fn conllu_meta(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ").trim().to_string()
}

/// A CoNLL-U column value: never empty (→ `_`), never a tab/newline.
fn conllu_field(s: &str) -> String {
    let t = s.replace(['\t', '\n', '\r'], " ");
    let t = t.trim();
    if t.is_empty() { "_".to_string() } else { t.to_string() }
}

/// A `sent_id` token — whitespace collapsed to underscores.
fn conllu_sent_id(name: &str) -> String {
    let s: String =
        name.trim().chars().map(|c| if c.is_whitespace() { '_' } else { c }).collect();
    if s.is_empty() { "s".to_string() } else { s }
}

/// Map a lexicon part-of-speech string to a UD universal POS tag. Accepts the
/// common spellings and abbreviations; unknown → `X`.
fn conllu_upos(pos: &str) -> &'static str {
    match pos.trim().to_lowercase().as_str() {
        "noun" | "n" => "NOUN",
        "proper noun" | "propn" | "proper" | "name" => "PROPN",
        "verb" | "v" => "VERB",
        "auxiliary" | "aux" => "AUX",
        "adjective" | "adj" | "a" => "ADJ",
        "adverb" | "adv" => "ADV",
        "pronoun" | "pron" => "PRON",
        "preposition" | "postposition" | "adposition" | "adp" => "ADP",
        "conjunction" | "conj" | "coordinating conjunction" | "cconj" => "CCONJ",
        "subordinating conjunction" | "sconj" => "SCONJ",
        "determiner" | "det" | "article" => "DET",
        "numeral" | "num" | "number" => "NUM",
        "interjection" | "intj" => "INTJ",
        "particle" | "part" | "ptcl" => "PART",
        "symbol" | "sym" => "SYM",
        "punctuation" | "punct" => "PUNCT",
        _ => "X",
    }
}

/// Extract UD morphological features from a word's Leipzig gloss (or its
/// per-morpheme segment glosses). Grammatical tags — all-caps and/or digits —
/// map to UD `Key=Value` pairs; lexical (lowercase) glosses are skipped. The
/// pairs are keyed in a `BTreeMap`, so the output is UD-canonical (alphabetical,
/// `|`-joined). `_` when the word carries no recognised feature.
fn conllu_feats(gloss: Option<&str>, segments: &[crate::conlang::morphology::paradigm::MorphSeg]) -> String {
    use std::collections::BTreeMap;
    let mut feats: BTreeMap<&'static str, &'static str> = BTreeMap::new();
    let mut consume = |g: &str| {
        for part in g.split(['-', '.', '=', ':']) {
            let p = part.trim();
            if is_grammatical_tag(p) {
                if let Some((k, v)) = leipzig_to_ud(&p.to_uppercase()) {
                    feats.insert(k, v);
                }
            }
        }
    };
    if !segments.is_empty() {
        for s in segments {
            consume(&s.gloss);
        }
    } else if let Some(g) = gloss {
        consume(g);
    }
    if feats.is_empty() {
        "_".to_string()
    } else {
        feats.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("|")
    }
}

/// A Leipzig grammatical gloss is written in caps and/or digits (`NOM`, `PL`,
/// `3`, `PST`); a lexical gloss carries lowercase letters (`stone`).
fn is_grammatical_tag(s: &str) -> bool {
    !s.is_empty()
        && s.chars().any(|c| c.is_alphanumeric())
        && s.chars().all(|c| c.is_ascii_digit() || (c.is_alphabetic() && c.is_uppercase()))
}

/// Map a single Leipzig grammatical abbreviation to a UD feature `Key=Value`.
/// Language-independent — the abbreviations are standard across the world's
/// languages, so this serves Russian, an invented language, or anything else.
fn leipzig_to_ud(tag: &str) -> Option<(&'static str, &'static str)> {
    Some(match tag {
        // Number
        "SG" => ("Number", "Sing"),
        "PL" => ("Number", "Plur"),
        "DU" => ("Number", "Dual"),
        // Case
        "NOM" => ("Case", "Nom"),
        "ACC" => ("Case", "Acc"),
        "GEN" => ("Case", "Gen"),
        "DAT" => ("Case", "Dat"),
        "INS" | "INSTR" => ("Case", "Ins"),
        "LOC" | "PREP" | "PRP" => ("Case", "Loc"),
        "ABL" => ("Case", "Abl"),
        "VOC" => ("Case", "Voc"),
        "ERG" => ("Case", "Erg"),
        "ABS" => ("Case", "Abs"),
        // Person
        "1" => ("Person", "1"),
        "2" => ("Person", "2"),
        "3" => ("Person", "3"),
        // Gender
        "M" | "MASC" => ("Gender", "Masc"),
        "F" | "FEM" => ("Gender", "Fem"),
        "N" | "NEUT" => ("Gender", "Neut"),
        // Tense
        "PST" | "PAST" => ("Tense", "Past"),
        "PRS" | "PRES" => ("Tense", "Pres"),
        "FUT" => ("Tense", "Fut"),
        // Aspect
        "PFV" | "PF" | "PERF" => ("Aspect", "Perf"),
        "IPFV" | "IPF" | "IMPF" => ("Aspect", "Imp"),
        // Mood
        "IND" => ("Mood", "Ind"),
        "SBJV" | "SUBJ" => ("Mood", "Sub"),
        "IMP" => ("Mood", "Imp"),
        "COND" | "CND" => ("Mood", "Cnd"),
        // Voice
        "PASS" => ("Voice", "Pass"),
        "ACT" => ("Voice", "Act"),
        "MID" => ("Voice", "Mid"),
        // Definiteness / degree / polarity
        "DEF" => ("Definite", "Def"),
        "INDF" | "INDEF" => ("Definite", "Ind"),
        "CMPR" | "COMP" => ("Degree", "Cmp"),
        "SUP" | "SUPL" => ("Degree", "Sup"),
        "NEG" => ("Polarity", "Neg"),
        // Non-finite verb forms
        "INF" => ("VerbForm", "Inf"),
        "PTCP" => ("VerbForm", "Part"),
        "GER" | "CVB" => ("VerbForm", "Conv"),
        _ => return None,
    })
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

// ── Importers ─────────────────────────────────────────────────────────────
//
// Parse foreign lexicon formats into a neutral lexeme the CLI maps onto its
// `ImportEntry` (and thence the proposal-gated dictionary writer). Parsers are
// tolerant: an unrecognised field is skipped, never fatal, so a real-world
// export with extra markers still imports its core data.

/// A single imported headword, format-agnostic. Only the fields Inkhaven's
/// dictionary models are pulled; everything else in the source is ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportedLexeme {
    pub word: String,
    pub pos: String,
    pub translation: String,
    pub example: String,
    pub pronunciation: String,
    pub etymology: String,
    pub notes: String,
}

impl ImportedLexeme {
    fn is_empty(&self) -> bool {
        self.word.trim().is_empty()
    }
}

/// Parse a Toolbox / MDF **Standard Format** (SFM) database — the lingua franca
/// of descriptive lexicography (SIL Toolbox, FieldWorks, and **Lexique Pro** all
/// read and write it). Records open at a `\lx` marker and run to the next one;
/// `\mkr value` lines map by marker, and unmarked continuation lines fold into
/// the preceding marker. The standard MDF marker set is recognised:
///
/// - `\lx` headword · `\ph` pronunciation · `\ps` part of speech
/// - `\ge`/`\gn`/`\gloss` gloss → translation (first non-empty wins)
/// - `\de` definition (fallback translation) · `\xv` example · `\et` etymology
/// - `\nt`/`\cf` notes / cross-references
pub fn parse_toolbox(src: &str) -> Vec<ImportedLexeme> {
    // Fold continuation lines: a line that does not start with `\` belongs to
    // the previous marker. Produces a flat list of (marker, value).
    let mut fields: Vec<(String, String)> = Vec::new();
    for raw in src.lines() {
        let line = raw.trim_end();
        if line.trim_start().starts_with('\\') {
            let body = line.trim_start().trim_start_matches('\\');
            let (mkr, val) = match body.split_once(char::is_whitespace) {
                Some((m, v)) => (m.to_string(), v.trim().to_string()),
                None => (body.to_string(), String::new()),
            };
            fields.push((mkr.to_ascii_lowercase(), val));
        } else if let Some(last) = fields.last_mut() {
            let extra = line.trim();
            if !extra.is_empty() {
                if !last.1.is_empty() {
                    last.1.push(' ');
                }
                last.1.push_str(extra);
            }
        }
    }

    let mut out: Vec<ImportedLexeme> = Vec::new();
    let mut cur = ImportedLexeme::default();
    // The definition (`\de`) is only a fallback for the gloss; keep it separate so
    // field order can't let a definition win over a gloss it precedes.
    let mut def = String::new();
    let flush = |cur: &mut ImportedLexeme, def: &mut String, out: &mut Vec<ImportedLexeme>| {
        if cur.translation.is_empty() {
            cur.translation = std::mem::take(def);
        } else {
            def.clear();
        }
        if !cur.is_empty() {
            out.push(std::mem::take(cur));
        } else {
            *cur = ImportedLexeme::default();
        }
    };
    for (mkr, val) in fields {
        match mkr.as_str() {
            "lx" => {
                flush(&mut cur, &mut def, &mut out);
                cur.word = val;
            }
            "ph" => cur.pronunciation = val,
            "ps" => cur.pos = val,
            // The gloss is preferred; the definition (`\de`) is only used when no
            // gloss is given — applied at flush, so order doesn't matter.
            "ge" | "gn" | "gloss" | "g" => {
                if cur.translation.is_empty() {
                    cur.translation = val;
                }
            }
            "de" => {
                if def.is_empty() {
                    def = val;
                }
            }
            "xv" => {
                if cur.example.is_empty() {
                    cur.example = val;
                }
            }
            "et" => cur.etymology = val,
            "nt" | "cf" => {
                if !val.is_empty() {
                    if !cur.notes.is_empty() {
                        cur.notes.push_str("; ");
                    }
                    cur.notes.push_str(&val);
                }
            }
            _ => {}
        }
    }
    flush(&mut cur, &mut def, &mut out);
    out
}

/// Parse a **PolyGlot** dictionary. PolyGlot's native `.pgd` is a ZIP whose
/// `PGDictionary.xml` holds the lexicon; pass that XML here (the CLI unzips).
/// Words live in `<word>` elements with `<conWord>` (the invented form),
/// `<localWord>` (the natural-language equivalent), `<definition>`,
/// `<pronunciation>`, and a `<wordTypeId>` resolved against the part-of-speech
/// table (`<wordGrammarClass>`/`<wordTypeNode>` → id + name). Tolerant of the
/// schema drift across PolyGlot versions: unknown tags are skipped.
pub fn parse_polyglot(xml: &str) -> Result<Vec<ImportedLexeme>, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use std::collections::BTreeMap;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // The POS-node table (id → name) and the words are both filled during the one
    // streaming pass, but a word can appear BEFORE the table entry it references,
    // so POS is resolved in a genuine second pass after the loop — each word is
    // kept with its raw `wordTypeId` until the whole table is known.
    let mut pos_table: BTreeMap<String, String> = BTreeMap::new();
    let mut words: Vec<(ImportedLexeme, String)> = Vec::new();

    // Streaming state.
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut cur_word: Option<ImportedLexeme> = None;
    let mut cur_word_type_id = String::new();
    let mut cur_pos: Option<(String, String)> = None; // (id, name)
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(format!(
                    "PolyGlot XML parse error at {}: {e}",
                    reader.buffer_position()
                ))
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let lname = name.to_ascii_lowercase();
                match lname.as_str() {
                    "word" => {
                        cur_word = Some(ImportedLexeme::default());
                        cur_word_type_id.clear();
                    }
                    // The POS-table node name varies by version; match the
                    // common ones. It carries an id + a class name.
                    "wordtypenode" | "wordgrammarclass" | "partofspeech"
                    | "posnode" => {
                        cur_pos = Some((String::new(), String::new()));
                    }
                    _ => {}
                }
                path.push(lname);
                text.clear();
            }
            Ok(Event::Text(t)) => {
                text.push_str(&t.unescape().unwrap_or_default());
            }
            Ok(Event::End(_)) => {
                let lname = path.pop().unwrap_or_default();
                let val = text.trim().to_string();
                if let Some(w) = cur_word.as_mut() {
                    match lname.as_str() {
                        "conword" => w.word = val.clone(),
                        "localword" => {
                            if w.translation.is_empty() {
                                w.translation = val.clone();
                            }
                        }
                        "definition" => {
                            // Definitions can carry HTML; strip tags crudely so
                            // the gloss reads cleanly, and only use it when no
                            // localWord supplied a translation.
                            let stripped = strip_html(&val);
                            if w.translation.is_empty() {
                                w.translation = stripped.clone();
                            } else if w.notes.is_empty() && !stripped.is_empty() {
                                w.notes = stripped;
                            }
                        }
                        "pronunciation" => w.pronunciation = val.clone(),
                        "wordtypeid" | "wordclassid" | "pos" => {
                            cur_word_type_id = val.clone()
                        }
                        "wordetymologynotes" | "etymology" => w.etymology = val.clone(),
                        "word" => {
                            // Closing the word: keep it with its raw type id; POS
                            // is resolved after the loop (the table may not be
                            // built yet — words can precede it).
                            let w = cur_word.take().unwrap();
                            if !w.is_empty() {
                                words.push((w, cur_word_type_id.clone()));
                            }
                        }
                        _ => {}
                    }
                }
                if let Some((id, pname)) = cur_pos.as_mut() {
                    match lname.as_str() {
                        "id" | "wordtypeid" | "classid" => *id = val.clone(),
                        "value" | "name" | "wordtypename" | "classname" => {
                            *pname = val.clone()
                        }
                        "wordtypenode" | "wordgrammarclass" | "partofspeech"
                        | "posnode" => {
                            if !id.is_empty() {
                                pos_table.insert(id.clone(), pname.clone());
                            }
                            cur_pos = None;
                        }
                        _ => {}
                    }
                }
                text.clear();
            }
            _ => {}
        }
        buf.clear();
    }
    // Second pass: now the POS table is complete, resolve every word's type id.
    Ok(words
        .into_iter()
        .map(|(mut w, tid)| {
            if let Some(name) = pos_table.get(&tid) {
                w.pos = name.clone();
            }
            w
        })
        .collect())
}

/// Crude HTML-tag stripper for PolyGlot definitions, which may be rich text.
/// Drops `<...>` spans and collapses whitespace — enough to recover the gloss.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
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
    fn igt_linguex_emits_gll_and_translation() {
        use crate::conlang::igt::{Igt, IgtWord};
        use crate::conlang::morphology::paradigm::MorphSeg;
        let seg = |s: &str, g: &str| MorphSeg { surface: s.into(), gloss: g.into() };
        let igt = Igt {
            text: "katat nilo".into(),
            words: vec![
                IgtWord {
                    surface: "katat".into(),
                    root: Some("kata".into()),
                    gloss: Some("stone-DAT".into()),
                    segments: vec![seg("kata", "stone"), seg("t", "DAT")],
                },
                IgtWord { surface: "nilo".into(), root: None, gloss: None, segments: vec![] },
            ],
            translation: "stone friend".into(),
            recognised: 1,
        };
        let out = igt_linguex("Eldar", &[("t1".into(), igt)]);
        assert!(out.contains("\\usepackage{linguex}"));
        // The segmented line over the gloss line, word-aligned.
        assert!(out.contains("\\gll kata-t nilo\\\\"), "{out}");
        assert!(out.contains("stone-DAT nilo\\\\"), "{out}");
        assert!(out.contains("\\glt `stone friend'"));
    }

    #[test]
    fn igt_linguex_keeps_multiword_glosses_aligned() {
        use crate::conlang::igt::{Igt, IgtWord};
        let igt = Igt {
            text: "x".into(),
            words: vec![IgtWord {
                surface: "x".into(),
                root: Some("x".into()),
                gloss: Some("old man".into()),
                segments: vec![],
            }],
            translation: "old man".into(),
            recognised: 1,
        };
        let out = igt_linguex("T", &[("t".into(), igt)]);
        // The multi-word gloss becomes one aligned unit via a non-breaking space.
        assert!(out.contains("old~man\\\\"), "{out}");
    }

    #[test]
    fn conllu_emits_morphological_columns_and_metadata() {
        use crate::conlang::igt::{Igt, IgtWord};
        use crate::conlang::morphology::paradigm::MorphSeg;
        use crate::language_entry::DictionaryEntry;
        let seg = |s: &str, g: &str| MorphSeg { surface: s.into(), gloss: g.into() };
        let igt = Igt {
            text: "katat nilo".into(),
            words: vec![
                IgtWord {
                    surface: "katat".into(),
                    root: Some("kata".into()),
                    gloss: Some("stone-PL-DAT".into()),
                    segments: vec![seg("kata", "stone"), seg("t", "PL"), seg("", "DAT")],
                },
                // Unrecognised word → UPOS X, no features.
                IgtWord { surface: "nilo".into(), root: None, gloss: None, segments: vec![] },
            ],
            translation: "to the stones, nilo".into(),
            recognised: 1,
        };
        let entries = vec![DictionaryEntry { word: "kata".into(), pos: "noun".into(), ..Default::default() }];
        let out = igt_conllu(&[("my text".into(), igt)], &entries);
        // Metadata: sent_id with spaces collapsed, text, translation.
        assert!(out.contains("# sent_id = my_text"), "{out}");
        assert!(out.contains("# text = katat nilo"), "{out}");
        assert!(out.contains("# text_translation = to the stones, nilo"), "{out}");
        // Token 1: FORM LEMMA UPOS XPOS FEATS, features UD-canonical (alphabetical).
        assert!(out.contains("1\tkatat\tkata\tNOUN\tnoun\tCase=Dat|Number=Plur\t_\t_\t_\t_"), "{out}");
        // Token 2: unrecognised → lemma falls back to surface, UPOS X, no feats.
        assert!(out.contains("2\tnilo\tnilo\tX\t_\t_\t_\t_\t_\t_"), "{out}");
    }

    #[test]
    fn conllu_maps_russian_gloss_features() {
        use crate::conlang::igt::{Igt, IgtWord};
        use crate::language_entry::DictionaryEntry;
        // Russian: окно 'window', genitive singular neuter → окна.
        let igt = Igt {
            text: "окна".into(),
            words: vec![IgtWord {
                surface: "окна".into(),
                root: Some("окно".into()),
                gloss: Some("window-GEN.SG.N".into()),
                segments: vec![],
            }],
            translation: "of the window".into(),
            recognised: 1,
        };
        let entries = vec![DictionaryEntry { word: "окно".into(), pos: "noun".into(), ..Default::default() }];
        let out = igt_conllu(&[("ru1".into(), igt)], &entries);
        // Portmanteau gloss split on '.', mapped to canonical UD FEATS.
        assert!(out.contains("1\tокна\tокно\tNOUN\tnoun\tCase=Gen|Gender=Neut|Number=Sing\t_\t_\t_\t_"), "{out}");
    }

    #[test]
    fn toolbox_parses_records_and_markers() {
        let src = "\\lx kira\n\\ph ˈki.ɾa\n\\ps n\n\\ge bird\n\\de a small flying creature\n\\xv kira nami\n\\et from proto *kir\n\\nt totem animal\n\n\\lx pata\n\\ps n\n\\ge stone\n";
        let got = parse_toolbox(src);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].word, "kira");
        assert_eq!(got[0].pronunciation, "ˈki.ɾa");
        assert_eq!(got[0].pos, "n");
        assert_eq!(got[0].translation, "bird"); // \ge wins over \de
        assert_eq!(got[0].example, "kira nami");
        assert_eq!(got[0].etymology, "from proto *kir");
        assert_eq!(got[0].notes, "totem animal");
        assert_eq!(got[1].word, "pata");
        assert_eq!(got[1].translation, "stone");
    }

    #[test]
    fn toolbox_folds_continuation_lines_and_falls_back_to_de() {
        let src = "\\lx mira\n\\de bright, shining,\n   radiant\n";
        let got = parse_toolbox(src);
        assert_eq!(got.len(), 1);
        // \de used as translation when no \ge; continuation line folded in.
        assert_eq!(got[0].translation, "bright, shining, radiant");
    }

    #[test]
    fn toolbox_prefers_gloss_over_definition_regardless_of_order() {
        // \de precedes \ge here; the gloss must still win (was a field-order bug).
        let src = "\\lx mira\n\\de a long definition\n\\ge light\n";
        let got = parse_toolbox(src);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].translation, "light");
    }

    #[test]
    fn polyglot_parses_words_and_resolves_pos() {
        let xml = r#"<dictionary>
          <PartOfSpeechCollection>
            <wordTypeNode><wordTypeId>1</wordTypeId><wordTypeName>noun</wordTypeName></wordTypeNode>
          </PartOfSpeechCollection>
          <word>
            <conWord>kira</conWord>
            <localWord>bird</localWord>
            <wordTypeId>1</wordTypeId>
            <pronunciation>kira</pronunciation>
            <definition>&lt;b&gt;a bird&lt;/b&gt;</definition>
          </word>
          <word>
            <conWord>pata</conWord>
            <localWord>stone</localWord>
            <wordTypeId>1</wordTypeId>
          </word>
        </dictionary>"#;
        let got = parse_polyglot(xml).expect("parse");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].word, "kira");
        assert_eq!(got[0].translation, "bird"); // localWord wins
        assert_eq!(got[0].pos, "noun"); // resolved from the type table
        assert_eq!(got[0].pronunciation, "kira");
        assert_eq!(got[0].notes, "a bird"); // definition HTML stripped into notes
        assert_eq!(got[1].word, "pata");
        assert_eq!(got[1].pos, "noun");
    }

    #[test]
    fn polyglot_uses_definition_when_no_localword() {
        let xml = "<dictionary><word><conWord>sol</conWord><definition>the sun</definition></word></dictionary>";
        let got = parse_polyglot(xml).expect("parse");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].translation, "the sun");
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
