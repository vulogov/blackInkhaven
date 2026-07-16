//! LANG-1 P6 — creative text generators.
//!
//! The "fun" surfaces that prove a language is alive: phonotactic **names**,
//! grammatical **sample prose** assembled through the syntax engine, and
//! metered **verse**. All three are deterministic (seeded) and grounded — names
//! obey the phonotactics, prose runs the real word-order/case/agreement
//! machinery, poetry counts real syllables — so nothing is invented out of thin
//! air. The register-themed AI modes (blessing / curse / incantation) build on
//! the prompt helper here but run in the CLI where the model client lives; they
//! are constrained to the existing lexicon, so the model arranges real words
//! rather than coining new ones.

use crate::conlang::generate::word::generate_word;
use crate::conlang::phonology::syllable::syllabify;
use crate::conlang::syntax::{self, Clause, NounPhrase, RenderedClause, Word};
use crate::conlang::types::morphology::Morphology;
use crate::conlang::types::{Phonology, TemplateRole};
use crate::language_entry::DictionaryEntry;
use std::collections::BTreeMap;

/// Uppercase the first character (Unicode-aware), leaving the rest untouched.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Spread a base seed into a distinct, deterministic per-index seed.
fn seed_at(base: u64, i: usize) -> u64 {
    base.wrapping_add((i as u64).wrapping_mul(2654435761))
}

/// Generate up to `count` phonotactically-valid, deduplicated **names** —
/// capitalised words drawn from the language's `root` templates (the same
/// deterministic generator that feeds the lexicon, so every name is sayable).
/// Returns fewer than `count` only when the inventory is too small to produce
/// that many distinct forms.
pub fn names(phon: &Phonology, count: usize, seed: u64) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let limit = count.saturating_mul(40) + 50;
    let mut i = 0;
    while out.len() < count && i < limit {
        if let Some(w) = generate_word(phon, TemplateRole::Root, seed_at(seed, i)) {
            let name = capitalize(&w);
            if !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
        i += 1;
    }
    out
}

/// Pick the entries of a given part of speech.
fn by_pos<'a>(entries: &'a [DictionaryEntry], pos: &str) -> Vec<&'a DictionaryEntry> {
    entries
        .iter()
        .filter(|e| e.pos.eq_ignore_ascii_case(pos) && !e.word.trim().is_empty())
        .collect()
}

fn word_of(e: &DictionaryEntry) -> Word {
    Word {
        root: e.word.clone(),
        gloss: e.translation.clone(),
    }
}

/// Assemble up to `count` grammatical **sample sentences** from the lexicon,
/// running the full syntax engine (word order, case, agreement). Subjects and
/// objects are drawn from nouns, verbs from verbs; adjectives decorate when the
/// language has them. Deterministic in `seed`. Returns nothing when the lexicon
/// lacks the nouns/verbs a clause needs.
pub fn prose(
    phon: &Phonology,
    morph: &Morphology,
    typology: &BTreeMap<String, String>,
    entries: &[DictionaryEntry],
    count: usize,
    seed: u64,
) -> Vec<RenderedClause> {
    let nouns = by_pos(entries, "noun");
    let verbs = by_pos(entries, "verb");
    let adjs = by_pos(entries, "adjective");
    if nouns.is_empty() || verbs.is_empty() {
        return Vec::new();
    }
    // Cap the batch — an unbounded `--count` would build a runaway Vec.
    let count = count.min(crate::conlang::generate::word::MAX_GENERATE_BATCH);
    let mut out = Vec::new();
    for i in 0..count {
        let s = seed_at(seed, i);
        let subj = nouns[(s as usize) % nouns.len()];
        let verb = verbs[(s as usize / 7 + i) % verbs.len()];
        // Object: a different noun when there's more than one; else intransitive.
        let object = if nouns.len() > 1 {
            let mut oi = (s as usize / 13 + 1) % nouns.len();
            if oi == (s as usize) % nouns.len() {
                oi = (oi + 1) % nouns.len();
            }
            Some(nouns[oi])
        } else {
            None
        };
        // Decorate the object with an adjective every other sentence, when available.
        let adj = if !adjs.is_empty() && i % 2 == 1 {
            Some(word_of(adjs[(s as usize / 17) % adjs.len()]))
        } else {
            None
        };
        let clause = Clause {
            subject: Some(NounPhrase {
                head: word_of(subj),
                number: "sg".into(),
                adjective: None,
            }),
            verb: Some(word_of(verb)),
            verb_person: "3".into(),
            object: object.map(|o| NounPhrase {
                head: word_of(o),
                number: "sg".into(),
                adjective: adj.clone(),
            }),
            noun_paradigm: "noun".into(),
            verb_paradigm: "verb".into(),
            ..Default::default()
        };
        let r = syntax::assemble(phon, morph, typology, &clause);
        if !r.words.is_empty() {
            out.push(r);
        }
    }
    out
}

/// One line of generated verse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoemLine {
    pub text: String,
    /// Target syllable count for the line (from the meter).
    pub target: usize,
    /// Syllables actually achieved.
    pub syllables: usize,
}

/// Count the syllables in a written word via the phonology (segment → syllabify).
/// A word with no detectable nucleus counts as one syllable so it still scans.
fn syllable_count(phon: &Phonology, word: &str) -> usize {
    let segs = phon.segment(word);
    syllabify(phon, &segs).len().max(1)
}

/// Build a candidate word pool for verse: the lexicon's headwords when it has
/// any, else freshly generated phonotactic words so an empty dictionary still
/// scans.
fn verse_pool(phon: &Phonology, entries: &[DictionaryEntry], seed: u64) -> Vec<String> {
    let mut pool: Vec<String> = entries
        .iter()
        .map(|e| e.word.clone())
        .filter(|w| !w.trim().is_empty())
        .collect();
    if pool.len() < 4 {
        for i in 0..16 {
            if let Some(w) = generate_word(phon, TemplateRole::Root, seed_at(seed, i + 1000)) {
                pool.push(w);
            }
        }
    }
    pool
}

/// Generate metered **verse**: one line per entry in `meter` (e.g. `[5,7,5]` for
/// a haiku-like form). Each line greedily draws words from the lexicon until it
/// hits its target syllable count, accepting the closest fit without overshoot
/// where possible. Deterministic in `seed`. Words come from the lexicon (or are
/// generated when the dictionary is nearly empty).
pub fn poem(
    phon: &Phonology,
    entries: &[DictionaryEntry],
    meter: &[usize],
    seed: u64,
) -> Vec<PoemLine> {
    let pool = verse_pool(phon, entries, seed);
    if pool.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut cursor = 0usize;
    for (li, &target) in meter.iter().enumerate() {
        let mut words: Vec<String> = Vec::new();
        let mut total = 0usize;
        let mut tries = 0usize;
        while total < target && tries < target.saturating_mul(4).saturating_add(8) {
            let w = &pool[(seed_at(seed, li * 31 + cursor) as usize) % pool.len()];
            cursor += 1;
            tries += 1;
            let sc = syllable_count(phon, w);
            if total + sc > target && total > 0 {
                // Would overshoot; stop and take the closest fit so far.
                break;
            }
            words.push(capitalize_first_of_line(w, words.is_empty()));
            total += sc;
            if total >= target {
                break;
            }
        }
        lines.push(PoemLine {
            text: words.join(" "),
            target,
            syllables: total,
        });
    }
    lines
}

/// Capitalise the first word of a line for a verse-like look; leave others bare.
fn capitalize_first_of_line(word: &str, first: bool) -> String {
    if first {
        capitalize(word)
    } else {
        word.to_string()
    }
}

/// Build the system+user prompt for a register-themed AI text (blessing / curse
/// / incantation). The model is constrained to *arrange existing words*: it is
/// given the full word list (surface + gloss) and the grammar summary and told
/// to compose using only those words, returning the native text, an interlinear
/// gloss, and a working-language translation. Pure string-building so the prompt
/// is testable; the actual model call lives in the CLI.
pub fn themed_prompt(
    language: &str,
    register: &str,
    working_language: &str,
    typology_summary: &str,
    entries: &[DictionaryEntry],
) -> (String, String) {
    let register_desc = match register.to_ascii_lowercase().as_str() {
        "blessing" => "a short ceremonial blessing — warm, formal, hopeful",
        "curse" => "a short curse or malediction — dark, formal, threatening",
        "incantation" | "ceremony" => {
            "a short ritual incantation — solemn, rhythmic, with a repeated line"
        }
        _ => "a short evocative passage",
    };
    let system = format!(
        "You are a poet composing in the constructed language {language}. \
         You compose {register_desc}. CRITICAL CONSTRAINTS: use ONLY words from \
         the provided lexicon — never invent a word or a form. Order words by the \
         language's grammar. Keep it to 2–5 short lines. Output exactly three \
         labelled blocks and nothing else:\n\
         NATIVE: the text in {language}\n\
         GLOSS: a word-by-word interlinear gloss\n\
         TRANSLATION: a natural rendering in {working_language}."
    );
    let mut lexicon = String::new();
    for e in entries.iter().take(120) {
        if e.word.trim().is_empty() {
            continue;
        }
        lexicon.push_str(&format!(
            "- {} = {} [{}]\n",
            e.word,
            e.translation,
            if e.pos.is_empty() { "?" } else { &e.pos }
        ));
    }
    let user = format!(
        "Grammar: {typology_summary}\n\nLexicon (use ONLY these words):\n{lexicon}\n\
         Compose {register_desc} now."
    );
    (system, user)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::template::TemplateAtom;
    use crate::conlang::types::{Phoneme, PhonemeKind, SyllableTemplate};

    /// A `CVCV` root template built from explicit atoms.
    fn cvcv() -> SyllableTemplate {
        SyllableTemplate {
            pattern: vec![
                TemplateAtom::Class("C".into()),
                TemplateAtom::Class("V".into()),
                TemplateAtom::Class("C".into()),
                TemplateAtom::Class("V".into()),
            ],
            weight: 1.0,
        }
    }

    fn toy_phon() -> Phonology {
        // CV language: consonants k,t,n,m,r,s; vowels a,i,u; one CV template.
        let cons = ["k", "t", "n", "m", "r", "s"];
        let vows = ["a", "i", "u"];
        let mut phonemes = Vec::new();
        for c in cons {
            phonemes.push(Phoneme {
                ipa: c.into(),
                romanize: None,
                kind: PhonemeKind::Consonant,
                sonority: None,
            });
        }
        for v in vows {
            phonemes.push(Phoneme {
                ipa: v.into(),
                romanize: None,
                kind: PhonemeKind::Vowel,
                sonority: None,
            });
        }
        let mut classes = BTreeMap::new();
        classes.insert("C".to_string(), cons.iter().map(|s| s.to_string()).collect());
        classes.insert("V".to_string(), vows.iter().map(|s| s.to_string()).collect());
        let mut templates = BTreeMap::new();
        templates.insert("root".to_string(), vec![cvcv()]);
        Phonology {
            phonemes,
            classes,
            templates,
            ..Default::default()
        }
    }

    fn entry(word: &str, pos: &str, tr: &str) -> DictionaryEntry {
        DictionaryEntry {
            word: word.into(),
            pos: pos.into(),
            translation: tr.into(),
            ..Default::default()
        }
    }

    #[test]
    fn names_are_distinct_capitalised_and_deterministic() {
        let phon = toy_phon();
        let a = names(&phon, 5, 42);
        let b = names(&phon, 5, 42);
        assert_eq!(a, b, "deterministic for a fixed seed");
        assert_eq!(a.len(), 5);
        // distinct + capitalised
        let set: std::collections::BTreeSet<_> = a.iter().collect();
        assert_eq!(set.len(), 5);
        for n in &a {
            assert!(n.chars().next().unwrap().is_uppercase());
        }
        // a different seed gives a different set (overwhelmingly likely)
        assert_ne!(names(&phon, 5, 99), a);
    }

    #[test]
    fn prose_assembles_sentences_from_the_lexicon() {
        let phon = toy_phon();
        let morph = Morphology::default();
        let typ = BTreeMap::new(); // defaults: svo, nom-acc
        let entries = vec![
            entry("kira", "noun", "bird"),
            entry("pata", "noun", "stone"),
            entry("nami", "verb", "see"),
        ];
        let lines = prose(&phon, &morph, &typ, &entries, 3, 7);
        assert_eq!(lines.len(), 3);
        // every sentence uses real words and renders a surface + literal
        for l in &lines {
            assert!(!l.surface.trim().is_empty());
            assert!(!l.literal.trim().is_empty());
            assert!(l.words.len() >= 2);
        }
        // deterministic
        assert_eq!(
            prose(&phon, &morph, &typ, &entries, 3, 7)[0].surface,
            lines[0].surface
        );
    }

    #[test]
    fn prose_empty_without_nouns_and_verbs() {
        let phon = toy_phon();
        let morph = Morphology::default();
        let typ = BTreeMap::new();
        let entries = vec![entry("kira", "noun", "bird")]; // no verb
        assert!(prose(&phon, &morph, &typ, &entries, 3, 1).is_empty());
    }

    #[test]
    fn poem_lines_scan_to_their_meter() {
        let phon = toy_phon();
        let entries = vec![
            entry("kira", "noun", "bird"),
            entry("pata", "noun", "stone"),
            entry("nami", "verb", "see"),
            entry("muru", "noun", "river"),
        ];
        let meter = [5, 7, 5];
        let lines = poem(&phon, &entries, &meter, 3);
        assert_eq!(lines.len(), 3);
        for (i, l) in lines.iter().enumerate() {
            assert_eq!(l.target, meter[i]);
            assert!(!l.text.trim().is_empty());
            // CV-CV words are 2 syllables each; lines reach their target or the
            // closest fit just under it (never overshoot past target).
            assert!(l.syllables <= l.target, "line {i} overshot");
            assert!(l.syllables >= l.target - 1, "line {i} fell short: {}", l.syllables);
        }
    }

    #[test]
    fn themed_prompt_constrains_to_lexicon() {
        let entries = vec![entry("kira", "noun", "bird"), entry("sol", "noun", "sun")];
        let (system, user) = themed_prompt("Eldar", "blessing", "english", "word order: SOV", &entries);
        assert!(system.contains("ONLY words from the provided lexicon"));
        assert!(system.contains("blessing"));
        assert!(user.contains("kira = bird"));
        assert!(user.contains("sol = sun"));
        assert!(user.contains("word order: SOV"));
    }
}
