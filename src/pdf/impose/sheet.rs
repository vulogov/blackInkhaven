//! 1.3.0 PDF-1 P1 — sheet emission (RFC §8.2.2).
//!
//! Each source page becomes a **Form XObject** (a reusable, transformable
//! graphics block carrying the page's content + resources); the imposed
//! sheet's content stream places those XObjects at the computed
//! coordinates with a `cm` (concatenate-matrix) operator for translation
//! + creep.  This is the standard imposition technique and is robust to
//! fonts / images / vector content (all carried through the resources).

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use super::super::doc::{PdfDoc, PdfSource};
use super::super::geometry::{mm_to_pt, Size};
use super::super::{Error, Result};
use super::layout::{Layout, Slot};
use super::ImpositionParams;

/// Emit the imposed document: front + back page per sheet (duplex order).
pub fn emit(src: &PdfDoc, layout: &Layout, params: &ImpositionParams) -> Result<PdfDoc> {
    let mut inner = src.document().clone();
    let page_ids = src.page_ids().to_vec();

    // 1. Each source page → a Form XObject (content embedded, resources
    //    referenced).  `xobjs[i]` = (xobject id, page size) for source
    //    page `i` (0-based).
    let mut xobjs: Vec<(ObjectId, Size)> = Vec::with_capacity(page_ids.len());
    for (i, &pid) in page_ids.iter().enumerate() {
        let content = inner.get_page_content(pid).map_err(Error::Lopdf)?;
        let rect = src
            .page_size(i)
            .unwrap_or_else(|| super::super::geometry::Rect::from_size(Size::new(612.0, 792.0)));
        let bbox = vec![rect.x0, rect.y0, rect.x1, rect.y1];
        let resources = page_resources_object(&inner, pid);
        let mut form = lopdf::xobject::form(bbox, vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0], content);
        if let Some(r) = resources {
            form.dict.set("Resources", r);
        }
        let xid = inner.add_object(form);
        xobjs.push((xid, rect.size()));
    }

    // 2. Build the imposed pages.  Mark geometry is layout-wide (uniform
    //    page size assumed); the imposed block is centred on the sheet.
    let page_size = xobjs.first().map(|&(_, sz)| sz).unwrap_or(Size::new(612.0, 792.0));
    let cols = layout.columns_per_side;
    let block_w = cols as f32 * page_size.width;
    let mark_geom = super::marks::MarkGeometry {
        sheet: params.sheet_size,
        page: page_size,
        columns: cols,
        block_x0: ((params.sheet_size.width - block_w) / 2.0).max(0.0),
        block_y0: ((params.sheet_size.height - page_size.height) / 2.0).max(0.0),
        crop_offset_mm: params.crop_offset_mm,
        fold_len_mm: params.fold_mark_length_mm,
    };
    let is_folded = layout.style.is_folded();
    let total_sigs = layout.signatures;
    let needs_font = params.marks.needs_font();

    let pages_root = inner.new_object_id();
    let mut kids: Vec<ObjectId> = Vec::new();
    for sheet in &layout.sheets {
        let creep_pt = mm_to_pt(super::creep::creep_shift_mm(
            params.creep,
            sheet.sheet_in_sig,
            params.paper_thickness_mm,
        ));
        let marks =
            super::marks::marks_ops(&params.marks, &mark_geom, sheet.signature, total_sigs, is_folded);
        for side in [&sheet.front, &sheet.back] {
            kids.push(build_side_page(
                &mut inner,
                pages_root,
                side,
                &xobjs,
                cols,
                params.sheet_size,
                creep_pt,
                &marks,
                needs_font,
            )?);
        }
    }

    // 3. Fresh page tree + catalog rooted at the imposed pages.
    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set(
        "Kids",
        kids.iter().map(|&id| Object::Reference(id)).collect::<Vec<_>>(),
    );
    pages.set("Count", kids.len() as i64);
    inner.objects.insert(pages_root, Object::Dictionary(pages));
    let mut cat = Dictionary::new();
    cat.set("Type", "Catalog");
    cat.set("Pages", Object::Reference(pages_root));
    let cat_id = inner.add_object(cat);
    inner.trailer.set("Root", Object::Reference(cat_id));

    // 4. Drop the now-orphaned source pages + their old content streams
    //    (the XObjects embed the content + hold the resources alive).
    inner.prune_objects();

    Ok(PdfDoc::from_document(inner, PdfSource::External))
}

/// One physical side (front or back) of a sheet: a Page sized to the
/// target sheet, placing each non-blank slot's source-page XObject.
#[allow(clippy::too_many_arguments)]
fn build_side_page(
    inner: &mut Document,
    parent: ObjectId,
    side: &[Slot],
    xobjs: &[(ObjectId, Size)],
    columns_per_side: usize,
    sheet_size: Size,
    creep_pt: f32,
    marks: &str,
    needs_font: bool,
) -> Result<ObjectId> {
    let mut xobj_dict = Dictionary::new();
    let mut content = String::new();
    for slot in side {
        let Some(page_num) = slot.page else {
            continue; // blank — nothing to draw
        };
        let Some(&(xid, page_size)) = xobjs.get(page_num - 1) else {
            continue;
        };
        let name = format!("Pg{page_num}");
        xobj_dict.set(name.as_bytes().to_vec(), Object::Reference(xid));
        let (tx, ty) = place(slot.column, columns_per_side, page_size, sheet_size, creep_pt);
        content.push_str(&format!("q 1 0 0 1 {tx:.4} {ty:.4} cm /{name} Do Q\n"));
    }
    // Printer marks are drawn on top of the placed pages.
    content.push_str(marks);

    let content_id = inner.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
    let mut res = Dictionary::new();
    res.set("XObject", Object::Dictionary(xobj_dict));
    if needs_font {
        // Base-14 Helvetica — no embedding needed; carries the signature
        // numeral.
        let mut helv = Dictionary::new();
        helv.set("Type", "Font");
        helv.set("Subtype", "Type1");
        helv.set("BaseFont", "Helvetica");
        let mut font = Dictionary::new();
        font.set("F1", Object::Dictionary(helv));
        res.set("Font", Object::Dictionary(font));
    }
    let mut page = Dictionary::new();
    page.set("Type", "Page");
    page.set("Parent", Object::Reference(parent));
    page.set(
        "MediaBox",
        Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(sheet_size.width),
            Object::Real(sheet_size.height),
        ]),
    );
    page.set("Resources", Object::Dictionary(res));
    page.set("Contents", Object::Reference(content_id));
    Ok(inner.add_object(page))
}

/// Lower-left placement (pt) of a source page on the sheet.  2-up centres
/// the two-page block; creep pulls each page toward the spine (centre
/// fold).  1-up centres the single page.
fn place(column: usize, columns: usize, page: Size, sheet: Size, creep_pt: f32) -> (f32, f32) {
    let ty = ((sheet.height - page.height) / 2.0).max(0.0);
    if columns >= 2 {
        let block_w = 2.0 * page.width;
        let block_x0 = ((sheet.width - block_w) / 2.0).max(0.0);
        let tx = if column == 0 {
            block_x0 + creep_pt // left (verso): spine on the right → shift right
        } else {
            block_x0 + page.width - creep_pt // right (recto): spine on the left → shift left
        };
        (tx, ty)
    } else {
        (((sheet.width - page.width) / 2.0).max(0.0), ty)
    }
}

/// The page's effective `/Resources` as an `Object` (inline dict cloned,
/// or a reference to the nearest resources object in the page→parent
/// chain).  `None` if the page declares no resources.
fn page_resources_object(doc: &Document, page_id: ObjectId) -> Option<Object> {
    let (inline, ids) = doc.get_page_resources(page_id).ok()?;
    if let Some(d) = inline {
        Some(Object::Dictionary(d.clone()))
    } else {
        ids.first().map(|&id| Object::Reference(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{impose, BindingStyle, BlankPolicy, CreepStrategy, ImpositionParams};
    use crate::pdf::test_support::minimal_pdf;
    use crate::pdf::PdfDoc;

    fn params(style: BindingStyle, sheet: Size) -> ImpositionParams {
        ImpositionParams {
            style,
            sheets_per_signature: 1,
            blank: BlankPolicy::Append,
            sheet_size: sheet,
            creep: CreepStrategy::None,
            paper_thickness_mm: 0.1,
            marks: super::super::marks::MarkConfig::default(),
            crop_offset_mm: 5.0,
            fold_mark_length_mm: 8.0,
        }
    }

    #[test]
    fn saddle_8_pages_makes_four_2up_sheet_sides() {
        let src = PdfDoc::load_mem(&minimal_pdf(8, 612.0, 792.0)).unwrap();
        let mut out = impose(&src, &params(BindingStyle::SaddleStitch, Size::new(1224.0, 792.0))).unwrap();
        // 2 sheets × 2 sides = 4 imposed pages, each sheet-sized.
        assert_eq!(out.page_count(), 4);
        let sz = out.page_size(0).unwrap();
        assert!((sz.width() - 1224.0).abs() < 1.0);
        assert!((sz.height() - 792.0).abs() < 1.0);
        // round-trips through bytes (valid PDF structure).
        let reloaded = PdfDoc::load_mem(&out.to_bytes().unwrap()).unwrap();
        assert_eq!(reloaded.page_count(), 4);
    }

    #[test]
    fn stab_5_pages_makes_three_1up_leaves_six_sides() {
        let src = PdfDoc::load_mem(&minimal_pdf(5, 612.0, 792.0)).unwrap();
        let mut out = impose(&src, &params(BindingStyle::Stab, Size::new(612.0, 792.0))).unwrap();
        // 3 leaves × 2 sides = 6 pages (last back is blank).
        assert_eq!(out.page_count(), 6);
        assert_eq!(PdfDoc::load_mem(&out.to_bytes().unwrap()).unwrap().page_count(), 6);
    }

    #[test]
    fn creep_shifts_placement() {
        // left page (col 0) shifts +creep, right page (col 1) shifts -creep
        let (l, _) = place(0, 2, Size::new(300.0, 400.0), Size::new(600.0, 400.0), 5.0);
        let (r, _) = place(1, 2, Size::new(300.0, 400.0), Size::new(600.0, 400.0), 5.0);
        assert!((l - 5.0).abs() < 0.01); // block_x0=0, +creep
        assert!((r - 295.0).abs() < 0.01); // block_x0+page_w-creep = 0+300-5
    }
}
