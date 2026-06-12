//! 1.3.0 PDF-1 — `inkhaven pdf` subcommand.
//!
//! The page-level operations + metadata + outline built in P0, over the
//! `crate::pdf` library.  Imposition / cover / barcode / preflight arrive
//! with their phases (P1/P2).  Output is always written through
//! `PdfDoc::save` (atomic `io_atomic`); a mutating op never overwrites
//! its input unless `--out` points there — by default it writes a
//! `<stem>-<op>.pdf` sibling.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::pdf::ops::{self, PageSpec, Rotation, SplitMode};
use crate::pdf::outline::OutlineItem;
use crate::pdf::{self, PdfDoc};

use super::PdfCommand;

pub fn run(cmd: PdfCommand) -> Result<()> {
    match cmd {
        PdfCommand::Info { input } => info(&input),
        PdfCommand::Extract { input, pages, out } => {
            let doc = load(&input)?;
            let spec = PageSpec::parse(&pages).map_err(pdferr)?;
            let mut result = ops::extract(&doc, &spec).map_err(pdferr)?;
            let path = out_or_default(&input, out, "extract");
            write_pdf(&mut result, &path)?;
            println!(
                "pdf extract: {} page(s) → {}",
                result.page_count(),
                path.display()
            );
            Ok(())
        }
        PdfCommand::Delete { input, pages, out } => {
            let mut doc = load(&input)?;
            let spec = PageSpec::parse(&pages).map_err(pdferr)?;
            ops::delete(&mut doc, &spec).map_err(pdferr)?;
            let path = out_or_default(&input, out, "deleted");
            write_pdf(&mut doc, &path)?;
            println!(
                "pdf delete: {} page(s) remain → {}",
                doc.page_count(),
                path.display()
            );
            Ok(())
        }
        PdfCommand::Rotate {
            input,
            pages,
            degrees,
            out,
        } => {
            let mut doc = load(&input)?;
            let spec = PageSpec::parse(&pages).map_err(pdferr)?;
            let rot = Rotation::from_degrees(degrees).map_err(pdferr)?;
            ops::rotate(&mut doc, &spec, rot).map_err(pdferr)?;
            let path = out_or_default(&input, out, "rotated");
            write_pdf(&mut doc, &path)?;
            println!("pdf rotate: {degrees}° → {}", path.display());
            Ok(())
        }
        PdfCommand::Reorder { input, mapping, out } => {
            let mut doc = load(&input)?;
            // 1-based page numbers from the user → 0-based for the op.
            let map: Vec<usize> = mapping
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| {
                    s.parse::<usize>()
                        .ok()
                        .filter(|&n| n >= 1)
                        .map(|n| n - 1)
                        .ok_or_else(|| {
                            Error::Store(
                                "pdf reorder: --mapping must be comma-separated 1-based page numbers"
                                    .into(),
                            )
                        })
                })
                .collect::<Result<_>>()?;
            ops::reorder(&mut doc, &map).map_err(pdferr)?;
            let path = out_or_default(&input, out, "reordered");
            write_pdf(&mut doc, &path)?;
            println!("pdf reorder: → {}", path.display());
            Ok(())
        }
        PdfCommand::Split {
            input,
            every,
            at,
            out_dir,
        } => split(&input, every, at, out_dir),
        PdfCommand::Merge { inputs, out } => {
            if inputs.len() < 2 {
                return Err(Error::Store("pdf merge: need at least two inputs".into()));
            }
            let docs: Vec<PdfDoc> = inputs.iter().map(|p| load(p)).collect::<Result<_>>()?;
            let mut merged = ops::merge(&docs).map_err(pdferr)?;
            write_pdf(&mut merged, &out)?;
            println!(
                "pdf merge: {} files → {} ({} page(s))",
                inputs.len(),
                out.display(),
                merged.page_count()
            );
            Ok(())
        }
        PdfCommand::Metadata {
            input,
            strip,
            title,
            author,
            subject,
            keywords,
            out,
        } => metadata(&input, strip, title, author, subject, keywords, out),
        PdfCommand::Outline { input } => outline_list(&input),
    }
}

fn info(input: &Path) -> Result<()> {
    let doc = load(input)?;
    println!("{}", input.display());
    println!("  pages: {}", doc.page_count());
    if let Some(sz) = doc.page_size(0) {
        println!(
            "  page 1: {:.1} × {:.1} pt  ({:.0} × {:.0} mm)",
            sz.width(),
            sz.height(),
            pdf::geometry::pt_to_mm(sz.width()),
            pdf::geometry::pt_to_mm(sz.height()),
        );
    }
    println!(
        "  source: {}",
        if doc.is_inkhaven() {
            "inkhaven"
        } else {
            "external"
        }
    );
    let m = pdf::meta::read_metadata(&doc);
    if let Some(t) = &m.title {
        println!("  title: {t}");
    }
    if let Some(a) = &m.author {
        println!("  author: {a}");
    }
    println!(
        "  outline: {} top-level bookmark(s)",
        pdf::outline::read_outline(&doc).len()
    );
    Ok(())
}

fn split(
    input: &Path,
    every: Option<usize>,
    at: Option<String>,
    out_dir: Option<PathBuf>,
) -> Result<()> {
    let doc = load(input)?;
    let mode = match (every, at) {
        (Some(n), _) => SplitMode::EveryNPages(n),
        (None, Some(at)) => {
            let cuts: Vec<usize> = at
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            SplitMode::OnPages(cuts)
        }
        (None, None) => {
            return Err(Error::Store(
                "pdf split: pass --every <n> or --at <pages>".into(),
            ));
        }
    };
    let parts = ops::split(&doc, &mode).map_err(pdferr)?;
    let dir = out_dir.unwrap_or_else(|| {
        input
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    });
    let stem = file_stem(input);
    for (i, mut part) in parts.into_iter().enumerate() {
        let path = dir.join(format!("{stem}-part-{:02}.pdf", i + 1));
        write_pdf(&mut part, &path)?;
        println!("  → {} ({} page(s))", path.display(), part.page_count());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn metadata(
    input: &Path,
    strip: bool,
    title: Option<String>,
    author: Option<String>,
    subject: Option<String>,
    keywords: Option<String>,
    out: Option<PathBuf>,
) -> Result<()> {
    let mut doc = load(input)?;
    let any_set =
        title.is_some() || author.is_some() || subject.is_some() || keywords.is_some();
    if strip {
        pdf::meta::strip_metadata(&mut doc).map_err(pdferr)?;
        let path = out_or_default(input, out, "stripped");
        write_pdf(&mut doc, &path)?;
        println!("pdf metadata: stripped → {}", path.display());
    } else if any_set {
        let mut m = pdf::meta::read_metadata(&doc);
        if let Some(t) = title {
            m.title = Some(t);
        }
        if let Some(a) = author {
            m.author = Some(a);
        }
        if let Some(s) = subject {
            m.subject = Some(s);
        }
        if let Some(k) = keywords {
            m.keywords = k
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect();
        }
        pdf::meta::write_metadata(&mut doc, &m).map_err(pdferr)?;
        let path = out_or_default(input, out, "meta");
        write_pdf(&mut doc, &path)?;
        println!("pdf metadata: updated → {}", path.display());
    } else {
        let m = pdf::meta::read_metadata(&doc);
        print_field("title", m.title.as_deref());
        print_field("author", m.author.as_deref());
        print_field("subject", m.subject.as_deref());
        if !m.keywords.is_empty() {
            println!("keywords: {}", m.keywords.join(", "));
        }
        print_field("creator", m.creator.as_deref());
        print_field("producer", m.producer.as_deref());
    }
    Ok(())
}

fn outline_list(input: &Path) -> Result<()> {
    let doc = load(input)?;
    let items = pdf::outline::read_outline(&doc);
    if items.is_empty() {
        println!("pdf outline: (no bookmarks)");
        return Ok(());
    }
    print_outline(&items, 0);
    Ok(())
}

fn print_outline(items: &[OutlineItem], depth: usize) {
    for it in items {
        println!("{}{}  → p.{}", "  ".repeat(depth), it.title, it.page + 1);
        print_outline(&it.children, depth + 1);
    }
}

fn print_field(name: &str, val: Option<&str>) {
    if let Some(v) = val {
        println!("{name}: {v}");
    }
}

fn load(p: &Path) -> Result<PdfDoc> {
    PdfDoc::load(p).map_err(pdferr)
}

/// Save atomically, creating the parent directory first (so `--out` /
/// `--out-dir` may point at a not-yet-existing folder).
fn write_pdf(doc: &mut PdfDoc, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Store(format!("pdf: create {}: {e}", parent.display())))?;
        }
    }
    doc.save(path).map_err(pdferr)
}

fn pdferr(e: pdf::Error) -> Error {
    Error::Store(e.to_string())
}

fn file_stem(p: &Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".into())
}

/// `out` if given, else a `<stem>-<suffix>.pdf` sibling of `input` — never
/// silently overwrites the source.
fn out_or_default(input: &Path, out: Option<PathBuf>, suffix: &str) -> PathBuf {
    out.unwrap_or_else(|| {
        let dir = input.parent().unwrap_or_else(|| Path::new("."));
        dir.join(format!("{}-{suffix}.pdf", file_stem(input)))
    })
}
