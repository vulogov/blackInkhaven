//! NARR-1 — language-sensitive metrics built on the embedded lexicons:
//! modal/epistemic-hedging density, free-indirect-discourse interiority ratio
//! (+ German *erlebte Rede* particle density), and Tier-2 sensory channel
//! balance. All return `None` for unsupported (`Other`) languages.

use super::{CompiledLexicon, ProseLanguage, SensoryChannel, tokenize};

/// Proportion of word tokens that are epistemic-hedging hits (unigrams + bigram
/// / trigram phrases). `None` for unsupported languages.
pub(crate) fn modal_density(
    text: &str,
    lang: &ProseLanguage,
    lx: &CompiledLexicon,
) -> Option<f32> {
    if !lang.is_supported() {
        return None;
    }
    let tokens = tokenize(text);
    if tokens.is_empty() {
        return Some(0.0);
    }
    let refs: Vec<&str> = tokens.iter().map(String::as_str).collect();
    Some(lx.count_modal_tokens(&refs) as f32 / refs.len() as f32)
}

/// FID interiority. Returns `(ratio, de_particle_density)`:
/// - `ratio` — proportion of sentences accessing inner life. A sentence with a
///   reporting-verb phrase contributes 1.0; for German, a non-interrogative
///   sentence with an *erlebte Rede* modal particle (but no reporting phrase)
///   contributes 0.5 (weaker signal).
/// - `de_particle_density` — `Some` only for German: particle hits in
///   non-interrogative sentences ÷ total tokens (its own reported metric).
///
/// Both are `None` for unsupported languages.
pub(crate) fn interiority(
    sentences: &[String],
    lang: &ProseLanguage,
    lx: &CompiledLexicon,
) -> (Option<f32>, Option<f32>) {
    if !lang.is_supported() {
        return (None, None);
    }
    let is_de = matches!(lang, ProseLanguage::De);
    if sentences.is_empty() {
        return (Some(0.0), is_de.then_some(0.0));
    }
    let mut contribution = 0.0f64;
    let mut particle_hits = 0usize;
    let mut total_tokens = 0usize;

    for s in sentences {
        let toks = tokenize(s);
        let refs: Vec<&str> = toks.iter().map(String::as_str).collect();
        total_tokens += refs.len();
        let interrogative = s.trim_end().ends_with('?');
        // erlebte Rede particles only signal FID in declarative sentences.
        let particles = if is_de && !interrogative {
            lx.erlebte_particle_count(&refs)
        } else {
            0
        };
        particle_hits += particles;

        if lx.sentence_has_interiority(&refs) {
            contribution += 1.0;
        } else if particles > 0 {
            contribution += 0.5;
        }
    }

    let ratio = (contribution / sentences.len() as f64) as f32;
    let de_density = is_de.then(|| {
        if total_tokens == 0 {
            0.0
        } else {
            particle_hits as f32 / total_tokens as f32
        }
    });
    (Some(ratio), de_density)
}

fn channel_index(c: SensoryChannel) -> usize {
    match c {
        SensoryChannel::Visual => 0,
        SensoryChannel::Auditory => 1,
        SensoryChannel::Olfactory => 2,
        SensoryChannel::Tactile => 3,
        SensoryChannel::Kinesthetic => 4,
    }
}

/// Tier-2 sensory channel balance: `[visual, auditory, olfactory, tactile,
/// kinesthetic]`, each the proportion of tokens in that channel (sum ≤ 1.0; the
/// remainder is unclassified). `None` for unsupported languages.
pub(crate) fn sensory_balance(
    text: &str,
    lang: &ProseLanguage,
    lx: &CompiledLexicon,
) -> Option<[f32; 5]> {
    if !lang.is_supported() {
        return None;
    }
    let tokens = tokenize(text);
    if tokens.is_empty() {
        return Some([0.0; 5]);
    }
    let mut counts = [0usize; 5];
    for t in &tokens {
        if let Some(ch) = lx.sensory_channel(t) {
            counts[channel_index(ch)] += 1;
        }
    }
    let total = tokens.len() as f32;
    Some([
        counts[0] as f32 / total,
        counts[1] as f32 / total,
        counts[2] as f32 / total,
        counts[3] as f32 / total,
        counts[4] as f32 / total,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseLanguage::*;
    use crate::prose::{CompiledLexicon, segment};

    fn lx(l: &crate::prose::ProseLanguage) -> CompiledLexicon {
        CompiledLexicon::for_language(l)
    }

    #[test]
    fn modal_density_basic() {
        // 3 hits (might, perhaps, could) over 12 tokens.
        let text = "She might have known, perhaps, but could not be sure now.";
        let d = modal_density(text, &En, &lx(&En)).unwrap();
        let n = tokenize(text).len();
        assert!((d - 3.0 / n as f32).abs() < 1e-6, "{d} over {n}");
        // Unsupported → None.
        let other = Other("it".into());
        assert_eq!(modal_density(text, &other, &lx(&other)), None);
    }

    #[test]
    fn interiority_en_ratio() {
        let sents: Vec<String> = vec![
            "She thought it was over.".into(), // FID 1.0
            "The wind was cold.".into(),       // 0
        ];
        let (r, de) = interiority(&sents, &En, &lx(&En));
        assert!((r.unwrap() - 0.5).abs() < 1e-6);
        assert_eq!(de, None); // particle density is DE-only
    }

    #[test]
    fn interiority_de_particles_and_density() {
        // Sentence 1: reporting verb → 1.0. Sentence 2: declarative + particles
        // (no reporting phrase) → 0.5. Average 0.75.
        let sents: Vec<String> = vec![
            "Sie dachte an den See.".into(),
            "Das war ja doch klar.".into(),
        ];
        let (r, de) = interiority(&sents, &De, &lx(&De));
        assert!((r.unwrap() - 0.75).abs() < 1e-6, "{:?}", r);
        // Particle density is Some and > 0 (ja, doch in the declarative).
        assert!(de.unwrap() > 0.0);
        // Interrogative particles don't count toward density.
        let q: Vec<String> = vec!["War das ja doch klar?".into()];
        let (_, de_q) = interiority(&q, &De, &lx(&De));
        assert_eq!(de_q.unwrap(), 0.0);
    }

    #[test]
    fn interiority_other_languages() {
        for (lang, sent) in [
            (Ru, "ей казалось, что всё кончено."),
            (Fr, "elle pensait à lui."),
            (Es, "ella pensaba en silencio."),
        ] {
            let s = vec![sent.to_string()];
            let (r, _) = interiority(&s, &lang, &lx(&lang));
            assert!((r.unwrap() - 1.0).abs() < 1e-6, "{}", lang.as_code());
        }
    }

    #[test]
    fn sensory_balance_ratios() {
        // "shadow" visual, "murmur"/"silence" auditory, over a known token count.
        let text = "A shadow fell and a murmur broke the silence here.";
        let b = sensory_balance(text, &En, &lx(&En)).unwrap();
        let n = tokenize(text).len() as f32;
        assert!((b[0] - 1.0 / n).abs() < 1e-6, "visual {b:?}");
        assert!((b[1] - 2.0 / n).abs() < 1e-6, "auditory {b:?}");
        assert!(b.iter().sum::<f32>() <= 1.0 + 1e-6);
        assert_eq!(sensory_balance(text, &Other("it".into()), &lx(&Other("it".into()))), None);
    }

    #[test]
    fn metrics_run_over_segmented_chapter() {
        // Smoke: the language metrics compose with the N-P1 splitter.
        let text = "Er wurde gerufen. Sie dachte nach. Das war ja klar.";
        let sents = segment::split_sentences(text, &De);
        assert_eq!(sents.len(), 3);
        assert!(interiority(&sents, &De, &lx(&De)).0.unwrap() > 0.0);
        assert!(modal_density(text, &De, &lx(&De)).unwrap() >= 0.0);
    }
}
