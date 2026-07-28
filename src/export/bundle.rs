//! ARXIV-1 (1.6.16+) — the arXiv / preprint submission bundle.
//!
//! `inkhaven export tex --bundle <dir|zip>` writes a self-contained LaTeX
//! submission: the `.tex`, `sources.bib`, every referenced figure (copied flat
//! with its `\includegraphics` path rewritten), and a `MANIFEST.txt`. arXiv
//! compiles the source on its server, so the bundle must carry everything the
//! build needs.
//!
//! Two `tylax` quirks are corrected here so the result actually compiles with a
//! working bibliography:
//! * tylax emits a bibliography citation `@key` as `\ref{key}`, not `\cite{key}`.
//!   Because bibliography keys are distinct from cross-reference label names, we
//!   rewrite `\ref{<key>}` → `\cite{<key>}` for exactly the keys in `sources.bib`.
//! * tylax emits `\bibliographystyle{"ieee"}` with literal quotes and a Typst CSL
//!   style name; we strip the quotes and map the name to a LaTeX `.bst`.
//!
//! The CLI tex export drops Image nodes, so only figures referenced inline in
//! paragraph prose (`#image("…")` / `#figure(image("…"))`) reach the bundle;
//! their paths resolve relative to the project root, mirroring Typst compilation.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

/// What the bundle wrote — for the CLI summary line.
pub struct BundleReport {
    pub tex_name: String,
    pub bib_entries: usize,
    /// Basenames of figures copied into the bundle.
    pub figures: Vec<String>,
    /// Figure paths referenced in the source but not found on disk.
    pub missing_figures: Vec<String>,
    pub zipped: bool,
}

/// Harvest valid `BibEntry`s from the Sources book (mirrors the Book-assembly
/// harvest, reading paragraph body files from disk).
pub fn collect_bib_entries(
    layout: &ProjectLayout,
    h: &Hierarchy,
) -> Vec<crate::sources::BibEntry> {
    let Some(sources_book) = h.iter().find(|n| {
        n.kind == NodeKind::Book
            && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_SOURCES)
    }) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for id in h.collect_subtree(sources_book.id) {
        let Some(n) = h.get(id) else { continue };
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = &n.file else { continue };
        let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else {
            continue;
        };
        let body = strip_leading_heading(&raw);
        if let Some(e) = crate::sources::BibEntry::from_hjson(&body) {
            if e.is_valid() {
                entries.push(e);
            }
        }
    }
    entries
}

/// Strip a leading `= Title` editor heading before HJSON parsing.
fn strip_leading_heading(raw: &str) -> String {
    let t = raw.trim_start();
    if t.starts_with("= ") {
        t.splitn(2, '\n').nth(1).unwrap_or("").to_string()
    } else {
        raw.to_string()
    }
}

/// Map a Typst CSL style name to a LaTeX `.bst` that arXiv's TeXLive provides.
/// Unknown names fall back to `plain` (a universally available style — the
/// MANIFEST flags this so the author can pick a better one).
fn latex_bst(csl_style: &str) -> &'static str {
    match csl_style.trim().to_lowercase().as_str() {
        "ieee" => "IEEEtran",
        "apa" => "apalike",
        "chicago-author-date" | "chicago" => "plainnat",
        "mla" => "plain",
        "plain" | "" => "plain",
        _ => "plain",
    }
}

/// Rewrite tylax LaTeX for a compilable submission: `\ref{key}` → `\cite{key}`
/// for every bibliography key, and `\bibliographystyle{"csl"}` → a real `.bst`.
fn fix_citations_and_style(tex: &str, bib_keys: &[String], csl_style: &str) -> String {
    let mut out = tex.to_string();
    for key in bib_keys {
        out = out.replace(&format!("\\ref{{{key}}}"), &format!("\\cite{{{key}}}"));
    }
    // Replace the whole `\bibliographystyle{...}` argument with a mapped .bst.
    let marker = "\\bibliographystyle{";
    if let Some(start) = out.find(marker) {
        let after = start + marker.len();
        if let Some(rel) = out[after..].find('}') {
            let end = after + rel;
            let bst = latex_bst(csl_style);
            out = format!("{}{}{}", &out[..after], bst, &out[end..]);
        }
    }
    out
}

/// Extract every `\includegraphics[...]{path}` path from LaTeX, in order.
fn extract_graphics(tex: &str) -> Vec<String> {
    let needle = "\\includegraphics";
    let mut out = Vec::new();
    let mut idx = 0usize;
    while let Some(pos) = tex[idx..].find(needle) {
        let mut p = idx + pos + needle.len();
        let skip_ws = |b: &[u8], mut q: usize| {
            while q < b.len() && b[q].is_ascii_whitespace() {
                q += 1;
            }
            q
        };
        let bytes = tex.as_bytes();
        p = skip_ws(bytes, p);
        // Optional [options].
        if p < bytes.len() && bytes[p] == b'[' {
            if let Some(rel) = tex[p..].find(']') {
                p = p + rel + 1;
            }
        }
        p = skip_ws(bytes, p);
        // Required {path}.
        if p < bytes.len() && bytes[p] == b'{' {
            if let Some(rel) = tex[p + 1..].find('}') {
                let path = &tex[p + 1..p + 1 + rel];
                if !path.trim().is_empty() {
                    out.push(path.trim().to_string());
                }
                idx = p + 1 + rel + 1;
                continue;
            }
        }
        idx = idx + pos + needle.len();
    }
    out
}

/// A filesystem-safe, unique basename for a figure, avoiding collisions when two
/// source paths share a basename.
fn unique_basename(path: &str, used: &mut BTreeSet<String>) -> String {
    let base = Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "figure".to_string());
    if used.insert(base.clone()) {
        return base;
    }
    for i in 1.. {
        let cand = format!("{i}-{base}");
        if used.insert(cand.clone()) {
            return cand;
        }
    }
    unreachable!()
}

/// Lowercase alphanumeric slug for the `.tex` filename.
fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "paper".to_string() } else { out }
}

/// Build the arXiv/preprint bundle from an already-assembled Typst source (front
/// matter prepended). Resolves figures against `layout.root`. `out` is a
/// directory, or a `.zip` archive when its extension is `zip`.
pub fn build_bundle(
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    combined_typst: &str,
    title: &str,
    out: &Path,
) -> Result<BundleReport> {
    // 1. Sources → BibTeX.
    let entries = collect_bib_entries(layout, h);
    let bib_keys: Vec<String> = entries.iter().map(|e| e.key.clone()).collect();
    let (bib_text, bib_count) = crate::sources::compile_bibtex(&entries);

    // 2. Inject the bibliography into the Typst if there are entries and the
    //    author hasn't placed one by hand.
    let mut typ = combined_typst.to_string();
    if bib_count > 0 && !typ.contains("#bibliography(") {
        typ.push_str(&format!(
            "\n#bibliography(\"sources.bib\", style: \"{}\")\n",
            cfg.sources.bibliography_style
        ));
    }

    // 3. tylax → LaTeX, then correct the citation + style quirks.
    let mut tex = crate::export::tex::typst_to_tex(&typ, &cfg.tex_export);
    tex = fix_citations_and_style(&tex, &bib_keys, &cfg.sources.bibliography_style);

    // 4. Figures: resolve, copy, rewrite paths to flat basenames.
    let mut assigned: BTreeMap<String, String> = BTreeMap::new();
    let mut used_names: BTreeSet<String> = BTreeSet::new();
    let mut figure_files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut missing_figures: Vec<String> = Vec::new();
    for path in extract_graphics(&tex) {
        if assigned.contains_key(&path) {
            continue;
        }
        match std::fs::read(layout.root.join(&path)) {
            Ok(bytes) => {
                let dest = unique_basename(&path, &mut used_names);
                figure_files.push((dest.clone(), bytes));
                assigned.insert(path, dest);
            }
            Err(_) => {
                missing_figures.push(path.clone());
                assigned.insert(path.clone(), path); // left as-is in the .tex
            }
        }
    }
    // Rewrite each resolved figure path to its flat basename.
    for (src, dest) in &assigned {
        if src != dest {
            tex = tex.replace(&format!("{{{src}}}"), &format!("{{{dest}}}"));
        }
    }

    // 5. Assemble the file set.
    let tex_name = format!("{}.tex", slugify(title));
    let figures: Vec<String> = figure_files.iter().map(|(n, _)| n.clone()).collect();
    let manifest = build_manifest(
        title,
        &tex_name,
        &cfg.tex_export.document_class,
        &cfg.sources.bibliography_style,
        latex_bst(&cfg.sources.bibliography_style),
        bib_count,
        &figures,
        &missing_figures,
    );

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    files.push((tex_name.clone(), tex.into_bytes()));
    if bib_count > 0 {
        files.push(("sources.bib".to_string(), bib_text.into_bytes()));
    }
    files.extend(figure_files);
    files.push(("MANIFEST.txt".to_string(), manifest.into_bytes()));

    // 6. Write — a `.zip` archive or a directory.
    let zipped = out
        .extension()
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);
    if zipped {
        write_zip(out, &files)?;
    } else {
        write_dir(out, &files)?;
    }

    Ok(BundleReport {
        tex_name,
        bib_entries: bib_count,
        figures,
        missing_figures,
        zipped,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_manifest(
    title: &str,
    tex_name: &str,
    document_class: &str,
    csl_style: &str,
    bst: &str,
    bib_count: usize,
    figures: &[String],
    missing: &[String],
) -> String {
    let class = if document_class.trim().is_empty() {
        "article (tylax default)"
    } else {
        document_class.trim()
    };
    let mut s = String::new();
    s.push_str(&format!("arXiv / preprint bundle — {title}\n"));
    s.push_str("Generated by `inkhaven export tex --bundle`.\n\n");
    s.push_str("Contents:\n");
    s.push_str(&format!("  {tex_name}   — the LaTeX source\n"));
    if bib_count > 0 {
        s.push_str(&format!(
            "  sources.bib   — {bib_count} bibliography {}\n",
            if bib_count == 1 { "entry" } else { "entries" }
        ));
    }
    if figures.is_empty() {
        s.push_str("  (no figures referenced)\n");
    } else {
        s.push_str(&format!("  {} figure(s):\n", figures.len()));
        for f in figures {
            s.push_str(&format!("    {f}\n"));
        }
    }
    s.push_str("\nBefore uploading, verify:\n");
    s.push_str(&format!(
        "  · \\documentclass is right for your venue (currently: {class}); bundle any\n    journal .cls/.sty not in arXiv's TeXLive.\n"
    ));
    if bib_count > 0 {
        s.push_str(&format!(
            "  · \\bibliographystyle is `{bst}` (mapped from the Typst `{csl_style}` style);\n    arXiv runs BibTeX over sources.bib. Swap the .bst if your venue needs another.\n"
        ));
        s.push_str(
            "  · citations: `@key` was rewritten to \\cite{key}; cross-references stay \\ref{…}.\n",
        );
    }
    if missing.is_empty() {
        s.push_str("  · all referenced figures were found and copied.\n");
    } else {
        s.push_str(&format!(
            "  · {} figure(s) referenced but NOT found on disk — add them by hand:\n",
            missing.len()
        ));
        for m in missing {
            s.push_str(&format!("      {m}\n"));
        }
    }
    s
}

fn write_dir(out: &Path, files: &[(String, Vec<u8>)]) -> Result<()> {
    std::fs::create_dir_all(out).map_err(Error::Io)?;
    for (name, bytes) in files {
        std::fs::write(out.join(name), bytes).map_err(Error::Io)?;
    }
    Ok(())
}

fn write_zip(out: &Path, files: &[(String, Vec<u8>)]) -> Result<()> {
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    // Stream into a sibling `.part` and rename on success so an interrupted
    // bundle can't leave a truncated, unopenable archive in place of a good one.
    let tmp = out.with_extension("part");
    let file = std::fs::File::create(&tmp).map_err(Error::Io)?;
    let mut zw = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in files {
        zw.start_file(name, opts)
            .map_err(|e| Error::Store(format!("zip {name}: {e}")))?;
        zw.write_all(bytes).map_err(Error::Io)?;
    }
    zw.finish()
        .map_err(|e| Error::Store(format!("zip finish: {e}")))?;
    std::fs::rename(&tmp, out).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        Error::Io(e)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_bib_ref_to_cite_but_leaves_labels() {
        let tex = "See \\ref{smith2020} and figure \\ref{fig:flux}.\n";
        let out = fix_citations_and_style(tex, &["smith2020".to_string()], "ieee");
        assert!(out.contains("\\cite{smith2020}"), "{out}");
        assert!(out.contains("\\ref{fig:flux}"), "{out}"); // cross-ref untouched
    }

    #[test]
    fn fixes_bibliographystyle_quotes_and_maps_bst() {
        let tex = "\\bibliographystyle{\"ieee\"}\n\\bibliography{sources}\n";
        let out = fix_citations_and_style(tex, &[], "ieee");
        assert!(out.contains("\\bibliographystyle{IEEEtran}"), "{out}");
        assert!(!out.contains("\"ieee\""), "{out}");
    }

    #[test]
    fn unknown_style_falls_back_to_plain() {
        assert_eq!(latex_bst("some-obscure-style"), "plain");
        assert_eq!(latex_bst("apa"), "apalike");
    }

    #[test]
    fn extract_graphics_handles_options_and_multiple() {
        let tex = "\\includegraphics{a.png}\n\\includegraphics[width=0.5\\textwidth]{sub/b.pdf}\n";
        assert_eq!(extract_graphics(tex), vec!["a.png", "sub/b.pdf"]);
    }

    #[test]
    fn unique_basename_disambiguates_collisions() {
        let mut used = BTreeSet::new();
        assert_eq!(unique_basename("a/flux.png", &mut used), "flux.png");
        assert_eq!(unique_basename("b/flux.png", &mut used), "1-flux.png");
    }

    #[test]
    fn slugify_is_filesystem_safe() {
        assert_eq!(slugify("On Off-Target Effects!"), "on-off-target-effects");
        assert_eq!(slugify(""), "paper");
    }
}
