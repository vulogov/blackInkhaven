//! CHORUS-1 (CH-P2) — the distinctiveness matrix (the flagship's headline).
//!
//! Do any two characters read *identically*? Each voice becomes a feature vector
//! over the comparable, language-safe metrics; the vectors are **z-scored across
//! the cast** (genre-relative — the baseline is this book's own spread, so a
//! terse thriller and a florid literary novel are judged on their own terms);
//! and the distance between two voices is the **RMS of their per-metric
//! z-differences** (≈ "how many pooled std-devs apart per metric, on average").
//! Two voices below the threshold — and not in the author's ignore list — are
//! flagged **indistinguishable**.
//!
//! Only `Confidence::is_comparable` voices take part; a character with a handful
//! of lines is noise, and both the z-baseline and the flags exclude them.

use super::voices::CharacterVoice;
use crate::prose::VoiceProfile;

/// The comparable, language-safe axes of a voice. The language-sensitive axes
/// (hedging, interiority) default to 0 when absent; across a single-language
/// cast they are all present or all absent, so a `0` adds zero variance and does
/// not distort the z-space.
const FEATURES: usize = 6;

pub(crate) fn feature_vector(p: &VoiceProfile) -> [f32; FEATURES] {
    [
        p.p50,                             // median sentence length
        p.cv,                              // rhythm variety
        p.burstiness,                      // rhythm clustering
        p.mattr,                           // lexical diversity
        p.modal_density.unwrap_or(0.0),    // hedging
        p.interiority_ratio.unwrap_or(0.0),
    ]
}

/// One pair of comparable voices and the distance between them.
#[derive(Debug, Clone)]
pub(crate) struct VoicePair {
    pub a: String,
    pub b: String,
    pub distance: f32,
}

/// The distinctiveness of a cast.
pub(crate) struct DistinctMatrix {
    /// The comparable voices, in cast order.
    pub names: Vec<String>,
    /// Every comparable pair, sorted by distance ascending (closest first).
    pub pairs: Vec<VoicePair>,
    /// The pairs flagged indistinguishable (below threshold, not ignored).
    pub indistinguishable: Vec<VoicePair>,
}

impl DistinctMatrix {
    /// The closest pair — the two voices most at risk of sounding alike.
    pub(crate) fn closest(&self) -> Option<&VoicePair> {
        self.pairs.first()
    }
    /// The most distinct pair.
    pub(crate) fn most_distinct(&self) -> Option<&VoicePair> {
        self.pairs.last()
    }
}

/// Build the matrix over the comparable voices. Fewer than two comparable voices
/// → an empty matrix (nothing to compare).
pub(crate) fn matrix(voices: &[CharacterVoice], threshold: f32, ignore: &[String]) -> DistinctMatrix {
    let comparable: Vec<&CharacterVoice> =
        voices.iter().filter(|v| v.confidence.is_comparable()).collect();
    let names: Vec<String> = comparable.iter().map(|v| v.name.clone()).collect();
    if comparable.len() < 2 {
        return DistinctMatrix { names, pairs: Vec::new(), indistinguishable: Vec::new() };
    }

    let raw: Vec<[f32; FEATURES]> = comparable.iter().map(|v| feature_vector(&v.profile)).collect();
    let z = z_score(&raw);

    let mut pairs = Vec::new();
    for i in 0..comparable.len() {
        for j in (i + 1)..comparable.len() {
            pairs.push(VoicePair {
                a: names[i].clone(),
                b: names[j].clone(),
                distance: rms_distance(&z[i], &z[j]),
            });
        }
    }
    pairs.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

    let indistinguishable = pairs
        .iter()
        .filter(|p| p.distance < threshold && !is_ignored(ignore, &p.a, &p.b))
        .cloned()
        .collect();

    DistinctMatrix { names, pairs, indistinguishable }
}

/// Z-score each feature across the cast (population mean/std). A zero-variance
/// feature contributes 0 (it can't discriminate).
fn z_score(raw: &[[f32; FEATURES]]) -> Vec<[f32; FEATURES]> {
    let n = raw.len() as f32;
    let mut mean = [0.0f32; FEATURES];
    for r in raw {
        for d in 0..FEATURES {
            mean[d] += r[d];
        }
    }
    for m in &mut mean {
        *m /= n;
    }
    let mut std = [0.0f32; FEATURES];
    for r in raw {
        for d in 0..FEATURES {
            let dv = r[d] - mean[d];
            std[d] += dv * dv;
        }
    }
    for s in &mut std {
        *s = (*s / n).sqrt();
    }
    raw.iter()
        .map(|r| {
            let mut zz = [0.0f32; FEATURES];
            for d in 0..FEATURES {
                zz[d] = if std[d] > 1e-9 { (r[d] - mean[d]) / std[d] } else { 0.0 };
            }
            zz
        })
        .collect()
}

/// Root-mean-square of per-feature z-differences.
fn rms_distance(a: &[f32; FEATURES], b: &[f32; FEATURES]) -> f32 {
    let sum: f32 = (0..FEATURES).map(|d| (a[d] - b[d]).powi(2)).sum();
    (sum / FEATURES as f32).sqrt()
}

/// A pair matches an ignore entry `"A|B"` regardless of order and case
/// (Unicode-aware, so Russian names compare correctly).
fn is_ignored(ignore: &[String], a: &str, b: &str) -> bool {
    let (al, bl) = (a.to_lowercase(), b.to_lowercase());
    ignore.iter().any(|entry| {
        let mut parts = entry.split('|').map(|s| s.trim().to_lowercase());
        match (parts.next(), parts.next()) {
            (Some(x), Some(y)) => (x == al && y == bl) || (x == bl && y == al),
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chorus::voices::Confidence;
    use crate::prose::{CompiledLexicon, ProseLanguage, VoiceScope, compute_profile_with};

    fn voice(name: &str, text: &str, confidence: Confidence) -> CharacterVoice {
        let lx = CompiledLexicon::for_language_with(&ProseLanguage::En, &[], &[]);
        let profile = compute_profile_with(
            text,
            VoiceScope::Character(name.into()),
            &ProseLanguage::En,
            &lx,
            false,
            100,
        );
        CharacterVoice { name: name.into(), profile, confidence, utterances: 30 }
    }

    const CLIPPED: &str = "Yes. No. Go. Stop. Now. Wait. Fine. Leave. Run. Hide. Down. Up.";
    const FLOWING: &str = "The evening light fell slowly across the wide and silent water, and \
                           she wondered whether the tide would ever turn again before the long \
                           grey dawn came creeping over the eastern hills once more.";

    #[test]
    fn identical_voices_are_flagged_distinct_ones_are_not() {
        // Two characters with identical dialogue read identically; a third is
        // clearly different.
        let voices = vec![
            voice("Mara", CLIPPED, Confidence::High),
            voice("Joren", CLIPPED, Confidence::High),
            voice("Sela", FLOWING, Confidence::High),
        ];
        let m = matrix(&voices, 0.5, &[]);
        assert_eq!(m.names.len(), 3);
        // Mara ≈ Joren (distance ~0) is the one flagged pair.
        assert_eq!(m.indistinguishable.len(), 1);
        let flagged = &m.indistinguishable[0];
        let pair = {
            let mut p = [flagged.a.as_str(), flagged.b.as_str()];
            p.sort();
            p
        };
        assert_eq!(pair, ["Joren", "Mara"]);
        assert!(flagged.distance < 1e-3, "identical voices should be ~0 apart");
        // The closest pair IS the identical one; the flowing voice is far.
        assert_eq!(m.closest().unwrap().distance, flagged.distance);
        assert!(m.most_distinct().unwrap().distance > 0.5);
    }

    #[test]
    fn low_confidence_voices_never_participate() {
        let voices = vec![
            voice("Mara", CLIPPED, Confidence::High),
            voice("Joren", CLIPPED, Confidence::Low), // identical text, but sparse
        ];
        let m = matrix(&voices, 0.5, &[]);
        // Only one comparable voice → no pairs, nothing flagged.
        assert_eq!(m.names, vec!["Mara".to_string()]);
        assert!(m.pairs.is_empty());
        assert!(m.indistinguishable.is_empty());
    }

    #[test]
    fn ignore_list_suppresses_a_deliberate_pair() {
        let voices = vec![
            voice("Mara", CLIPPED, Confidence::High),
            voice("Joren", CLIPPED, Confidence::High),
        ];
        // Order- and case-insensitive.
        let m = matrix(&voices, 0.5, &["joren|mara".to_string()]);
        assert!(m.pairs.len() == 1 && m.indistinguishable.is_empty());
    }
}
