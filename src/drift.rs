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

use serde::Serialize;
use uuid::Uuid;

/// Which entity book a description belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct DescriptionSnippet {
    pub chapter: String,
    pub paragraph: Uuid,
    pub text: String,
}

/// The description snippets retrieved for one entity, chapter-ordered.
#[derive(Debug, Clone, Serialize)]
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
/// actually **mention** `entity` (the name anchor that kills topical
/// false-positives the vector search drags in), dedup by paragraph, take the
/// top `max_snippets` by relevance, then present them in **chapter order** so
/// the judge reads the description as a timeline. Pure.
pub fn assemble_descriptions(
    entity: &str,
    candidates: &[Candidate],
    max_snippets: usize,
) -> Vec<DescriptionSnippet> {
    let needle = entity.trim().to_lowercase();
    if needle.is_empty() || max_snippets == 0 {
        return Vec::new();
    }
    let mut seen = std::collections::HashSet::new();
    let mut kept: Vec<&Candidate> = Vec::new();
    for c in candidates {
        if kept.len() >= max_snippets {
            break;
        }
        if !c.text.to_lowercase().contains(&needle) {
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
        let out = assemble_descriptions("The Drunken Goose", &cands, 8);
        assert_eq!(out.len(), 2, "the un-named inn paragraph is filtered out");
        assert!(out[0].text.contains("cramped"));
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
        let out = assemble_descriptions("Mara", &cands, 2);
        assert_eq!(out.len(), 2, "dup collapses, cap=2 honoured");
        assert_eq!(out[0].chapter, "ch-1", "presented in chapter order");
        assert_eq!(out[1].chapter, "ch-9");
    }

    #[test]
    fn empty_entity_or_zero_cap_returns_nothing() {
        let cands = vec![cand(1, "ch-1", "anything")];
        assert!(assemble_descriptions("", &cands, 8).is_empty());
        assert!(assemble_descriptions("x", &cands, 0).is_empty());
    }
}
