//! LING-1 L-P2 — `inkhaven language metrics <lang>`: deterministic quantitative
//! metrics over a language's lexicon + phonology (entropy, Zipf fit, phonotactic
//! saturation, mora weight). The information-theoretic complement to
//! `language stats` (descriptive counts). Read-only; `--json` for machine use.

use std::path::Path;

use crate::conlang::metrics::LanguageMetrics;
use crate::conlang::pairs::PairsReport;
use crate::error::{Error, Result};

use super::*;

pub(crate) fn metrics(project: &Path, language: &str, json: bool) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let m = crate::conlang::metrics::metrics(&phon, &entries);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&m)
                .map_err(|e| Error::Store(format!("serializing metrics: {e}")))?
        );
        return Ok(());
    }

    print_report(language, &m);
    Ok(())
}

fn print_report(language: &str, m: &LanguageMetrics) {
    println!("language metrics · {language}");
    if m.analyzable_words == 0 {
        println!("  (no analyzable words — add dictionary entries that parse as the language's phonemes)");
        return;
    }
    println!(
        "  corpus    · {} analyzable word(s), {} segments",
        m.analyzable_words, m.total_segments
    );
    println!(
        "  entropy   · {:.2} bits (max {:.2}) · evenness {:.0}% · perplexity {:.1}",
        m.phoneme_entropy,
        m.phoneme_entropy_max,
        m.phoneme_evenness * 100.0,
        m.phoneme_perplexity,
    );
    println!(
        "  zipf      · slope {:.2} (≈−1 is Zipfian) · fit R² {:.2}",
        m.zipf_slope, m.zipf_r2
    );
    println!(
        "  syllables · {} attested / {} possible · saturation {:.0}%",
        m.attested_syllables,
        m.possible_syllables,
        m.syllable_saturation * 100.0,
    );
    println!(
        "  prosody   · {:.2} moras/word · {:.0}% heavy syllables",
        m.mean_moras,
        m.heavy_ratio * 100.0,
    );
}

/// LING-1 Wave-2 — `inkhaven language pairs <lang>`: minimal pairs in the lexicon
/// and the distinctive feature each turns on (the functional load of the
/// language's contrasts).
pub(crate) fn pairs(project: &Path, language: &str, limit: usize, json: bool) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let report = crate::conlang::pairs::minimal_pairs(&phon, &entries, limit.max(1));

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| Error::Store(format!("serializing pairs: {e}")))?
        );
        return Ok(());
    }

    print_pairs(language, &report);
    Ok(())
}

fn print_pairs(language: &str, r: &PairsReport) {
    println!("minimal pairs · {language}");
    if r.analyzable_words == 0 {
        println!("  (no analyzable words — define a phoneme inventory and add dictionary entries)");
        return;
    }
    println!(
        "  {} minimal pair(s) across {} analyzable words",
        r.pair_count, r.analyzable_words
    );
    if !r.contrast_load.is_empty() {
        println!("  functional load (single-feature contrasts):");
        let max = r.contrast_load.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
        for (feat, count) in &r.contrast_load {
            let bar = "█".repeat(((*count * 20) / max).max(1));
            println!("      {feat:<12} {bar} {count}");
        }
    }
    if r.complex_contrasts > 0 {
        println!("  {} pair(s) contrast in more than one feature", r.complex_contrasts);
    }
    if !r.pairs.is_empty() {
        println!("  examples:");
        for p in &r.pairs {
            let contrast = if p.features.is_empty() {
                "(outside the feature matrix)".to_string()
            } else {
                format!("[{}]", p.features.join(", "))
            };
            println!("      {} / {}   {}~{} {}", p.a, p.b, p.seg_a, p.seg_b, contrast);
        }
    }
}
