//! NARR-1 — Tier-1 voice metrics. All language-agnostic: pure token counting
//! and arithmetic over per-sentence word counts. Valid for any prose language,
//! including `Other`.

use std::collections::HashSet;

/// Linear-interpolated percentile (`p` in 0..=100) over a **sorted** slice.
pub(crate) fn percentile(sorted: &[usize], p: f64) -> f32 {
    match sorted.len() {
        0 => 0.0,
        1 => sorted[0] as f32,
        n => {
            let rank = (p / 100.0) * (n - 1) as f64;
            let lo = rank.floor() as usize;
            let hi = rank.ceil() as usize;
            let frac = rank - lo as f64;
            (sorted[lo] as f64 + (sorted[hi] as f64 - sorted[lo] as f64) * frac) as f32
        }
    }
}

/// Population mean and standard deviation of a length series.
pub(crate) fn mean_std(xs: &[usize]) -> (f64, f64) {
    if xs.is_empty() {
        return (0.0, 0.0);
    }
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<usize>() as f64 / n;
    let var = xs.iter().map(|&x| (x as f64 - mean).powi(2)).sum::<f64>() / n;
    (mean, var.sqrt())
}

/// Coefficient of variation σ/μ (0 when μ = 0). Low → metronomic prose; high →
/// rhythmically varied.
pub(crate) fn coefficient_of_variation(lengths: &[usize]) -> f32 {
    let (mean, std) = mean_std(lengths);
    if mean == 0.0 {
        0.0
    } else {
        (std / mean) as f32
    }
}

/// Goh-Barabási burstiness B = (σ − μ) / (σ + μ), range [−1, +1]. 0 when σ+μ=0.
///
/// NOTE: with σ and μ taken over the *distribution* of sentence lengths (not a
/// timing sequence), B is a monotone transform of CV — `B = (CV − 1)/(CV + 1)` —
/// so it carries the same information as `cv`, just bounded to [−1, 1]. It is
/// **not** order-sensitive, despite what intuition about "burstiness" suggests;
/// a genuine order/memory metric is a candidate for a later phase.
pub(crate) fn burstiness(lengths: &[usize]) -> f32 {
    let (mean, std) = mean_std(lengths);
    let denom = std + mean;
    if denom == 0.0 {
        0.0
    } else {
        ((std - mean) / denom) as f32
    }
}

/// Moving-Average Type-Token Ratio over a `window`-token sliding window,
/// averaged across windows. Length-corrected lexical diversity. Falls back to
/// plain TTR when the text is shorter than one window.
pub(crate) fn mattr(tokens: &[&str], window: usize) -> f32 {
    let n = tokens.len();
    if n == 0 {
        return 0.0;
    }
    let w = window.clamp(1, n);
    if n == w {
        let uniq = tokens.iter().collect::<HashSet<_>>().len();
        return uniq as f32 / n as f32;
    }
    let windows = n - w + 1;
    let mut sum = 0.0f64;
    for start in 0..windows {
        let uniq = tokens[start..start + w].iter().collect::<HashSet<_>>().len();
        sum += uniq as f64 / w as f64;
    }
    (sum / windows as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseLanguage::En;

    #[test]
    fn percentiles_interpolate() {
        let mut v = [1usize, 2, 3, 4, 5];
        v.sort_unstable();
        assert_eq!(percentile(&v, 50.0), 3.0);
        assert_eq!(percentile(&v, 0.0), 1.0);
        assert_eq!(percentile(&v, 100.0), 5.0);
        assert!((percentile(&v, 25.0) - 2.0).abs() < 0.001);
    }

    #[test]
    fn cv_and_burstiness_relationship() {
        // Uniform lengths → σ=0 → CV 0, B = -1.
        let uni = [5usize, 5, 5, 5];
        assert_eq!(coefficient_of_variation(&uni), 0.0);
        assert!((burstiness(&uni) + 1.0).abs() < 1e-6);
        // Varied lengths → CV > 0; B = (CV-1)/(CV+1) holds.
        let varied = [2usize, 8, 3, 20, 5];
        let cv = coefficient_of_variation(&varied) as f64;
        let b = burstiness(&varied) as f64;
        assert!(((cv - 1.0) / (cv + 1.0) - b).abs() < 1e-5);
    }

    #[test]
    fn mattr_bounds() {
        // All-unique window → 1.0.
        let uniq = ["a", "b", "c", "d"];
        assert!((mattr(&uniq, 2) - 1.0).abs() < 1e-6);
        // All-identical → 1/window.
        let same = ["x", "x", "x", "x"];
        assert!((mattr(&same, 2) - 0.5).abs() < 1e-6);
        assert_eq!(mattr(&[], 100), 0.0);
    }

    #[test]
    fn tier1_integration() {
        use crate::prose::VoiceScope;
        use crate::prose::profile::compute_profile;
        let text = "The cat sat. The dog ran fast across the wide green field today. Up.";
        let t = compute_profile(text, VoiceScope::Book, &En, false, 100);
        assert_eq!(t.sentence_count, 3);
        // 3 + 10 + 1 = 14 words.
        assert_eq!(t.word_count, 14);
        assert!(t.cv > 0.0);
        assert!(t.p50 >= t.p10);
        assert!(t.mattr > 0.0 && t.mattr <= 1.0);
    }
}
