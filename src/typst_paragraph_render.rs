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

/// The result of rendering one paragraph. The image is RGBA8.
pub struct RenderedParagraph {
    pub width: u32,
    pub height: u32,
    pub png_bytes: Vec<u8>,
    pub image: image::DynamicImage,
    /// Total page count of the laid-out document. The preview
    /// modal only shows page 1; surface this so the UI can tell
    /// the user "showing 1/N" when their paragraph overflowed a
    /// single page.
    pub total_pages: usize,
}

/// Render `source` to a PNG at the given `pixel_per_pt`. Returns
/// the human-readable error text (resembling `typst compile`'s
/// stderr) when the compile failed — the caller surfaces it in
/// the editor's status bar instead of opening the preview.
pub fn render(
    source: &str,
    settings: WorldSettings,
    pixel_per_pt: f32,
) -> Result<RenderedParagraph, String> {
    let world = InkhavenWorld::in_memory(
        std::env::temp_dir(),
        source.to_owned(),
        settings,
    );
    let Warned { output, warnings: _ } =
        typst::compile::<PagedDocument>(&world);
    let document = output.map_err(|errors| format_errors(&errors))?;
    let page = document
        .pages
        .first()
        .ok_or_else(|| "compile produced zero pages".to_owned())?;
    let pixmap = typst_render::render(page, pixel_per_pt);
    let width = pixmap.width();
    let height = pixmap.height();
    let png_bytes = pixmap
        .encode_png()
        .map_err(|e| format!("encode PNG: {e}"))?;
    // RGBA8 conversion for the in-memory image view (used by
    // ratatui-image). Pixmap stores premultiplied RGBA — the
    // image crate accepts the same layout via `from_raw`.
    let raw = pixmap.data().to_vec();
    let rgba = image::RgbaImage::from_raw(width, height, raw)
        .ok_or_else(|| "image dimensions did not match buffer".to_owned())?;
    Ok(RenderedParagraph {
        width,
        height,
        png_bytes,
        image: image::DynamicImage::ImageRgba8(rgba),
        total_pages: document.pages.len(),
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
        let out = render(
            source,
            WorldSettings {
                bundle_fonts: true,
                use_system_fonts: true,
                packages_enabled: false,
            },
            2.0,
        )
        .expect("render");
        assert!(out.width > 0, "width was 0");
        assert!(out.height > 0, "height was 0");
        assert!(!out.png_bytes.is_empty(), "no PNG bytes");
        // PNG magic header.
        assert_eq!(
            &out.png_bytes[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        );
        assert!(out.total_pages >= 1);
    }
}
