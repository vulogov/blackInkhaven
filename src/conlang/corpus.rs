//! CORPUS-1 (Wave 4, Track B) — a concordance + frequency engine over the
//! language's stored interlinear texts.
//!
//! The Annotation Workbench gathers glossed texts; this reads them back as a
//! *corpus* — a body of usage you can query. It answers the two questions a corpus
//! tool exists for: how *often* does each word occur (frequency), and *where* and
//! in what context (a KWIC concordance) — plus the descriptive statistics real
//! usage data unlocks (type–token ratio, the word-frequency Zipf fit) that a bare
//! dictionary cannot give. Because each token carries its lemma from the gloss,
//! every query works on surface forms or on lemmas (grouping a root's inflected
//! forms). Pure and deterministic.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::conlang::igt::Igt;

/// One token of the corpus: its surface form and its lemma — the root headword,
/// or the surface itself when the word was never recognised. Both lowercased.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub surface: String,
    pub lemma: String,
}

/// One text of the corpus, as an ordered token stream.
#[derive(Debug, Clone, PartialEq)]
pub struct CorpusText {
    pub name: String,
    pub tokens: Vec<Token>,
}

/// The corpus: every stored interlinear as a token stream.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Corpus {
    pub texts: Vec<CorpusText>,
}

/// One keyword-in-context concordance line.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Kwic {
    pub text: String,
    pub left: String,
    pub keyword: String,
    pub right: String,
}

/// One collocate of a target word: how often it falls within the target's window,
/// how often it occurs overall, and how *distinctive* that association is.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Collocate {
    pub word: String,
    /// Times it appears within the window of the target.
    pub cooccur: usize,
    /// Its total occurrences in the corpus.
    pub total: usize,
    /// Pointwise mutual information — how much more the word appears in the
    /// target's context than its corpus-wide rate predicts (0 = as expected,
    /// positive = attracted, negative = repelled).
    pub pmi: f64,
}

/// Descriptive statistics over the corpus.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct CorpusStats {
    pub texts: usize,
    /// Total word tokens.
    pub tokens: usize,
    /// Distinct surface forms.
    pub types: usize,
    /// Distinct lemmas.
    pub lemmas: usize,
    /// Type–token ratio (`types / tokens`).
    pub ttr: f64,
    /// Slope of the word-frequency Zipf fit (≈ −1 for a Zipfian distribution).
    pub zipf_slope: f64,
    pub zipf_r2: f64,
}

impl Corpus {
    /// Build the corpus from the language's stored interlinear texts.
    pub fn from_texts(texts: &[(String, Igt)]) -> Corpus {
        let texts = texts
            .iter()
            .map(|(name, igt)| {
                let tokens = igt
                    .words
                    .iter()
                    .map(|w| {
                        let surface = w.surface.to_lowercase();
                        let lemma = w.root.as_ref().map(|r| r.to_lowercase()).unwrap_or_else(|| surface.clone());
                        Token { surface, lemma }
                    })
                    .collect();
                CorpusText { name: name.clone(), tokens }
            })
            .collect();
        Corpus { texts }
    }

    /// Frequency list, most frequent first (ties broken alphabetically). `by_lemma`
    /// counts lemmas (grouping a root's inflected forms), else surface forms.
    pub fn frequency(&self, by_lemma: bool) -> Vec<(String, usize)> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for t in &self.texts {
            for tok in &t.tokens {
                let key = if by_lemma { &tok.lemma } else { &tok.surface };
                *counts.entry(key.clone()).or_default() += 1;
            }
        }
        let mut v: Vec<(String, usize)> = counts.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        v
    }

    /// KWIC concordance: every occurrence of `target` — matched on the lemma when
    /// `by_lemma`, else the surface, case-insensitively — with up to `window`
    /// words of context on each side.
    pub fn concordance(&self, target: &str, by_lemma: bool, window: usize) -> Vec<Kwic> {
        let needle = target.to_lowercase();
        let mut out = Vec::new();
        for t in &self.texts {
            for (i, tok) in t.tokens.iter().enumerate() {
                let hay = if by_lemma { &tok.lemma } else { &tok.surface };
                if hay != &needle {
                    continue;
                }
                let lo = i.saturating_sub(window);
                let hi = (i + 1 + window).min(t.tokens.len());
                let join = |slice: &[Token]| slice.iter().map(|x| x.surface.as_str()).collect::<Vec<_>>().join(" ");
                out.push(Kwic {
                    text: t.name.clone(),
                    left: join(&t.tokens[lo..i]),
                    keyword: tok.surface.clone(),
                    right: join(&t.tokens[i + 1..hi]),
                });
            }
        }
        out
    }

    /// The collocates of `target`: the words that fall within its `window` across
    /// the corpus, ranked by co-occurrence and scored by PMI (so a distinctive
    /// neighbour outranks a merely-frequent one). Matches on lemma when `by_lemma`,
    /// else surface; the target word itself is excluded from its own list.
    pub fn collocates(&self, target: &str, by_lemma: bool, window: usize) -> Vec<Collocate> {
        let needle = target.to_lowercase();
        let key = |t: &Token| if by_lemma { t.lemma.clone() } else { t.surface.clone() };

        let totals: BTreeMap<String, usize> = self.frequency(by_lemma).into_iter().collect();
        let n: usize = totals.values().sum();

        // Count each neighbour's occurrences within the target's window.
        let mut cooccur: BTreeMap<String, usize> = BTreeMap::new();
        let mut context_total = 0usize;
        for t in &self.texts {
            for (i, tok) in t.tokens.iter().enumerate() {
                if key(tok) != needle {
                    continue;
                }
                let lo = i.saturating_sub(window);
                let hi = (i + 1 + window).min(t.tokens.len());
                for (pos, nb) in (lo..hi).zip(&t.tokens[lo..hi]) {
                    if pos == i {
                        continue;
                    }
                    let k = key(nb);
                    if k == needle {
                        continue; // the target's own repetitions aren't collocates
                    }
                    *cooccur.entry(k).or_default() += 1;
                    context_total += 1;
                }
            }
        }

        let mut out: Vec<Collocate> = cooccur
            .into_iter()
            .map(|(word, c)| {
                let total = totals.get(&word).copied().unwrap_or(0);
                // PMI: how enriched the word is in the target's context vs the
                // corpus overall — log2( p(w|context) / p(w) ).
                let pmi = if context_total > 0 && total > 0 && n > 0 {
                    ((c as f64 * n as f64) / (context_total as f64 * total as f64)).log2()
                } else {
                    0.0
                };
                Collocate { word, cooccur: c, total, pmi }
            })
            .collect();
        out.sort_by(|a, b| {
            b.cooccur
                .cmp(&a.cooccur)
                .then(b.pmi.partial_cmp(&a.pmi).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.word.cmp(&b.word))
        });
        out
    }

    /// Descriptive statistics over the corpus.
    pub fn stats(&self) -> CorpusStats {
        let by_surface = self.frequency(false);
        let tokens: usize = by_surface.iter().map(|(_, c)| c).sum();
        let types = by_surface.len();
        let lemmas = self.frequency(true).len();
        let ttr = if tokens == 0 { 0.0 } else { types as f64 / tokens as f64 };
        let freqs_desc: Vec<usize> = by_surface.iter().map(|(_, c)| *c).collect();
        let (zipf_slope, zipf_r2) = crate::conlang::metrics::zipf_fit(&freqs_desc);
        CorpusStats { texts: self.texts.len(), tokens, types, lemmas, ttr, zipf_slope, zipf_r2 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::igt::IgtWord;

    /// Build a stored-text entry from `(surface, root)` word pairs.
    fn text(name: &str, words: &[(&str, Option<&str>)]) -> (String, Igt) {
        let igt = Igt {
            text: words.iter().map(|(s, _)| *s).collect::<Vec<_>>().join(" "),
            words: words
                .iter()
                .map(|(s, r)| IgtWord {
                    surface: s.to_string(),
                    root: r.map(String::from),
                    gloss: None,
                    segments: vec![],
                })
                .collect(),
            translation: String::new(),
            recognised: words.iter().filter(|(_, r)| r.is_some()).count(),
        };
        (name.to_string(), igt)
    }

    #[test]
    fn frequency_counts_surface_forms() {
        let c = Corpus::from_texts(&[text(
            "t1",
            &[("kata", Some("kata")), ("kata", Some("kata")), ("nilo", None)],
        )]);
        let f = c.frequency(false);
        assert_eq!(f[0], ("kata".to_string(), 2));
        assert_eq!(f[1], ("nilo".to_string(), 1));
    }

    #[test]
    fn lemma_frequency_groups_inflected_forms() {
        // "kata" and "katai" share the lemma "kata".
        let c = Corpus::from_texts(&[text("t1", &[("kata", Some("kata")), ("katai", Some("kata"))])]);
        let by_surface = c.frequency(false);
        assert_eq!(by_surface.len(), 2); // two distinct surfaces
        let by_lemma = c.frequency(true);
        assert_eq!(by_lemma, vec![("kata".to_string(), 2)]);
    }

    #[test]
    fn concordance_shows_context_around_the_keyword() {
        let c = Corpus::from_texts(&[text(
            "t1",
            &[("a", None), ("b", None), ("mira", Some("mira")), ("c", None), ("d", None)],
        )]);
        let k = c.concordance("mira", false, 1);
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].left, "b");
        assert_eq!(k[0].keyword, "mira");
        assert_eq!(k[0].right, "c");
        assert_eq!(k[0].text, "t1");
    }

    #[test]
    fn concordance_by_lemma_finds_inflected_forms() {
        // Searching the lemma finds an inflected surface form.
        let c = Corpus::from_texts(&[text("t1", &[("x", None), ("katai", Some("kata"))])]);
        let by_surface = c.concordance("kata", false, 2);
        assert!(by_surface.is_empty(), "surface search shouldn't find the inflected form");
        let by_lemma = c.concordance("kata", true, 2);
        assert_eq!(by_lemma.len(), 1);
        assert_eq!(by_lemma[0].keyword, "katai");
    }

    #[test]
    fn stats_report_tokens_types_and_ttr() {
        let c = Corpus::from_texts(&[
            text("t1", &[("kata", Some("kata")), ("kata", Some("kata")), ("nilo", None)]),
            text("t2", &[("mira", Some("mira"))]),
        ]);
        let s = c.stats();
        assert_eq!(s.texts, 2);
        assert_eq!(s.tokens, 4);
        assert_eq!(s.types, 3); // kata, nilo, mira
        assert!((s.ttr - 3.0 / 4.0).abs() < 1e-9);
    }

    #[test]
    fn collocates_rank_neighbours_by_co_occurrence() {
        // "mira" at positions 1 and 3; with window 1 its neighbours are a, b, b, c.
        let c = Corpus::from_texts(&[text(
            "t1",
            &[("a", None), ("mira", Some("mira")), ("b", None), ("mira", Some("mira")), ("c", None)],
        )]);
        let cols = c.collocates("mira", false, 1);
        // `b` sits beside both occurrences → co-occurs twice, ranked first.
        assert_eq!(cols[0].word, "b");
        assert_eq!(cols[0].cooccur, 2);
        assert!(cols.iter().any(|x| x.word == "a" && x.cooccur == 1));
        assert!(cols.iter().any(|x| x.word == "c" && x.cooccur == 1));
        // The target itself is not among its collocates.
        assert!(!cols.iter().any(|x| x.word == "mira"));
    }

    #[test]
    fn collocates_exclude_the_target_and_have_finite_pmi() {
        let c = Corpus::from_texts(&[text("t1", &[("mira", Some("mira")), ("mira", Some("mira"))])]);
        // Only the target beside itself → no collocates.
        assert!(c.collocates("mira", false, 1).is_empty());
        // A real neighbour gets a finite PMI.
        let c2 = Corpus::from_texts(&[text("t1", &[("mira", Some("mira")), ("kata", Some("kata"))])]);
        let cols = c2.collocates("mira", false, 1);
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].word, "kata");
        assert!(cols[0].pmi.is_finite());
    }

    #[test]
    fn an_empty_corpus_is_well_behaved() {
        let c = Corpus::from_texts(&[]);
        let s = c.stats();
        assert_eq!(s.tokens, 0);
        assert_eq!(s.ttr, 0.0);
        assert!(c.concordance("x", false, 3).is_empty());
    }
}
