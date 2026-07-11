//! ARG-1 — the argument outline. Pure pieces: the finding model and the parser for
//! the model's `CLAIM|||…` / `ORPHAN|||…` response, with a quote-verification guard
//! that drops any claim not actually present in the chapter (the anti-hallucination
//! bar). The AI call itself lives in `crate::cli::argue`.

/// One central claim and the support the chapter gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgClaim {
    /// The claim, quoted from the chapter.
    pub claim: String,
    /// Its support: a `@key`, a reasoning phrase, or empty when unsupported.
    pub support: String,
    /// Whether the chapter backs the claim at all.
    pub supported: bool,
}

/// A citation that supports no identified claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanCite {
    pub key: String,
    pub detail: String,
}

/// Parse the model's response into claims + orphan citations. A `CLAIM` whose text is
/// not (fuzzily) present in `chapter_prose` is dropped — the guard against the model
/// inventing an argument the author never made.
pub fn parse_argument(raw: &str, chapter_prose: &str) -> (Vec<ArgClaim>, Vec<OrphanCite>) {
    let prose_lc = chapter_prose.to_lowercase();
    let mut claims = Vec::new();
    let mut orphans = Vec::new();

    for line in raw.lines() {
        let parts: Vec<&str> = line.trim().splitn(3, "|||").map(str::trim).collect();
        match parts.first().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("CLAIM") => {
                let Some(claim) = parts.get(1).map(|s| s.trim_matches('"').trim()) else { continue };
                if claim.is_empty() || !quote_present(&claim.to_lowercase(), &prose_lc) {
                    continue;
                }
                let support = parts.get(2).map(|s| s.trim().to_string()).unwrap_or_default();
                let supported = !support.is_empty()
                    && !support.eq_ignore_ascii_case("none")
                    && !support.eq_ignore_ascii_case("(none)");
                claims.push(ArgClaim { claim: claim.to_string(), support, supported });
            }
            Some("ORPHAN") => {
                let Some(raw_key) = parts.get(1) else { continue };
                let key = raw_key.trim_start_matches('@').trim().to_string();
                if key.is_empty() {
                    continue;
                }
                orphans.push(OrphanCite {
                    key,
                    detail: parts.get(2).map(|s| s.to_string()).unwrap_or_default(),
                });
            }
            _ => {}
        }
    }
    (claims, orphans)
}

/// Whether a claim is present in the prose: at least 60% of its significant words
/// (≥4 chars) appear, else an exact substring for very short claims.
fn quote_present(claim_lc: &str, prose_lc: &str) -> bool {
    let words: Vec<&str> = claim_lc
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4)
        .collect();
    if words.is_empty() {
        return prose_lc.contains(claim_lc.trim());
    }
    let hits = words.iter().filter(|w| prose_lc.contains(**w)).count();
    hits * 100 >= words.len() * 60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claims_and_orphans_and_verifies_quotes() {
        let prose = "Deaths from war fell across the period. The decline is real, not an \
                     artefact of population growth. Peace is self-reinforcing.";
        let raw = "CLAIM ||| Deaths from war fell across the period ||| @pinker2011\n\
                   CLAIM ||| \"Peace is self-reinforcing\" ||| NONE\n\
                   CLAIM ||| Aliens caused the collapse of ancient empires ||| @xfiles\n\
                   ORPHAN ||| @keegan1993 ||| cited but supports no claim\n\
                   some preamble the model added";
        let (claims, orphans) = parse_argument(raw, prose);
        // The alien claim is absent from the prose → dropped by the quote guard.
        assert_eq!(claims.len(), 2, "got {claims:?}");
        assert!(claims[0].supported);
        assert_eq!(claims[0].support, "@pinker2011");
        assert!(!claims[1].supported, "NONE → unsupported");
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].key, "keegan1993");
    }

    #[test]
    fn quote_guard_rejects_paraphrase() {
        let prose = "The homicide rate has fallen steadily since the medieval period.";
        // A paraphrase sharing few words is dropped.
        let (claims, _) = parse_argument("CLAIM ||| Civilisation tames our violent instincts ||| NONE", prose);
        assert!(claims.is_empty());
        // A faithful quote survives.
        let (claims, _) = parse_argument("CLAIM ||| the homicide rate has fallen steadily ||| NONE", prose);
        assert_eq!(claims.len(), 1);
    }
}
