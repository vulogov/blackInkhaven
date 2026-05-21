use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::ExportFormat;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::export;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

pub fn run(project: &Path, format: ExportFormat, output: Option<&Path>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;

    let combined = build_combined(&layout, &h)?;

    match format {
        ExportFormat::Typst => write_typst(&combined, output),
        ExportFormat::Pdf => write_pdf(&combined, output),
        ExportFormat::Markdown => write_artefact(
            export::build_markdown(&combined),
            output,
            "markdown",
        ),
        ExportFormat::Tex => write_artefact(
            export::build_tex(&combined),
            output,
            "tex",
        ),
        ExportFormat::Epub => {
            // Markdown is the EPUB intermediate. We re-use the same
            // typst→markdown converter so what the user sees in the
            // .md export is exactly what's inside the .epub.
            let md = export::markdown::typst_to_markdown(&combined);
            let title = project
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("inkhaven book")
                .to_string();
            let artefact = export::build_epub(&md, &title)
                .map_err(|e| Error::Store(format!("epub: {e:#}")))?;
            write_artefact(artefact, output, "epub")
        }
    }
}

fn write_artefact(
    artefact: export::Artefact,
    output: Option<&Path>,
    fmt_label: &str,
) -> Result<()> {
    match output {
        Some(path) => {
            artefact.write_to(path).map_err(|e| {
                Error::Store(format!("write {fmt_label}: {e:#}"))
            })?;
            eprintln!("wrote {} ({fmt_label})", path.display());
        }
        None => match &artefact {
            export::Artefact::Markdown(s) | export::Artefact::Tex(s) => {
                print!("{s}");
            }
            export::Artefact::Epub(_) => {
                return Err(Error::Store(
                    "epub export needs --output <path.epub> (binary archive)".into(),
                ));
            }
        },
    }
    Ok(())
}

/// Concatenate every paragraph's `.typ` file in DFS preorder. Branch nodes
/// don't emit anything themselves — paragraphs carry the headings via the
/// `= Title` template `inkhaven add paragraph` writes. The user controls
/// document structure by ordering paragraphs at each level (book-level
/// paragraphs come first → that's where Typst config like `#set page(...)`
/// belongs).
fn build_combined(layout: &ProjectLayout, h: &Hierarchy) -> Result<String> {
    let mut out = String::new();
    for (node, _depth) in h.flatten() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = node.file.as_ref() else {
            continue;
        };
        let abs = layout.root.join(rel);
        let body = std::fs::read_to_string(&abs).map_err(Error::Io)?;
        if !out.is_empty() && !out.ends_with("\n\n") {
            if out.ends_with('\n') {
                out.push('\n');
            } else {
                out.push_str("\n\n");
            }
        }
        out.push_str(&body);
        if !body.ends_with('\n') {
            out.push('\n');
        }
    }
    Ok(out)
}

fn write_typst(combined: &str, output: Option<&Path>) -> Result<()> {
    match output {
        Some(path) => {
            std::fs::write(path, combined.as_bytes()).map_err(Error::Io)?;
            eprintln!("wrote {} bytes to {}", combined.len(), path.display());
        }
        None => {
            print!("{combined}");
        }
    }
    Ok(())
}

fn write_pdf(combined: &str, output: Option<&Path>) -> Result<()> {
    let output = output.ok_or_else(|| {
        Error::Store("PDF export needs --output <path.pdf>".into())
    })?;
    if which("typst").is_none() {
        return Err(Error::Store(
            "the `typst` binary is not on PATH — install it from https://typst.app/ \
             or run `inkhaven export typst -o file.typ` and compile manually"
                .into(),
        ));
    }

    // Write the intermediate .typ alongside the requested PDF so the user can
    // inspect / re-compile manually if something is off.
    let typ_path: PathBuf = output.with_extension("typ");
    std::fs::write(&typ_path, combined.as_bytes()).map_err(Error::Io)?;

    let status = Command::new("typst")
        .arg("compile")
        .arg(&typ_path)
        .arg(output)
        .status()
        .map_err(|e| Error::Store(format!("failed to spawn `typst`: {e}")))?;
    if !status.success() {
        return Err(Error::Store(format!(
            "`typst compile` exited with {status}; intermediate source kept at {}",
            typ_path.display()
        )));
    }
    eprintln!("wrote {} (source: {})", output.display(), typ_path.display());
    Ok(())
}

/// Minimal `which` — returns the first match on PATH, or None.
fn which(prog: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(prog);
        if candidate.is_file() {
            return Some(candidate);
        }
        // Windows compatibility — never hit on macOS/Linux but cheap.
        let with_ext = dir.join(format!("{prog}.exe"));
        if with_ext.is_file() {
            return Some(with_ext);
        }
    }
    None
}
