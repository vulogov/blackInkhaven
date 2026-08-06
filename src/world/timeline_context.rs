//! WORLD-5 — the shared **timeline context provider**. It gives the WORLD-4
//! fact-checker (and INNER_SOCRATES-1's Slow track) access to the timeline's
//! events so a paragraph can be checked against *when* it happens, not just what
//! the world looks like. Read-only over the timeline; the analysis is pure.
//!
//! P0 ships the core: gather the project's events, build a paragraph's
//! [`TimelineContext`] (its linked events → effective date → season, plus nearby
//! events), and the event lookups (`events_near`, `events_for_character`,
//! `events_for_place`). Later phases feed this into the categories and add the two
//! new ones. Degrades to empty context when the project has no timeline.

use uuid::Uuid;

use crate::store::hierarchy::Hierarchy;
use crate::timeline::calendar::{Calendar, TimelinePoint};

/// A timeline event projected for the fact-checker.
#[derive(Debug, Clone, PartialEq)]
pub struct TlEvent {
    pub id: Uuid,
    pub title: String,
    pub start_ticks: i64,
    pub end_ticks: Option<i64>,
    pub linked_paragraphs: Vec<Uuid>,
    pub characters: Vec<Uuid>,
    pub places: Vec<Uuid>,
}

impl TlEvent {
    /// The event's time span as `[start, end]` ticks (`end` = `start` for an
    /// instant event).
    pub fn span(&self) -> (i64, i64) {
        (self.start_ticks, self.end_ticks.unwrap_or(self.start_ticks))
    }

    /// Do two events' time spans overlap?
    pub fn overlaps(&self, other: &TlEvent) -> bool {
        let (a0, a1) = self.span();
        let (b0, b1) = other.span();
        a0 <= b1 && b0 <= a1
    }
}

/// Where a paragraph's effective date came from (quality gate for findings).
#[derive(Debug, Clone, PartialEq)]
pub enum DateSource {
    /// A linked event provides the date — high confidence.
    ExplicitLink(Uuid),
    /// Inferred from a nearby event in the same chapter — medium confidence.
    InferredFromNearby(Uuid),
    /// No date could be established.
    Unknown,
}

/// The per-paragraph timeline context the predicates consume.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineContext {
    pub paragraph_id: Uuid,
    /// Events explicitly linked to this paragraph.
    pub linked_events: Vec<Uuid>,
    /// Events near in world-time (not the linked ones).
    pub nearby_events: Vec<Uuid>,
    /// The paragraph's effective world-time, in ticks.
    pub effective_date: Option<i64>,
    pub date_source: DateSource,
    /// The season of the effective date, per the world's calendar.
    pub effective_season: Option<String>,
}

impl TimelineContext {
    /// An empty context (no timeline, or nothing established).
    pub fn empty(paragraph_id: Uuid) -> Self {
        TimelineContext {
            paragraph_id,
            linked_events: Vec::new(),
            nearby_events: Vec::new(),
            effective_date: None,
            date_source: DateSource::Unknown,
            effective_season: None,
        }
    }

    /// Whether any timeline signal applies to this paragraph.
    pub fn is_empty(&self) -> bool {
        self.linked_events.is_empty() && self.nearby_events.is_empty()
    }
}

/// Gather the project's events from the hierarchy (nodes carrying `EventData`),
/// ordered by world-time. Thin I/O adapter; the analysis below is pure.
pub fn gather_events(hierarchy: &Hierarchy) -> Vec<TlEvent> {
    let mut out: Vec<TlEvent> = hierarchy
        .iter()
        .filter_map(|n| {
            let ev = n.event.as_ref()?;
            Some(TlEvent {
                id: n.id,
                title: n.title.clone(),
                start_ticks: ev.start_ticks,
                end_ticks: ev.end_ticks,
                linked_paragraphs: n.linked_paragraphs.clone(),
                characters: ev.characters.clone(),
                places: ev.places.clone(),
            })
        })
        .collect();
    out.sort_by_key(|e| e.start_ticks);
    out
}

/// Character/Place entries under their system books, as `(id, lowercase title,
/// is_place)`. Used to detect which entities a scene names.
fn entity_entries(hierarchy: &Hierarchy) -> Vec<(Uuid, String, bool)> {
    let mut roots: Vec<(Uuid, bool)> = Vec::new();
    for n in hierarchy.iter() {
        match n.system_tag.as_deref() {
            Some(crate::store::SYSTEM_TAG_CHARACTERS) => roots.push((n.id, false)),
            Some(crate::store::SYSTEM_TAG_PLACES) => roots.push((n.id, true)),
            _ => {}
        }
    }
    let mut out: Vec<(Uuid, String, bool)> = Vec::new();
    for (root, is_place) in roots {
        let mut stack: Vec<Uuid> =
            hierarchy.children_of(Some(root)).iter().map(|n| n.id).collect();
        while let Some(id) = stack.pop() {
            let Some(n) = hierarchy.get(id) else { continue };
            if matches!(n.kind, crate::store::node::NodeKind::Paragraph) {
                let t = n.title.trim().to_lowercase();
                if !t.is_empty() {
                    out.push((n.id, t, is_place));
                }
            }
            stack.extend(hierarchy.children_of(Some(id)).iter().map(|n| n.id));
        }
    }
    out
}

/// Pure core of the scene-mention derivation: the entity ids whose (lowercased)
/// title is named whole-word in `body_lc`, split into (characters, places).
fn entities_named_in(
    body_lc: &str,
    entries: &[(Uuid, String, bool)],
) -> (Vec<Uuid>, Vec<Uuid>) {
    let mut chars = Vec::new();
    let mut places = Vec::new();
    for (id, title_lc, is_place) in entries {
        if crate::drift::mentions(body_lc, title_lc) {
            let list = if *is_place { &mut places } else { &mut chars };
            if !list.contains(id) {
                list.push(*id);
            }
        }
    }
    (chars, places)
}

/// For every event, the Character/Place entities *named in its linked scenes* —
/// a proxy for "present at the event", derived fresh from the prose. This feeds
/// the ADVISORY continuity checks (co-location, fuzzy overlap) only; KEN
/// presence deliberately ignores it and stays on explicit participants, so a
/// merely-mentioned character can never mask a knowledge leak.
///
/// Matching is whole-word on the entity's title (proper nouns are not stemmed),
/// so a multi-word name matches as a run and a first-name-only mention is missed
/// — acceptable for an advisory signal.
pub fn derived_participants(
    store: &crate::store::Store,
    hierarchy: &Hierarchy,
) -> std::collections::HashMap<Uuid, (Vec<Uuid>, Vec<Uuid>)> {
    let entries = entity_entries(hierarchy);
    let mut map: std::collections::HashMap<Uuid, (Vec<Uuid>, Vec<Uuid>)> =
        std::collections::HashMap::new();
    if entries.is_empty() {
        return map;
    }
    let mut body_cache: std::collections::HashMap<Uuid, String> =
        std::collections::HashMap::new();
    for n in hierarchy.iter() {
        if n.event.is_none() {
            continue;
        }
        let mut chars: Vec<Uuid> = Vec::new();
        let mut places: Vec<Uuid> = Vec::new();
        for pid in &n.linked_paragraphs {
            let body = body_cache.entry(*pid).or_insert_with(|| {
                store
                    .get_content(*pid)
                    .ok()
                    .flatten()
                    .and_then(|b| String::from_utf8(b).ok())
                    .unwrap_or_default()
                    .to_lowercase()
            });
            let (c, p) = entities_named_in(body, &entries);
            for id in c {
                if !chars.contains(&id) {
                    chars.push(id);
                }
            }
            for id in p {
                if !places.contains(&id) {
                    places.push(id);
                }
            }
        }
        if !chars.is_empty() || !places.is_empty() {
            map.insert(n.id, (chars, places));
        }
    }
    map
}

/// Like [`gather_events`], but each event's participant lists are the explicit
/// participants UNIONed with the entities named in its linked scenes (see
/// [`derived_participants`]). For the advisory checks only — never KEN.
pub fn gather_events_augmented(
    store: &crate::store::Store,
    hierarchy: &Hierarchy,
) -> Vec<TlEvent> {
    let mut events = gather_events(hierarchy);
    let derived = derived_participants(store, hierarchy);
    for ev in &mut events {
        if let Some((chars, places)) = derived.get(&ev.id) {
            for c in chars {
                if !ev.characters.contains(c) {
                    ev.characters.push(*c);
                }
            }
            for p in places {
                if !ev.places.contains(p) {
                    ev.places.push(*p);
                }
            }
        }
    }
    events
}

/// Events whose start falls within `window` ticks of `ticks`.
pub fn events_near(events: &[TlEvent], ticks: i64, window: i64) -> Vec<&TlEvent> {
    events.iter().filter(|e| (e.start_ticks - ticks).abs() <= window).collect()
}

/// Events involving `character`.
pub fn events_for_character(events: &[TlEvent], character: Uuid) -> Vec<&TlEvent> {
    events.iter().filter(|e| e.characters.contains(&character)).collect()
}

/// Events at `place`.
pub fn events_for_place(events: &[TlEvent], place: Uuid) -> Vec<&TlEvent> {
    events.iter().filter(|e| e.places.contains(&place)).collect()
}

/// Build the per-paragraph timeline context (pure). The effective date is the
/// earliest linked event's start; the season comes from the calendar; nearby
/// events are those within `window` ticks (excluding the linked ones).
pub fn build_context(
    paragraph_id: Uuid,
    events: &[TlEvent],
    calendar: &Calendar,
    window: i64,
) -> TimelineContext {
    let linked: Vec<&TlEvent> =
        events.iter().filter(|e| e.linked_paragraphs.contains(&paragraph_id)).collect();
    if linked.is_empty() {
        return TimelineContext::empty(paragraph_id);
    }
    let anchor = linked.iter().min_by_key(|e| e.start_ticks).unwrap();
    let effective_date = Some(anchor.start_ticks);
    let date_source = DateSource::ExplicitLink(anchor.id);
    let effective_season = calendar.season_for(TimelinePoint::from_ticks(anchor.start_ticks));

    let linked_ids: std::collections::HashSet<Uuid> = linked.iter().map(|e| e.id).collect();
    let nearby_events = events_near(events, anchor.start_ticks, window)
        .into_iter()
        .filter(|e| !linked_ids.contains(&e.id))
        .map(|e| e.id)
        .collect();

    TimelineContext {
        paragraph_id,
        linked_events: linked.iter().map(|e| e.id).collect(),
        nearby_events,
        effective_date,
        date_source,
        effective_season,
    }
}

/// WORLD-5 — a character placed in two different places in overlapping event
/// windows. The timeline alone reveals it; no prose needed.
#[derive(Debug, Clone, PartialEq)]
pub struct CoLocationConflict {
    pub character: Uuid,
    pub event_a: Uuid,
    pub event_b: Uuid,
    pub title_a: String,
    pub title_b: String,
    pub place_a: Uuid,
    pub place_b: Uuid,
}

/// Find every co-location conflict in the events: a character whose events place
/// them at two *different* places (sharing none) in overlapping time. Pure +
/// deterministic; the caller resolves character/place names and applies the magic
/// ledger (a `teleportation` rule may excuse it).
pub fn co_location_conflicts(events: &[TlEvent]) -> Vec<CoLocationConflict> {
    let mut characters: std::collections::BTreeSet<Uuid> = std::collections::BTreeSet::new();
    for e in events {
        characters.extend(e.characters.iter().copied());
    }
    let mut out = Vec::new();
    for ch in characters {
        let evs: Vec<&TlEvent> = events.iter().filter(|e| e.characters.contains(&ch)).collect();
        for i in 0..evs.len() {
            for j in (i + 1)..evs.len() {
                let (a, b) = (evs[i], evs[j]);
                if !a.overlaps(b) {
                    continue;
                }
                // A conflict only if they share no place and each names one.
                if a.places.iter().any(|p| b.places.contains(p)) {
                    continue;
                }
                if let (Some(pa), Some(pb)) = (a.places.first(), b.places.first()) {
                    out.push(CoLocationConflict {
                        character: ch,
                        event_a: a.id,
                        event_b: b.id,
                        title_a: a.title.clone(),
                        title_b: b.title.clone(),
                        place_a: *pa,
                        place_b: *pb,
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::calendar::{Calendar, CalendarConfig};

    fn gregorian() -> Calendar {
        Calendar::from_config(CalendarConfig { preset: "gregorian".into(), ..Default::default() })
    }

    fn ev(id: Uuid, t: i64, linked: &[Uuid], chars: &[Uuid], places: &[Uuid]) -> TlEvent {
        TlEvent {
            id,
            title: "e".into(),
            start_ticks: t,
            end_ticks: None,
            linked_paragraphs: linked.to_vec(),
            characters: chars.to_vec(),
            places: places.to_vec(),
        }
    }

    #[test]
    fn entities_named_in_matches_whole_word_titles() {
        let mira = Uuid::new_v4();
        let bryn = Uuid::new_v4();
        let mole = Uuid::new_v4();
        let entries = vec![
            (mira, "mira".to_string(), false),
            (bryn, "bryn crane".to_string(), false),
            (mole, "the long mole".to_string(), true),
        ];
        let body = "mira walked out onto the long mole, and bryn followed.".to_lowercase();
        let (chars, places) = entities_named_in(&body, &entries);
        assert!(chars.contains(&mira), "Mira should be detected");
        assert!(
            !chars.contains(&bryn),
            "a first-name-only mention must not match the full title"
        );
        assert_eq!(places, vec![mole], "the multi-word place is matched as a run");
        // whole-word: `mira` inside `admiral` must not match.
        let (chars2, _) = entities_named_in("the admiral arrived", &entries);
        assert!(chars2.is_empty(), "a substring inside a word must not match");
    }

    #[test]
    fn build_context_links_date_and_season() {
        let cal = gregorian();
        let para = Uuid::new_v4();
        // A linked event ~6 months in (July → summer); ticks_per month is 1*30 in
        // the gregorian preset (day-base), so 6 months ≈ 180 days.
        let day = cal.ticks_per("day").unwrap_or(1);
        let month = cal.ticks_per("month").unwrap_or(day * 30);
        let summer_tick = 6 * month;
        let near = Uuid::new_v4();
        let far = Uuid::new_v4();
        let events = vec![
            ev(Uuid::new_v4(), summer_tick, &[para], &[], &[]),
            ev(near, summer_tick + month, &[], &[], &[]), // ~1 month away → nearby
            ev(far, summer_tick + month * 11, &[], &[], &[]), // ~11 months → not nearby
        ];
        let ctx = build_context(para, &events, &cal, 90 * day);
        assert_eq!(ctx.linked_events.len(), 1);
        assert_eq!(ctx.effective_date, Some(summer_tick));
        assert_eq!(ctx.effective_season.as_deref(), Some("summer"));
        assert!(matches!(ctx.date_source, DateSource::ExplicitLink(_)));
        assert_eq!(ctx.nearby_events, vec![near], "the ~1-month event is nearby, the ~11-month one isn't");
    }

    #[test]
    fn no_link_gives_empty_context() {
        let cal = gregorian();
        let para = Uuid::new_v4();
        let events = vec![ev(Uuid::new_v4(), 100, &[Uuid::new_v4()], &[], &[])];
        let ctx = build_context(para, &events, &cal, 1000);
        assert!(ctx.is_empty());
        assert_eq!(ctx.date_source, DateSource::Unknown);
    }

    #[test]
    fn character_and_place_lookups() {
        let mara = Uuid::new_v4();
        let velmaril = Uuid::new_v4();
        let a = ev(Uuid::new_v4(), 10, &[], &[mara], &[velmaril]);
        let b = ev(Uuid::new_v4(), 20, &[], &[], &[]);
        let events = vec![a.clone(), b];
        assert_eq!(events_for_character(&events, mara), vec![&events[0]]);
        assert_eq!(events_for_place(&events, velmaril), vec![&events[0]]);
        assert!(events_for_character(&events, Uuid::new_v4()).is_empty());
    }

    #[test]
    fn co_location_flags_one_character_two_places() {
        let mara = Uuid::new_v4();
        let velmaril = Uuid::new_v4();
        let korthun = Uuid::new_v4();
        // Two overlapping events place Mara in different cities.
        let a = TlEvent { end_ticks: Some(20), ..ev(Uuid::new_v4(), 10, &[], &[mara], &[velmaril]) };
        let b = ev(Uuid::new_v4(), 15, &[], &[mara], &[korthun]); // instant inside a's span
        // A third event, far apart in time — no conflict.
        let c = ev(Uuid::new_v4(), 900, &[], &[mara], &[korthun]);
        let conflicts = co_location_conflicts(&[a, b, c]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].character, mara);
        assert_eq!(conflicts[0].place_a, velmaril);
        assert_eq!(conflicts[0].place_b, korthun);
    }

    #[test]
    fn co_location_ignores_shared_place_and_no_place() {
        let mara = Uuid::new_v4();
        let velmaril = Uuid::new_v4();
        // Overlapping but at the SAME place → fine.
        let a = TlEvent { end_ticks: Some(20), ..ev(Uuid::new_v4(), 10, &[], &[mara], &[velmaril]) };
        let b = ev(Uuid::new_v4(), 15, &[], &[mara], &[velmaril]);
        assert!(co_location_conflicts(&[a, b]).is_empty());
        // Overlapping, different... but one event has no place → can't establish.
        let c = TlEvent { end_ticks: Some(20), ..ev(Uuid::new_v4(), 10, &[], &[mara], &[velmaril]) };
        let d = ev(Uuid::new_v4(), 15, &[], &[mara], &[]);
        assert!(co_location_conflicts(&[c, d]).is_empty());
    }

    #[test]
    fn overlap_detection() {
        let a = TlEvent { end_ticks: Some(20), ..ev(Uuid::nil(), 10, &[], &[], &[]) };
        let b = ev(Uuid::nil(), 15, &[], &[], &[]); // instant inside a's span
        let c = ev(Uuid::nil(), 50, &[], &[], &[]); // outside
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }
}
