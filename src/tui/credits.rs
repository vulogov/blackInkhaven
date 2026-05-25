//! Static content + line builder for the Ctrl+B V credits
//! modal. The PNG logo is embedded directly in the binary via
//! `include_bytes!` and decoded lazily on first open; the
//! crate list is hand-curated so the panel stays readable
//! (auto-pulling every transitive dep from Cargo.lock would
//! dump 200+ rows nobody would scroll through). Extracted
//! from `tui::app` in the 1.2.7 refactor.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// `logo.png` from the repo root embedded directly in the
/// binary. Decoded lazily the first time the credits modal
/// opens so the cost is paid once per session, not every
/// Ctrl+B V press. The PNG's size on disk is the binary-size
/// delta; keep the source PNG appropriately sized (~1–2 MB is
/// a sensible upper bound).
static EMBEDDED_LOGO: &[u8] = include_bytes!("../../logo.png");

static DECODED_LOGO: std::sync::OnceLock<Option<image::DynamicImage>> =
    std::sync::OnceLock::new();

pub(super) fn embedded_logo_image() -> Option<&'static image::DynamicImage> {
    DECODED_LOGO
        .get_or_init(|| image::load_from_memory(EMBEDDED_LOGO).ok())
        .as_ref()
}

/// Components Inkhaven directly depends on. Each entry is
/// `(crate-name, license, one-line description)`. The list is curated by
/// hand so the credits panel stays readable — auto-pulling every
/// transitive dep from Cargo.lock would dump 200+ rows nobody would
/// scroll through. When you add a new direct dep in Cargo.toml, add it
/// here too.
const CREDITS_COMPONENTS: &[(&str, &str, &str)] = &[
    ("duckdb",                "MIT",             "embedded SQL engine — metadata + blob stores"),
    ("vecstore",              "MIT",             "HNSW vector index — semantic search"),
    ("fastembed",             "Apache-2.0",      "multilingual ONNX text embeddings"),
    ("ratatui",               "MIT",             "TUI rendering framework"),
    ("tui-textarea",          "MIT",             "multi-line text widget (state model)"),
    ("crossterm",             "MIT",             "cross-platform terminal control"),
    ("tree-sitter",           "MIT",             "incremental parser engine"),
    ("tree-sitter-highlight", "MIT",             "syntax-highlight tagging on top of tree-sitter"),
    ("tree-sitter-typst",     "MIT",             "Typst grammar for tree-sitter (uben0)"),
    ("genai",                 "MIT / Apache-2.0", "provider-neutral LLM client (Gemini, DeepSeek, Ollama, OpenAI, …)"),
    ("pulldown-cmark",        "MIT",             "CommonMark parser — markdown rendering in the AI pane"),
    ("rust-stemmers",         "MIT",             "Snowball stemmers — multilingual lexicon overlay"),
    ("unicode-segmentation",  "MIT / Apache-2.0", "Unicode word boundaries"),
    ("regex",                 "MIT / Apache-2.0", "in-buffer find / replace"),
    ("tokio",                 "MIT",             "async runtime"),
    ("tokio-stream",          "MIT",             "Stream adapters for tokio"),
    ("futures-util",          "MIT / Apache-2.0", "futures combinators"),
    ("clap",                  "MIT / Apache-2.0", "CLI parser"),
    ("serde",                 "MIT / Apache-2.0", "serialisation framework"),
    ("serde_json",            "MIT / Apache-2.0", "JSON support for serde"),
    ("serde-hjson",           "MIT",             "HJSON parser — friendly config file format"),
    ("humantime",             "MIT / Apache-2.0", "human-readable duration parsing — backup max_age"),
    ("humantime-serde",       "MIT / Apache-2.0", "serde glue for humantime durations"),
    ("rodio",                 "MIT / Apache-2.0", "audio playback — typewriter SFX (Ctrl+B E)"),
    ("ratatui-image",         "MIT",             "in-TUI image preview — Enter on an Image node"),
    ("image",                 "MIT / Apache-2.0", "image decoder for the preview pane"),
    ("chrono",                "MIT / Apache-2.0", "timestamps, RFC-3339 formatting"),
    ("uuid",                  "Apache-2.0",      "UUIDv7 paragraph IDs"),
    ("zip",                   "MIT",             "backup / restore archive format"),
    ("walkdir",               "MIT / Unlicense", "recursive directory walking"),
    ("arboard",               "MIT / Apache-2.0", "system clipboard access"),
    ("directories",           "MIT / Apache-2.0", "per-user cache path resolution"),
    ("slug",                  "MIT / Apache-2.0", "URL-safe slug generation"),
    ("tracing",               "MIT",             "structured logging"),
    ("tracing-subscriber",    "MIT",             "log filtering and writer config"),
    ("anyhow",                "MIT / Apache-2.0", "error wrapping in application boundaries"),
    ("thiserror",             "MIT / Apache-2.0", "derive macros for typed errors"),
];

/// Build the styled `Line`s the credits modal renders. Returns one Line
/// per row — section headers in cyan-bold, crate names in the configured
/// modal-border colour, descriptions in dim. Each crate row is wrapped
/// to fit a reasonable terminal width; very long descriptions naturally
/// truncate at the right edge of the modal.
pub(super) fn build_credits_lines(
    theme: &super::theme::Theme,
    engine_summary: &str,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let bold_accent = Style::default()
        .fg(theme.modal_border)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().add_modifier(Modifier::DIM);

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        format!("  Inkhaven v{}", env!("CARGO_PKG_VERSION")),
        bold_accent,
    )]));
    lines.push(Line::from(Span::styled(
        format!("  {}", env!("CARGO_PKG_DESCRIPTION")),
        dim,
    )));
    lines.push(Line::from(""));

    // 1.2.5+: surface the active Typst engine so users can confirm
    // their HJSON setting took effect without going to the logs.
    lines.push(Line::from(vec![Span::styled(
        "  Typst engine".to_string(),
        bold_accent,
    )]));
    lines.push(Line::from(format!("    {engine_summary}")));
    lines.push(Line::from(""));

    lines.push(Line::from(vec![Span::styled(
        "  Author".to_string(),
        bold_accent,
    )]));
    for a in env!("CARGO_PKG_AUTHORS").split(':') {
        if !a.is_empty() {
            lines.push(Line::from(format!("    {a}")));
        }
    }
    lines.push(Line::from(""));

    lines.push(Line::from(vec![Span::styled(
        "  Project".to_string(),
        bold_accent,
    )]));
    lines.push(Line::from(format!(
        "    Repository: {}",
        env!("CARGO_PKG_REPOSITORY")
    )));
    lines.push(Line::from(format!(
        "    Licence:    {}",
        env!("CARGO_PKG_LICENSE")
    )));
    lines.push(Line::from(""));

    lines.push(Line::from(vec![Span::styled(
        "  Components used".to_string(),
        bold_accent,
    )]));
    lines.push(Line::from(Span::styled(
        "  Inkhaven stands on the shoulders of these open-source projects:".to_string(),
        dim,
    )));
    lines.push(Line::from(""));

    // Two-column rendering inside the credits body would be neat but
    // complicates wrapping; a single column with name + licence + tagline
    // reads cleanly even on narrow terminals.
    for (name, license, desc) in CREDITS_COMPONENTS {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                format!("{:<24}", name),
                Style::default()
                    .fg(theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  [{}]", license),
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            format!("        {desc}"),
            dim,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  And a long tail of transitive dependencies — every one is".to_string(),
        dim,
    )));
    lines.push(Line::from(Span::styled(
        "  listed in `Cargo.lock`. Thanks to every author.".to_string(),
        dim,
    )));
    lines.push(Line::from(""));

    lines
}
