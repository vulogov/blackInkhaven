//! SCHOLAR P1 (1.6.16+) — `/contradict`: structured, source-aware contradiction
//! analysis over the collected Facts.
//!
//! `/factcheck`'s consistency pass asks the model for `<a> ⇄ <b>` contradicting
//! pairs but never parses the reply — it's concatenated into one opaque string,
//! unattributed to any source. This module reuses that prompt + grouping, but
//! (1) joins each gathered fact to its recorded provenance (`SourcedFact`), and
//! (2) **parses** the reply into addressable `Clash` objects, so a contradiction
//! becomes "which sources disagree" — cross-source (two origins) vs within-source
//! (one origin contradicting itself). The engine is pure; the LLM call + report
//! surfacing live in the caller (`contradict_cli`, and later the research TUI).

use uuid::Uuid;

use super::factcheck::FactEntry;
use super::provenance::Provenance;

/// A gathered fact joined with its recorded provenance.
#[derive(Debug, Clone)]
pub(super) struct SourcedFact {
    pub id: Uuid,
    #[allow(dead_code)]
    pub location: String,
    pub text: String,
    /// `SourceRecord.origin` ("archive", "web", "model", …), or "manual" when none
    /// was recorded for this fact.
    pub origin: String,
    /// `SourceRecord.summary()` — a human source line, or "" when none.
    pub source: String,
}

impl SourcedFact {
    fn from_fact(f: &FactEntry, prov: &Provenance) -> SourcedFact {
        let rec = prov.for_node(&f.id.to_string());
        SourcedFact {
            id: f.id,
            location: f.location.clone(),
            text: f.text.clone(),
            origin: rec
                .map(|r| r.origin.clone())
                .filter(|o| !o.trim().is_empty())
                .unwrap_or_else(|| "manual".to_string()),
            source: rec.map(|r| r.summary()).unwrap_or_default(),
        }
    }
}

/// One structured, addressable contradiction finding. (P1 findings are all
/// contradictions; the graded stance arrives in SCHOLAR P2.)
#[derive(Debug, Clone)]
pub(super) struct Clash {
    pub a: SourcedFact,
    pub b: SourcedFact,
    pub reason: String,
}

impl Clash {
    /// True when the two facts come from different origins — a contradiction
    /// *between sources*, as opposed to a source contradicting itself.
    pub(super) fn is_cross_source(&self) -> bool {
        self.a.origin != self.b.origin
    }
}

/// Join an already-gathered fact list with provenance, preserving order (so a
/// `consistency_groups` partition over the same `FactEntry` list indexes into
/// both alike).
pub(super) fn join_provenance(facts: &[FactEntry], prov: &Provenance) -> Vec<SourcedFact> {
    facts.iter().map(|f| SourcedFact::from_fact(f, prov)).collect()
}

/// Number a group's facts for the consistency prompt (1-based) — the same shape
/// as `factcheck::consistency_user`, over `SourcedFact`.
pub(super) fn consistency_user(facts: &[SourcedFact]) -> String {
    let mut s = String::from("Facts:\n");
    for (i, f) in facts.iter().enumerate() {
        s.push_str(&format!("{}. {}\n", i + 1, f.text));
    }
    s
}

/// Parse a paired reply (`<a> SEP <b> — <reason>` lines) into structured pairs
/// over `facts` (1-based). `sep` is `⇄` for contradictions, `≈` for convergence.
/// Out-of-range and self-pairs are dropped.
pub(super) fn parse_pairs(reply: &str, facts: &[SourcedFact], sep: char) -> Vec<Clash> {
    let mut out = Vec::new();
    for line in reply.lines() {
        let line = line
            .trim()
            .trim_start_matches(['-', '*', '•', ' '])
            .trim();
        let Some((left, right)) = line.split_once(sep) else {
            continue;
        };
        let Some(a) = leading_index(left) else { continue };
        let (b_part, reason) = right
            .split_once('—')
            .or_else(|| right.split_once(" - "))
            .unwrap_or((right, ""));
        let Some(b) = leading_index(b_part) else { continue };
        if a == 0 || b == 0 || a == b || a > facts.len() || b > facts.len() {
            continue;
        }
        out.push(Clash {
            a: facts[a - 1].clone(),
            b: facts[b - 1].clone(),
            reason: reason.trim().to_string(),
        });
    }
    out
}

/// Parse a consistency reply (`<a> ⇄ <b> — <what conflicts>`) into clashes.
pub(super) fn parse_clashes(reply: &str, facts: &[SourcedFact]) -> Vec<Clash> {
    parse_pairs(reply, facts, '⇄')
}

/// Parse a convergence reply (`<a> ≈ <b> — <shared claim>`) into agreeing pairs.
pub(super) fn parse_concords(reply: &str, facts: &[SourcedFact]) -> Vec<Clash> {
    parse_pairs(reply, facts, '≈')
}

/// The decimal value of a digit char across the common scripts a model might
/// reply in — ASCII, Arabic-Indic, Extended Arabic-Indic (Persian/Urdu), and
/// Devanagari — so a numbered reply in a non-Latin script still parses. (The
/// five first-class languages all use ASCII digits; this covers beyond them.)
fn digit_value(c: char) -> Option<u32> {
    match c {
        '0'..='9' => Some(c as u32 - '0' as u32),
        '\u{0660}'..='\u{0669}' => Some(c as u32 - 0x0660), // Arabic-Indic ٠–٩
        '\u{06F0}'..='\u{06F9}' => Some(c as u32 - 0x06F0), // Extended Arabic-Indic ۰–۹
        '\u{0966}'..='\u{096F}' => Some(c as u32 - 0x0966), // Devanagari ०–९
        _ => None,
    }
}

/// The first run of digits in `s` (any of the supported scripts), as a `usize`
/// (e.g. "Fact 2" → 2, "٢." → 2).
fn leading_index(s: &str) -> Option<usize> {
    let mut val: Option<usize> = None;
    for c in s.chars() {
        match digit_value(c) {
            Some(d) => val = Some(val.unwrap_or(0) * 10 + d as usize),
            None if val.is_some() => break, // the digit run ended
            None => {}                       // skip leading non-digits
        }
    }
    val
}

/// Drop duplicate pairs (the cross-branch pass can re-find a within-branch pair),
/// keyed by the unordered fact-id pair.
pub(super) fn dedupe(clashes: Vec<Clash>) -> Vec<Clash> {
    use std::collections::HashSet;
    let mut seen: HashSet<(Uuid, Uuid)> = HashSet::new();
    let mut out = Vec::new();
    for c in clashes {
        let key = if c.a.id <= c.b.id { (c.a.id, c.b.id) } else { (c.b.id, c.a.id) };
        if seen.insert(key) {
            out.push(c);
        }
    }
    out
}

/// Render the clashes as a human report (CLI stdout / research-TUI body).
pub(super) fn render_report(clashes: &[Clash]) -> String {
    if clashes.is_empty() {
        return "No contradictions found across the collected facts.".to_string();
    }
    let cross = clashes.iter().filter(|c| c.is_cross_source()).count();
    let within = clashes.len() - cross;
    let mut s = format!(
        "Found {} contradiction(s) — {cross} between sources, {within} within a source.\n",
        clashes.len()
    );
    for (i, c) in clashes.iter().enumerate() {
        let tag = if c.is_cross_source() { "cross-source" } else { "within-source" };
        let sa = source_label(&c.a);
        let sb = source_label(&c.b);
        s.push_str(&format!(
            "\n{}. [{tag}]\n   · {}  ⟨{sa}⟩\n   ⇄ {}  ⟨{sb}⟩\n",
            i + 1,
            truncate(&c.a.text, 160),
            truncate(&c.b.text, 160),
        ));
        if !c.reason.is_empty() {
            s.push_str(&format!("   conflict: {}\n", c.reason));
        }
    }
    s
}

fn source_label(f: &SourcedFact) -> String {
    if f.source.trim().is_empty() {
        f.origin.clone()
    } else {
        f.source.clone()
    }
}

/// The convergence (agreeing-pairs) system prompt — the confirmation counterpart
/// to `factcheck::consistency_system`. Reuses `consistency_user` for the numbered
/// facts.
pub(super) fn agreement_system(language: &str) -> String {
    format!(
        "You are checking a writer's reference database for CONVERGING evidence. Below are \
         numbered facts. Identify every PAIR of facts that AGREE — that independently support \
         the same conclusion (not mere restatement; genuine mutual support). Respond with one \
         line per converging pair, in this exact shape:\n\
         <a> ≈ <b> — <the shared claim>\n\
         If no facts converge, reply exactly: No convergence found. Write the explanations in {language}."
    )
}

/// Render converging pairs. Cross-source convergence is *independent
/// triangulation* — the strongest evidence — so it's called out first.
pub(super) fn render_convergence(concords: &[Clash]) -> String {
    if concords.is_empty() {
        return "No convergence found across the collected facts.".to_string();
    }
    let triangulated = concords.iter().filter(|c| c.is_cross_source()).count();
    let same = concords.len() - triangulated;
    let mut s = format!(
        "Found {} point(s) of convergence — {triangulated} across independent sources (triangulated), {same} within a source.\n",
        concords.len()
    );
    for (i, c) in concords.iter().enumerate() {
        let tag = if c.is_cross_source() { "triangulated" } else { "same-source" };
        let sa = source_label(&c.a);
        let sb = source_label(&c.b);
        s.push_str(&format!(
            "\n{}. [{tag}]\n   · {}  ⟨{sa}⟩\n   ≈ {}  ⟨{sb}⟩\n",
            i + 1,
            truncate(&c.a.text, 160),
            truncate(&c.b.text, 160),
        ));
        if !c.reason.is_empty() {
            s.push_str(&format!("   shared: {}\n", c.reason));
        }
    }
    s
}

fn truncate(s: &str, n: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= n {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(n).collect::<String>())
    }
}

// ── SCHOLAR P2: the relation (graded stance) — contradiction AND confirmation ──

/// The graded relation between a claim and a source passage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stance {
    Contradicts,
    Tension,
    Qualifies,
    Agrees,
    Silent,
}

impl Stance {
    pub(crate) fn parse(word: &str) -> Option<Stance> {
        match word.trim().to_ascii_uppercase().as_str() {
            "CONTRADICTS" | "CONTRADICT" => Some(Stance::Contradicts),
            "TENSION" => Some(Stance::Tension),
            "QUALIFIES" | "QUALIFY" => Some(Stance::Qualifies),
            "AGREES" | "AGREE" | "SUPPORTS" | "SUPPORT" => Some(Stance::Agrees),
            "SILENT" => Some(Stance::Silent),
            _ => None,
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Stance::Contradicts => "contradicts",
            Stance::Tension => "tension",
            Stance::Qualifies => "qualifies",
            Stance::Agrees => "agrees",
            Stance::Silent => "silent",
        }
    }
    pub(crate) fn is_against(self) -> bool {
        matches!(self, Stance::Contradicts | Stance::Tension)
    }
    pub(crate) fn is_support(self) -> bool {
        matches!(self, Stance::Agrees | Stance::Qualifies)
    }
}

/// One retrieved piece of evidence for a claim (a fact or a source passage).
#[derive(Debug, Clone)]
pub(crate) struct Evidence {
    pub label: String,
    pub body: String,
}

/// A judged relation between the claim and one evidence item.
#[derive(Debug, Clone)]
pub(crate) struct Relation {
    pub label: String,
    pub stance: Stance,
    pub reason: String,
}

/// The graded-judge system prompt (generalizes `/triangulate`'s three-way judge).
pub(crate) fn relate_system(language: &str) -> String {
    format!(
        "You are relating a CLAIM to independent sources. Judge ONLY from the sources below — \
         do not use outside knowledge. For EACH source, output exactly one line:\n\
         <n>. CONTRADICTS | TENSION | QUALIFIES | AGREES | SILENT — <short reason>\n\
         CONTRADICTS = the source opposes the claim; TENSION = they sit uneasily together; \
         QUALIFIES = supports the claim but with a caveat; AGREES = confirms the claim; \
         SILENT = says nothing relevant. Write the reasons in {language}."
    )
}

/// The user message: the claim + numbered evidence.
pub(crate) fn relate_user(claim: &str, evidence: &[Evidence]) -> String {
    let mut s = format!("Claim:\n{claim}\n\nSources:\n");
    for (i, e) in evidence.iter().enumerate() {
        let body: String = e.body.chars().take(1200).collect();
        s.push_str(&format!("{}. [{}]\n{}\n\n", i + 1, e.label, body.trim()));
    }
    s
}

/// Parse the graded-judge reply (numbered `<n>. STANCE — reason` lines) against
/// the evidence order.
pub(crate) fn parse_relations(reply: &str, evidence: &[Evidence]) -> Vec<Relation> {
    let mut out = Vec::new();
    for line in reply.lines() {
        let line = line.trim();
        let Some((num_str, rest)) = line.split_once('.') else {
            continue;
        };
        let Some(num) = leading_index(num_str) else {
            continue;
        };
        if num == 0 || num > evidence.len() {
            continue;
        }
        let rest = rest.trim();
        let word = rest
            .split([' ', '\t', ':', '—', '-'])
            .find(|s| !s.is_empty())
            .unwrap_or("");
        let Some(stance) = Stance::parse(word) else {
            continue;
        };
        let reason = rest
            .split_once('—')
            .or_else(|| rest.split_once(" - "))
            .map(|(_, r)| r.trim().to_string())
            .unwrap_or_default();
        out.push(Relation {
            label: evidence[num - 1].label.clone(),
            stance,
            reason,
        });
    }
    out
}

/// Render the relations, splitting **against** (contradiction/tension — risks to
/// address) from **support** (agrees/qualifies — material to cite).
pub(super) fn render_relations(claim: &str, relations: &[Relation]) -> String {
    let against: Vec<&Relation> = relations.iter().filter(|r| r.stance.is_against()).collect();
    let support: Vec<&Relation> = relations.iter().filter(|r| r.stance.is_support()).collect();
    let silent = relations.iter().filter(|r| r.stance == Stance::Silent).count();

    let mut s = format!("Relating your claim to the sources:\n  “{}”\n", truncate(claim, 200));
    s.push_str(&format!(
        "\n{} against · {} supporting · {silent} silent\n",
        against.len(),
        support.len()
    ));
    if !against.is_empty() {
        s.push_str("\n⚠ Against your claim (address these):\n");
        for r in &against {
            s.push_str(&format!("  · [{}] {} — {}\n", r.stance.label(), r.label, r.reason));
        }
    }
    if !support.is_empty() {
        s.push_str("\n✓ Supporting your claim (cite these):\n");
        for r in &support {
            s.push_str(&format!("  · [{}] {} — {}\n", r.stance.label(), r.label, r.reason));
        }
    }
    if against.is_empty() && support.is_empty() {
        s.push_str("\nNo source in the corpus clearly bears on this claim.\n");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sf(id: u128, origin: &str, text: &str) -> SourcedFact {
        SourcedFact {
            id: Uuid::from_u128(id),
            location: "facts/x".into(),
            text: text.into(),
            origin: origin.into(),
            source: format!("{origin}: detail"),
        }
    }

    #[test]
    fn parses_indexed_pairs_and_maps_to_facts() {
        let facts = vec![
            sf(1, "archive", "Mercy overrides justice."),
            sf(2, "web", "Justice is absolute."),
            sf(3, "archive", "The date was 1876."),
        ];
        let reply = "2 ⇄ 1 — one says mercy overrides, the other absolute justice\n\
                     No other conflicts.\n";
        let clashes = parse_clashes(reply, &facts);
        assert_eq!(clashes.len(), 1);
        // Order preserved as written (a=2, b=1).
        assert_eq!(clashes[0].a.id, Uuid::from_u128(2));
        assert_eq!(clashes[0].b.id, Uuid::from_u128(1));
        assert!(clashes[0].reason.contains("mercy"));
        assert!(clashes[0].is_cross_source()); // web ⇄ archive
    }

    #[test]
    fn no_contradictions_and_malformed_lines_yield_nothing() {
        let facts = vec![sf(1, "a", "x"), sf(2, "a", "y")];
        assert!(parse_clashes("No contradictions found.", &facts).is_empty());
        // Out of range / self-pair / no arrow are all dropped.
        assert!(parse_clashes("1 ⇄ 9 — out of range", &facts).is_empty());
        assert!(parse_clashes("2 ⇄ 2 — self", &facts).is_empty());
        assert!(parse_clashes("just prose, no arrow", &facts).is_empty());
    }

    #[test]
    fn within_vs_cross_source_and_dedupe() {
        let facts = vec![sf(1, "archive", "a"), sf(2, "archive", "b"), sf(3, "web", "c")];
        // Same origin → within-source.
        let within = parse_clashes("1 ⇄ 2 — differ", &facts);
        assert!(!within[0].is_cross_source());
        // The cross-branch pass re-emits the same pair (2 ⇄ 1) → deduped.
        let mut all = parse_clashes("1 ⇄ 2 — differ", &facts);
        all.extend(parse_clashes("2 ⇄ 1 — differ again", &facts));
        assert_eq!(dedupe(all).len(), 1);
    }

    #[test]
    fn report_counts_cross_and_within() {
        let facts = vec![sf(1, "archive", "a"), sf(2, "web", "b"), sf(3, "web", "c")];
        let clashes = vec![
            parse_clashes("1 ⇄ 2 — x", &facts).remove(0), // cross
            parse_clashes("2 ⇄ 3 — y", &facts).remove(0), // within (web/web)
        ];
        let report = render_report(&clashes);
        assert!(report.contains("2 contradiction(s)"), "{report}");
        assert!(report.contains("1 between sources"), "{report}");
        assert!(report.contains("1 within a source"), "{report}");
        assert!(report.contains("cross-source"));
        assert!(report.contains("within-source"));
    }

    #[test]
    fn leading_index_extracts_first_number() {
        assert_eq!(leading_index("2"), Some(2));
        assert_eq!(leading_index("Fact 5"), Some(5));
        assert_eq!(leading_index("#3 "), Some(3));
        assert_eq!(leading_index("12"), Some(12));
        assert_eq!(leading_index("none"), None);
        // Non-Latin numerals: Arabic-Indic ٢, Persian ۵, Devanagari ३.
        assert_eq!(leading_index("٢"), Some(2));
        assert_eq!(leading_index("۵"), Some(5));
        assert_eq!(leading_index("३."), Some(3));
    }

    #[test]
    fn convergence_parses_and_triangulates() {
        let facts = vec![
            sf(1, "archive", "War deaths fell across the period."),
            sf(2, "web", "Battlefield mortality declined over the same era."),
            sf(3, "archive", "Peace lengthened between conflicts."),
        ];
        // `≈` pairs, not `⇄`.
        let concords = parse_concords("1 ≈ 2 — both report a decline in war deaths\n", &facts);
        assert_eq!(concords.len(), 1);
        assert!(concords[0].is_cross_source()); // archive ≈ web = independent
        // parse_clashes (⇄) must NOT pick up ≈ lines.
        assert!(parse_clashes("1 ≈ 2 — x", &facts).is_empty());
        let report = render_convergence(&concords);
        assert!(report.contains("1 point(s) of convergence"), "{report}");
        assert!(report.contains("triangulated"), "{report}");
    }

    #[test]
    fn parsers_accept_non_latin_numerals() {
        // A model replying in Arabic may number with Arabic-Indic digits.
        let facts = vec![sf(1, "quran", "a"), sf(2, "kant", "b")];
        let clashes = parse_clashes("١ ⇄ ٢ — يتعارضان", &facts);
        assert_eq!(clashes.len(), 1);
        assert!(clashes[0].is_cross_source());

        let evidence = vec![ev("source: q", "x"), ev("source: k", "y")];
        let rels = parse_relations("١. AGREES — يؤكد\n٢. CONTRADICTS — يعارض", &evidence);
        assert_eq!(rels.len(), 2);
        assert_eq!(rels[0].stance, Stance::Agrees);
        assert_eq!(rels[1].stance, Stance::Contradicts);
    }

    fn ev(label: &str, body: &str) -> Evidence {
        Evidence { label: label.into(), body: body.into() }
    }

    #[test]
    fn stance_parses_all_grades_and_synonyms() {
        assert_eq!(Stance::parse("CONTRADICTS"), Some(Stance::Contradicts));
        assert_eq!(Stance::parse("tension"), Some(Stance::Tension));
        assert_eq!(Stance::parse("Qualifies"), Some(Stance::Qualifies));
        assert_eq!(Stance::parse("SUPPORTS"), Some(Stance::Agrees)); // synonym
        assert_eq!(Stance::parse("SILENT"), Some(Stance::Silent));
        assert_eq!(Stance::parse("maybe"), None);
    }

    #[test]
    fn parse_relations_grades_each_line() {
        let evidence = vec![ev("fact: a", "x"), ev("source: b", "y"), ev("fact: c", "z")];
        let reply = "1. AGREES — confirms it\n\
                     2. CONTRADICTS — opposes it\n\
                     3. SILENT — unrelated\n\
                     9. AGREES — out of range, dropped\n";
        let rels = parse_relations(reply, &evidence);
        assert_eq!(rels.len(), 3);
        assert_eq!(rels[0].stance, Stance::Agrees);
        assert_eq!(rels[1].stance, Stance::Contradicts);
        assert_eq!(rels[2].stance, Stance::Silent);
        assert!(rels[0].reason.contains("confirms"));
    }

    #[test]
    fn render_relations_splits_support_from_against() {
        let evidence = vec![ev("fact: a", "x"), ev("source: b", "y")];
        let rels = parse_relations("1. AGREES — backs it\n2. TENSION — uneasy\n", &evidence);
        let out = render_relations("Mercy overrides justice.", &rels);
        assert!(out.contains("1 against · 1 supporting"), "{out}");
        assert!(out.contains("Against your claim"));
        assert!(out.contains("Supporting your claim"));
        assert!(out.contains("[agrees] fact: a"));
        assert!(out.contains("[tension] source: b"));
    }
}
