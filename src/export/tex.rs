//! Typst → LaTeX via the `tylax` crate.
//!
//! tylax does the heavy lifting and already emits a complete, standalone
//! document — its own `\documentclass{article}` plus a sensible math/graphics
//! preamble. inkhaven's contribution (PAPER, 1.6.15+) is to let a paper author
//! retarget that class: when [`TexExportConfig`] names a journal class we
//! rewrite tylax's `\documentclass` line and inject any extra packages /
//! preamble lines before `\begin{document}`. With the default (empty) config we
//! leave tylax's `article` output byte-for-byte untouched, so nothing regresses.
//!
//! [`TexExportConfig`]: crate::config::TexExportConfig

use crate::config::TexExportConfig;

/// Best-effort Typst → LaTeX conversion. Returns tylax's output, with the
/// document class / preamble adjusted per `cfg`. Never panics: tylax returns a
/// String on every path; untranslatable constructs surface as inline comments
/// in the emitted LaTeX (tylax's own behaviour).
pub fn typst_to_tex(input: &str, cfg: &TexExportConfig) -> String {
    // `typst_document_to_latex` is tylax's full-document entry point — it
    // applies the heading / list / image / math handlers and wraps the result
    // in a complete document (its own `\documentclass{article}` + preamble).
    let body = tylax::typst_document_to_latex(input);
    if body.contains("\\documentclass") {
        apply_config(body, cfg)
    } else {
        // Defensive: some tylax path emitted a bare fragment. Wrap it so the
        // `.tex` still compiles standalone.
        wrap_with_preamble(&body, cfg)
    }
}

/// `true` when the author has configured anything that should override tylax's
/// default document scaffolding.
fn overrides_configured(cfg: &TexExportConfig) -> bool {
    !cfg.document_class.trim().is_empty()
        || cfg.extra_packages.iter().any(|p| !p.trim().is_empty())
        || cfg.preamble.iter().any(|p| !p.trim().is_empty())
}

/// Build a `\documentclass[opts]{class}` line (brackets omitted when no opts).
fn documentclass_line(class: &str, opts: &str) -> String {
    if opts.is_empty() {
        format!("\\documentclass{{{class}}}")
    } else {
        format!("\\documentclass[{opts}]{{{class}}}")
    }
}

/// The extra `\usepackage` + raw preamble lines the author configured, in order.
fn extra_lines(cfg: &TexExportConfig) -> Vec<String> {
    let mut lines = Vec::new();
    for pkg in &cfg.extra_packages {
        let p = pkg.trim();
        if p.is_empty() {
            continue;
        }
        // A bare name (`amsmath`) becomes `\usepackage{amsmath}`; a full
        // `\usepackage[opts]{...}` line is passed through verbatim.
        if p.starts_with('\\') {
            lines.push(p.to_string());
        } else {
            lines.push(format!("\\usepackage{{{p}}}"));
        }
    }
    for line in &cfg.preamble {
        lines.push(line.clone());
    }
    lines
}

/// Rewrite tylax's `\documentclass` line to the configured journal class and
/// inject extra packages / preamble before `\begin{document}`. With no
/// overrides configured, returns `body` unchanged (tylax's `article` output).
fn apply_config(body: String, cfg: &TexExportConfig) -> String {
    if !overrides_configured(cfg) {
        return body;
    }
    let class = cfg.document_class.trim();
    let opts = cfg.class_options.trim();
    let trailing_newline = body.ends_with('\n');
    let mut lines: Vec<String> = body.lines().map(str::to_string).collect();

    if !class.is_empty() {
        if let Some(i) = lines
            .iter()
            .position(|l| l.trim_start().starts_with("\\documentclass"))
        {
            lines[i] = documentclass_line(class, opts);
        }
    }

    // Skip any extra that tylax's preamble already emits verbatim (e.g.
    // `\usepackage{amsmath}`), so a re-listed package isn't duplicated.
    let existing: std::collections::HashSet<&str> =
        lines.iter().map(|l| l.trim()).collect();
    let extras: Vec<String> = extra_lines(cfg)
        .into_iter()
        .filter(|l| !existing.contains(l.trim()))
        .collect();
    if !extras.is_empty() {
        let at = lines
            .iter()
            .position(|l| l.trim_start().starts_with("\\begin{document}"))
            .unwrap_or(lines.len());
        for (k, line) in extras.into_iter().enumerate() {
            lines.insert(at + k, line);
        }
    }

    let mut out = lines.join("\n");
    if trailing_newline {
        out.push('\n');
    }
    out
}

/// Fallback wrapper for the rare case tylax emits a bare fragment with no
/// `\documentclass`. Uses the configured class (default `article`).
fn wrap_with_preamble(body: &str, cfg: &TexExportConfig) -> String {
    let class = {
        let c = cfg.document_class.trim();
        if c.is_empty() { "article" } else { c }
    };
    let opts = cfg.class_options.trim();

    let mut out = String::new();
    out.push_str(&documentclass_line(class, opts));
    out.push('\n');
    out.push_str("\\usepackage[utf8]{inputenc}\n");
    out.push_str("\\usepackage[T1]{fontenc}\n");
    out.push_str("\\usepackage{graphicx}\n");
    out.push_str("\\usepackage{hyperref}\n");
    for line in extra_lines(cfg) {
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("\\begin{document}\n");
    out.push_str(body);
    out.push_str("\n\\end{document}\n");
    out
}

#[cfg(test)]
mod tests {
    use crate::config::TexExportConfig;

    #[test]
    fn default_config_leaves_tylax_article_untouched() {
        let raw = tylax::typst_document_to_latex("= Head\n\ntext\n");
        let out = super::typst_to_tex("= Head\n\ntext\n", &TexExportConfig::default());
        // Default config must not perturb tylax's own output at all.
        assert_eq!(out, raw);
        assert!(out.contains("\\documentclass{article}"));
    }

    #[test]
    fn journal_class_rewrites_and_injects_packages() {
        let cfg = TexExportConfig {
            document_class: "IEEEtran".into(),
            class_options: "conference".into(),
            extra_packages: vec!["amsmath".into(), "\\usepackage[numbers]{natbib}".into()],
            preamble: vec!["\\pagestyle{empty}".into()],
        };
        let out = super::typst_to_tex("= H\n\nbody\n", &cfg);
        assert!(out.contains("\\documentclass[conference]{IEEEtran}"), "{out}");
        // tylax's default article class is gone.
        assert!(!out.contains("\\documentclass{article}"), "{out}");
        assert!(out.contains("\\usepackage{amsmath}"));
        assert!(out.contains("\\usepackage[numbers]{natbib}"));
        assert!(out.contains("\\pagestyle{empty}"));
        // Extras land in the preamble, before the document body.
        let doc = out.find("\\begin{document}").unwrap();
        assert!(out.find("\\pagestyle{empty}").unwrap() < doc);
    }

    #[test]
    fn class_only_override_keeps_tylax_packages() {
        let cfg = TexExportConfig {
            document_class: "elsarticle".into(),
            class_options: String::new(),
            ..Default::default()
        };
        let out = super::typst_to_tex("body\n", &cfg);
        assert!(out.contains("\\documentclass{elsarticle}"), "{out}");
        assert!(!out.contains("\\documentclass[]"));
        // tylax's own packages (e.g. amsmath) survive.
        assert!(out.contains("\\usepackage{amsmath}"));
    }
}

