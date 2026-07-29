//! PDF-1 — an embedded Unicode font for the native (Typst-free) cover / spine
//! text.
//!
//! The cover is laid out geometrically in raw `lopdf`, with text drawn as base-14
//! Helvetica `Tj` operators. Helvetica has no Cyrillic and, with no `/Encoding`,
//! mis-renders even accented Latin — so a Russian (or `café`/`Müller`) title
//! printed as mojibake. This module embeds **DejaVu Sans Mono** (bundled by
//! `typst-assets`, TrueType with full Cyrillic + Latin coverage — and already in
//! the binary via typst-kit's `embed-fonts`, so no size cost) as a Type0 /
//! CIDFontType2 font with Identity-H encoding, and glyph-ID-encodes the text. It
//! is monospace — this is a geometric fallback cover, not typeset body — but it is
//! *correct* in every project language (en/ru/fr/de/es).
//!
//! Usage: `EmbeddedFont::load()` (falls back to `None` → caller keeps Helvetica),
//! `encode(text)` for each string (returns an Identity-H `<hex>` literal, records
//! the glyphs), `width(text, size)` for layout, then `finalize(doc)` once to build
//! the font-object chain and get the Type0 object id for the page `/Font` dict.

use std::collections::BTreeMap;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use ttf_parser::Face;

/// The bundled DejaVu Sans Mono *regular* face bytes (TrueType, has Cyrillic).
/// `None` if the expected font isn't in the typst-assets bundle.
fn dejavu_mono_regular() -> Option<&'static [u8]> {
    for data in typst_assets::fonts() {
        let Ok(face) = Face::parse(data, 0) else { continue };
        // The only bundled face that is TrueType (glyf outlines → FontFile2),
        // monospace, upright/regular, and carries Cyrillic is DejaVuSansMono.ttf.
        if face.tables().glyf.is_some()
            && face.is_monospaced()
            && !face.is_bold()
            && !face.is_italic()
            && face.glyph_index('\u{0410}').is_some() // Cyrillic capital A
        {
            return Some(data);
        }
    }
    None
}

/// An embedded Unicode font under construction. Records the glyphs each `encode`
/// touches so `finalize` can emit a matching ToUnicode map.
pub struct EmbeddedFont {
    data: &'static [u8],
    face: Face<'static>,
    units_per_em: f32,
    /// glyph id → a representative source char (for the ToUnicode CMap).
    used: BTreeMap<u16, char>,
}

impl EmbeddedFont {
    /// Load the bundled Unicode font, or `None` when unavailable (the caller then
    /// keeps the base-14 Helvetica path — correct for pure-ASCII text).
    pub fn load() -> Option<EmbeddedFont> {
        let data = dejavu_mono_regular()?;
        let face = Face::parse(data, 0).ok()?;
        let units_per_em = face.units_per_em() as f32;
        if units_per_em <= 0.0 {
            return None;
        }
        Some(EmbeddedFont { data, face, units_per_em, used: BTreeMap::new() })
    }

    /// Encode `text` as an Identity-H hex string (`<AABB…>` of 2-byte glyph ids)
    /// and record each glyph. A char with no glyph maps to `.notdef` (0).
    pub fn encode(&mut self, text: &str) -> String {
        let mut s = String::with_capacity(text.len() * 4 + 2);
        s.push('<');
        for ch in text.chars() {
            let gid = self.face.glyph_index(ch).map(|g| g.0).unwrap_or(0);
            self.used.insert(gid, ch);
            s.push_str(&format!("{gid:04X}"));
        }
        s.push('>');
        s
    }

    /// Printed width of `text` at `size` pt (sum of glyph advances).
    pub fn width(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|ch| {
                let adv = self
                    .face
                    .glyph_index(ch)
                    .and_then(|g| self.face.glyph_hor_advance(g))
                    .unwrap_or(0) as f32;
                adv / self.units_per_em * size
            })
            .sum()
    }

    /// Advance of a single glyph in em units (monospace → constant). Handy as the
    /// per-char factor for auto-fit maths that runs before the size is known.
    pub fn em_advance(&self) -> f32 {
        self.width("0", 1.0)
    }

    fn scale_1000(&self, v: f32) -> i64 {
        (v / self.units_per_em * 1000.0).round() as i64
    }

    /// Build the Type0 → CIDFontType2 → FontDescriptor → FontFile2 chain (plus a
    /// ToUnicode CMap) in `doc`, returning the Type0 object id to reference from
    /// the page `/Font` resource.
    pub fn finalize(self, doc: &mut Document) -> ObjectId {
        // FontFile2 — the raw TrueType program, flate-compressed, with the
        // uncompressed length in /Length1 (required for TrueType font files).
        let mut ff = Stream::new(Dictionary::new(), self.data.to_vec());
        ff.dict.set("Length1", Object::Integer(self.data.len() as i64));
        let _ = ff.compress();
        let ff_id = doc.add_object(ff);

        // FontDescriptor.
        let bbox = self.face.global_bounding_box();
        let cap = self.face.capital_height().unwrap_or_else(|| self.face.ascender());
        let mut fd = Dictionary::new();
        fd.set("Type", "FontDescriptor");
        fd.set("FontName", Object::Name(b"DejaVuSansMono".to_vec()));
        // FixedPitch (bit 1) + Nonsymbolic (bit 6): a monospace text font.
        fd.set("Flags", Object::Integer(33));
        fd.set(
            "FontBBox",
            vec![
                Object::Integer(self.scale_1000(bbox.x_min as f32)),
                Object::Integer(self.scale_1000(bbox.y_min as f32)),
                Object::Integer(self.scale_1000(bbox.x_max as f32)),
                Object::Integer(self.scale_1000(bbox.y_max as f32)),
            ],
        );
        fd.set("ItalicAngle", Object::Real(self.face.italic_angle()));
        fd.set("Ascent", Object::Integer(self.scale_1000(self.face.ascender() as f32)));
        fd.set("Descent", Object::Integer(self.scale_1000(self.face.descender() as f32)));
        fd.set("CapHeight", Object::Integer(self.scale_1000(cap as f32)));
        fd.set("StemV", Object::Integer(80)); // no reliable source; a safe estimate
        fd.set("FontFile2", Object::Reference(ff_id));
        let fd_id = doc.add_object(fd);

        // Default width (monospace → every glyph shares it; no /W array needed).
        let dw = self
            .face
            .glyph_index(' ')
            .and_then(|g| self.face.glyph_hor_advance(g))
            .map(|a| self.scale_1000(a as f32))
            .unwrap_or(600);

        // CIDFontType2 descendant.
        let mut cid_sys = Dictionary::new();
        cid_sys.set("Registry", Object::string_literal("Adobe"));
        cid_sys.set("Ordering", Object::string_literal("Identity"));
        cid_sys.set("Supplement", Object::Integer(0));
        let mut cid = Dictionary::new();
        cid.set("Type", "Font");
        cid.set("Subtype", "CIDFontType2");
        cid.set("BaseFont", Object::Name(b"DejaVuSansMono".to_vec()));
        cid.set("CIDSystemInfo", Object::Dictionary(cid_sys));
        cid.set("FontDescriptor", Object::Reference(fd_id));
        cid.set("CIDToGIDMap", Object::Name(b"Identity".to_vec()));
        cid.set("DW", Object::Integer(dw));
        let cid_id = doc.add_object(cid);

        // ToUnicode CMap (so the cover text stays selectable / searchable).
        let mut tu = Stream::new(Dictionary::new(), self.build_tounicode().into_bytes());
        let _ = tu.compress();
        let tu_id = doc.add_object(tu);

        // Type0 composite font.
        let mut t0 = Dictionary::new();
        t0.set("Type", "Font");
        t0.set("Subtype", "Type0");
        t0.set("BaseFont", Object::Name(b"DejaVuSansMono".to_vec()));
        t0.set("Encoding", Object::Name(b"Identity-H".to_vec()));
        t0.set("DescendantFonts", vec![Object::Reference(cid_id)]);
        t0.set("ToUnicode", Object::Reference(tu_id));
        doc.add_object(t0)
    }

    /// The ToUnicode CMap body mapping each used glyph id → its UTF-16BE scalar.
    fn build_tounicode(&self) -> String {
        let entries: Vec<(u16, char)> = self.used.iter().map(|(&g, &c)| (g, c)).collect();
        let mut body = String::new();
        // `beginbfchar` blocks are capped at 100 entries each by the spec.
        for chunk in entries.chunks(100) {
            body.push_str(&format!("{} beginbfchar\n", chunk.len()));
            for (g, c) in chunk {
                let mut buf = [0u16; 2];
                let hex: String = c.encode_utf16(&mut buf).iter().map(|u| format!("{u:04X}")).collect();
                body.push_str(&format!("<{g:04X}> <{hex}>\n"));
            }
            body.push_str("endbfchar\n");
        }
        format!(
            "/CIDInit /ProcSet findresource begin\n\
             12 dict begin\n\
             begincmap\n\
             /CIDSystemInfo <</Registry (Adobe) /Ordering (UCS) /Supplement 0>> def\n\
             /CMapName /Adobe-Identity-UCS def\n\
             /CMapType 2 def\n\
             1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n\
             {body}\
             endcmap\n\
             CMapName currentdict /CMap defineresource pop\n\
             end\nend\n"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_the_bundled_unicode_font() {
        let f = EmbeddedFont::load().expect("DejaVu Sans Mono should be bundled");
        // Cyrillic and accented Latin both resolve to real (non-.notdef) glyphs.
        let mut f = f;
        let hex = f.encode("Война café");
        assert!(hex.starts_with('<') && hex.ends_with('>'));
        assert!(!hex.contains("0000"), "every char should map to a real glyph: {hex}");
        assert!(f.width("Война", 12.0) > 0.0);
    }

    #[test]
    fn finalize_builds_a_type0_font_object() {
        let mut doc = Document::with_version("1.5");
        let mut f = EmbeddedFont::load().unwrap();
        let _ = f.encode("Мир");
        let id = f.finalize(&mut doc);
        let font = doc.get_object(id).unwrap().as_dict().unwrap();
        assert_eq!(font.get(b"Subtype").unwrap().as_name().unwrap(), b"Type0");
        assert_eq!(font.get(b"Encoding").unwrap().as_name().unwrap(), b"Identity-H");
        assert!(font.get(b"ToUnicode").is_ok());
        assert!(font.get(b"DescendantFonts").is_ok());
    }
}
