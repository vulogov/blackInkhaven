//! Fuzzy-precision overlap detection — a retained, timeline-internal critique.
//!
//! Events with `Season`/`Month` (and the coarser `Year`/`Week`) precision carry a
//! *fuzz window* around their nominal date. When two such windows collide in a
//! suspicious way — same track, shared character or place, tight overlap — the
//! prose probably can't place both events consistently. Detecting this depends on
//! the timeline's precision semantics, which only the timeline feature
//! understands; it fits neither the fact-checker nor the Socratic reader.
//!
//! The detector is pure. It scores pairwise overlaps by suspicion and finds 3+
//! event clusters (mutually overlapping fuzzy events sharing a common window),
//! emitting the cluster once instead of as pairwise noise.

use std::collections::BTreeSet;

use super::types::{
    CritiqueEvent, FuzzWindows, FuzzyOverlapFinding, Suspicion,
};
use uuid::Uuid;

/// Default minimum cluster size (3+ mutually overlapping fuzzy events).
pub const DEFAULT_CLUSTER_MIN_SIZE: usize = 3;

/// Cap on events listed in a cluster finding before summarising "and N more".
pub const CLUSTER_LIST_CAP: usize = 10;

/// The event's span widened by its precision's fuzz radius.
fn expanded_span(e: &CritiqueEvent, fw: &FuzzWindows) -> (i64, i64) {
    let (s, end) = e.span();
    let r = fw.radius(e.precision);
    (s - r, end + r)
}

fn windows_intersect(a: &CritiqueEvent, b: &CritiqueEvent, fw: &FuzzWindows) -> bool {
    let (a0, a1) = expanded_span(a, fw);
    let (b0, b1) = expanded_span(b, fw);
    a0 <= b1 && b0 <= a1
}

fn overlap_window(a: &CritiqueEvent, b: &CritiqueEvent, fw: &FuzzWindows) -> (i64, i64) {
    let (a0, a1) = expanded_span(a, fw);
    let (b0, b1) = expanded_span(b, fw);
    (a0.max(b0), a1.min(b1))
}

fn shared<T: Copy + Ord>(a: &[T], b: &[T]) -> Vec<T> {
    let bs: BTreeSet<T> = b.iter().copied().collect();
    let mut out: Vec<T> = a.iter().copied().filter(|x| bs.contains(x)).collect();
    out.sort();
    out.dedup();
    out
}

/// Suspicion of a single fuzzy pair from its factors. Two factors (same track +
/// shared entity) → High; one → Moderate; none → Low (filtered out).
fn pair_suspicion(a: &CritiqueEvent, b: &CritiqueEvent) -> (Suspicion, Vec<Uuid>, Vec<Uuid>) {
    let shared_chars = shared(&a.characters, &b.characters);
    let shared_places = shared(&a.places, &b.places);
    let same_track = a.track.eq_ignore_ascii_case(&b.track);
    let shared_entity = !shared_chars.is_empty() || !shared_places.is_empty();
    let factors = same_track as u8 + shared_entity as u8;
    let suspicion = match factors {
        2 => Suspicion::High,
        1 => Suspicion::Moderate,
        _ => Suspicion::Low,
    };
    (suspicion, shared_chars, shared_places)
}

fn suspicion_rank(s: Suspicion) -> u8 {
    match s {
        Suspicion::Low => 0,
        Suspicion::Moderate => 1,
        Suspicion::High => 2,
        Suspicion::Cluster => 2,
    }
}

/// Connected components over the "fuzzy windows intersect" relation. Simple
/// breadth-first labelling; `idx` are indices into `fuzzy`.
fn components(fuzzy: &[&CritiqueEvent], fw: &FuzzWindows) -> Vec<Vec<usize>> {
    let n = fuzzy.len();
    let mut seen = vec![false; n];
    let mut out = Vec::new();
    for start in 0..n {
        if seen[start] {
            continue;
        }
        let mut stack = vec![start];
        let mut comp = Vec::new();
        seen[start] = true;
        while let Some(i) = stack.pop() {
            comp.push(i);
            for j in 0..n {
                if !seen[j] && windows_intersect(fuzzy[i], fuzzy[j], fw) {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        comp.sort();
        out.push(comp);
    }
    out
}

/// Common window shared by every event in the group (max of starts, min of ends).
/// Returns `None` when the intersection is empty — a transitively-connected but not
/// genuinely co-occurring group is not a cluster.
fn common_window(group: &[&CritiqueEvent], fw: &FuzzWindows) -> Option<(i64, i64)> {
    let mut lo = i64::MIN;
    let mut hi = i64::MAX;
    for e in group {
        let (s, end) = expanded_span(e, fw);
        lo = lo.max(s);
        hi = hi.min(end);
    }
    (lo <= hi).then_some((lo, hi))
}

/// Run fuzzy-overlap detection. Only fuzzy-precision events (Year/Season/Month/
/// Week) participate. Pairs scoring at or above `min_suspicion` emit pairwise
/// findings; groups of `cluster_min_size`+ mutually overlapping events that share a
/// common window emit one cluster finding instead, and their internal pairs are
/// suppressed to avoid noise.
pub fn detect(
    events: &[CritiqueEvent],
    fw: &FuzzWindows,
    min_suspicion: Suspicion,
    cluster_min_size: usize,
) -> Vec<FuzzyOverlapFinding> {
    let fuzzy: Vec<&CritiqueEvent> =
        events.iter().filter(|e| FuzzWindows::is_fuzzy(e.precision)).collect();
    let cluster_min = cluster_min_size.max(2);
    let mut findings = Vec::new();
    let mut clustered: BTreeSet<Uuid> = BTreeSet::new();

    // Clusters first, so pairwise can skip events already covered by one.
    for comp in components(&fuzzy, fw) {
        if comp.len() < cluster_min {
            continue;
        }
        let group: Vec<&CritiqueEvent> = comp.iter().map(|&i| fuzzy[i]).collect();
        let Some(window) = common_window(&group, fw) else {
            continue; // connected but no genuine common overlap
        };
        for e in &group {
            clustered.insert(e.id);
        }
        // Characters / places shared by *every* event in the cluster.
        let mut shared_chars: Option<Vec<Uuid>> = None;
        let mut shared_places: Option<Vec<Uuid>> = None;
        for e in &group {
            shared_chars = Some(match shared_chars {
                None => e.characters.clone(),
                Some(acc) => shared(&acc, &e.characters),
            });
            shared_places = Some(match shared_places {
                None => e.places.clone(),
                Some(acc) => shared(&acc, &e.places),
            });
        }
        let shared_chars = shared_chars.unwrap_or_default();
        let shared_places = shared_places.unwrap_or_default();

        let track = if group.iter().all(|e| e.track.eq_ignore_ascii_case(&group[0].track)) {
            group[0].track.clone()
        } else {
            "(mixed tracks)".to_string()
        };
        let listed: Vec<&&CritiqueEvent> = group.iter().take(CLUSTER_LIST_CAP).collect();
        let titles: Vec<String> = listed.iter().map(|e| e.title.clone()).collect();
        let ids: Vec<Uuid> = listed.iter().map(|e| e.id).collect();
        let mut reasons = vec![format!(
            "{} fuzzy-precision events share an overlapping window.",
            group.len()
        )];
        if group.len() > CLUSTER_LIST_CAP {
            reasons.push(format!("and {} more events", group.len() - CLUSTER_LIST_CAP));
        }
        if !shared_places.is_empty() {
            reasons.push("All events share a common place.".to_string());
        }
        if !shared_chars.is_empty() {
            reasons.push("All events share a common character.".to_string());
        }
        findings.push(FuzzyOverlapFinding {
            event_ids: ids,
            titles,
            track,
            overlap_window: window,
            suspicion: Suspicion::Cluster,
            is_cluster: true,
            shared_characters: shared_chars,
            shared_places,
            severity: Suspicion::Cluster.severity(),
            reasons,
        });
    }

    // Pairwise findings for non-clustered pairs at/above the threshold.
    for i in 0..fuzzy.len() {
        for j in (i + 1)..fuzzy.len() {
            let (a, b) = (fuzzy[i], fuzzy[j]);
            if clustered.contains(&a.id) && clustered.contains(&b.id) {
                continue;
            }
            if !windows_intersect(a, b, fw) {
                continue;
            }
            let (suspicion, shared_chars, shared_places) = pair_suspicion(a, b);
            if suspicion_rank(suspicion) < suspicion_rank(min_suspicion) {
                continue;
            }
            let window = overlap_window(a, b, fw);
            let mut reasons = vec![format!(
                "Two {}-precision events have overlapping windows.",
                a.precision.as_str()
            )];
            if a.track.eq_ignore_ascii_case(&b.track) {
                reasons.push(format!("Both on the \"{}\" track.", a.track));
            }
            if !shared_chars.is_empty() {
                reasons.push("They share a character.".to_string());
            }
            if !shared_places.is_empty() {
                reasons.push("They share a place.".to_string());
            }
            findings.push(FuzzyOverlapFinding {
                event_ids: vec![a.id, b.id],
                titles: vec![a.title.clone(), b.title.clone()],
                track: if a.track.eq_ignore_ascii_case(&b.track) {
                    a.track.clone()
                } else {
                    "(mixed tracks)".to_string()
                },
                overlap_window: window,
                suspicion,
                is_cluster: false,
                shared_characters: shared_chars,
                shared_places,
                severity: suspicion.severity(),
                reasons,
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Precision;

    fn fw() -> FuzzWindows {
        FuzzWindows { year: 360, season: 90, month: 30, week: 7, day: 1 }
    }

    fn ev(
        title: &str,
        start: i64,
        precision: Precision,
        track: &str,
        chars: &[Uuid],
        places: &[Uuid],
    ) -> CritiqueEvent {
        CritiqueEvent {
            id: Uuid::new_v4(),
            title: title.into(),
            start_ticks: start,
            end_ticks: None,
            precision,
            track: track.into(),
            is_orphan: false,
            linked_paragraph_count: 1,
            characters: chars.to_vec(),
            places: places.to_vec(),
            age_days: None,
        }
    }

    #[test]
    fn same_track_shared_character_is_high() {
        let mara = Uuid::new_v4();
        // Two Season-precision events ~30 days apart → windows (±90) overlap.
        let a = ev("Training", 0, Precision::Season, "main", &[mara], &[]);
        let b = ev("Journey", 30, Precision::Season, "main", &[mara], &[]);
        let f = detect(&[a, b], &fw(), Suspicion::Moderate, DEFAULT_CLUSTER_MIN_SIZE);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].suspicion, Suspicion::High);
        assert_eq!(f[0].shared_characters, vec![mara]);
        assert!(!f[0].is_cluster);
    }

    #[test]
    fn different_track_no_shared_entity_is_filtered() {
        // Overlap exists, but no suspicious factor → Low → filtered at Moderate.
        let a = ev("A", 0, Precision::Season, "main", &[], &[]);
        let b = ev("B", 30, Precision::Season, "flashback", &[], &[]);
        let f = detect(&[a, b], &fw(), Suspicion::Moderate, DEFAULT_CLUSTER_MIN_SIZE);
        assert!(f.is_empty());
        // But visible at Low threshold.
        let a2 = ev("A", 0, Precision::Season, "main", &[], &[]);
        let b2 = ev("B", 30, Precision::Season, "flashback", &[], &[]);
        let f2 = detect(&[a2, b2], &fw(), Suspicion::Low, DEFAULT_CLUSTER_MIN_SIZE);
        assert_eq!(f2.len(), 1);
        assert_eq!(f2[0].suspicion, Suspicion::Low);
    }

    #[test]
    fn day_precision_events_do_not_participate() {
        let mara = Uuid::new_v4();
        let a = ev("A", 0, Precision::Day, "main", &[mara], &[]);
        let b = ev("B", 1, Precision::Day, "main", &[mara], &[]);
        assert!(detect(&[a, b], &fw(), Suspicion::Low, DEFAULT_CLUSTER_MIN_SIZE).is_empty());
    }

    #[test]
    fn three_overlapping_events_emit_one_cluster() {
        let velmaril = Uuid::new_v4();
        // Three Season events tightly clustered, all sharing a place.
        let a = ev("Coronation", 0, Precision::Season, "main", &[], &[velmaril]);
        let b = ev("Investigation", 20, Precision::Season, "main", &[], &[velmaril]);
        let c = ev("Town meeting", 40, Precision::Season, "main", &[], &[velmaril]);
        let f = detect(&[a, b, c], &fw(), Suspicion::Moderate, DEFAULT_CLUSTER_MIN_SIZE);
        // One cluster finding, no pairwise duplicates.
        assert_eq!(f.len(), 1);
        assert!(f[0].is_cluster);
        assert_eq!(f[0].suspicion, Suspicion::Cluster);
        assert_eq!(f[0].event_ids.len(), 3);
        assert_eq!(f[0].shared_places, vec![velmaril]);
    }

    #[test]
    fn transitive_chain_without_common_window_is_not_a_cluster() {
        // A–B overlap, B–C overlap, but A and C are far apart: no common window.
        // Month precision (±30). A at 0, B at 50, C at 100: A-B gap 50 (>60? no,
        // windows ±30 each → A:[-30,30] B:[20,80] overlap; B-C:[70,130] overlap;
        // A-C:[−30,30] vs [70,130] → no common window across all three.
        let a = ev("A", 0, Precision::Month, "main", &[], &[]);
        let b = ev("B", 50, Precision::Month, "main", &[], &[]);
        let c = ev("C", 100, Precision::Month, "main", &[], &[]);
        let f = detect(&[a, b, c], &fw(), Suspicion::Moderate, DEFAULT_CLUSTER_MIN_SIZE);
        // No cluster (no common window); pairwise A-B and B-C are same-track only
        // → Moderate, so they surface as pairs.
        assert!(f.iter().all(|x| !x.is_cluster));
        assert!(f.iter().all(|x| x.suspicion == Suspicion::Moderate));
    }
}
