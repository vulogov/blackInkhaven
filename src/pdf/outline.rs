//! 1.3.0 PDF-1 — PDF outline / bookmarks (`/Outlines`; RFC §8.7).
//!
//! [`inject_outline`] writes a hierarchical bookmark tree; [`read_outline`]
//! reads one back (typst-pdf emits a heading outline by default, so this
//! also normalises / inspects that).  Correlating bookmarks to inkhaven
//! tree nodes — for `ByChapter` ops — uses the additive `#metadata`
//! markers the `assemble` step now emits; that compile-time lookup wires
//! in at book-take.

use std::collections::HashMap;

use lopdf::{Dictionary, Document, Object, ObjectId};

use super::doc::PdfDoc;
use super::{decode_pdf_string, pdf_string, Error, Result};

/// One bookmark entry: a title, the 0-based page it jumps to, and nested
/// children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineItem {
    pub title: String,
    /// 0-based page index.
    pub page: usize,
    pub children: Vec<OutlineItem>,
}

impl OutlineItem {
    pub fn new(title: impl Into<String>, page: usize) -> Self {
        Self {
            title: title.into(),
            page,
            children: Vec::new(),
        }
    }
    pub fn with_children(mut self, children: Vec<OutlineItem>) -> Self {
        self.children = children;
        self
    }
}

/// Replace the document outline with `items`.  A `page` past the end
/// clamps to the last page.  Empty `items` writes an empty outline.
pub fn inject_outline(doc: &mut PdfDoc, items: &[OutlineItem]) -> Result<()> {
    let page_ids = doc.page_ids().to_vec();
    if page_ids.is_empty() {
        return Err(Error::Other("inject_outline: document has no pages".into()));
    }
    let root = doc.document_mut().new_object_id();
    let built = build_level(doc.document_mut(), items, root, &page_ids);

    let mut rd = Dictionary::new();
    rd.set("Type", "Outlines");
    match built {
        Some((first, last, count)) => {
            rd.set("First", Object::Reference(first));
            rd.set("Last", Object::Reference(last));
            rd.set("Count", count);
        }
        None => {
            rd.set("Count", 0i64);
        }
    }
    doc.document_mut().objects.insert(root, Object::Dictionary(rd));
    doc.document_mut()
        .catalog_mut()?
        .set("Outlines", Object::Reference(root));
    Ok(())
}

/// Recursively emit one level of items under `parent`, returning
/// `(first_id, last_id, total_open_descendants)`.
fn build_level(
    doc: &mut Document,
    items: &[OutlineItem],
    parent: ObjectId,
    page_ids: &[ObjectId],
) -> Option<(ObjectId, ObjectId, i64)> {
    if items.is_empty() {
        return None;
    }
    // Allocate this level's ids up front so Prev/Next can reference them.
    let ids: Vec<ObjectId> = items.iter().map(|_| doc.new_object_id()).collect();
    let mut total = 0i64;
    for (i, item) in items.iter().enumerate() {
        let id = ids[i];
        let child = build_level(doc, &item.children, id, page_ids);

        let mut d = Dictionary::new();
        d.set("Title", pdf_string(&item.title));
        d.set("Parent", Object::Reference(parent));
        if i > 0 {
            d.set("Prev", Object::Reference(ids[i - 1]));
        }
        if i + 1 < ids.len() {
            d.set("Next", Object::Reference(ids[i + 1]));
        }
        let pidx = item.page.min(page_ids.len() - 1);
        d.set(
            "Dest",
            Object::Array(vec![
                Object::Reference(page_ids[pidx]),
                Object::Name(b"Fit".to_vec()),
            ]),
        );
        if let Some((cf, cl, ccount)) = child {
            d.set("First", Object::Reference(cf));
            d.set("Last", Object::Reference(cl));
            d.set("Count", ccount); // positive → open
            total += ccount;
        }
        total += 1;
        doc.objects.insert(id, Object::Dictionary(d));
    }
    Some((ids[0], ids[ids.len() - 1], total))
}

/// Read the document outline back into a tree (empty if none).
pub fn read_outline(doc: &PdfDoc) -> Vec<OutlineItem> {
    let d = doc.document();
    let page_index: HashMap<ObjectId, usize> = doc
        .page_ids()
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();
    let first = d
        .catalog()
        .ok()
        .and_then(|c| c.get(b"Outlines").ok())
        .and_then(|o| o.as_reference().ok())
        .and_then(|root| d.get_dictionary(root).ok())
        .and_then(|root| root.get(b"First").ok())
        .and_then(|o| o.as_reference().ok());
    read_siblings(d, first, &page_index, 0)
}

fn read_siblings(
    d: &Document,
    mut cur: Option<ObjectId>,
    page_index: &HashMap<ObjectId, usize>,
    depth: usize,
) -> Vec<OutlineItem> {
    let mut out = Vec::new();
    let mut guard = 0;
    while let Some(id) = cur {
        guard += 1;
        if guard > 100_000 || depth > 32 {
            break;
        }
        let Ok(item) = d.get_dictionary(id) else {
            break;
        };
        let title = item
            .get(b"Title")
            .ok()
            .and_then(|o| match o {
                Object::String(b, _) => Some(decode_pdf_string(b)),
                _ => None,
            })
            .unwrap_or_default();
        let page = dest_page(d, item, page_index).unwrap_or(0);
        let first_child = item.get(b"First").ok().and_then(|o| o.as_reference().ok());
        let children = read_siblings(d, first_child, page_index, depth + 1);
        out.push(OutlineItem {
            title,
            page,
            children,
        });
        cur = item.get(b"Next").ok().and_then(|o| o.as_reference().ok());
    }
    out
}

/// 0-based destination page of an outline item — handles a `/Dest`
/// array and a `/A << /S /GoTo /D [...] >>` action (what typst emits).
fn dest_page(d: &Document, item: &Dictionary, page_index: &HashMap<ObjectId, usize>) -> Option<usize> {
    let arr = item
        .get(b"Dest")
        .ok()
        .and_then(|o| dest_array(d, o))
        .or_else(|| {
            item.get(b"A")
                .ok()
                .and_then(|a| a.as_dict().ok())
                .and_then(|ad| ad.get(b"D").ok())
                .and_then(|o| dest_array(d, o))
        })?;
    let first = arr.first()?.as_reference().ok()?;
    page_index.get(&first).copied()
}

fn dest_array<'a>(d: &'a Document, o: &'a Object) -> Option<&'a Vec<Object>> {
    match o {
        Object::Array(a) => Some(a),
        Object::Reference(r) => match d.get_object(*r) {
            Ok(Object::Array(a)) => Some(a),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_support::minimal_pdf;
    use crate::pdf::PdfDoc;

    #[test]
    fn inject_then_read_round_trips_nested() {
        let mut doc = PdfDoc::load_mem(&minimal_pdf(5, 612.0, 792.0)).unwrap();
        let items = vec![
            OutlineItem::new("Chapter 1", 0)
                .with_children(vec![OutlineItem::new("Scene A", 1)]),
            OutlineItem::new("Chapter 2", 3),
        ];
        inject_outline(&mut doc, &items).unwrap();
        // round-trip through bytes (proves it serializes into the catalog).
        let bytes = doc.to_bytes().unwrap();
        let reloaded = PdfDoc::load_mem(&bytes).unwrap();
        assert_eq!(read_outline(&reloaded), items);
    }

    #[test]
    fn page_index_clamps_and_unicode_title() {
        let mut doc = PdfDoc::load_mem(&minimal_pdf(2, 612.0, 792.0)).unwrap();
        let items = vec![OutlineItem::new("Глава · 日本語", 99)]; // page out of range
        inject_outline(&mut doc, &items).unwrap();
        let got = read_outline(&doc);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title, "Глава · 日本語");
        assert_eq!(got[0].page, 1); // clamped to last page (index 1)
    }

    #[test]
    fn empty_outline_reads_empty() {
        let mut doc = PdfDoc::load_mem(&minimal_pdf(1, 612.0, 792.0)).unwrap();
        inject_outline(&mut doc, &[]).unwrap();
        assert!(read_outline(&doc).is_empty());
        // a doc that never had an outline reads empty too
        let plain = PdfDoc::load_mem(&minimal_pdf(1, 612.0, 792.0)).unwrap();
        assert!(read_outline(&plain).is_empty());
    }
}
