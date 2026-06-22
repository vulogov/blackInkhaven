//! LANG-3 Tier 1 (RBMT) — a deterministic, dependency-free English analyzer.
//!
//! This is the first cut of the source-side parser RFC LANG-3's P0 describes. It
//! recovers a simple declarative clause — subject / verb / object — from English,
//! handling articles, number (`-s` plural), 3rd-person `-s` verbs, and subject
//! pronouns (for the verb's grammatical person). It is intentionally small and
//! predictable: the neural POS+dependency parser the RFC specifies is a later
//! swap *behind this same `analyze` interface*, at which point richer structure
//! (multi-word noun phrases, adjectives on any noun, subordinate clauses) comes
//! online. Until then the scope is one content word per constituent.
//!
//! Pure and deterministic: no I/O, no model, no allocation beyond the result.

/// A noun phrase recovered from English: a singular noun lemma, its number, and
/// an optional attributive adjective.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnglishNp {
    /// The noun, reduced to its singular lemma.
    pub head: String,
    /// `"sg"` or `"pl"` (the morphology paradigms' convention).
    pub number: String,
    /// An attributive adjective, when one is recovered. The positional
    /// [`analyze`] fallback never sets it; the lexicon-aware structuring pass in
    /// the parent module does.
    pub adjective: Option<String>,
}

impl EnglishNp {
    fn bare(head: String, number: String) -> Self {
        EnglishNp { head, number, adjective: None }
    }
}

/// A clause recovered from English. Any of the slots may be empty (an
/// unparseable input yields an all-`None` clause).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnglishClause {
    pub subject: Option<EnglishNp>,
    /// The verb, reduced to its lemma.
    pub verb: Option<String>,
    /// The subject's grammatical person — `"1"`, `"2"`, or `"3"` — from a subject
    /// pronoun, defaulting to `"3"`.
    pub verb_person: String,
    pub object: Option<EnglishNp>,
}

/// Articles dropped before analysis.
fn is_article(t: &str) -> bool {
    matches!(t, "the" | "a" | "an")
}

/// Subject pronouns → `(person, is_plural)`. Used to set the verb's person and,
/// when no nominal subject is present, to leave the subject slot empty (a
/// pronoun is not a lexicon headword) while still agreeing the verb.
fn subject_pronoun(t: &str) -> Option<(&'static str, bool)> {
    Some(match t {
        "i" => ("1", false),
        "we" => ("1", true),
        "you" => ("2", false),
        "he" | "she" | "it" => ("3", false),
        "they" => ("3", true),
        _ => return None,
    })
}

/// The article-stripped, pronoun-resolved token stream — the input a
/// lexicon-aware structuring pass works over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    /// Verb person from a leading subject pronoun (`"1"`/`"2"`/`"3"`).
    pub person: String,
    /// Whether a subject *pronoun* filled the subject slot (so no nominal
    /// subject is expected).
    pub pronoun_subject: bool,
    /// The remaining content tokens, lowercased, articles and any leading
    /// subject pronoun removed.
    pub tokens: Vec<String>,
}

/// Tokenize, lowercase, drop articles, and peel a leading subject pronoun
/// (recording its person). The leftover tokens are handed to the structuring
/// pass, which assigns roles using the lexicon's parts of speech.
pub fn prepare(text: &str) -> Prepared {
    let mut tokens: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .filter(|t| !is_article(t))
        .collect();
    let mut person = "3".to_string();
    let mut pronoun_subject = false;
    if let Some(first) = tokens.first() {
        if let Some((p, _plural)) = subject_pronoun(first) {
            person = p.to_string();
            pronoun_subject = true;
            tokens.remove(0);
        }
    }
    Prepared { person, pronoun_subject, tokens }
}

/// Reduce an English noun to `(singular_lemma, is_plural)`.
pub fn depluralize(noun: &str) -> (String, bool) {
    let lower = noun;
    if let Some(stem) = lower.strip_suffix("ies") {
        // "cities" → "city", but not short -ie words ("dies" → "die", below).
        if stem.len() >= 3 {
            return (format!("{stem}y"), true);
        }
    }
    for suf in ["ses", "xes", "zes", "ches", "shes"] {
        if let Some(stem) = lower.strip_suffix(suf) {
            // "boxes" → "box", "watches" → "watch"
            return (format!("{stem}{}", &suf[..suf.len() - 2]), true);
        }
    }
    if lower.ends_with("ss") {
        // "grass" — not a plural.
        return (lower.to_string(), false);
    }
    if let Some(stem) = lower.strip_suffix('s') {
        if stem.len() >= 2 {
            return (stem.to_string(), true);
        }
    }
    (lower.to_string(), false)
}

/// Reduce an English present-tense verb to its lemma (undo the 3rd-singular
/// `-s` / `-es`). Leaves a bare-stem verb (plural subject: "birds *see*")
/// untouched.
pub fn delemmatize_verb(verb: &str) -> String {
    if let Some(stem) = verb.strip_suffix("ies") {
        if stem.len() >= 3 {
            return format!("{stem}y"); // "carries" → "carry"
        }
        // short -ie verbs: "dies" → "die", "lies" → "lie" (strip just -s below).
    }
    for suf in ["ses", "xes", "zes", "ches", "shes"] {
        if let Some(stem) = verb.strip_suffix(suf) {
            return format!("{stem}{}", &suf[..suf.len() - 2]); // "watches" → "watch"
        }
    }
    if verb.ends_with("ss") {
        return verb.to_string();
    }
    if let Some(stem) = verb.strip_suffix('s') {
        if stem.len() >= 2 {
            return stem.to_string();
        }
    }
    verb.to_string()
}

/// Analyze a simple English clause into subject / verb / object.
///
/// Recognised shapes (after dropping articles): an optional leading subject
/// pronoun, then a nominal subject (unless a pronoun took that role), a verb,
/// and an optional object — one content word each. Unrecognised input degrades
/// to whatever slots can be filled, leaving the rest `None`.
pub fn analyze(text: &str) -> EnglishClause {
    let tokens: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .filter(|t| !is_article(t))
        .collect();

    let mut clause = EnglishClause { verb_person: "3".into(), ..Default::default() };
    if tokens.is_empty() {
        return clause;
    }

    // A leading subject pronoun sets person; the subject slot then stays empty
    // (pronouns are not headwords), and the remaining tokens are verb [+ object].
    let mut rest = tokens.as_slice();
    if let Some((person, _plural)) = subject_pronoun(&tokens[0]) {
        clause.verb_person = person.to_string();
        rest = &tokens[1..];
        match rest {
            [v] => clause.verb = Some(delemmatize_verb(v)),
            [v, o] => {
                clause.verb = Some(delemmatize_verb(v));
                let (lemma, plural) = depluralize(o);
                clause.object = Some(EnglishNp::bare(lemma, number(plural)));
            }
            [v, rest_obj @ ..] if !rest_obj.is_empty() => {
                clause.verb = Some(delemmatize_verb(v));
                let o = rest_obj.last().unwrap();
                let (lemma, plural) = depluralize(o);
                clause.object = Some(EnglishNp::bare(lemma, number(plural)));
            }
            _ => {}
        }
        return clause;
    }

    // Nominal subject: [subject, verb] or [subject, verb, object].
    match rest {
        [s, v] => {
            let (lemma, plural) = depluralize(s);
            clause.subject = Some(EnglishNp::bare(lemma, number(plural)));
            clause.verb = Some(delemmatize_verb(v));
        }
        [s, v, o, ..] => {
            let (slemma, splural) = depluralize(s);
            clause.subject = Some(EnglishNp::bare(slemma, number(splural)));
            clause.verb = Some(delemmatize_verb(v));
            let (olemma, oplural) = depluralize(o);
            clause.object = Some(EnglishNp::bare(olemma, number(oplural)));
        }
        [s] => {
            // A lone noun: treat as a bare subject (no verb).
            let (lemma, plural) = depluralize(s);
            clause.subject = Some(EnglishNp::bare(lemma, number(plural)));
        }
        _ => {}
    }
    clause
}

/// The grammatical-number value, in the abbreviated convention the morphology
/// paradigms and the `sentence` command use (`sg` / `pl`).
fn number(plural: bool) -> String {
    if plural { "pl".into() } else { "sg".into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svo_with_articles() {
        let c = analyze("the bird sees the stone");
        assert_eq!(c.subject, Some(EnglishNp::bare("bird".into(), "sg".into())));
        assert_eq!(c.verb.as_deref(), Some("see"));
        assert_eq!(c.object, Some(EnglishNp::bare("stone".into(), "sg".into())));
        assert_eq!(c.verb_person, "3");
    }

    #[test]
    fn plural_subject_and_object() {
        let c = analyze("birds see stones");
        assert_eq!(c.subject, Some(EnglishNp::bare("bird".into(), "pl".into())));
        assert_eq!(c.verb.as_deref(), Some("see"));
        assert_eq!(c.object, Some(EnglishNp::bare("stone".into(), "pl".into())));
    }

    #[test]
    fn pronoun_subject_sets_person() {
        let c = analyze("I see the stone");
        assert!(c.subject.is_none());
        assert_eq!(c.verb_person, "1");
        assert_eq!(c.verb.as_deref(), Some("see"));
        assert_eq!(c.object, Some(EnglishNp::bare("stone".into(), "sg".into())));
    }

    #[test]
    fn intransitive() {
        let c = analyze("the warrior sleeps");
        assert_eq!(c.subject, Some(EnglishNp::bare("warrior".into(), "sg".into())));
        assert_eq!(c.verb.as_deref(), Some("sleep"));
        assert!(c.object.is_none());
    }

    #[test]
    fn tricky_plurals() {
        assert_eq!(depluralize("boxes"), ("box".into(), true));
        assert_eq!(depluralize("cities"), ("city".into(), true));
        assert_eq!(depluralize("grass"), ("grass".into(), false));
        assert_eq!(depluralize("stone"), ("stone".into(), false));
    }

    #[test]
    fn empty_is_empty() {
        let c = analyze("   ");
        assert!(c.subject.is_none() && c.verb.is_none() && c.object.is_none());
    }
}
