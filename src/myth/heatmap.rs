//! MYTH-1 (M-P8) — the symbolic-density heatmap, rendered as Markdown for the
//! Thoughts pane (TUI) and stdout (CLI). Symbols show block-density bars per
//! chapter-bucket, motifs show dot counts, archetypes show presence bars.

use anyhow::Result;

use super::store::MythStore;

/// Chapter-bucket ranges `(lo, hi)` (1-based, inclusive) for `total` chapters
/// split into `buckets`.
fn bucket_ranges(total: u32, buckets: usize) -> Vec<(u32, u32)> {
    let buckets = buckets.max(1) as u32;
    if total == 0 {
        return Vec::new();
    }
    let n = buckets.min(total);
    (0..n)
        .map(|b| {
            let lo = b * total / n + 1;
            let hi = (b + 1) * total / n;
            (lo, hi.max(lo))
        })
        .collect()
}

/// A block-density cell for a count relative to the row's max.
fn density_cell(count: u32, row_max: u32) -> &'static str {
    if count == 0 {
        "░░"
    } else if row_max == 0 || count * 3 >= row_max * 2 {
        "███"
    } else if count * 3 >= row_max {
        "██"
    } else {
        "█"
    }
}

/// Sum per-chapter counts into the buckets.
fn bucketize(per_chapter: &[(u32, u32)], ranges: &[(u32, u32)]) -> Vec<u32> {
    ranges
        .iter()
        .map(|(lo, hi)| {
            per_chapter.iter().filter(|(c, _)| *c >= *lo && *c <= *hi).map(|(_, n)| *n).sum()
        })
        .collect()
}

/// Count chapter-presence (a list of ordinals) into the buckets.
fn bucketize_presence(chapters: &[u32], ranges: &[(u32, u32)]) -> Vec<u32> {
    ranges
        .iter()
        .map(|(lo, hi)| chapters.iter().filter(|c| **c >= *lo && **c <= *hi).count() as u32)
        .collect()
}

/// Build the Markdown heatmap. `archetype_presence` is `(label, chapters)` per
/// declared archetype (computed by the caller via the mention scan).
pub(crate) fn build_heatmap(
    store: &MythStore,
    book_slug: &str,
    book_title: &str,
    total_chapters: u32,
    buckets: usize,
    archetype_presence: &[(String, Vec<u32>)],
) -> Result<String> {
    let ranges = bucket_ranges(total_chapters, buckets);
    let symbols = store.symbols(book_slug)?;
    let motifs = store.motifs(book_slug)?;

    let mut out = String::new();
    out.push_str(&format!("## Symbolic density — \"{book_title}\" ({total_chapters} chapters)\n\n"));

    if ranges.is_empty() {
        out.push_str("_No chapters yet._\n");
        return Ok(out);
    }

    // Header: bucket ranges.
    let header: Vec<String> = ranges
        .iter()
        .map(|(lo, hi)| if lo == hi { format!("{lo}") } else { format!("{lo}–{hi}") })
        .collect();
    out.push_str("```\n");
    out.push_str(&format!("{:<22}{}\n", "", header.join("  ")));

    // Symbols.
    for s in &symbols {
        let per_chapter = store.density_for_symbol(book_slug, &s.para_id)?;
        let buckets_v = bucketize(&per_chapter, &ranges);
        let row_max = buckets_v.iter().copied().max().unwrap_or(0);
        let label = format!("⊛ {}", s.vocabulary.first().cloned().unwrap_or_default());
        let cells: Vec<String> = buckets_v
            .iter()
            .map(|c| format!("{:<5}", density_cell(*c, row_max)))
            .collect();
        out.push_str(&format!("{label:<22}{}\n", cells.join(" ")));
    }

    // Motifs.
    if !motifs.is_empty() {
        out.push('\n');
        for m in &motifs {
            let chapters = store.motif_chapters(book_slug, &m.para_id)?;
            let buckets_v = bucketize_presence(&chapters, &ranges);
            let label = format!("∿ {}", m.name);
            let cells: Vec<String> = buckets_v
                .iter()
                .map(|c| format!("{:<5}", if *c == 0 { "·".to_string() } else { "●".repeat((*c).min(4) as usize) }))
                .collect();
            out.push_str(&format!("{label:<22}{}\n", cells.join(" ")));
        }
    }

    // Archetypes.
    if !archetype_presence.is_empty() {
        out.push('\n');
        for (label, chapters) in archetype_presence {
            let buckets_v = bucketize_presence(chapters, &ranges);
            let row_max = buckets_v.iter().copied().max().unwrap_or(0);
            let cells: Vec<String> = buckets_v
                .iter()
                .map(|c| format!("{:<5}", density_cell(*c, row_max)))
                .collect();
            out.push_str(&format!("{:<22}{}\n", format!("⍟ {label}"), cells.join(" ")));
        }
    }
    out.push_str("```\n");

    // Findings summary.
    let findings = store.findings(book_slug, false)?;
    if !findings.is_empty() {
        out.push_str("\n**Findings:**\n");
        for f in &findings {
            out.push_str(&format!("- ⚠ {}\n", f.description));
        }
    }
    out.push_str("\n_Run `inkhaven myth check` for LLM consistency & completeness analysis._\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_ranges_split_evenly() {
        assert_eq!(bucket_ranges(42, 8).len(), 8);
        assert_eq!(bucket_ranges(42, 8)[0], (1, 5));
        assert_eq!(bucket_ranges(42, 8)[7], (37, 42));
        // Fewer chapters than buckets → one bucket per chapter.
        assert_eq!(bucket_ranges(3, 8).len(), 3);
        assert!(bucket_ranges(0, 8).is_empty());
    }

    #[test]
    fn bucketize_and_cells() {
        let ranges = bucket_ranges(10, 2); // (1,5),(6,10)
        let per = vec![(2u32, 3u32), (7, 1)];
        assert_eq!(bucketize(&per, &ranges), vec![3, 1]);
        assert_eq!(density_cell(0, 5), "░░");
        assert_eq!(density_cell(5, 5), "███");
    }
}
