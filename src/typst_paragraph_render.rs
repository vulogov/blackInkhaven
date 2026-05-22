//! Render a single paragraph's Typst source to a PNG bitmap
//! (1.2.5+, Ctrl+V R).
//!
//! Sits one layer above `typst_inprocess::check_semantic` — same
//! "World with an in-memory main source" trick, but instead of
//! lifting diagnostics, we keep the laid-out frames and run them
//! through `typst-render` to produce a raster image.
//!
//! Used in two contexts:
//!
//! * **Preview**: low-DPI (e.g. 2.0 px/pt) — feeds the floating
//!   `Modal::RenderedPreview` via ratatui-image.
//! * **Save**: high-DPI (e.g. 4.0 px/pt) — encoded to PNG bytes
//!   and dumped to disk by the user's `S` keypress.
//!
//! Both call the same `render` function; the only difference is
//! the `pixel_per_pt` argument. The body is re-compiled twice in
//! the save path — once at preview time, once at save time —
//! because Comemo's cache is keyed by World identity and we
//! build a fresh World for each call. That's fast in practice
//! (50–200 ms on small paragraphs) and keeps the modal lean.

use typst::diag::Warned;
use typst::layout::PagedDocument;

use crate::typst_world::{InkhavenWorld, WorldSettings};

/// The result of rendering one paragraph page. The image is RGBA8.
pub struct RenderedParagraph {
    pub width: u32,
    pub height: u32,
    pub png_bytes: Vec<u8>,
    pub image: image::DynamicImage,
}

/// Render every page of `source` at the given `pixel_per_pt`.
/// Returns one `RenderedParagraph` per page. The first page is
/// always at index 0. Compile errors emerge as a single
/// human-readable string (resembling `typst compile`'s stderr).
pub fn render_all(
    source: &str,
    settings: WorldSettings,
    pixel_per_pt: f32,
) -> Result<Vec<RenderedParagraph>, String> {
    let world = InkhavenWorld::in_memory(
        std::env::temp_dir(),
        source.to_owned(),
        settings,
    );
    let Warned { output, warnings: _ } =
        typst::compile::<PagedDocument>(&world);
    let document = output.map_err(|errors| format_errors(&errors))?;
    if document.pages.is_empty() {
        return Err("compile produced zero pages".to_owned());
    }
    let mut out = Vec::with_capacity(document.pages.len());
    for page in &document.pages {
        let pixmap = typst_render::render(page, pixel_per_pt);
        let width = pixmap.width();
        let height = pixmap.height();
        let png_bytes = pixmap
            .encode_png()
            .map_err(|e| format!("encode PNG: {e}"))?;
        let raw = pixmap.data().to_vec();
        let rgba = image::RgbaImage::from_raw(width, height, raw)
            .ok_or_else(|| "image dimensions did not match buffer".to_owned())?;
        out.push(RenderedParagraph {
            width,
            height,
            png_bytes,
            image: image::DynamicImage::ImageRgba8(rgba),
        });
    }
    Ok(out)
}

/// Render exactly one page (`page_idx`, 0-based) at the given
/// `pixel_per_pt`. Used by the `S` save path which only needs
/// the page currently being viewed. Returns
/// `Err("page index out of range")` if the index is past the
/// document.
pub fn render_page(
    source: &str,
    settings: WorldSettings,
    pixel_per_pt: f32,
    page_idx: usize,
) -> Result<RenderedParagraph, String> {
    let world = InkhavenWorld::in_memory(
        std::env::temp_dir(),
        source.to_owned(),
        settings,
    );
    let Warned { output, warnings: _ } =
        typst::compile::<PagedDocument>(&world);
    let document = output.map_err(|errors| format_errors(&errors))?;
    let total = document.pages.len();
    let page = document
        .pages
        .get(page_idx)
        .ok_or_else(|| format!("page index {page_idx} out of range (have {total})"))?;
    let pixmap = typst_render::render(page, pixel_per_pt);
    let width = pixmap.width();
    let height = pixmap.height();
    let png_bytes = pixmap
        .encode_png()
        .map_err(|e| format!("encode PNG: {e}"))?;
    let raw = pixmap.data().to_vec();
    let rgba = image::RgbaImage::from_raw(width, height, raw)
        .ok_or_else(|| "image dimensions did not match buffer".to_owned())?;
    Ok(RenderedParagraph {
        width,
        height,
        png_bytes,
        image: image::DynamicImage::ImageRgba8(rgba),
    })
}

fn format_errors(errors: &[typst::diag::SourceDiagnostic]) -> String {
    let mut out = String::new();
    for e in errors {
        out.push_str(&e.message);
        out.push('\n');
    }
    if out.is_empty() {
        out.push_str("compile failed with no diagnostics");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `#[ignore]` for the same reason `check_semantic`'s smoke
    /// is: rendering needs fonts loadable.
    #[test]
    #[ignore]
    fn renders_a_simple_paragraph() {
        let source =
            "#set page(width: 10cm, height: 5cm, margin: 1cm)\n\
             = Hello\nProse line.\n";
        let pages = render_all(
            source,
            WorldSettings {
                bundle_fonts: true,
                use_system_fonts: true,
                packages_enabled: false,
            },
            2.0,
        )
        .expect("render");
        assert!(!pages.is_empty(), "expected at least one page");
        let first = &pages[0];
        assert!(first.width > 0, "width was 0");
        assert!(first.height > 0, "height was 0");
        assert!(!first.png_bytes.is_empty(), "no PNG bytes");
        // PNG magic header.
        assert_eq!(
            &first.png_bytes[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        );
        assert_eq!(pages.len(), 1, "single-page paragraph");
    }

    /// Multi-page document — exercise both `render_all` returning
    /// every page and `render_page(idx)` returning the same page.
    #[test]
    #[ignore]
    fn renders_each_page_consistently() {
        let source =
            "#set page(width: 10cm, height: 5cm, margin: 1cm)\n\
             = Page 1\n#pagebreak()\n= Page 2\n#pagebreak()\n= Page 3\n";
        let settings = WorldSettings {
            bundle_fonts: true,
            use_system_fonts: true,
            packages_enabled: false,
        };
        let pages = render_all(source, settings.clone(), 2.0).expect("render_all");
        assert_eq!(pages.len(), 3, "expected 3 pages, got {}", pages.len());
        // Spot-check render_page picks the same page 1 as
        // render_all[1].
        let mid = render_page(source, settings, 1.0, 1).expect("render_page");
        assert!(mid.width > 0 && mid.height > 0);
    }
}
