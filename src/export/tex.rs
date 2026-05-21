//! Typst → LaTeX via the `tylax` crate.
//!
//! Pure delegation today: tylax does all the heavy lifting. The
//! one inkhaven-specific tweak is to prepend a minimal LaTeX
//! preamble when the converter's output doesn't already include
//! `\documentclass`, so the resulting `.tex` compiles standalone
//! under `pdflatex` / `xelatex` without further editing.

/// Best-effort Typst → LaTeX conversion. Returns whatever tylax
/// emits, wrapped in a minimal preamble if missing. Never panics:
/// tylax itself returns String on every code path; the only
/// failure mode is "tylax couldn't translate part of the
/// document", which surfaces as inline comments inside the
/// emitted LaTeX (tylax's own behaviour).
pub fn typst_to_tex(input: &str) -> String {
    // `typst_document_to_latex` is tylax's full-document entry
    // point — applies the same heading / list / image handlers
    // as the basic API plus document-level scaffolding (math
    // environments, bibliography, etc.).
    let body = tylax::typst_document_to_latex(input);
    if body.contains("\\documentclass") {
        body
    } else {
        wrap_with_preamble(&body)
    }
}

fn wrap_with_preamble(body: &str) -> String {
    format!(
        "\\documentclass[11pt,a4paper]{{book}}\n\
         \\usepackage[utf8]{{inputenc}}\n\
         \\usepackage[T1]{{fontenc}}\n\
         \\usepackage{{graphicx}}\n\
         \\usepackage{{hyperref}}\n\
         \\begin{{document}}\n\
         {body}\n\
         \\end{{document}}\n"
    )
}
