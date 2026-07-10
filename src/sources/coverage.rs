//! NF-CITE — the Sourcing pass. Finds sentences that make a checkable factual claim
//! (a statistic, a date, a quotation, an attributed finding) and carry no `@key`
//! citation, so a nonfiction author can source them before publishing. Deterministic,
//! high-precision signals only — a noisy checker gets ignored.

use crate::continuity::split_sentences;
use crate::sources::extract_cite_keys;

/// One sentence flagged as an uncited claim.
#[derive(Debug, Clone)]
pub struct UncitedClaim {
    pub sentence: String,
    /// Why it reads as a claim: any of `statistic` / `date` / `quotation` / `attribution`.
    pub signals: Vec<&'static str>,
}

/// Scan a paragraph body for uncited claims.
pub fn scan(body: &str) -> Vec<UncitedClaim> {
    let mut out = Vec::new();
    for sentence in split_sentences(body) {
        // A sentence that already cites something is sourced.
        if !extract_cite_keys(&sentence).is_empty() {
            continue;
        }
        let signals = claim_signals(&sentence);
        if !signals.is_empty() {
            out.push(UncitedClaim { sentence: sentence.trim().to_string(), signals });
        }
    }
    out
}

/// Emit an uncited-claim finding to the Output pane, anchored on `para_id`.
pub fn emit_finding(para_id: uuid::Uuid, claim: &UncitedClaim) {
    use crate::pane::output::{emit, kinds, Lifetime, Message, Severity};
    let msg = Message::new(
        kinds::SOURCING,
        Severity::Warning,
        Lifetime::UntilActedOn,
        serde_json::json!({
            "text": format!("uncited claim ({})", claim.signals.join(", ")),
            "category": "sourcing",
            "sentence": claim.sentence,
            "signals": claim.signals.join(", "),
        }),
    )
    .with_source_paragraph(para_id);
    emit(&msg);
}

fn claim_signals(s: &str) -> Vec<&'static str> {
    let mut sig = Vec::new();
    if has_statistic(s) {
        sig.push("statistic");
    }
    if has_year(s) {
        sig.push("date");
    }
    if has_quotation(s) {
        sig.push("quotation");
    }
    if has_attribution(s) {
        sig.push("attribution");
    }
    sig
}

/// A percentage, a comma-grouped number, or a number before a scale word.
fn has_statistic(s: &str) -> bool {
    // Percentage: a digit immediately before `%`.
    for (i, c) in s.char_indices() {
        if c == '%' && s[..i].chars().last().is_some_and(|p| p.is_ascii_digit()) {
            return true;
        }
    }
    if has_grouped_number(s) {
        return true;
    }
    let lower = s.to_lowercase();
    for unit in ["percent", "million", "billion", "trillion", "thousand"] {
        if let Some(pos) = lower.find(unit) {
            let before = lower[..pos].trim_end();
            if before.chars().last().is_some_and(|c| c.is_ascii_digit()) || ends_with_number_word(before) {
                return true;
            }
        }
    }
    false
}

/// True when the text contains a `d,ddd` grouped number (e.g. `1,200`, `3,400,000`).
fn has_grouped_number(s: &str) -> bool {
    let ch: Vec<char> = s.chars().collect();
    for i in 1..ch.len() {
        if ch[i] == ','
            && ch[i - 1].is_ascii_digit()
            && i + 3 < ch.len()
            && ch[i + 1].is_ascii_digit()
            && ch[i + 2].is_ascii_digit()
            && ch[i + 3].is_ascii_digit()
        {
            return true;
        }
    }
    false
}

fn ends_with_number_word(before: &str) -> bool {
    const WORDS: &[&str] = &[
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "hundred", "several", "many", "few", "dozens", "hundreds",
    ];
    before
        .rsplit(|c: char| !c.is_ascii_alphabetic())
        .find(|w| !w.is_empty())
        .is_some_and(|w| WORDS.contains(&w))
}

/// A standalone 4-digit year in 1000–2099.
fn has_year(s: &str) -> bool {
    for tok in s.split(|c: char| !c.is_ascii_digit()) {
        if tok.len() == 4 {
            if let Ok(n) = tok.parse::<u32>() {
                if (1000..=2099).contains(&n) {
                    return true;
                }
            }
        }
    }
    false
}

/// A quotation (straight or curly) of at least six words — a quote wants attribution.
fn has_quotation(s: &str) -> bool {
    quote_between(s, '"', '"') || quote_between(s, '\u{201C}', '\u{201D}')
}

fn quote_between(s: &str, open: char, close: char) -> bool {
    let Some(start) = s.find(open) else { return false };
    let after = &s[start + open.len_utf8()..];
    let Some(end) = after.find(close) else { return false };
    after[..end].split_whitespace().count() >= 6
}

/// A phrase that announces a sourced finding.
fn has_attribution(s: &str) -> bool {
    let l = s.to_lowercase();
    const PHRASES: &[&str] = &[
        "according to",
        "a study",
        "the study",
        "studies show",
        "studies have shown",
        "research shows",
        "research suggests",
        "researchers found",
        "researchers say",
        "data show",
        "data indicate",
        "data suggest",
        "a survey",
        "surveyed",
        "estimated",
        "estimates that",
        "statistics",
        "reported that",
        "scientists say",
        "scientists found",
        "experts say",
        "experts believe",
    ];
    PHRASES.iter().any(|p| l.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_statistic_date_quote_attribution() {
        let body = "Deaths fell by 42% between 1946 and 1991. \
                    According to the report, the trend held. \
                    She said \"the peace has quietly held for decades now\" at the summit. \
                    The population reached 3,400,000 people.";
        let claims = scan(body);
        assert_eq!(claims.len(), 4, "got: {claims:?}");
        assert!(claims[0].signals.contains(&"statistic"));
        assert!(claims[0].signals.contains(&"date"));
        assert!(claims[1].signals.contains(&"attribution"));
        assert!(claims[2].signals.contains(&"quotation"));
        assert!(claims[3].signals.contains(&"statistic"));
    }

    #[test]
    fn a_cited_sentence_is_not_flagged() {
        let body = "Deaths fell by 42% between 1946 and 1991 @pinker2011.";
        assert!(scan(body).is_empty());
    }

    #[test]
    fn curly_quote_of_six_words_flags() {
        let body = "He wrote \u{201C}the long peace has quietly held for decades\u{201D}.";
        let claims = scan(body);
        assert_eq!(claims.len(), 1);
        assert!(claims[0].signals.contains(&"quotation"));
    }

    #[test]
    fn ordinary_prose_and_small_numbers_are_quiet() {
        let body = "She opened the door and walked in. There were three chairs. \
                    The argument has two parts, and we take them in turn.";
        assert!(scan(body).is_empty(), "got: {:?}", scan(body));
    }
}
