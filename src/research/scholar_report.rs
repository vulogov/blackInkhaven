//! SCHOLAR P3 (1.6.18+) — the persistent, topic-clustered SCHOLAR report.
//!
//! `/contradict`, `/converge`, and `/relate` render their findings into the chat
//! and then drop them. P3 taps the deduped findings at their finish sites,
//! projects them into serializable rows **keyed by topic** (the Facts-tree
//! branch for pair findings; the claim for relations), and merges them into a
//! persistent report at `.inkhaven/scholar_report.json` — accumulating across
//! sessions and across scan kinds. `/report` renders it, grouped by topic, and
//! flags **staleness** when the Facts corpus has changed since the findings were
//! gathered (an order-independent hash of the fact texts, like `facts_scan`).
//!
//! Persistence mirrors `provenance` / `facts_scan`: load-or-default, atomic write
//! via `io_atomic`. Headings localize to the project language (en/ru/fr/de/es).

use serde::{Deserialize, Serialize};

use crate::project::ProjectLayout;

use super::contradiction::{Clash, Relation};
use super::factcheck::{self, FactEntry};

const VERSION: u32 = 1;

/// One contradiction/convergence pair, flattened for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PairRow {
    topic: String,
    a_text: String,
    b_text: String,
    a_origin: String,
    b_origin: String,
    cross_source: bool,
    reason: String,
}

impl PairRow {
    fn from_clash(c: &Clash) -> PairRow {
        PairRow {
            topic: factcheck::branch_label(&c.a.location),
            a_text: c.a.text.clone(),
            b_text: c.b.text.clone(),
            a_origin: c.a.origin.clone(),
            b_origin: c.b.origin.clone(),
            cross_source: c.is_cross_source(),
            reason: c.reason.clone(),
        }
    }
}

/// One graded relation between a claim and a piece of evidence, flattened.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelationRow {
    /// The claim the relation was judged against — the grouping topic.
    claim: String,
    label: String,
    stance: String,
    against: bool,
    reason: String,
}

/// The persisted SCHOLAR report: the latest contradiction + convergence scans and
/// the accumulated per-claim relations, plus a staleness hash of the corpus.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct ScholarReport {
    version: u32,
    language: String,
    updated_at: String,
    /// Order-independent hash of the Facts corpus at the last write — `/report`
    /// compares it against a fresh hash to warn when findings may be stale.
    corpus_hash: u64,
    contradictions: Vec<PairRow>,
    convergences: Vec<PairRow>,
    relations: Vec<RelationRow>,
}

impl ScholarReport {
    fn path(layout: &ProjectLayout) -> std::path::PathBuf {
        layout.root.join(".inkhaven").join("scholar_report.json")
    }

    /// Load the report, or an empty one when absent / unreadable.
    pub(super) fn load(layout: &ProjectLayout) -> ScholarReport {
        match std::fs::read_to_string(ScholarReport::path(layout)) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => ScholarReport::default(),
        }
    }

    fn save(&self, layout: &ProjectLayout) -> std::io::Result<()> {
        let dir = layout.root.join(".inkhaven");
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::io_atomic::write(&ScholarReport::path(layout), json.as_bytes())
    }

    /// Order-independent hash of the fact texts (staleness signal), matching
    /// `facts_scan::compute_hash`.
    pub(super) fn corpus_hash(facts: &[FactEntry]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut texts: Vec<&str> = facts.iter().map(|f| f.text.as_str()).collect();
        texts.sort_unstable();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for t in texts {
            t.hash(&mut h);
        }
        h.finish()
    }

    fn stamp(&mut self, language: &str, hash: u64) {
        self.version = VERSION;
        self.language = language.to_string();
        self.corpus_hash = hash;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Replace the contradiction- or convergence-scan slice with a fresh scan's
    /// findings and persist. Called at the `/contradict` and `/converge` finish
    /// sites (and their CLI twins).
    pub(super) fn record_pairs(
        layout: &ProjectLayout,
        language: &str,
        converge: bool,
        pairs: &[Clash],
        hash: u64,
    ) {
        let mut report = ScholarReport::load(layout);
        let rows: Vec<PairRow> = pairs.iter().map(PairRow::from_clash).collect();
        if converge {
            report.convergences = rows;
        } else {
            report.contradictions = rows;
        }
        report.stamp(language, hash);
        let _ = report.save(layout);
    }

    /// Merge a `/relate` scan's relations for one claim (replacing any prior rows
    /// for the same claim) and persist.
    pub(super) fn record_relations(
        layout: &ProjectLayout,
        language: &str,
        claim: &str,
        relations: &[Relation],
        hash: u64,
    ) {
        let mut report = ScholarReport::load(layout);
        let claim = claim.trim().to_string();
        report.relations.retain(|r| r.claim != claim);
        for r in relations {
            report.relations.push(RelationRow {
                claim: claim.clone(),
                label: r.label.clone(),
                stance: r.stance.label().to_string(),
                against: r.stance.is_against(),
                reason: r.reason.clone(),
            });
        }
        report.stamp(language, hash);
        let _ = report.save(layout);
    }

    /// Set the display language when the report has none stored yet (unwritten) —
    /// so `/report` on an empty corpus still localizes its hint.
    pub(super) fn set_language_if_unset(&mut self, lang: &str) {
        if self.language.trim().is_empty() {
            self.language = lang.to_string();
        }
    }

    fn is_empty(&self) -> bool {
        self.contradictions.is_empty() && self.convergences.is_empty() && self.relations.is_empty()
    }

    /// Render the report as human text, grouped by topic within each section.
    /// `current_hash` is a fresh corpus hash; a mismatch prepends a staleness
    /// warning (the findings predate the current facts).
    pub(super) fn render(&self, current_hash: u64) -> String {
        let l = Labels::for_language(&self.language);
        if self.is_empty() {
            return l.empty.to_string();
        }
        let mut s = String::new();
        s.push_str(l.title);
        if !self.updated_at.is_empty() {
            s.push_str(&format!("  ({} {})\n", l.updated, short_date(&self.updated_at)));
        } else {
            s.push('\n');
        }
        if self.corpus_hash != 0 && current_hash != self.corpus_hash {
            s.push_str(&format!("\n⚠ {}\n", l.stale));
        }

        if !self.contradictions.is_empty() {
            s.push_str(&format!("\n{} ({})\n", l.contradictions, self.contradictions.len()));
            render_pairs(&mut s, &self.contradictions, '⇄', &l);
        }
        if !self.convergences.is_empty() {
            s.push_str(&format!("\n{} ({})\n", l.convergences, self.convergences.len()));
            render_pairs(&mut s, &self.convergences, '≈', &l);
        }
        if !self.relations.is_empty() {
            s.push_str(&format!("\n{} ({})\n", l.relations, self.relations.len()));
            render_relations_grouped(&mut s, &self.relations, &l);
        }
        s
    }
}

/// Group pair rows by topic and render each cluster.
fn render_pairs(s: &mut String, rows: &[PairRow], sep: char, l: &Labels) {
    for (topic, group) in group_by(rows, |r| r.topic.clone()) {
        s.push_str(&format!("\n  ▸ {topic}\n"));
        for r in group {
            let tag = if r.cross_source { l.cross_source } else { l.within_source };
            s.push_str(&format!(
                "    · {}  ⟨{}⟩\n    {sep} {}  ⟨{}⟩\n",
                truncate(&r.a_text, 140),
                r.a_origin,
                truncate(&r.b_text, 140),
                r.b_origin,
            ));
            s.push_str(&format!("      [{tag}]"));
            if !r.reason.is_empty() {
                s.push_str(&format!(" — {}", truncate(&r.reason, 160)));
            }
            s.push('\n');
        }
    }
}

/// Group relation rows by claim, splitting against vs supporting.
fn render_relations_grouped(s: &mut String, rows: &[RelationRow], l: &Labels) {
    for (claim, group) in group_by(rows, |r| r.claim.clone()) {
        s.push_str(&format!("\n  ▸ “{}”\n", truncate(&claim, 160)));
        let against = group.iter().filter(|r| r.against).count();
        let support = group.len() - against;
        s.push_str(&format!("    {against} {} · {support} {}\n", l.against, l.supporting));
        for r in group {
            let mark = if r.against { "⚠" } else { "✓" };
            s.push_str(&format!("    {mark} [{}] {}", r.stance, r.label));
            if !r.reason.is_empty() {
                s.push_str(&format!(" — {}", truncate(&r.reason, 160)));
            }
            s.push('\n');
        }
    }
}

/// Stable group-by preserving first-seen key order.
fn group_by<T: Clone, F: Fn(&T) -> String>(rows: &[T], key: F) -> Vec<(String, Vec<T>)> {
    let mut order: Vec<String> = Vec::new();
    let mut map: std::collections::HashMap<String, Vec<T>> = std::collections::HashMap::new();
    for r in rows {
        let k = key(r);
        if !map.contains_key(&k) {
            order.push(k.clone());
        }
        map.entry(k).or_default().push(r.clone());
    }
    order.into_iter().map(|k| (k.clone(), map.remove(&k).unwrap_or_default())).collect()
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let cut: String = t.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// `2026-07-13T…` → `2026-07-13`.
fn short_date(rfc3339: &str) -> &str {
    rfc3339.split('T').next().unwrap_or(rfc3339)
}

/// Localized section headings (en/ru/fr/de/es), mirroring the house
/// `Labels::for_language` pattern.
struct Labels {
    title: &'static str,
    updated: &'static str,
    stale: &'static str,
    empty: &'static str,
    contradictions: &'static str,
    convergences: &'static str,
    relations: &'static str,
    cross_source: &'static str,
    within_source: &'static str,
    against: &'static str,
    supporting: &'static str,
}

impl Labels {
    fn for_language(lang: &str) -> Labels {
        match lang.trim().to_lowercase().as_str() {
            "ru" | "russian" | "русский" => Labels {
                title: "СВОДКА SCHOLAR — противоречия, схождения и связи",
                updated: "обновлено",
                stale: "Факты изменились с момента анализа — перезапустите /contradict, /converge, /relate.",
                empty: "Отчёт SCHOLAR пуст — запустите /contradict, /converge или /relate.",
                contradictions: "Противоречия",
                convergences: "Схождения (триангуляция)",
                relations: "Связи с утверждениями",
                cross_source: "между источниками",
                within_source: "внутри источника",
                against: "против",
                supporting: "в поддержку",
            },
            "fr" | "french" | "français" => Labels {
                title: "RAPPORT SCHOLAR — contradictions, convergences et relations",
                updated: "mis à jour",
                stale: "Les faits ont changé depuis l'analyse — relancez /contradict, /converge, /relate.",
                empty: "Le rapport SCHOLAR est vide — lancez /contradict, /converge ou /relate.",
                contradictions: "Contradictions",
                convergences: "Convergences (triangulées)",
                relations: "Relations aux affirmations",
                cross_source: "entre sources",
                within_source: "dans une source",
                against: "contre",
                supporting: "en appui",
            },
            "de" | "german" | "deutsch" => Labels {
                title: "SCHOLAR-BERICHT — Widersprüche, Konvergenzen und Bezüge",
                updated: "aktualisiert",
                stale: "Die Fakten haben sich seit der Analyse geändert — /contradict, /converge, /relate erneut ausführen.",
                empty: "Der SCHOLAR-Bericht ist leer — /contradict, /converge oder /relate ausführen.",
                contradictions: "Widersprüche",
                convergences: "Konvergenzen (trianguliert)",
                relations: "Bezüge zu Behauptungen",
                cross_source: "quellenübergreifend",
                within_source: "innerhalb einer Quelle",
                against: "dagegen",
                supporting: "stützend",
            },
            "es" | "spanish" | "español" => Labels {
                title: "INFORME SCHOLAR — contradicciones, convergencias y relaciones",
                updated: "actualizado",
                stale: "Los hechos han cambiado desde el análisis — vuelve a ejecutar /contradict, /converge, /relate.",
                empty: "El informe SCHOLAR está vacío — ejecuta /contradict, /converge o /relate.",
                contradictions: "Contradicciones",
                convergences: "Convergencias (trianguladas)",
                relations: "Relaciones con afirmaciones",
                cross_source: "entre fuentes",
                within_source: "dentro de una fuente",
                against: "en contra",
                supporting: "en apoyo",
            },
            _ => Labels {
                title: "SCHOLAR REPORT — contradictions, convergences, and relations",
                updated: "updated",
                stale: "The facts have changed since these were gathered — re-run /contradict, /converge, /relate.",
                empty: "The SCHOLAR report is empty — run /contradict, /converge, or /relate first.",
                contradictions: "Contradictions",
                convergences: "Convergences (triangulated)",
                relations: "Relations to claims",
                cross_source: "cross-source",
                within_source: "within-source",
                against: "against",
                supporting: "supporting",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::contradiction::{Relation, Stance};

    fn report_with(pairs_converge: bool) -> ScholarReport {
        let mut r = ScholarReport::default();
        r.contradictions = vec![PairRow {
            topic: "Geography".into(),
            a_text: "The tower is 90m tall.".into(),
            b_text: "The tower is 120m tall.".into(),
            a_origin: "archive".into(),
            b_origin: "web".into(),
            cross_source: true,
            reason: "conflicting heights".into(),
        }];
        if pairs_converge {
            r.convergences = vec![PairRow {
                topic: "Dates".into(),
                a_text: "Founded 1701.".into(),
                b_text: "Established in 1701.".into(),
                a_origin: "archive".into(),
                b_origin: "wikisource".into(),
                cross_source: true,
                reason: "same founding year".into(),
            }];
        }
        r.relations = vec![RelationRow {
            claim: "The city was a trade hub.".into(),
            label: "source: Herodotus".into(),
            stance: "agrees".into(),
            against: false,
            reason: "describes its markets".into(),
        }];
        r.stamp("en", 42);
        r
    }

    #[test]
    fn corpus_hash_is_order_independent() {
        let fe = |t: &str| FactEntry { id: uuid::Uuid::now_v7(), location: "x".into(), text: t.into() };
        let h1 = ScholarReport::corpus_hash(&[fe("one"), fe("two")]);
        let h2 = ScholarReport::corpus_hash(&[fe("two"), fe("one")]);
        assert_eq!(h1, h2);
    }

    #[test]
    fn renders_grouped_sections_and_flags_staleness() {
        let r = report_with(true);
        // Same hash → no staleness warning.
        let fresh = r.render(42);
        assert!(fresh.contains("Contradictions"));
        assert!(fresh.contains("Convergences"));
        assert!(fresh.contains("Relations to claims"));
        assert!(fresh.contains("▸ Geography"));
        assert!(fresh.contains("cross-source"));
        assert!(!fresh.contains("⚠"), "no staleness warning when hashes match");
        // Different hash → staleness warning.
        assert!(r.render(99).contains("⚠"));
    }

    #[test]
    fn empty_report_renders_hint() {
        let r = ScholarReport::default();
        assert!(r.render(0).contains("empty") || r.render(0).contains("run /contradict"));
    }

    #[test]
    fn relations_replace_per_claim_not_append() {
        // Two records for the same claim keep only the latest set.
        let rels_a = vec![Relation { label: "source: A".into(), stance: Stance::Agrees, reason: "x".into() }];
        let rels_b = vec![
            Relation { label: "source: B".into(), stance: Stance::Contradicts, reason: "y".into() },
            Relation { label: "source: C".into(), stance: Stance::Qualifies, reason: "z".into() },
        ];
        let mut r = ScholarReport::default();
        // Simulate record_relations' merge logic inline (no disk).
        let claim = "same claim";
        for rels in [&rels_a, &rels_b] {
            r.relations.retain(|row| row.claim != claim);
            for rel in rels.iter() {
                r.relations.push(RelationRow {
                    claim: claim.into(),
                    label: rel.label.clone(),
                    stance: rel.stance.label().into(),
                    against: rel.stance.is_against(),
                    reason: rel.reason.clone(),
                });
            }
        }
        assert_eq!(r.relations.len(), 2, "latest set replaces the earlier one");
        assert!(r.relations.iter().any(|row| row.label == "source: B"));
        assert!(!r.relations.iter().any(|row| row.label == "source: A"));
    }

    #[test]
    fn localized_headings_differ_by_language() {
        assert_eq!(Labels::for_language("ru").contradictions, "Противоречия");
        assert_eq!(Labels::for_language("fr").contradictions, "Contradictions");
        assert_eq!(Labels::for_language("de").contradictions, "Widersprüche");
        assert_eq!(Labels::for_language("es").contradictions, "Contradicciones");
        assert_eq!(Labels::for_language("en").contradictions, "Contradictions");
    }
}
