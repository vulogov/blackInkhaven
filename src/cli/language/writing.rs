//! `inkhaven language` writing-system surface: font compilation from glyph
//! SVGs or a language's `font` config block, glyph drafting/linting, spatial
//! layout, and transliteration. Split out of the flat handler.

use std::path::Path;

use crate::error::{Error, Result};
use crate::store::hierarchy::Hierarchy;
use crate::store::{NodeKind, Store};

use super::*;

/// LANG-1 P5.2/P5.3/P5.4 — compile a font, either from a loose directory of
/// glyph SVGs (`--glyphs`) or from a language's own `font` config block
/// (`--language`).
pub(crate) fn font_build(
    project: &Path,
    family: Option<&str>,
    language: Option<&str>,
    glyphs_dir: Option<&Path>,
    out: Option<&Path>,
    upm: Option<f64>,
    format: &str,
) -> Result<()> {
    let (want_ufo, want_ttf) = match format.to_ascii_lowercase().as_str() {
        "ufo" => (true, false),
        "ttf" => (false, true),
        "both" => (true, true),
        other => {
            return Err(Error::Config(format!(
                "unknown --format `{other}` (expected ufo, ttf, or both)"
            )))
        }
    };

    let (resolved_family, resolved_upm, sources, skipped) = match (language, glyphs_dir) {
        (Some(lang), _) => collect_glyphs_from_config(project, lang, family, upm)?,
        (None, Some(dir)) => {
            let f = family
                .ok_or_else(|| Error::Config("a family name is required with --glyphs".into()))?;
            let (sources, skipped) = collect_glyphs_from_dir(dir)?;
            (f.to_string(), upm.unwrap_or(DEFAULT_UPM), sources, skipped)
        }
        (None, None) => {
            return Err(Error::Config(
                "specify either --language <lang> (config-driven) or a family + --glyphs <dir>"
                    .into(),
            ))
        }
    };

    emit_font(&resolved_family, resolved_upm, &sources, skipped, out, want_ufo, want_ttf)
}

/// Build glyph sources from a directory of `.svg` files (filename stem → glyph
/// name; a single-character stem also sets the Unicode codepoint).
pub(crate) fn collect_glyphs_from_dir(glyphs_dir: &Path) -> Result<(Vec<GlyphSource>, usize)> {
    use crate::conlang::writing::preflight;

    let mut svgs: Vec<std::path::PathBuf> = std::fs::read_dir(glyphs_dir)
        .map_err(|e| Error::Config(format!("reading {}: {e}", glyphs_dir.display())))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("svg")))
        .collect();
    svgs.sort();
    if svgs.is_empty() {
        return Err(Error::Config(format!("no .svg files in {}", glyphs_dir.display())));
    }

    let mut sources = Vec::new();
    let mut skipped = 0usize;
    for path in &svgs {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if stem.is_empty() {
            continue;
        }
        let svg = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  skip {}: {e}", path.display());
                skipped += 1;
                continue;
            }
        };
        let report = preflight::lint_svg(&svg);
        if !report.is_usable() {
            eprintln!("  skip {} — {}", stem, report.errors.join("; "));
            skipped += 1;
            continue;
        }
        let codepoint = (stem.chars().count() == 1).then(|| stem.chars().next().unwrap());
        let name = codepoint
            .map(|c| format!("uni{:04X}", c as u32))
            .unwrap_or_else(|| stem.clone());
        sources.push(GlyphSource { name, codepoint, svg });
    }
    Ok((sources, skipped))
}

/// Build glyph sources from a language's `font` config block + glyph store.
/// Returns the resolved family (`--family` > config > language name) and upm
/// (`--upm` > config).
pub(crate) fn collect_glyphs_from_config(
    project: &Path,
    language: &str,
    family_override: Option<&str>,
    upm_override: Option<f64>,
) -> Result<(String, f64, Vec<GlyphSource>, usize)> {
    use crate::conlang::writing::preflight;

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let cfg = load_font_config(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no `font` block — add glyphs with \
             `inkhaven language font-import-glyph {language} --svg …`"
        ))
    })?;
    if cfg.glyphs.is_empty() {
        return Err(Error::Config(format!(
            "language `{language}` declares no glyphs in its `font` block"
        )));
    }

    let family = family_override
        .map(str::to_string)
        .or_else(|| cfg.family.clone())
        .unwrap_or_else(|| lang_book.title.clone());
    let upm = upm_override.unwrap_or(cfg.upm);
    let dir = glyph_store_dir(store.project_root(), language);

    let mut sources = Vec::new();
    let mut skipped = 0usize;
    for g in &cfg.glyphs {
        let path = dir.join(format!("{}.svg", g.name));
        let svg = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("  skip {} — no artwork at {}", g.name, path.display());
                skipped += 1;
                continue;
            }
        };
        let report = preflight::lint_svg(&svg);
        if !report.is_usable() {
            eprintln!("  skip {} — {}", g.name, report.errors.join("; "));
            skipped += 1;
            continue;
        }
        sources.push(GlyphSource { name: g.name.clone(), codepoint: g.codepoint, svg });
    }
    Ok((family, upm, sources, skipped))
}

/// Shared tail: build the UFO and emit UFO / TTF artifacts per the format.
pub(crate) fn emit_font(
    family: &str,
    upm: f64,
    sources: &[GlyphSource],
    skipped: usize,
    out: Option<&Path>,
    want_ufo: bool,
    want_ttf: bool,
) -> Result<()> {
    use crate::conlang::writing::compile;

    if sources.is_empty() {
        return Err(Error::Config("no usable glyphs to compile".into()));
    }
    let font = crate::conlang::writing::font::build_ufo(family, upm, sources).map_err(Error::Config)?;

    // `--out` sets the stem; the extension follows the format. When both are
    // requested, the UFO and TTF share that stem.
    let stem = out
        .map(|p| p.with_extension(""))
        .unwrap_or_else(|| std::path::PathBuf::from(family));

    let skipped_note = if skipped > 0 { format!(", {skipped} skipped") } else { String::new() };
    println!("font `{family}` · {} glyph(s){skipped_note} @ {upm:.0} upm", sources.len());

    if want_ufo {
        let ufo_path = stem.with_extension("ufo");
        font.save(&ufo_path)
            .map_err(|e| Error::Store(format!("saving UFO: {e}")))?;
        println!("  UFO source → {}", ufo_path.display());
        if !want_ttf {
            eprintln!("  (compile to TTF/OTF with `--format ttf`, fontc / fontmake, or FontForge)");
        }
    }
    if want_ttf {
        let ttf = compile::compile_ttf(&font, upm).map_err(Error::Config)?;
        let ttf_path = stem.with_extension("ttf");
        crate::io_atomic::write(&ttf_path, &ttf).map_err(Error::Io)?;
        println!("  TrueType font → {} ({} bytes)", ttf_path.display(), ttf.len());
    }
    Ok(())
}

/// A filesystem-safe slug for a language name.
pub(crate) fn lang_slug(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() { "language".to_string() } else { s }
}

/// `<project>/.inkhaven/glyphs/<lang-slug>/` — the glyph artwork store.
pub(crate) fn glyph_store_dir(project_root: &Path, language: &str) -> std::path::PathBuf {
    project_root
        .join(".inkhaven")
        .join("glyphs")
        .join(lang_slug(language))
}

/// Load a language's `font` config block from its Phonology chapter.
pub(crate) fn load_font_config(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<Option<crate::conlang::types::font::FontConfig>> {
    use crate::conlang::types::font::FontConfig;
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Phonology"))
        .cloned()
    else {
        return Ok(None);
    };
    for para in hierarchy.children_of(Some(chapter.id)) {
        if para.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(para.id) else { continue };
        if let Ok(Some(c)) = FontConfig::from_hjson(&String::from_utf8_lossy(&bytes)) {
            return Ok(Some(c));
        }
    }
    Ok(None)
}

/// Find the Phonology paragraph that holds the `font` block (for in-place
/// replacement).
pub(crate) fn find_font_paragraph(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Option<crate::store::node::Node> {
    use crate::conlang::types::font::FontConfig;
    let chapter = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Phonology"))?;
    for para in hierarchy.children_of(Some(chapter.id)) {
        if para.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(para.id) else { continue };
        if matches!(FontConfig::from_hjson(&String::from_utf8_lossy(&bytes)), Ok(Some(_))) {
            return Some(para.clone());
        }
    }
    None
}

/// Serialize a `FontConfig` into the `{ font: { … } }` HJSON paragraph and
/// upsert it into the Phonology chapter.
pub(crate) fn write_font_config(
    store: &Store,
    cfg: &Config,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
    font: &crate::conlang::types::font::FontConfig,
) -> Result<()> {
    use serde_json::json;
    let glyphs: Vec<serde_json::Value> = font
        .glyphs
        .iter()
        .map(|g| {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), json!(g.name));
            if let Some(c) = g.codepoint {
                // Printable ASCII stays a literal (`"a"`); everything else —
                // PUA, combining marks, non-Latin — is written as readable hex
                // so the book never carries an invisible/fragile character.
                let cp = if c.is_ascii_graphic() {
                    c.to_string()
                } else {
                    format!("U+{:04X}", c as u32)
                };
                m.insert("codepoint".into(), json!(cp));
            }
            if let Some(p) = &g.phoneme {
                m.insert("phoneme".into(), json!(p));
            }
            serde_json::Value::Object(m)
        })
        .collect();
    let mut font_obj = serde_json::Map::new();
    if let Some(f) = &font.family {
        font_obj.insert("family".into(), json!(f));
    }
    font_obj.insert("upm".into(), json!(font.upm));
    font_obj.insert("glyphs".into(), json!(glyphs));
    let body = serde_json::to_string_pretty(&json!({ "font": font_obj }))
        .map_err(|e| Error::Store(format!("serializing font config: {e}")))?;

    let existing = find_font_paragraph(store, hierarchy, lang_book);
    upsert_chapter_paragraph(store, cfg, lang_book, "Phonology", "Writing system", existing, &body)
}

/// LANG-1 P5.4 — import a glyph SVG, binding it to a phoneme/codepoint and
/// recording it in the language's `font` config block.
pub(crate) fn font_import_glyph(
    project: &Path,
    language: &str,
    svg: &Path,
    phoneme: Option<&str>,
    codepoint: Option<&str>,
    name: Option<&str>,
) -> Result<()> {
    let svg_text = std::fs::read_to_string(svg)
        .map_err(|e| Error::Config(format!("reading {}: {e}", svg.display())))?;
    let stem = svg.file_stem().and_then(|s| s.to_str());
    bind_glyph_text(project, language, &svg_text, phoneme, codepoint, name, stem, &svg.display().to_string())
}

/// Preflight an SVG, copy it into the glyph store, and bind it in the language's
/// `font` block. Shared by `font-import-glyph` (artwork from a file) and
/// `glyph-draft --yes` (artwork from the AI). `fallback_name` is a last-resort
/// glyph-name source (e.g. the SVG filename stem); `label` is used in errors.
pub(crate) fn bind_glyph_text(
    project: &Path,
    language: &str,
    svg_text: &str,
    phoneme: Option<&str>,
    codepoint: Option<&str>,
    name: Option<&str>,
    fallback_name: Option<&str>,
    label: &str,
) -> Result<()> {
    use crate::conlang::types::font::{self, FontGlyph};
    use crate::conlang::writing::preflight;

    let report = preflight::lint_svg(svg_text);
    if !report.is_usable() {
        return Err(Error::Config(format!(
            "{label} is not suitable for a font glyph — {} (run `language glyph-lint` to inspect)",
            report.errors.join("; ")
        )));
    }
    for w in &report.warnings {
        eprintln!("note: {w}");
    }

    // Resolve the codepoint: explicit > a single-character glyph name.
    let cp = match codepoint {
        Some(c) => Some(font::parse_codepoint(c).map_err(Error::Config)?),
        None => None,
    };
    // Resolve the glyph name: explicit > uniXXXX (from the codepoint) > phoneme
    // > the fallback (e.g. SVG filename stem).
    let glyph_name = match name {
        Some(n) => n.to_string(),
        None => match cp {
            Some(c) => format!("uni{:04X}", c as u32),
            None => phoneme
                .map(str::to_string)
                .or_else(|| fallback_name.map(str::to_string))
                .ok_or_else(|| {
                    Error::Config("could not derive a glyph name — pass --name".into())
                })?,
        },
    };
    // A single-character name implies its own codepoint when none was given.
    let cp = cp.or_else(|| {
        (glyph_name.chars().count() == 1).then(|| glyph_name.chars().next().unwrap())
    });

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let layered = Config::load_layered(&ProjectLayout::new(project).config_path())?;

    // Copy the artwork into the glyph store.
    let dir = glyph_store_dir(store.project_root(), language);
    std::fs::create_dir_all(&dir)
        .map_err(|e| Error::Store(format!("creating {}: {e}", dir.display())))?;
    let dest = dir.join(format!("{glyph_name}.svg"));
    crate::io_atomic::write(&dest, svg_text.as_bytes()).map_err(Error::Io)?;

    // Record the binding.
    let mut font = load_font_config(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    if font.family.is_none() {
        font.family = Some(lang_book.title.clone());
    }
    font.upsert(FontGlyph {
        name: glyph_name.clone(),
        codepoint: cp,
        phoneme: phoneme.map(str::to_string),
    });
    let total = font.glyphs.len();
    write_font_config(&store, &layered, &hierarchy, &lang_book, &font)?;

    let cp_note = cp.map(|c| format!(" U+{:04X}", c as u32)).unwrap_or_default();
    let ph_note = phoneme.map(|p| format!(" /{p}/")).unwrap_or_default();
    println!("glyph `{glyph_name}`{cp_note}{ph_note} → {}", dest.display());
    println!("{language} font now has {total} glyph(s)");
    Ok(())
}

/// LANG-1 P5.4 — show a language's `font` config (bindings + artwork status).
pub(crate) fn font_config_show(project: &Path, language: &str, json: bool) -> Result<()> {
    use crate::conlang::writing::preflight;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let Some(font) = load_font_config(&store, &hierarchy, &lang_book)? else {
        return Err(Error::Config(format!(
            "language `{language}` has no `font` block yet"
        )));
    };

    if json {
        let glyphs: Vec<_> = font
            .glyphs
            .iter()
            .map(|g| {
                serde_json::json!({
                    "name": g.name,
                    "codepoint": g.codepoint.map(|c| format!("U+{:04X}", c as u32)),
                    "phoneme": g.phoneme,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "family": font.family,
                "upm": font.upm,
                "glyphs": glyphs,
            }))
            .map_err(|e| Error::Store(format!("serializing: {e}")))?
        );
        return Ok(());
    }

    let dir = glyph_store_dir(store.project_root(), language);
    println!(
        "font · {} · {} upm · {} glyph(s)",
        font.family.as_deref().unwrap_or(&lang_book.title),
        font.upm,
        font.glyphs.len()
    );
    for g in &font.glyphs {
        let cp = g.codepoint.map(|c| format!("U+{:04X}", c as u32)).unwrap_or_else(|| "—".into());
        let ph = g.phoneme.as_deref().map(|p| format!("/{p}/")).unwrap_or_default();
        let status = match std::fs::read_to_string(dir.join(format!("{}.svg", g.name))) {
            Ok(svg) if preflight::lint_svg(&svg).is_usable() => "✓",
            Ok(_) => "⚠ unusable",
            Err(_) => "✗ missing",
        };
        println!("  {:<14} {:<8} {:<6} {status}", g.name, cp, ph);
    }
    Ok(())
}

/// Resolve a template by name: a config `templates` entry wins over a built-in
/// of the same name.
pub(crate) fn resolve_template(
    font: &crate::conlang::types::font::FontConfig,
    name: &str,
) -> Result<crate::conlang::types::spatial::SpatialTemplate> {
    use crate::conlang::types::spatial::{builtin_template, BUILTIN_TEMPLATES};
    font.templates
        .iter()
        .find(|t| t.name == name)
        .cloned()
        .or_else(|| builtin_template(name))
        .ok_or_else(|| {
            Error::Config(format!(
                "unknown template `{name}` (built-ins: {})",
                BUILTIN_TEMPLATES.join(", ")
            ))
        })
}

/// LANG-1 P5.6 — list the spatial templates available to a language (built-in
/// plus any defined in its `font` block).
pub(crate) fn font_templates(project: &Path, language: &str) -> Result<()> {
    use crate::conlang::types::spatial::{builtin_template, BUILTIN_TEMPLATES};
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let font = load_font_config(&store, &hierarchy, &lang_book)?.unwrap_or_default();

    println!("spatial templates · {language}");
    let mut shown = std::collections::BTreeSet::new();
    for t in &font.templates {
        shown.insert(t.name.clone());
        println!("  {:<10} (config)   slots: {}", t.name, t.slots().join(", "));
    }
    for name in BUILTIN_TEMPLATES {
        if shown.contains(*name) {
            continue;
        }
        let t = builtin_template(name).unwrap();
        println!("  {:<10} (built-in) slots: {}", t.name, t.slots().join(", "));
    }
    Ok(())
}

/// LANG-1 P5.6 — compose component glyphs into a precomposed block per a
/// spatial template (Hangul-style syllable square, quadrat). Advisory: previews
/// the composite + preflight; `--yes` binds it like `font-import-glyph`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn font_compose(
    project: &Path,
    language: &str,
    template_name: &str,
    name: &str,
    codepoint: Option<&str>,
    phoneme: Option<&str>,
    slots: &[String],
    out: Option<&Path>,
    yes: bool,
) -> Result<()> {
    use crate::conlang::writing::{compose, preflight};
    use std::collections::BTreeMap;

    // Phase 1 — gather everything that needs the store, then drop it before
    // `bind_glyph_text` re-opens (DuckDB is single-writer).
    let (composed, report) = {
        let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
        let font = load_font_config(&store, &hierarchy, &lang_book)?.unwrap_or_default();
        let template = resolve_template(&font, template_name)?;

        // --slot SLOT=GLYPH, each glyph read from the store.
        let dir = glyph_store_dir(store.project_root(), language);
        let mut comps: BTreeMap<String, String> = BTreeMap::new();
        for s in slots {
            let (slot, glyph) = s.split_once('=').ok_or_else(|| {
                Error::Config(format!("bad --slot `{s}` (expected SLOT=GLYPH)"))
            })?;
            let path = dir.join(format!("{glyph}.svg"));
            let svg = std::fs::read_to_string(&path).map_err(|_| {
                Error::Config(format!(
                    "slot `{slot}`: no glyph `{glyph}` in {language}'s store ({})",
                    path.display()
                ))
            })?;
            comps.insert(slot.to_string(), svg);
        }
        let cells = template.slots();
        for slot in comps.keys() {
            if !cells.contains(&slot.as_str()) {
                eprintln!("note: slot `{slot}` is not used by template `{template_name}`");
            }
        }

        let composed = compose::compose_block(&template, &comps).map_err(Error::Config)?;
        let report = preflight::lint_svg(&composed);
        (composed, report)
    };

    // Phase 2 — preview + advisory bind.
    if let Some(p) = out {
        crate::io_atomic::write(p, composed.as_bytes()).map_err(Error::Io)?;
        println!("composed block → {}", p.display());
    } else {
        println!("{composed}");
    }
    if !report.is_usable() {
        eprintln!("preflight: ✗ {}", report.errors.join("; "));
        return Ok(());
    }
    for w in &report.warnings {
        eprintln!("note: {w}");
    }
    if yes {
        bind_glyph_text(project, language, &composed, phoneme, codepoint, Some(name), None, "the composed block")
    } else {
        eprintln!("preflight: ✓ usable — re-run with --yes to bind it as `{name}`");
        Ok(())
    }
}

/// LANG-1 P5.6c — input method: transliterate romanized/phonemic text into the
/// script's codepoints using the `font` block's glyph→phoneme bindings.
pub(crate) fn transliterate(project: &Path, language: &str, text: &str, json: bool) -> Result<()> {
    use crate::conlang::writing::input;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let font = load_font_config(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no `font` block to type with"))
    })?;
    let out = input::to_script(&font, text);

    if json {
        let codepoints: Vec<String> =
            out.script.chars().map(|c| format!("U+{:04X}", c as u32)).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "input": text,
                "script": out.script,
                "codepoints": codepoints,
                "mapped": out.mapped,
                "unmatched": out.unmatched.iter().collect::<String>(),
            }))
            .map_err(|e| Error::Store(format!("serializing: {e}")))?
        );
        return Ok(());
    }

    // The script chars are typically PUA (invisible in a terminal); print them
    // on stdout (capturable / insertable) and the readable codepoints on stderr.
    println!("{}", out.script);
    let codepoints: Vec<String> = out
        .script
        .chars()
        .map(|c| if c.is_whitespace() { "·".into() } else { format!("U+{:04X}", c as u32) })
        .collect();
    eprintln!("  {} glyph(s) mapped · {}", out.mapped, codepoints.join(" "));
    if !out.unmatched.is_empty() {
        let u: String = out.unmatched.iter().collect();
        eprintln!("  ⚠ no glyph for: {u} (bind one with `font-import-glyph --phoneme`)");
    }
    eprintln!(
        "(renders in the `{}` font)",
        font.family.as_deref().unwrap_or(&lang_book.title)
    );
    Ok(())
}

/// LANG-1 P5.6 — binding-time B: emit a Typst quadrat that arranges component
/// glyphs spatially at layout time (the hieroglyphic path — no precomposed font
/// glyph). Components render as characters of the language's font, so each must
/// have a codepoint.
pub(crate) fn spatial_typst(
    project: &Path,
    language: &str,
    template_name: &str,
    name: &str,
    slots: &[String],
    size: &str,
    out: Option<&Path>,
) -> Result<()> {
    use crate::conlang::writing::compose;
    use std::collections::BTreeMap;

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let font = load_font_config(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no `font` block"))
    })?;
    let template = resolve_template(&font, template_name)?;
    let family = font.family.clone().unwrap_or_else(|| lang_book.title.clone());

    // --slot SLOT=GLYPH, each glyph resolved to its codepoint (Typst renders by
    // character).
    let mut chars: BTreeMap<String, char> = BTreeMap::new();
    for s in slots {
        let (slot, glyph) = s
            .split_once('=')
            .ok_or_else(|| Error::Config(format!("bad --slot `{s}` (expected SLOT=GLYPH)")))?;
        let g = font
            .glyphs
            .iter()
            .find(|g| g.name == glyph)
            .ok_or_else(|| Error::Config(format!("slot `{slot}`: no glyph `{glyph}` in {language}'s font")))?;
        let cp = g.codepoint.ok_or_else(|| {
            Error::Config(format!(
                "glyph `{glyph}` has no codepoint — Typst renders by character; \
                 give it one with `font-import-glyph --codepoint`"
            ))
        })?;
        chars.insert(slot.to_string(), cp);
    }
    let cells = template.slots();
    for slot in chars.keys() {
        if !cells.contains(&slot.as_str()) {
            eprintln!("note: slot `{slot}` is not used by template `{template_name}`");
        }
    }

    let typ = compose::quadrat_typst(name, &template, &family, &chars, size).map_err(Error::Config)?;
    if let Some(p) = out {
        crate::io_atomic::write(p, typ.as_bytes()).map_err(Error::Io)?;
        println!("quadrat `{name}` → {}", p.display());
    } else {
        print!("{typ}");
    }
    eprintln!(
        "(uses the `{family}` font — build it with `font-build --language {language} --format ttf` and embed it in your Typst document)"
    );
    Ok(())
}

/// LANG-1 P5.5 — AI text-to-SVG glyph draft. Advisory: previews the drafted
/// glyph + its preflight verdict; only `--yes` (and only a usable result)
/// binds it into the language's `font` block.
#[allow(clippy::too_many_arguments)]
pub(crate) fn glyph_draft(
    project: &Path,
    language: &str,
    describe: &str,
    phoneme: Option<&str>,
    codepoint: Option<&str>,
    name: Option<&str>,
    provider: Option<&str>,
    out: Option<&Path>,
    yes: bool,
) -> Result<()> {
    use crate::conlang::writing::{draft, preflight};

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    eprintln!("inkhaven language glyph-draft · {language} · model: {model}");

    let phon_clause = phoneme
        .map(|p| format!(" It renders the phoneme /{p}/."))
        .unwrap_or_default();
    let prompt = format!(
        "Draft a glyph for the constructed writing system of the language '{language}'.{phon_clause}\n\n\
         Description: {describe}"
    );
    let raw = crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(GLYPH_DRAFT_SYSTEM.to_string()),
        prompt,
    )
    .map_err(|e| Error::Store(format!("inference error: {e}")))?;

    let svg = draft::extract_svg(&raw)
        .ok_or_else(|| Error::Store("the model did not return an SVG glyph".into()))?;
    let report = preflight::lint_svg(&svg);

    // Always make the draft inspectable.
    if let Some(path) = out {
        crate::io_atomic::write(path, svg.as_bytes()).map_err(Error::Io)?;
        println!("draft SVG → {}", path.display());
    } else {
        println!("{svg}");
    }

    if report.is_usable() {
        println!("preflight: ✓ usable{}", if report.warnings.is_empty() {
            String::new()
        } else {
            format!(" ({})", report.warnings.join("; "))
        });
    } else {
        eprintln!("preflight: ✗ not usable — {}", report.errors.join("; "));
        eprintln!("(refine the description and re-run; not bound)");
        return Ok(());
    }

    if yes {
        bind_glyph_text(project, language, &svg, phoneme, codepoint, name, None, "the AI draft")?;
    } else {
        eprintln!("(advisory — re-run with --yes to bind it into {language}'s font)");
    }
    Ok(())
}

/// LANG-1 P5.1 — lint a glyph SVG file for font suitability.
pub(crate) fn glyph_lint(svg: &Path) -> Result<()> {
    let body = std::fs::read_to_string(svg)
        .map_err(|e| Error::Config(format!("reading {}: {e}", svg.display())))?;
    let report = crate::conlang::writing::preflight::lint_svg(&body);

    println!("glyph lint · {}", svg.display());
    for i in &report.info {
        println!("  · {i}");
    }
    for w in &report.warnings {
        println!("  ⚠ {w}");
    }
    for e in &report.errors {
        println!("  ✗ {e}");
    }
    if report.is_usable() {
        println!(
            "\n  ✓ usable as a font glyph{}",
            if report.warnings.is_empty() { "" } else { " (with the warnings above)" }
        );
    } else {
        println!("\n  ✗ not usable as-is — fix the errors above");
    }
    Ok(())
}
