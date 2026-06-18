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

pub fn run(cmd: PdfCommand, project: &Path) -> Result<()> {
    match cmd {
        PdfCommand::Info { input } => info(&input),
        PdfCommand::Impose {
            input,
            config,
            out,
            dry_run,
        } => impose(&input, &config, project, out, dry_run),
        PdfCommand::Booklet {
            input,
            out,
            sheet,
            creep,
            no_marks,
            dry_run,
        } => booklet(&input, out, sheet.as_deref(), creep, no_marks, dry_run),
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
        PdfCommand::Preflight {
            input,
            profile,
            dpi,
        } => preflight_cmd(&input, &profile, dpi, project),
        PdfCommand::Barcode {
            isbn,
            out,
            height_mm,
            module_mm,
            no_text,
        } => barcode_cmd(&isbn, &out, height_mm, module_mm, no_text),
        PdfCommand::Cover {
            out,
            pages,
            title,
            author,
            back,
            image,
            fit,
            isbn,
            spine_mm,
            width_mm,
            height_mm,
        } => cover_cmd(CoverArgs {
            out,
            pages,
            title,
            author,
            back,
            image,
            fit,
            isbn,
            spine_mm,
            width_mm,
            height_mm,
            project,
        }),
        PdfCommand::Grayscale { input, out } => {
            let mut doc = load(&input)?;
            let imgs = pdf::transform::to_grayscale(&mut doc).map_err(pdferr)?;
            let path = out_or_default(&input, out, "gray");
            write_pdf(&mut doc, &path)?;
            println!("pdf grayscale: {imgs} image(s) converted → {}", path.display());
            Ok(())
        }
        PdfCommand::Optimize { input, out } => {
            let mut doc = load(&input)?;
            let r = pdf::transform::optimize(&mut doc).map_err(pdferr)?;
            let path = out_or_default(&input, out, "opt");
            write_pdf(&mut doc, &path)?;
            println!(
                "pdf optimize: {} → {} objects ({} pruned) → {}",
                r.objects_before,
                r.objects_after,
                r.pruned,
                path.display()
            );
            Ok(())
        }
        PdfCommand::Watermark {
            input,
            text,
            image,
            opacity,
            rotation,
            size,
            position,
            pages,
            out,
        } => watermark_cmd(WatermarkArgs {
            input,
            text,
            image,
            opacity,
            rotation,
            size,
            position,
            pages,
            out,
        }),
        PdfCommand::Sample { input, count, out } => {
            let src = load(&input)?;
            let mut sampled = ops::sample(&src, count).map_err(pdferr)?;
            let path = out_or_default(&input, out, "sample");
            write_pdf(&mut sampled, &path)?;
            println!(
                "pdf sample: {} of {} page(s) → {}",
                sampled.page_count(),
                src.page_count(),
                path.display()
            );
            Ok(())
        }
    }
}

struct WatermarkArgs {
    input: PathBuf,
    text: Option<String>,
    image: Option<PathBuf>,
    opacity: Option<f32>,
    rotation: Option<f32>,
    size: Option<f32>,
    position: String,
    pages: Option<String>,
    out: Option<PathBuf>,
}

fn watermark_cmd(a: WatermarkArgs) -> Result<()> {
    use crate::pdf::watermark::{apply_watermark, WatermarkSpec, WmPosition};
    if a.text.is_none() && a.image.is_none() {
        return Err(Error::Store(
            "pdf watermark: pass --text and/or --image".into(),
        ));
    }
    let position = WmPosition::parse(&a.position)
        .ok_or_else(|| Error::Store(format!("pdf watermark: bad --position `{}`", a.position)))?;
    let pages = match &a.pages {
        Some(p) => Some(PageSpec::parse(p).map_err(pdferr)?),
        None => None,
    };
    let mut spec = WatermarkSpec {
        text: a.text,
        image: a.image,
        position,
        pages,
        ..Default::default()
    };
    if let Some(o) = a.opacity {
        spec.opacity = o;
    }
    if let Some(r) = a.rotation {
        spec.rotation_deg = r;
    }
    if let Some(s) = a.size {
        spec.font_size_pt = s;
    }
    let mut doc = load(&a.input)?;
    let n = apply_watermark(&mut doc, &spec).map_err(pdferr)?;
    let path = out_or_default(&a.input, a.out, "watermark");
    write_pdf(&mut doc, &path)?;
    println!("pdf watermark: {n} page(s) stamped → {}", path.display());
    Ok(())
}

/// Load the merged project + global config (for `cover:` / `preflight:`
/// blocks); falls back to built-in defaults if no config loads.
fn project_config(project: &Path) -> crate::config::Config {
    let cfg_path = crate::project::ProjectLayout::new(project).config_path();
    crate::config::Config::load_layered(&cfg_path).unwrap_or_default()
}

fn preflight_cmd(input: &Path, profile: &str, dpi: Option<u32>, project: &Path) -> Result<()> {
    let prof = project_config(project)
        .preflight
        .resolve(profile, dpi)
        .map_err(Error::Store)?;
    let doc = load(input)?;
    let r = pdf::preflight::preflight(&doc, prof);
    println!("{}", input.display());
    println!("  pages: {}", r.page_count);
    println!(
        "  page size: {}",
        if r.consistent_page_size {
            "consistent"
        } else {
            "INCONSISTENT"
        }
    );
    println!("  target: {} dpi", prof.target_dpi());
    if r.fonts.is_empty() {
        println!("  fonts: (none)");
    } else {
        for f in &r.fonts {
            println!(
                "  font: {} [{}]",
                f.name,
                if f.embedded { "embedded" } else { "NOT EMBEDDED" }
            );
        }
    }
    for img in &r.images {
        println!(
            "  image: p.{} {} {}×{}px @ {} dpi ({})",
            img.page, img.name, img.pixel_w, img.pixel_h, img.effective_dpi, img.colorspace
        );
    }
    if !r.color_pages.is_empty() {
        println!("  colour pages: {:?}", r.color_pages);
    }
    if !r.overprint_pages.is_empty() {
        println!("  overprint pages: {:?}", r.overprint_pages);
    }
    if !r.transparency_pages.is_empty() {
        println!("  transparency pages: {:?}", r.transparency_pages);
    }
    if !r.spot_colors.is_empty() {
        println!("  spot colours: {}", r.spot_colors.join(", "));
    }
    if !r.blank_pages.is_empty() {
        println!("  blank pages: {:?}", r.blank_pages);
    }
    if r.warnings.is_empty() {
        println!("  ✓ no warnings");
    } else {
        println!("  {} warning(s):", r.warnings.len());
        for w in &r.warnings {
            println!("    ⚠ {w}");
        }
    }
    Ok(())
}

fn barcode_cmd(
    isbn: &str,
    out: &Path,
    height_mm: Option<f32>,
    module_mm: Option<f32>,
    no_text: bool,
) -> Result<()> {
    use crate::pdf::barcode::BarcodeSpec;
    let mut spec = BarcodeSpec {
        isbn: isbn.to_string(),
        include_human_readable: !no_text,
        ..Default::default()
    };
    if let Some(h) = height_mm {
        spec.height_mm = h;
    }
    if let Some(m) = module_mm {
        spec.module_width_mm = m;
    }
    let mut doc = pdf::barcode::build_barcode_pdf(&spec).map_err(pdferr)?;
    write_pdf(&mut doc, out)?;
    println!("pdf barcode: {isbn} → {}", out.display());
    Ok(())
}

struct CoverArgs<'a> {
    out: PathBuf,
    pages: usize,
    title: Option<String>,
    author: Option<String>,
    back: Option<String>,
    image: Option<PathBuf>,
    fit: Option<String>,
    isbn: Option<String>,
    spine_mm: Option<f32>,
    width_mm: Option<f32>,
    height_mm: Option<f32>,
    project: &'a Path,
}

fn cover_cmd(a: CoverArgs) -> Result<()> {
    use crate::pdf::cover::{build_cover, CoverRequest};
    let mut cfg = project_config(a.project).cover;
    if let Some(w) = a.width_mm {
        cfg.front_width_mm = w;
    }
    if let Some(h) = a.height_mm {
        cfg.front_height_mm = h;
    }
    if let Some(fit) = a.fit {
        cfg.image_fit = fit;
    }
    let req = CoverRequest {
        page_count: a.pages,
        title: a.title,
        author: a.author,
        back_text: a.back,
        front_image: a.image,
        isbn: a.isbn,
        spine_mm_override: a.spine_mm,
    };
    let spec = cfg.build_spec(&req);
    let spine = spec.spine_width_mm;
    let mut doc = build_cover(&spec).map_err(pdferr)?;
    write_pdf(&mut doc, &a.out)?;
    println!(
        "pdf cover: {} pages → spine {spine:.1} mm → {}",
        a.pages,
        a.out.display()
    );
    Ok(())
}

/// Imposition profiles come through the full config cascade (project +
/// global); falls back to the built-in defaults if no config loads.
fn imposition_config(project: &Path) -> crate::pdf::impose::config::ImpositionConfig {
    let cfg_path = crate::project::ProjectLayout::new(project).config_path();
    crate::config::Config::load_layered(&cfg_path)
        .map(|c| c.imposition)
        .unwrap_or_default()
}

fn impose(
    input: &Path,
    profile: &str,
    project: &Path,
    out: Option<PathBuf>,
    dry_run: bool,
) -> Result<()> {
    let params = imposition_config(project)
        .resolve(profile)
        .map_err(Error::Store)?;
    let src = load(input)?;
    if dry_run {
        // Preview the plan (RFC App. D) — the same generator the TUI
        // overlay renders — without imposing.
        let preview = pdf::impose::preview::build(profile, src.page_count(), &params);
        for line in preview.lines() {
            println!("{line}");
        }
        return Ok(());
    }
    // Layout summary for the report (cheap to recompute).
    let layout = pdf::impose::layout::plan(
        params.style,
        params.sheets_per_signature,
        src.page_count(),
        params.blank,
    );
    let mut out_doc = pdf::impose::impose(&src, &params).map_err(pdferr)?;
    let path = out_or_default(input, out, "imposed");
    write_pdf(&mut out_doc, &path)?;
    println!(
        "pdf impose: `{profile}` · {} signature(s), {} sheet(s) → {} ({} imposed page(s))",
        layout.signatures,
        layout.sheets.len(),
        path.display(),
        out_doc.page_count(),
    );
    Ok(())
}

/// Quick saddle-stitch booklet (P4) — no profile, no config file.
/// Builds [`ImpositionParams`] directly: the press sheet auto-fits to
/// two source pages side-by-side (so any trim size works, not just the
/// preset-bound `chapbook` profile), all pages nest into one folded
/// signature, blanks balance around the centre fold. `--sheet` centres
/// the spread on a named preset instead; `--creep` adds shingle
/// compensation; `--no-marks` drops crop/fold marks.
fn booklet(
    input: &Path,
    out: Option<PathBuf>,
    sheet: Option<&str>,
    creep: bool,
    no_marks: bool,
    dry_run: bool,
) -> Result<()> {
    use pdf::impose::{BindingStyle, BlankPolicy, CreepStrategy, ImpositionParams};

    let src = load(input)?;
    let page = src
        .page_size(0)
        .ok_or_else(|| Error::Store("pdf booklet: source has no pages".into()))?;

    // Press sheet: a named preset (forced landscape — two columns across)
    // or auto-fit to exactly two source pages side-by-side.
    let sheet_size = match sheet {
        Some(name) => pdf::geometry::page_size(name)
            .ok_or_else(|| Error::Store(format!("pdf booklet: unknown sheet preset `{name}`")))?
            .landscape(),
        None => pdf::geometry::Size {
            width: page.width() * 2.0,
            height: page.height(),
        },
    };

    let thickness = pdf::paper::paper_stock(pdf::paper::DEFAULT_INTERIOR)
        .map(|s| s.thickness_mm)
        .unwrap_or(0.1);

    let params = ImpositionParams {
        style: BindingStyle::SaddleStitch,
        sheets_per_signature: 1, // saddle = one nested signature; ignored anyway
        blank: BlankPolicy::Balance,
        sheet_size,
        creep: if creep { CreepStrategy::Shingle } else { CreepStrategy::None },
        paper_thickness_mm: thickness,
        marks: pdf::impose::marks::MarkConfig {
            crop: !no_marks,
            fold: !no_marks,
            registration: false,
            spine_marker: false,
            signature_number: false,
            color_bar: false,
        },
        crop_offset_mm: 5.0,
        fold_mark_length_mm: 8.0,
    };

    if dry_run {
        let preview = pdf::impose::preview::build("booklet", src.page_count(), &params);
        for line in preview.lines() {
            println!("{line}");
        }
        return Ok(());
    }

    let layout = pdf::impose::layout::plan(
        params.style,
        params.sheets_per_signature,
        src.page_count(),
        params.blank,
    );
    let mut out_doc = pdf::impose::impose(&src, &params).map_err(pdferr)?;
    let path = out_or_default(input, out, "booklet");
    write_pdf(&mut out_doc, &path)?;
    let (sw, sh) = (
        pdf::geometry::pt_to_mm(sheet_size.width),
        pdf::geometry::pt_to_mm(sheet_size.height),
    );
    println!(
        "pdf booklet: saddle-stitch · sheet {sw:.0}×{sh:.0} mm · {} sheet(s) → {} ({} imposed page(s))",
        layout.sheets.len(),
        path.display(),
        out_doc.page_count(),
    );
    Ok(())
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
