//! AI glyph drafting (LANG-1 P5.5).
//!
//! The AI half is a thin layer (a prompt + a blocking inference call in the
//! CLI); the deterministic, testable half is here: pulling a clean SVG out of
//! a model reply (which may wrap it in prose or markdown fences) so it can be
//! run through the [`super::preflight`] suitability gate before anything is
//! bound. AI-drafted artwork is advisory — proposal-gated, committed only on
//! `--yes`, exactly like every other AI feature in the suite.

/// Extract the `<svg>…</svg>` markup from a model reply. Tolerates leading
/// prose, markdown code fences, and trailing commentary by taking the span
/// from the first `<svg` to the last `</svg>`.
pub fn extract_svg(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    // ASCII-only needles, so byte offsets line up with the original `text`.
    let start = lower.find("<svg")?;
    let end_rel = lower[start..].rfind("</svg>")?;
    let end = start + end_rel + "</svg>".len();
    Some(text[start..end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulls_svg_from_fenced_reply() {
        let reply = "Here is your glyph:\n\n```svg\n<svg viewBox=\"0 0 10 10\">\
            <path d=\"M0 0 L10 0 L10 10 Z\" fill=\"black\"/></svg>\n```\nEnjoy!";
        let svg = extract_svg(reply).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(!svg.contains("```"));
        assert!(!svg.contains("Enjoy"));
    }

    #[test]
    fn handles_uppercase_tags() {
        let reply = "<SVG><PATH d=\"M0 0\"/></SVG>";
        assert_eq!(extract_svg(reply).unwrap(), reply);
    }

    #[test]
    fn takes_outermost_when_nested_close_text() {
        let reply = "<svg><g></g></svg>";
        assert_eq!(extract_svg(reply).unwrap(), reply);
    }

    #[test]
    fn none_without_svg() {
        assert!(extract_svg("sorry, I can't draw that").is_none());
        assert!(extract_svg("<svg> unterminated").is_none());
    }

    // The P5.5 contract: a well-formed model reply extracts to a glyph the
    // preflight accepts; a stroke-only one is caught before binding.
    #[test]
    fn drafted_glyph_passes_preflight() {
        use crate::conlang::writing::preflight;
        let reply = "Sure! Here's a bold vertical bar:\n```svg\n\
            <svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1000 1000\">\
            <path d=\"M400 100 L600 100 L600 900 L400 900 Z\" fill=\"black\"/></svg>\n```";
        let svg = extract_svg(reply).unwrap();
        assert!(preflight::lint_svg(&svg).is_usable());
    }

    #[test]
    fn stroke_only_draft_is_rejected() {
        use crate::conlang::writing::preflight;
        let reply = "<svg viewBox=\"0 0 1000 1000\">\
            <path d=\"M100 500 L900 500\" stroke=\"black\" fill=\"none\"/></svg>";
        let svg = extract_svg(reply).unwrap();
        assert!(!preflight::lint_svg(&svg).is_usable());
    }
}
