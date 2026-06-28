//! NARR-1 — drift "violations": chapter metrics that cross their configured
//! threshold versus the baseline chapter. These surface as **informational**
//! `prose` findings (Output pane) and via `ink.prose.violations`. Never a
//! warning or error — they tell the author WHERE the voice moved, not that it
//! is wrong.

use crate::config::ProseThresholds;

use super::profile::{VoiceProfile, VoiceScope};

/// One metric exceeding its drift threshold in a chapter, relative to baseline.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Violation {
    pub chapter: u32,
    pub metric: &'static str,
    pub baseline: f32,
    pub value: f32,
    pub delta: f32,
}

fn push_if(
    out: &mut Vec<Violation>,
    chapter: u32,
    metric: &'static str,
    value: f32,
    baseline: f32,
    threshold: f32,
) {
    let delta = value - baseline;
    if delta.abs() >= threshold {
        out.push(Violation { chapter, metric, baseline, value, delta });
    }
}

/// Every metric in every chapter whose delta vs `baseline_ord` meets or exceeds
/// its threshold. Empty when the baseline chapter has no profile. Optional
/// (language-sensitive / Tier-2) metrics are only compared when present in both.
pub(crate) fn violations(
    profiles: &[VoiceProfile],
    baseline_ord: u32,
    thr: &ProseThresholds,
) -> Vec<Violation> {
    let Some(base) = profiles
        .iter()
        .find(|p| p.scope == VoiceScope::Chapter(baseline_ord))
    else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for p in profiles {
        let Some(ord) = p.scope.chapter_ord() else { continue };
        if ord == baseline_ord {
            continue;
        }
        push_if(&mut out, ord, "sent_len_cv", p.cv, base.cv, thr.sent_len_cv);
        push_if(&mut out, ord, "burstiness_b", p.burstiness, base.burstiness, thr.burstiness_b);
        push_if(&mut out, ord, "mattr", p.mattr, base.mattr, thr.mattr);

        if let (Some(v), Some(b)) = (p.modal_density, base.modal_density) {
            push_if(&mut out, ord, "modal_density", v, b, thr.modal_density);
        }
        if let (Some(v), Some(b)) = (p.interiority_ratio, base.interiority_ratio) {
            push_if(&mut out, ord, "interiority_ratio", v, b, thr.interiority_ratio);
        }
        if let (Some(v), Some(b)) = (
            p.de_erlebte_rede_particle_density,
            base.de_erlebte_rede_particle_density,
        ) {
            push_if(&mut out, ord, "de_erlebte_rede_particle_density", v, b, thr.de_erlebte_rede_particle_density);
        }
        if let (Some(t), Some(bt)) = (&p.tier2, &base.tier2) {
            push_if(&mut out, ord, "active_passive_ratio", t.active_passive_ratio, bt.active_passive_ratio, thr.active_passive_ratio);
            const CHANNELS: [&str; 5] = [
                "sensory_visual", "sensory_auditory", "sensory_olfactory", "sensory_tactile",
                "sensory_kinesthetic",
            ];
            for (i, name) in CHANNELS.iter().enumerate() {
                push_if(&mut out, ord, name, t.sensory[i], bt.sensory[i], thr.sensory_channel_max);
            }
        }
    }
    out
}

/// Emit one violation to the Output pane as an informational `prose` finding,
/// navigating (when known) to the first paragraph of the flagged chapter.
pub(crate) fn emit_violation(v: &Violation, source: Option<uuid::Uuid>) {
    use crate::pane::output::{Lifetime, Message, Severity, kinds};
    let dir = if v.delta >= 0.0 { "rose" } else { "fell" };
    let text = format!(
        "[ch.{}] {} {dir} to {:.3} (baseline {:.3}, Δ {:+.3})",
        v.chapter, v.metric, v.value, v.baseline, v.delta
    );
    let mut msg = Message::new(
        kinds::PROSE_DRIFT,
        Severity::Info,
        Lifetime::UntilActedOn,
        serde_json::json!({
            "text": text,
            "category": "prose",
            "metric": v.metric,
            "chapter": v.chapter,
            "delta": v.delta,
        }),
    );
    if let Some(id) = source {
        msg = msg.with_source_paragraph(id);
    }
    crate::pane::output::emit(&msg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseLanguage::En;
    use crate::prose::profile::compute_profile;

    #[test]
    fn flags_cv_and_modal_drift() {
        // Baseline: uniform, no hedging. Chapter 2: highly varied + hedged.
        let base = compute_profile(
            "He ran. She ate. They sat. We read. I slept. You won.",
            VoiceScope::Chapter(1),
            &En,
            false,
            100,
        );
        let drifted = compute_profile(
            "Perhaps he might possibly have wandered far across the wide and shadowed plain, \
             wondering. Yes.",
            VoiceScope::Chapter(2),
            &En,
            false,
            100,
        );
        let thr = ProseThresholds::default();
        let v = violations(&[base, drifted], 1, &thr);
        assert!(v.iter().any(|x| x.metric == "sent_len_cv" && x.chapter == 2), "{v:?}");
        assert!(v.iter().any(|x| x.metric == "modal_density"), "{v:?}");
    }

    #[test]
    fn identical_chapters_have_no_violations() {
        let text = "She walked home slowly. It was late and very cold outside.";
        let a = compute_profile(text, VoiceScope::Chapter(1), &En, false, 100);
        let b = compute_profile(text, VoiceScope::Chapter(2), &En, false, 100);
        assert!(violations(&[a, b], 1, &ProseThresholds::default()).is_empty());
    }

    #[test]
    fn missing_baseline_is_empty() {
        let p = compute_profile("Whatever.", VoiceScope::Chapter(2), &En, false, 100);
        assert!(violations(&[p], 1, &ProseThresholds::default()).is_empty());
    }
}
