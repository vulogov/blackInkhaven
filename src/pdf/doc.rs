//! 1.3.0 PDF-1 — `PdfDoc`, the value type every pipeline stage takes and
//! returns (RFC §8.1).  Wraps a `lopdf::Document`, caches the page order
//! + sizes, and records where the PDF came from.

use std::path::{Path, PathBuf};

use lopdf::{Document, Object, ObjectId};

use super::geometry::{Rect, Size};
use super::Result;

/// Where a `PdfDoc` came from.  Tree-aware features (outline injection,
/// by-chapter split/extract) require inkhaven authorship; everything that
/// operates purely on PDF object structure works on either.
#[derive(Debug, Clone)]
pub enum PdfSource {
    /// Produced by inkhaven from a Typst tree.  The tree reference needed
    /// for outline injection is added when that lands (P0, later).
    Inkhaven { typst_root: PathBuf },
    /// Any other PDF (or one whose origin we don't track).
    External,
}

/// A parsed PDF plus cached structure.  Page operations mutate the inner
/// document and call [`PdfDoc::reindex`]; pure inspection reads the cache.
pub struct PdfDoc {
    inner: Document,
    page_ids: Vec<ObjectId>,
    page_sizes: Vec<Rect>,
    source: PdfSource,
}

impl PdfDoc {
    /// Load from disk (source: External).
    pub fn load(path: &Path) -> Result<Self> {
        let inner = Document::load(path)?;
        Ok(Self::wrap(inner, PdfSource::External))
    }

    /// Load from bytes (source: External).
    pub fn load_mem(bytes: &[u8]) -> Result<Self> {
        let inner = Document::load_mem(bytes)?;
        Ok(Self::wrap(inner, PdfSource::External))
    }

    /// Wrap an already-parsed document with a known source (e.g. the
    /// book-take path, which knows the PDF is inkhaven-authored).
    pub fn from_document(inner: Document, source: PdfSource) -> Self {
        Self::wrap(inner, source)
    }

    fn wrap(inner: Document, source: PdfSource) -> Self {
        // `get_pages` returns the pages in reading order (keyed by 1-based
        // page number), so `.values()` is the page order.
        let page_ids: Vec<ObjectId> = inner.get_pages().values().copied().collect();
        let page_sizes = page_ids
            .iter()
            .map(|&id| page_mediabox(&inner, id))
            .collect();
        Self {
            inner,
            page_ids,
            page_sizes,
            source,
        }
    }

    /// Recompute the page-order + size cache after a structural edit.
    pub fn reindex(&mut self) {
        self.page_ids = self.inner.get_pages().values().copied().collect();
        self.page_sizes = self
            .page_ids
            .iter()
            .map(|&id| page_mediabox(&self.inner, id))
            .collect();
    }

    pub fn page_count(&self) -> usize {
        self.page_ids.len()
    }

    pub fn page_ids(&self) -> &[ObjectId] {
        &self.page_ids
    }

    /// The `MediaBox` of page `idx` (0-based), inherited from the page
    /// tree if not set directly on the page.
    pub fn page_size(&self, idx: usize) -> Option<Rect> {
        self.page_sizes.get(idx).copied()
    }

    pub fn source(&self) -> &PdfSource {
        &self.source
    }

    pub fn is_inkhaven(&self) -> bool {
        matches!(self.source, PdfSource::Inkhaven { .. })
    }

    /// Borrow the inner `lopdf` document (read-only).
    pub fn document(&self) -> &Document {
        &self.inner
    }

    /// Mutable access for page-tree operations.  Callers must
    /// [`reindex`](Self::reindex) afterward if they change the page set.
    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.inner
    }

    /// Serialize to bytes (lopdf updates the xref/trailer, hence `&mut`).
    pub fn to_bytes(&mut self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.inner.save_to(&mut buf)?;
        Ok(buf)
    }

    /// Write atomically (tmp + rename via `io_atomic`) — a crash never
    /// leaves a torn PDF on disk.
    pub fn save(&mut self, path: &Path) -> Result<()> {
        let bytes = self.to_bytes()?;
        crate::io_atomic::write(path, &bytes)?;
        Ok(())
    }
}

/// `MediaBox` for `page_id`, walking the `Parent` chain since it's an
/// inheritable page attribute.  Falls back to US Letter if absent/malformed.
fn page_mediabox(doc: &Document, page_id: ObjectId) -> Rect {
    let mut cur = Some(page_id);
    let mut guard = 0;
    while let Some(id) = cur {
        guard += 1;
        if guard > 64 {
            break; // cycle guard
        }
        let Ok(dict) = doc.get_dictionary(id) else {
            break;
        };
        if let Ok(mb) = dict.get(b"MediaBox") {
            if let Ok(arr) = mb.as_array() {
                if let Some(r) = mediabox_from_array(arr) {
                    return r;
                }
            }
        }
        cur = dict.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
    }
    Rect::from_size(Size::new(612.0, 792.0))
}

fn mediabox_from_array(arr: &[Object]) -> Option<Rect> {
    if arr.len() != 4 {
        return None;
    }
    let v: Vec<f32> = arr.iter().filter_map(object_as_f32).collect();
    if v.len() != 4 {
        return None;
    }
    Some(Rect::from_mediabox([v[0], v[1], v[2], v[3]]))
}

fn object_as_f32(o: &Object) -> Option<f32> {
    match o {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_support::minimal_pdf;
    use lopdf::Document;

    #[test]
    fn loads_pages_sizes_and_source() {
        let bytes = minimal_pdf(3, 612.0, 792.0);
        let pdf = PdfDoc::load_mem(&bytes).unwrap();
        assert_eq!(pdf.page_count(), 3);
        assert_eq!(pdf.page_ids().len(), 3);
        let sz = pdf.page_size(0).unwrap();
        assert!((sz.width() - 612.0).abs() < 0.01);
        assert!((sz.height() - 792.0).abs() < 0.01);
        assert!(pdf.page_size(99).is_none());
        assert!(matches!(pdf.source(), PdfSource::External));
        assert!(!pdf.is_inkhaven());
    }

    #[test]
    fn round_trips_to_bytes() {
        let bytes = minimal_pdf(2, 595.0, 842.0);
        let mut pdf = PdfDoc::load_mem(&bytes).unwrap();
        let out = pdf.to_bytes().unwrap();
        assert!(out.starts_with(b"%PDF-"));
        let reloaded = PdfDoc::load_mem(&out).unwrap();
        assert_eq!(reloaded.page_count(), 2);
        assert!((reloaded.page_size(1).unwrap().width() - 595.0).abs() < 0.01);
    }

    #[test]
    fn inkhaven_source_flag() {
        let bytes = minimal_pdf(1, 612.0, 792.0);
        let inner = Document::load_mem(&bytes).unwrap();
        let pdf = PdfDoc::from_document(
            inner,
            PdfSource::Inkhaven {
                typst_root: std::path::PathBuf::from("/tmp/proj"),
            },
        );
        assert!(pdf.is_inkhaven());
    }
}
