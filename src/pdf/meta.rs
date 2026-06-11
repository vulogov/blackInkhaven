//! 1.3.0 PDF-1 — document metadata (the `Info` dictionary; RFC §8.8).
//!
//! Read / write / strip the standard `Title` / `Author` / `Subject` /
//! `Keywords` / `Creator` / `Producer` fields.  On book-take these are
//! auto-populated from project HJSON (`book.title`/`author`/`keywords`).

use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};

use super::doc::PdfDoc;
use super::Result;

/// The standard PDF document-information fields.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Vec<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
}

/// Read the `Info` dictionary; missing fields → `None` / empty.
pub fn read_metadata(doc: &PdfDoc) -> PdfMetadata {
    let d = doc.document();
    let info = info_id(d).and_then(|id| d.get_dictionary(id).ok());
    let Some(info) = info else {
        return PdfMetadata::default();
    };
    let get = |k: &[u8]| info.get(k).ok().and_then(as_pdf_string);
    PdfMetadata {
        title: get(b"Title"),
        author: get(b"Author"),
        subject: get(b"Subject"),
        keywords: get(b"Keywords").map(|s| split_keywords(&s)).unwrap_or_default(),
        creator: get(b"Creator"),
        producer: get(b"Producer"),
    }
}

/// Write the `Info` dictionary, creating it if absent.  A `None` field
/// (or empty `keywords`) removes that entry.
pub fn write_metadata(doc: &mut PdfDoc, m: &PdfMetadata) -> Result<()> {
    let id = ensure_info(doc.document_mut());
    let info = doc.document_mut().get_dictionary_mut(id)?;
    set_opt(info, "Title", m.title.as_deref());
    set_opt(info, "Author", m.author.as_deref());
    set_opt(info, "Subject", m.subject.as_deref());
    if m.keywords.is_empty() {
        info.remove(b"Keywords");
    } else {
        info.set("Keywords", pdf_string(&m.keywords.join(", ")));
    }
    set_opt(info, "Creator", m.creator.as_deref());
    set_opt(info, "Producer", m.producer.as_deref());
    Ok(())
}

/// Clear every standard field from the `Info` dictionary (privacy).
pub fn strip_metadata(doc: &mut PdfDoc) -> Result<()> {
    if let Some(id) = info_id(doc.document()) {
        if let Ok(info) = doc.document_mut().get_dictionary_mut(id) {
            for k in [
                &b"Title"[..],
                b"Author",
                b"Subject",
                b"Keywords",
                b"Creator",
                b"Producer",
            ] {
                info.remove(k);
            }
        }
    }
    Ok(())
}

fn info_id(d: &Document) -> Option<ObjectId> {
    d.trailer.get(b"Info").ok().and_then(|o| o.as_reference().ok())
}

fn ensure_info(d: &mut Document) -> ObjectId {
    if let Some(id) = info_id(d) {
        return id;
    }
    let id = d.add_object(Dictionary::new());
    d.trailer.set("Info", Object::Reference(id));
    id
}

fn set_opt(info: &mut Dictionary, key: &str, val: Option<&str>) {
    match val {
        Some(s) => {
            info.set(key, pdf_string(s));
        }
        None => {
            info.remove(key.as_bytes());
        }
    }
}

fn split_keywords(s: &str) -> Vec<String> {
    s.split([',', ';'])
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect()
}

fn as_pdf_string(o: &Object) -> Option<String> {
    match o {
        Object::String(b, _) => Some(decode_pdf_string(b)),
        _ => None,
    }
}

/// Encode a Rust string as a PDF text string — ASCII as a literal,
/// otherwise UTF-16BE with a BOM (the PDF way to carry non-ASCII).
fn pdf_string(s: &str) -> Object {
    if s.is_ascii() {
        Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
    } else {
        let mut bytes = vec![0xFE, 0xFF];
        for u in s.encode_utf16() {
            bytes.extend_from_slice(&u.to_be_bytes());
        }
        Object::String(bytes, StringFormat::Literal)
    }
}

fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u16s)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_support::minimal_pdf;
    use crate::pdf::PdfDoc;

    #[test]
    fn write_then_read_round_trips() {
        let mut doc = PdfDoc::load_mem(&minimal_pdf(1, 612.0, 792.0)).unwrap();
        let m = PdfMetadata {
            title: Some("The Lantern Room".into()),
            author: Some("V. Ulogov".into()),
            subject: Some("a novel".into()),
            keywords: vec!["fiction".into(), "literary".into()],
            creator: Some("Inkhaven 1.3.0".into()),
            producer: Some("typst + inkhaven".into()),
        };
        write_metadata(&mut doc, &m).unwrap();
        // round-trip through bytes too (proves it serializes).
        let bytes = doc.to_bytes().unwrap();
        let reloaded = PdfDoc::load_mem(&bytes).unwrap();
        assert_eq!(read_metadata(&reloaded), m);
    }

    #[test]
    fn strip_clears_fields() {
        let mut doc = PdfDoc::load_mem(&minimal_pdf(1, 612.0, 792.0)).unwrap();
        write_metadata(
            &mut doc,
            &PdfMetadata {
                title: Some("secret".into()),
                author: Some("me".into()),
                ..Default::default()
            },
        )
        .unwrap();
        strip_metadata(&mut doc).unwrap();
        assert_eq!(read_metadata(&doc), PdfMetadata::default());
    }

    #[test]
    fn non_ascii_title_round_trips_via_utf16() {
        let mut doc = PdfDoc::load_mem(&minimal_pdf(1, 612.0, 792.0)).unwrap();
        let m = PdfMetadata {
            title: Some("Café — Война и мир 日本語".into()),
            ..Default::default()
        };
        write_metadata(&mut doc, &m).unwrap();
        let bytes = doc.to_bytes().unwrap();
        let reloaded = PdfDoc::load_mem(&bytes).unwrap();
        assert_eq!(read_metadata(&reloaded).title, m.title);
    }
}
