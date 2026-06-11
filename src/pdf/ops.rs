//! 1.3.0 PDF-1 — page operations on the PDF page tree (RFC §8.3).
//!
//! Structural ops that work on any PDF: extract, split, merge, rotate,
//! delete, reorder.  Tree-aware variants (`ByChapter` / `ByBookmark`)
//! arrive with outline injection; for now [`PageSpec`] / [`SplitMode`]
//! cover literal page numbers, which is all the page-level commands need.

use std::collections::BTreeSet;

use lopdf::{Dictionary, Document, Object, ObjectId};

use super::doc::{PdfDoc, PdfSource};
use super::{Error, Result};

/// A set of pages, **1-based** (the way users + the CLI talk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageSpec {
    All,
    Single(usize),
    /// Inclusive.
    Range(usize, usize),
    List(Vec<PageSpec>),
}

impl PageSpec {
    /// Parse a CLI spec like `1,3,5-8,12` (or `all` / empty → `All`).
    pub fn parse(s: &str) -> Result<PageSpec> {
        let t = s.trim();
        if t.is_empty() || t.eq_ignore_ascii_case("all") {
            return Ok(PageSpec::All);
        }
        let mut parts = Vec::new();
        for tok in t.split(',') {
            let tok = tok.trim();
            if tok.is_empty() {
                continue;
            }
            if let Some((a, b)) = tok.split_once('-') {
                parts.push(PageSpec::Range(parse_n(a)?, parse_n(b)?));
            } else {
                parts.push(PageSpec::Single(parse_n(tok)?));
            }
        }
        if parts.is_empty() {
            return Ok(PageSpec::All);
        }
        Ok(PageSpec::List(parts))
    }

    /// Resolve to sorted, deduped 1-based page numbers within `[1, count]`.
    pub fn resolve(&self, count: usize) -> Vec<u32> {
        let mut set = BTreeSet::new();
        self.collect(count, &mut set);
        set.into_iter().collect()
    }

    fn collect(&self, count: usize, set: &mut BTreeSet<u32>) {
        match self {
            PageSpec::All => {
                for p in 1..=count {
                    set.insert(p as u32);
                }
            }
            PageSpec::Single(p) => {
                if *p >= 1 && *p <= count {
                    set.insert(*p as u32);
                }
            }
            PageSpec::Range(a, b) => {
                let (lo, hi) = if a <= b { (*a, *b) } else { (*b, *a) };
                for p in lo.max(1)..=hi.min(count) {
                    set.insert(p as u32);
                }
            }
            PageSpec::List(v) => {
                for s in v {
                    s.collect(count, set);
                }
            }
        }
    }
}

fn parse_n(s: &str) -> Result<usize> {
    s.trim()
        .parse::<usize>()
        .map_err(|_| Error::Other(format!("bad page number `{}`", s.trim())))
}

/// Rotation in 90° steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    D90,
    D180,
    D270,
}

impl Rotation {
    pub fn degrees(self) -> i64 {
        match self {
            Rotation::D90 => 90,
            Rotation::D180 => 180,
            Rotation::D270 => 270,
        }
    }
    pub fn from_degrees(d: i64) -> Result<Rotation> {
        match ((d % 360) + 360) % 360 {
            90 => Ok(Rotation::D90),
            180 => Ok(Rotation::D180),
            270 => Ok(Rotation::D270),
            other => Err(Error::Other(format!(
                "rotation must be 90/180/270, got {other}"
            ))),
        }
    }
}

/// Keep only `pages` (in document order), returning a new document.
pub fn extract(src: &PdfDoc, pages: &PageSpec) -> Result<PdfDoc> {
    let count = src.page_count();
    let keep: BTreeSet<u32> = pages.resolve(count).into_iter().collect();
    if keep.is_empty() {
        return Err(Error::Other("extract: no pages selected".into()));
    }
    let drop: Vec<u32> = (1..=count as u32).filter(|p| !keep.contains(p)).collect();
    let mut inner = src.document().clone();
    inner.delete_pages(&drop);
    inner.prune_objects();
    Ok(PdfDoc::from_document(inner, src.source().clone()))
}

/// Delete `pages` in place (errors if it would empty the document).
pub fn delete(doc: &mut PdfDoc, pages: &PageSpec) -> Result<()> {
    let count = doc.page_count();
    let nums = pages.resolve(count);
    if nums.is_empty() {
        return Ok(());
    }
    if nums.len() >= count {
        return Err(Error::Other("delete would remove every page".into()));
    }
    doc.document_mut().delete_pages(&nums);
    doc.document_mut().prune_objects();
    doc.reindex();
    Ok(())
}

/// Rotate `pages` by `rot`, added to any existing `/Rotate`.
pub fn rotate(doc: &mut PdfDoc, pages: &PageSpec, rot: Rotation) -> Result<()> {
    let nums = pages.resolve(doc.page_count());
    let page_map = doc.document().get_pages();
    let targets: Vec<ObjectId> = nums.iter().filter_map(|n| page_map.get(n).copied()).collect();
    let delta = rot.degrees();
    for id in targets {
        let dict = doc.document_mut().get_dictionary_mut(id)?;
        let cur = match dict.get(b"Rotate") {
            Ok(Object::Integer(i)) => *i,
            _ => 0,
        };
        let next = (((cur + delta) % 360) + 360) % 360;
        dict.set("Rotate", Object::Integer(next));
    }
    Ok(())
}

/// Reorder pages by a 0-based permutation: new page `i` is old page
/// `mapping[i]`.  Flattens the page tree into the given order.
pub fn reorder(doc: &mut PdfDoc, mapping: &[usize]) -> Result<()> {
    let ids = doc.page_ids().to_vec();
    let count = ids.len();
    if mapping.len() != count {
        return Err(Error::Other(
            "reorder: mapping length != page count".into(),
        ));
    }
    let mut seen = vec![false; count];
    for &m in mapping {
        if m >= count || seen[m] {
            return Err(Error::Other("reorder: mapping is not a permutation".into()));
        }
        seen[m] = true;
    }
    let new_order: Vec<ObjectId> = mapping.iter().map(|&i| ids[i]).collect();
    let pages_root = pages_root_id(doc.document())?;
    for &pid in &new_order {
        doc.document_mut()
            .get_dictionary_mut(pid)?
            .set("Parent", Object::Reference(pages_root));
    }
    let kids: Vec<Object> = new_order.iter().map(|&id| Object::Reference(id)).collect();
    let pages = doc.document_mut().get_dictionary_mut(pages_root)?;
    pages.set("Kids", kids);
    pages.set("Count", count as i64);
    doc.reindex();
    Ok(())
}

/// How to split a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitMode {
    /// Each piece is `n` pages (the last may be shorter).
    EveryNPages(usize),
    /// Split *before* each of these 1-based page numbers.
    OnPages(Vec<usize>),
}

/// Split into multiple documents.
pub fn split(src: &PdfDoc, mode: &SplitMode) -> Result<Vec<PdfDoc>> {
    let count = src.page_count();
    if count == 0 {
        return Ok(Vec::new());
    }
    let ranges: Vec<(usize, usize)> = match mode {
        SplitMode::EveryNPages(n) => {
            let n = (*n).max(1);
            (0..count)
                .step_by(n)
                .map(|start| (start + 1, (start + n).min(count)))
                .collect()
        }
        SplitMode::OnPages(cuts) => {
            let mut cut: Vec<usize> = cuts.iter().copied().filter(|&p| p > 1 && p <= count).collect();
            cut.sort_unstable();
            cut.dedup();
            let mut ranges = Vec::new();
            let mut start = 1;
            for &c in &cut {
                ranges.push((start, c - 1));
                start = c;
            }
            ranges.push((start, count));
            ranges
        }
    };
    ranges
        .into_iter()
        .map(|(lo, hi)| extract(src, &PageSpec::Range(lo, hi)))
        .collect()
}

/// Concatenate documents.  Each source's page subtree is kept intact and
/// reparented under a fresh top node, so inherited attributes (MediaBox,
/// Resources) survive.  Outlines / named-destinations are not merged.
pub fn merge(docs: &[PdfDoc]) -> Result<PdfDoc> {
    if docs.is_empty() {
        return Err(Error::Other("merge: no documents".into()));
    }
    let mut out = Document::with_version("1.5");
    let mut next_id = 1u32;
    let mut sub_roots: Vec<ObjectId> = Vec::new();
    let mut total = 0usize;
    for d in docs {
        let mut doc = d.document().clone();
        doc.renumber_objects_with(next_id);
        next_id = doc.max_id + 1;
        sub_roots.push(pages_root_id(&doc)?);
        total += d.page_count();
        out.objects.extend(doc.objects);
        if doc.max_id > out.max_id {
            out.max_id = doc.max_id;
        }
    }
    let top = out.new_object_id();
    for &sr in &sub_roots {
        out.get_dictionary_mut(sr)?
            .set("Parent", Object::Reference(top));
    }
    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set(
        "Kids",
        sub_roots
            .iter()
            .map(|&id| Object::Reference(id))
            .collect::<Vec<_>>(),
    );
    pages.set("Count", total as i64);
    out.objects.insert(top, Object::Dictionary(pages));
    let mut cat = Dictionary::new();
    cat.set("Type", "Catalog");
    cat.set("Pages", Object::Reference(top));
    let cat_id = out.add_object(cat);
    out.trailer.set("Root", Object::Reference(cat_id));
    Ok(PdfDoc::from_document(out, PdfSource::External))
}

fn pages_root_id(doc: &Document) -> Result<ObjectId> {
    let cat = doc.catalog()?;
    cat.get(b"Pages")
        .map_err(Error::Lopdf)?
        .as_reference()
        .map_err(Error::Lopdf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_support::minimal_pdf;
    use crate::pdf::PdfDoc;

    fn load(n: usize) -> PdfDoc {
        PdfDoc::load_mem(&minimal_pdf(n, 612.0, 792.0)).unwrap()
    }

    #[test]
    fn page_spec_parse_and_resolve() {
        assert_eq!(PageSpec::parse("all").unwrap(), PageSpec::All);
        assert_eq!(PageSpec::parse("  ").unwrap(), PageSpec::All);
        assert_eq!(PageSpec::parse("1,3,5-8,12").unwrap().resolve(20), vec![1, 3, 5, 6, 7, 8, 12]);
        // out-of-range clamped, dupes merged, reversed range handled
        assert_eq!(PageSpec::parse("8-5").unwrap().resolve(6), vec![5, 6]);
        assert_eq!(PageSpec::All.resolve(3), vec![1, 2, 3]);
        assert!(PageSpec::parse("1,x").is_err());
    }

    #[test]
    fn extract_keeps_selected_in_order() {
        let src = load(5);
        let out = extract(&src, &PageSpec::parse("2,4").unwrap()).unwrap();
        assert_eq!(out.page_count(), 2);
        // source untouched
        assert_eq!(src.page_count(), 5);
    }

    #[test]
    fn delete_removes_and_guards_emptying() {
        let mut doc = load(5);
        delete(&mut doc, &PageSpec::Single(1)).unwrap();
        assert_eq!(doc.page_count(), 4);
        assert!(delete(&mut doc, &PageSpec::All).is_err());
    }

    #[test]
    fn rotate_accumulates() {
        let mut doc = load(2);
        rotate(&mut doc, &PageSpec::Single(1), Rotation::D90).unwrap();
        rotate(&mut doc, &PageSpec::Single(1), Rotation::D270).unwrap();
        // 90 + 270 = 360 → 0
        let id = doc.document().get_pages()[&1];
        let r = match doc.document().get_dictionary(id).unwrap().get(b"Rotate") {
            Ok(Object::Integer(i)) => *i,
            _ => 0,
        };
        assert_eq!(r, 0);
    }

    #[test]
    fn reorder_permutes_and_validates() {
        let mut doc = load(3);
        let first_before = doc.page_ids()[0];
        reorder(&mut doc, &[2, 1, 0]).unwrap();
        assert_eq!(doc.page_count(), 3);
        assert_eq!(doc.page_ids()[2], first_before); // old first is now last
        assert!(reorder(&mut doc, &[0, 0, 1]).is_err()); // not a permutation
        assert!(reorder(&mut doc, &[0, 1]).is_err()); // wrong length
    }

    #[test]
    fn split_every_n_and_on_pages() {
        let src = load(5);
        let parts = split(&src, &SplitMode::EveryNPages(2)).unwrap();
        assert_eq!(parts.iter().map(|p| p.page_count()).collect::<Vec<_>>(), vec![2, 2, 1]);
        let parts = split(&src, &SplitMode::OnPages(vec![3])).unwrap();
        assert_eq!(parts.iter().map(|p| p.page_count()).collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn merge_concatenates_and_round_trips() {
        let a = load(2);
        let b = load(3);
        let mut merged = merge(&[a, b]).unwrap();
        assert_eq!(merged.page_count(), 5);
        let bytes = merged.to_bytes().unwrap();
        let reloaded = PdfDoc::load_mem(&bytes).unwrap();
        assert_eq!(reloaded.page_count(), 5);
        assert!(merge(&[]).is_err());
    }
}
