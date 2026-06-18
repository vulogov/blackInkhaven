//! Semantic drift — the soft-consistency layer (WORLD-2, 1.3.10).
//!
//! 1.3.8 caught *hard* contradictions (a fact clashing with a fact). Drift
//! catches *soft* ones: two descriptions of the **same** entity that diverge
//! without a clean factual clash — a tavern "cramped and smoky" in ch.2,
//! "airy and bright" in ch.20.
//!
//! The division of labour is the honest one: **embeddings retrieve** the
//! handful of paragraphs that describe an entity (via the existing on-save
//! vector index — pure cosine similarity can't tell contradiction from
//! topical relatedness), and an **AI pass adjudicates** which pairs actually
//! contradict (P1).
//!
//! This module is the pure core — the entity model + the retrieval-result
//! assembly. The impure retrieval (vector search, content reads) and the AI
//! judge live in `cli::drift`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Which entity book a description belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityKind {
    Character,
    Place,
    Artefact,
}

impl EntityKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::Place => "place",
            Self::Artefact => "artefact",
        }
    }
}

/// One paragraph that describes an entity, with where it sits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptionSnippet {
    pub chapter: String,
    pub paragraph: Uuid,
    pub text: String,
}

/// The description snippets retrieved for one entity, chapter-ordered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDescriptions {
    pub entity: String,
    pub kind: EntityKind,
    pub snippets: Vec<DescriptionSnippet>,
}

/// A retrieval candidate: a paragraph the vector search returned, with its
/// chapter ordinal (for ordering), chapter title, and flattened plain text.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub paragraph: Uuid,
    pub chapter_order: usize,
    pub chapter_title: String,
    pub text: String,
}

/// From relevance-ranked retrieval `candidates`, keep the paragraphs that
/// describe `entity` — either they **mention** it by name (the anchor that
/// kills topical false-positives the vector search drags in) **or** they were
/// coreference-attributed to it (`coref`, a pronoun-only description adjacent
/// to a named mention — 1.3.11). Dedup by paragraph, take the top
/// `max_snippets` by relevance, present in **chapter order** so the judge
/// reads the description as a timeline. Pure.
pub fn assemble_descriptions(
    entity: &str,
    candidates: &[Candidate],
    max_snippets: usize,
    coref: &HashSet<Uuid>,
) -> Vec<DescriptionSnippet> {
    let needle = entity.trim().to_lowercase();
    if needle.is_empty() || max_snippets == 0 {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    let mut kept: Vec<&Candidate> = Vec::new();
    for c in candidates {
        if kept.len() >= max_snippets {
            break;
        }
        if !c.text.to_lowercase().contains(&needle) && !coref.contains(&c.paragraph) {
            continue;
        }
        if !seen.insert(c.paragraph) {
            continue;
        }
        kept.push(c);
    }
    kept.sort_by_key(|c| c.chapter_order);
    kept.into_iter()
        .map(|c| DescriptionSnippet {
            chapter: c.chapter_title.clone(),
            paragraph: c.paragraph,
            text: c.text.clone(),
        })
        .collect()
}

/// The kind-matching pronouns (lowercased, a closed class) whose presence in a
/// name-less paragraph marks a continued description of the current anchor
/// entity — keyed by the manuscript `language`. Built-in for the five
/// supported languages (English / Russian / French / German / Spanish);
/// unknown languages fall back to English. Articles and other high-frequency
/// homographs (French `le`/`la`, Spanish `la`/`lo`, German `das`) are
/// deliberately excluded to keep attribution precise. Note: pro-drop
/// languages (Spanish) omit subject pronouns, so recall there is naturally
/// lower — possessives/objects carry most of the signal.
fn pronouns(language: &str, kind: EntityKind) -> &'static [&'static str] {
    use EntityKind::*;
    match language.trim().to_lowercase().as_str() {
        "russian" | "русский" => match kind {
            Character => &[
                "он", "его", "ему", "им", "нём", "нем", "она", "её", "ее", "ей", "ней", "они",
                "их", "ими", "них",
            ],
            Place => &["оно", "там", "тут", "здесь", "туда", "сюда"],
            Artefact => &["оно", "его", "ему", "им"],
        },
        "french" | "français" | "francais" => match kind {
            Character => &["il", "elle", "ils", "elles", "lui", "eux", "leur", "leurs"],
            Place => &["y", "là", "ici", "ça", "cela"],
            Artefact => &["ça", "cela", "celui", "celle"],
        },
        "german" | "deutsch" => match kind {
            Character => &[
                "er", "ihn", "ihm", "sein", "seine", "sie", "ihr", "ihre", "ihnen",
            ],
            Place => &["es", "da", "dort", "dorthin", "hier", "dahin"],
            Artefact => &["es", "dies", "dieses"],
        },
        "spanish" | "español" | "espanol" => match kind {
            Character => &[
                "él", "ella", "ellos", "ellas", "le", "les", "su", "sus", "suyo", "suya",
            ],
            Place => &["ahí", "allí", "allá", "aquí", "acá"],
            Artefact => &["ello", "eso", "esto"],
        },
        // english + fallback
        _ => match kind {
            Character => &[
                "he", "him", "his", "she", "her", "hers", "they", "them", "their", "theirs",
            ],
            Place => &["it", "its", "there", "here"],
            Artefact => &["it", "its"],
        },
    }
}

/// True when `name` appears in `haystack_lc` (already lowercased) as a whole
/// word / phrase — bounded by a non-alphanumeric **Unicode** char (or the
/// string edge) on both sides — so "Sam" doesn't match "Samuel" and the
/// boundary check works for Cyrillic / accented names. Pure.
pub fn mentions(haystack_lc: &str, name_lc: &str) -> bool {
    if name_lc.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(rel) = haystack_lc[from..].find(name_lc) {
        let start = from + rel;
        let end = start + name_lc.len();
        let before_ok = haystack_lc[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after_ok = haystack_lc[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        // advance past this match's first char (a valid boundary)
        from = start + name_lc.chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Lowercased word tokens of `text` (split on non-alphanumeric).
fn word_set(text_lc: &str) -> HashSet<&str> {
    text_lc
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect()
}

/// 1.3.11 WORLD-3 — coreference-lite: attribute pronoun-only descriptive
/// paragraphs to the most recently, *unambiguously* named entity of the
/// matching kind. `chapters` are the user-book chapters (each a list of
/// `(paragraph, text)` in order); `lexicon` is every entity's `(name, kind)`.
/// Returns `paragraph → entity names` it was attributed to. Precision-
/// favouring: a single named entity of a kind becomes that kind's anchor, two
/// or more clear it, and attribution never crosses a chapter. Pure.
pub fn attribute_continuations(
    chapters: &[Vec<(Uuid, String)>],
    lexicon: &[(String, EntityKind)],
    language: &str,
) -> HashMap<Uuid, Vec<String>> {
    let lex_lc: Vec<(String, String, EntityKind)> = lexicon
        .iter()
        .map(|(n, k)| (n.clone(), n.to_lowercase(), *k))
        .collect();
    let mut out: HashMap<Uuid, Vec<String>> = HashMap::new();
    for chapter in chapters {
        // The most recent single-named entity of each kind (the anchor).
        let mut anchor: HashMap<EntityKind, String> = HashMap::new();
        for (pid, text) in chapter {
            let lc = text.to_lowercase();
            // Which entities (by kind) does this paragraph name?
            let mut named_by_kind: HashMap<EntityKind, Vec<&str>> = HashMap::new();
            for (name, name_lc, kind) in &lex_lc {
                if mentions(&lc, name_lc) {
                    named_by_kind.entry(*kind).or_default().push(name.as_str());
                }
            }
            if named_by_kind.is_empty() {
                // Name-less paragraph — attribute to any anchor whose pronoun
                // appears (a continued description).
                let words = word_set(&lc);
                for (kind, anchor_name) in &anchor {
                    if pronouns(language, *kind).iter().any(|p| words.contains(*p)) {
                        out.entry(*pid).or_default().push(anchor_name.clone());
                    }
                }
            } else {
                // Update each named kind's anchor; a kind not named here keeps
                // its anchor (so a later pronoun still attributes).
                for (kind, names) in &named_by_kind {
                    if names.len() == 1 {
                        anchor.insert(*kind, names[0].to_string());
                    } else {
                        anchor.remove(kind);
                    }
                }
            }
        }
    }
    out
}

/// One adjudicated drift: two descriptions of the same entity that contradict.
/// `a` is the earlier passage, `b` the later (divergent) one — `paragraph_b`
/// is the jump target so the editor lands where the drift shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftConflict {
    pub entity: String,
    pub kind: EntityKind,
    pub a: String,
    pub b: String,
    pub chapter_a: String,
    pub chapter_b: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraph_b: Option<Uuid>,
    pub detail: String,
}

/// The drift report — every adjudicated contradiction. Serialised to
/// `<project>/.inkhaven/drift.json`; read deterministically by the Editorial
/// Pass + the story bible.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriftReport {
    #[serde(default)]
    pub version: String,
    /// Hash of the retrieved description set — lets consumers note staleness
    /// when the prose has moved since the scan.
    #[serde(default)]
    pub content_hash: u64,
    pub conflicts: Vec<DriftConflict>,
    /// 1.3.10 P3 — the description snippets the scan retrieved, persisted so
    /// the story bible can render each flagged entity's trail without
    /// recomputing (no embedding load in the TUI). Empty in older sidecars.
    #[serde(default)]
    pub descriptions: Vec<EntityDescriptions>,
    /// 1.3.12 — manuscript fingerprint at scan time (see
    /// `world_report::manuscript_fingerprint`); consumers flag the findings
    /// stale when it no longer matches. 0 in older sidecars.
    #[serde(default)]
    pub manuscript_fingerprint: u64,
}

impl DriftReport {
    pub fn sidecar_path(project_root: &Path) -> PathBuf {
        project_root.join(".inkhaven").join("drift.json")
    }
    pub fn load(project_root: &Path) -> std::io::Result<Self> {
        let path = Self::sidecar_path(project_root);
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }
    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = Self::sidecar_path(project_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::io_atomic::write(&path, &body)
    }
    /// Hash the retrieved descriptions (order-independent) so a moved/edited
    /// manuscript invalidates the cache.
    pub fn compute_hash(descs: &[EntityDescriptions]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut rows: Vec<String> = Vec::new();
        for d in descs {
            for s in &d.snippets {
                rows.push(format!("{}\u{1}{}\u{1}{}", d.entity, s.chapter, s.text));
            }
        }
        rows.sort();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for r in rows {
            r.hash(&mut h);
        }
        h.finish()
    }
}

/// Parse the drift judge's reply — one contradiction per line, `i | j | why`,
/// where `i` and `j` are 1-based description NUMBERS. Returns 0-based index
/// pairs. Tolerant: list markers, a header row, a none-sentinel, blank +
/// malformed lines, non-numeric or out-of-range / self-referential indices
/// are all dropped. Pure. `n` is the snippet count (for range-checking).
pub fn parse_drift_pairs(raw: &str, n: usize) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim().trim_start_matches(['-', '*', '•', ' ']).trim();
        if line.is_empty() || !line.contains('|') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '|').map(str::trim).collect();
        if parts.len() != 3 || parts[2].is_empty() {
            continue;
        }
        // tolerate "[2]" / "#2" / "2." around the index
        let idx = |s: &str| -> Option<usize> {
            s.trim_matches(|c: char| !c.is_ascii_digit())
                .parse::<usize>()
                .ok()
        };
        let (Some(i), Some(j)) = (idx(parts[0]), idx(parts[1])) else {
            continue;
        };
        if i == 0 || j == 0 || i > n || j > n || i == j {
            continue;
        }
        out.push((i - 1, j - 1, parts[2].to_string()));
    }
    out
}

/// Resolve parsed index pairs against the entity's snippets into
/// `DriftConflict`s: the earlier-chapter snippet becomes `a`, the later one
/// `b` (its paragraph is the jump target), quotes truncated for the worklist.
/// Pure.
pub fn resolve_conflicts(
    entity: &str,
    kind: EntityKind,
    snippets: &[DescriptionSnippet],
    pairs: &[(usize, usize, String)],
) -> Vec<DriftConflict> {
    let mut out = Vec::new();
    for (i, j, why) in pairs {
        let (Some(si), Some(sj)) = (snippets.get(*i), snippets.get(*j)) else {
            continue;
        };
        // snippets are chapter-ordered, so the smaller index is earlier.
        let (earlier, later) = if i <= j { (si, sj) } else { (sj, si) };
        out.push(DriftConflict {
            entity: entity.to_string(),
            kind,
            a: quote(&earlier.text),
            b: quote(&later.text),
            chapter_a: earlier.chapter.clone(),
            chapter_b: later.chapter.clone(),
            paragraph_b: Some(later.paragraph),
            detail: why.trim().to_string(),
        });
    }
    out
}

/// Truncate a description to a worklist-friendly quote (first sentence-ish,
/// hard-capped). Pure.
fn quote(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let capped: String = one_line.chars().take(120).collect();
    if one_line.chars().count() > 120 {
        format!("{}…", capped.trim_end())
    } else {
        capped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(order: usize, chapter: &str, text: &str) -> Candidate {
        Candidate {
            paragraph: Uuid::now_v7(),
            chapter_order: order,
            chapter_title: chapter.into(),
            text: text.into(),
        }
    }

    #[test]
    fn keeps_only_paragraphs_that_mention_the_entity() {
        // retrieval drags in a topically-similar paragraph that never names
        // the tavern — the name anchor drops it.
        let cands = vec![
            cand(2, "ch-2", "The Drunken Goose was cramped and smoky."),
            cand(5, "ch-5", "The inn down the road smelled of woodsmoke."), // no name
            cand(8, "ch-8", "By winter the Drunken Goose felt airy and bright."),
        ];
        let out = assemble_descriptions("The Drunken Goose", &cands, 8, &HashSet::new());
        assert_eq!(out.len(), 2, "the un-named inn paragraph is filtered out");
        assert!(out[0].text.contains("cramped"));
        assert!(out[1].text.contains("airy"));
    }

    #[test]
    fn coref_attributed_paragraph_is_kept_despite_no_name() {
        let pronoun_para = Uuid::now_v7();
        let cands = vec![
            cand(2, "ch-2", "The Drunken Goose was cramped and smoky."),
            Candidate {
                paragraph: pronoun_para,
                chapter_order: 6,
                chapter_title: "ch-6".into(),
                text: "Inside, it felt airy and bright.".into(), // no name — coref rescue
            },
        ];
        let coref: HashSet<Uuid> = [pronoun_para].into_iter().collect();
        let out = assemble_descriptions("The Drunken Goose", &cands, 8, &coref);
        assert_eq!(out.len(), 2, "the coref-attributed pronoun paragraph is kept");
        assert!(out[1].text.contains("airy"));
    }

    #[test]
    fn dedups_and_orders_by_chapter_then_caps_by_relevance() {
        let p = Uuid::now_v7();
        // same paragraph twice (retrieval can repeat) → one survives
        let dup_a = Candidate { paragraph: p, chapter_order: 9, chapter_title: "ch-9".into(), text: "Mara spoke softly.".into() };
        let dup_b = Candidate { paragraph: p, chapter_order: 9, chapter_title: "ch-9".into(), text: "Mara spoke softly.".into() };
        // relevance order (input order) is 1,2,3; chapter order is 9,1,4 →
        // the cap takes the first `max` by relevance, output sorts by chapter.
        let cands = vec![
            dup_a,
            dup_b,
            cand(1, "ch-1", "Mara, soft-spoken as ever."),
            cand(4, "ch-4", "Mara's voice boomed across the hall."),
        ];
        let out = assemble_descriptions("Mara", &cands, 2, &HashSet::new());
        assert_eq!(out.len(), 2, "dup collapses, cap=2 honoured");
        assert_eq!(out[0].chapter, "ch-1", "presented in chapter order");
        assert_eq!(out[1].chapter, "ch-9");
    }

    #[test]
    fn empty_entity_or_zero_cap_returns_nothing() {
        let cands = vec![cand(1, "ch-1", "anything")];
        assert!(assemble_descriptions("", &cands, 8, &HashSet::new()).is_empty());
        assert!(assemble_descriptions("x", &cands, 0, &HashSet::new()).is_empty());
    }

    #[test]
    fn attribute_continuations_recency_and_ambiguity() {
        let p_named = Uuid::now_v7();
        let p_pron = Uuid::now_v7();
        let p_other = Uuid::now_v7();
        let lexicon = vec![
            ("Mara".to_string(), EntityKind::Character),
            ("Joss".to_string(), EntityKind::Character),
            ("The Goose".to_string(), EntityKind::Place),
        ];
        // ch-1: Mara named, then a pronoun-only descriptive para → attributes to Mara.
        // a third para names BOTH Mara and Joss (ambiguous), then a pronoun para
        // → no attribution (anchor cleared).
        let p_ambig = Uuid::now_v7();
        let p_after_ambig = Uuid::now_v7();
        let chapters = vec![vec![
            (p_named, "Mara crossed the yard.".to_string()),
            (p_pron, "She was taller than he remembered, her hair gone grey.".to_string()),
            (p_other, "The Goose stood at the corner.".to_string()),
            (p_ambig, "Mara and Joss argued by the door.".to_string()),
            (p_after_ambig, "She would not look at him.".to_string()),
        ]];
        let map = attribute_continuations(&chapters, &lexicon, "english");
        assert_eq!(map.get(&p_pron).map(|v| v.as_slice()), Some(&["Mara".to_string()][..]));
        assert!(!map.contains_key(&p_after_ambig), "ambiguous anchor → no attribution");
        // the place pronoun "there"/"it" never appeared, so The Goose attributes nothing
        assert!(map.values().all(|v| !v.contains(&"The Goose".to_string())));
    }

    #[test]
    fn attribute_continuations_does_not_cross_chapters() {
        let p_named = Uuid::now_v7();
        let p_next_chapter = Uuid::now_v7();
        let lexicon = vec![("Mara".to_string(), EntityKind::Character)];
        let chapters = vec![
            vec![(p_named, "Mara waited.".to_string())],
            vec![(p_next_chapter, "She sighed.".to_string())], // new chapter — no anchor
        ];
        let map = attribute_continuations(&chapters, &lexicon, "english");
        assert!(!map.contains_key(&p_next_chapter), "anchor resets per chapter");
    }

    #[test]
    fn coref_is_multilingual_russian() {
        let p_named = Uuid::now_v7();
        let p_pron = Uuid::now_v7();
        let lexicon = vec![("Мара".to_string(), EntityKind::Character)];
        // "Mara crossed the yard. She was tall." in Russian — the pronoun is "она".
        let chapters = vec![vec![
            (p_named, "Мара пересекла двор.".to_string()),
            (p_pron, "Она была выше, чем он помнил.".to_string()),
        ]];
        // English pronouns wouldn't match Russian text…
        assert!(
            !attribute_continuations(&chapters, &lexicon, "english").contains_key(&p_pron),
            "english pronoun set must not fire on Russian prose"
        );
        // …but the Russian set does.
        let ru = attribute_continuations(&chapters, &lexicon, "russian");
        assert_eq!(ru.get(&p_pron).map(|v| v.as_slice()), Some(&["Мара".to_string()][..]));
    }

    #[test]
    fn mentions_respects_word_boundaries_including_unicode() {
        assert!(mentions("mara crossed the yard", "mara"));
        assert!(!mentions("samuel spoke", "sam"), "no substring false-match");
        assert!(mentions("the drunken goose was loud", "drunken goose"));
        // Unicode: "мара" must not match inside "марашка"; must match standalone.
        assert!(mentions("мара пересекла двор", "мара"));
        assert!(!mentions("марашка сидела тихо", "мара"), "no Cyrillic substring false-match");
    }

    #[test]
    fn parse_drift_pairs_reads_indices_and_skips_noise() {
        let raw = "\
i | j | why\n\
- [1] | [2] | cramped vs airy\n\
2 | 4 | soft vs booming\n\
3 | 3 | self-reference (dropped)\n\
9 | 1 | out of range (dropped)\n\
none\n\
gibberish without a pipe\n";
        let pairs = parse_drift_pairs(raw, 4);
        // header ("i|j|why" — non-numeric → dropped), the two valid rows kept,
        // self-ref + out-of-range + sentinel + no-pipe all dropped.
        assert_eq!(pairs, vec![(0, 1, "cramped vs airy".into()), (1, 3, "soft vs booming".into())]);
    }

    #[test]
    fn resolve_conflicts_orders_earlier_first_and_sets_jump() {
        let s1 = DescriptionSnippet { chapter: "ch-2".into(), paragraph: Uuid::now_v7(), text: "cramped and smoky".into() };
        let s2 = DescriptionSnippet { chapter: "ch-20".into(), paragraph: Uuid::now_v7(), text: "airy and bright".into() };
        let snippets = vec![s1.clone(), s2.clone()];
        // pair given out of order (j<i) — resolver still puts the earlier first.
        let pairs = vec![(1, 0, "atmosphere flipped".to_string())];
        let out = resolve_conflicts("The Drunken Goose", EntityKind::Place, &snippets, &pairs);
        assert_eq!(out.len(), 1);
        let c = &out[0];
        assert_eq!(c.chapter_a, "ch-2", "earlier chapter is a");
        assert_eq!(c.chapter_b, "ch-20");
        assert_eq!(c.paragraph_b, Some(s2.paragraph), "jump targets the later, divergent passage");
        assert_eq!(c.kind, EntityKind::Place);
        assert!(c.a.contains("cramped") && c.b.contains("airy"));
    }

    #[test]
    fn report_hash_is_order_independent_and_round_trips() {
        let mk = |ch: &str, t: &str| DescriptionSnippet { chapter: ch.into(), paragraph: Uuid::now_v7(), text: t.into() };
        let a = EntityDescriptions { entity: "Mara".into(), kind: EntityKind::Character, snippets: vec![mk("ch-1", "soft"), mk("ch-4", "loud")] };
        let b = EntityDescriptions { entity: "Goose".into(), kind: EntityKind::Place, snippets: vec![mk("ch-2", "smoky")] };
        let h1 = DriftReport::compute_hash(&[a.clone(), b.clone()]);
        let h2 = DriftReport::compute_hash(&[b, a]);
        assert_eq!(h1, h2, "hash ignores entity/snippet order");

        let dir = tempfile::tempdir().unwrap();
        let report = DriftReport {
            version: "x".into(),
            content_hash: h1,
            conflicts: vec![DriftConflict {
                entity: "Mara".into(),
                kind: EntityKind::Character,
                a: "soft".into(),
                b: "loud".into(),
                chapter_a: "ch-1".into(),
                chapter_b: "ch-4".into(),
                paragraph_b: Some(Uuid::now_v7()),
                detail: "voice flipped".into(),
            }],
            descriptions: Vec::new(),
            manuscript_fingerprint: 0,
        };
        report.save(dir.path()).unwrap();
        let loaded = DriftReport::load(dir.path()).unwrap();
        assert_eq!(loaded.conflicts, report.conflicts);
        assert_eq!(loaded.content_hash, h1);
    }
}
