//! RESRCH-1 (R-P14) — `/verify` claim extraction + the confidence probe. The
//! heuristic picks sentences that carry a *specific* factual claim (a 4-digit
//! year, a quoted title, a numeric quantity with a unit, or a proper noun used
//! mid-sentence) so the probe only spends tokens on checkable statements.

/// A verdict line parsed from the probe response.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub(super) fn parse(line: &str) -> Option<Confidence> {
        let up = line.trim_start().to_ascii_uppercase();
        if up.starts_with("HIGH") {
            Some(Confidence::High)
        } else if up.starts_with("MEDIUM") {
            Some(Confidence::Medium)
        } else if up.starts_with("LOW") {
            Some(Confidence::Low)
        } else {
            None
        }
    }
}

/// The most claims one `/verify` probes at once — beyond this the single LLM call
/// grows unbounded with the response length, for diminishing signal.
pub(super) const MAX_CLAIMS: usize = 12;

/// Split text into sentences (naive: on `.?!` boundaries) and keep the ones that
/// look like specific factual claims of at least `min_words` words, capped at
/// [`MAX_CLAIMS`].
pub(super) fn extract_claims(text: &str, min_words: usize) -> Vec<String> {
    let mut claims = Vec::new();
    for sentence in split_sentences(text) {
        let trimmed = sentence.trim();
        if trimmed.split_whitespace().count() < min_words {
            continue;
        }
        if is_claim(trimmed) {
            claims.push(trimmed.to_string());
            if claims.len() >= MAX_CLAIMS {
                break;
            }
        }
    }
    claims
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '.' | '?' | '!') {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// Whether a sentence carries a specific, checkable claim.
fn is_claim(s: &str) -> bool {
    has_four_digit_year(s) || s.contains('"') || has_quantity_with_unit(s) || has_midsentence_proper_noun(s)
}

fn has_four_digit_year(s: &str) -> bool {
    let bytes: Vec<char> = s.chars().collect();
    let mut run = 0;
    for c in &bytes {
        if c.is_ascii_digit() {
            run += 1;
            if run == 4 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// A number immediately followed (optionally past a space) by a unit-ish token
/// (a letter run or `%`, `°`). e.g. "190,000 m³", "97 CE", "0.02%".
fn has_quantity_with_unit(s: &str) -> bool {
    let tokens: Vec<&str> = s.split_whitespace().collect();
    for (i, tok) in tokens.iter().enumerate() {
        let has_digit = tok.chars().any(|c| c.is_ascii_digit());
        if !has_digit {
            continue;
        }
        if tok.contains('%') || tok.contains('°') {
            return true;
        }
        // number then a unit word as the next token (short, alphabetic-ish).
        if let Some(next) = tokens.get(i + 1) {
            let unitish = next.chars().all(|c| c.is_alphabetic() || "³²/°%.".contains(c));
            if unitish && next.len() <= 6 && next.chars().any(|c| c.is_alphabetic() || "³²°%".contains(c)) {
                return true;
            }
        }
    }
    false
}

/// A capitalised word that is not the first token of the sentence (a named
/// entity rather than a sentence-initial capital).
fn has_midsentence_proper_noun(s: &str) -> bool {
    s.split_whitespace()
        .skip(1)
        .any(|w| w.chars().next().is_some_and(|c| c.is_uppercase()) && w.chars().skip(1).any(|c| c.is_lowercase()))
}

/// The confidence-probe system prompt (RFC §14).
pub(super) const PROBE_SYSTEM: &str =
    "You are assessing your own confidence in specific factual claims. \
     Do not explain a claim. Do not generate supporting evidence. \
     Assess only: is each claim reliably in your training data? \
     For each numbered claim respond on its own line with exactly one of:\n\
     HIGH — <one phrase reason>\n\
     MEDIUM — <one phrase reason>\n\
     LOW — <one phrase reason>";

/// Build the user message: the numbered claim list.
pub(super) fn probe_user(claims: &[String]) -> String {
    let mut s = String::from("Claims:\n");
    for (i, c) in claims.iter().enumerate() {
        s.push_str(&format!("{}. {}\n", i + 1, c));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_year_quote_quantity_propernoun() {
        let text = "This is short. The Aqua Claudia was documented in 97 CE. \
                    Frontinus wrote De Aquaeductu. It carried 190,000 m³ per day. \
                    Nothing specific here at all really though okay.";
        let claims = extract_claims(text, 4);
        assert!(claims.iter().any(|c| c.contains("97 CE")));
        assert!(claims.iter().any(|c| c.contains("Frontinus")));
        assert!(claims.iter().any(|c| c.contains("190,000")));
        // The vague closing sentence is not flagged.
        assert!(!claims.iter().any(|c| c.contains("Nothing specific")));
    }

    #[test]
    fn confidence_parsing() {
        assert_eq!(Confidence::parse("HIGH — well documented"), Some(Confidence::High));
        assert_eq!(Confidence::parse("  low — uncertain"), Some(Confidence::Low));
        assert_eq!(Confidence::parse("maybe"), None);
    }

    #[test]
    fn probe_user_numbers_claims() {
        let u = probe_user(&["a".into(), "b".into()]);
        assert!(u.contains("1. a"));
        assert!(u.contains("2. b"));
    }
}
