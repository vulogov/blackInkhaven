//! TDOC-4 — load the site templates: bundled defaults (embedded from
//! `examples/html_templates/`, the single source of truth shared with the shipped
//! sample set) overlaid per-file by a project's `<template_dir>/` overrides. The
//! `functional/` (machinery) and `theme/` (look) split lets a project restyle one
//! without touching the other.

use std::path::Path;

use anyhow::{Context, Result};
use minijinja::{AutoEscape, Environment};

const DEFAULT_PAGE: &str = include_str!("../../../examples/html_templates/functional/page.html");
const DEFAULT_HEADER: &str = include_str!("../../../examples/html_templates/theme/header.html");
const DEFAULT_FOOTER: &str = include_str!("../../../examples/html_templates/theme/footer.html");
const DEFAULT_CSS: &str = include_str!("../../../examples/html_templates/theme/theme.css");
const DEFAULT_README: &str = include_str!("../../../examples/html_templates/README.md");

/// The loaded site templates + stylesheet.
pub struct SiteTemplates {
    pub env: Environment<'static>,
    pub css: String,
}

impl SiteTemplates {
    /// Render `page.html` with the given context.
    pub fn render_page(&self, ctx: minijinja::Value) -> Result<String> {
        let tmpl = self.env.get_template("page.html")?;
        Ok(tmpl.render(ctx)?)
    }
}

/// Load templates, honouring per-file overrides found under `base/` (its
/// `functional/` and `theme/` subdirectories). `base` is already resolved (a
/// `--templates` path, or `<project>/<docs.html.template_dir>`).
pub fn load(base: &Path) -> Result<SiteTemplates> {
    let page = or_default(base, "functional/page.html", DEFAULT_PAGE);
    let header = or_default(base, "theme/header.html", DEFAULT_HEADER);
    let footer = or_default(base, "theme/footer.html", DEFAULT_FOOTER);
    let css = or_default(base, "theme/theme.css", DEFAULT_CSS);

    let mut env = Environment::new();
    env.set_auto_escape_callback(|name| {
        if name.ends_with(".html") {
            AutoEscape::Html
        } else {
            AutoEscape::None
        }
    });
    env.add_template_owned("page.html", page)?;
    env.add_template_owned("header.html", header)?;
    env.add_template_owned("footer.html", footer)?;

    Ok(SiteTemplates { env, css })
}

/// Write the bundled default templates to `dir` (its `functional/` + `theme/`
/// subdirectories) so a user can start customising from the real defaults —
/// works for a `cargo install`ed binary, which has no `examples/` on disk.
pub fn eject(dir: &Path) -> Result<()> {
    let functional = dir.join("functional");
    let theme = dir.join("theme");
    std::fs::create_dir_all(&functional).with_context(|| format!("create {}", functional.display()))?;
    std::fs::create_dir_all(&theme).with_context(|| format!("create {}", theme.display()))?;
    std::fs::write(functional.join("page.html"), DEFAULT_PAGE)?;
    std::fs::write(theme.join("header.html"), DEFAULT_HEADER)?;
    std::fs::write(theme.join("footer.html"), DEFAULT_FOOTER)?;
    std::fs::write(theme.join("theme.css"), DEFAULT_CSS)?;
    std::fs::write(dir.join("README.md"), DEFAULT_README)?;
    Ok(())
}

/// A project override at `<base>/<rel>` if it exists and reads; else the default.
fn or_default(base: &Path, rel: &str, default: &str) -> String {
    let path = base.join(rel);
    match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            if path.exists() {
                tracing::warn!(
                    target: "inkhaven::html",
                    "html: template override `{}` unreadable ({e}) — using default",
                    path.display()
                );
            }
            default.to_string()
        }
    }
}
