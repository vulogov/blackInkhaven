//! 1.2.18+ R.2 — `inkhaven audiobook` subcommand.
//!
//! Synthesises a user book to a single `.m4b` audiobook
//! with a chapter marker per Chapter node.  Drives the
//! 1.2.17 TTS engine for per-chapter synthesis, then
//! `ffmpeg` for the concat + chapter-metadata mux.
//!
//! ```bash
//! $ inkhaven audiobook --book-name "My Novel" --output my-novel.m4b
//! ```
//!
//! Requires `ffmpeg` + `ffprobe` on PATH (no pure-Rust
//! m4b muxer with chapter support exists).  TTS must be
//! enabled (`editor.tts.enabled = true`); the active
//! backend (Piper or System `say`) is resolved the same
//! way the TUI resolves it.

use std::path::{Path, PathBuf};

use crate::audiobook::{self, ChapterAudio};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

pub fn run(
    project: &Path,
    book_name: Option<&str>,
    output: Option<&Path>,
    title: Option<&str>,
    author: Option<&str>,
) -> Result<()> {
    // ── ffmpeg pre-flight ─────────────────────────────
    if !audiobook::binary_on_path("ffmpeg") {
        return Err(Error::Store(
            "audiobook: `ffmpeg` not found on PATH.  Install it \
             (brew install ffmpeg / apt install ffmpeg / \
             winget install ffmpeg) — it's required to mux the \
             .m4b with chapter markers."
                .into(),
        ));
    }
    if !audiobook::binary_on_path("ffprobe") {
        return Err(Error::Store(
            "audiobook: `ffprobe` not found on PATH (ships with \
             ffmpeg) — required to measure chapter durations."
                .into(),
        ));
    }

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;

    if !cfg.editor.tts.enabled {
        return Err(Error::Store(
            "audiobook: TTS is disabled.  Set editor.tts.enabled = \
             true in inkhaven.hjson + pick a voice (see Tutorial 56)."
                .into(),
        ));
    }

    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    let book = crate::cli::resolve_user_book(&h, book_name, "audiobook")
        .map_err(Error::Store)?
        .clone();

    // ── resolve TTS engine ────────────────────────────
    let mut engine =
        crate::tui::tts::TtsEngine::resolve(&cfg.editor.tts, &layout.root);
    engine
        .is_ready()
        .map_err(|e| Error::Store(format!("audiobook: TTS not ready — {e}")))?;
    let voice = engine
        .resolve_voice(&cfg.editor.tts.voice)
        .unwrap_or_default();
    let wpm = ((180.0 * cfg.editor.tts.speed.max(0.1)).round() as i32)
        .clamp(80, 400) as u16;
    eprintln!(
        "audiobook: backend={} voice={}",
        engine.label(),
        if voice.is_empty() { "engine default" } else { &voice },
    );

    // ── gather chapters ───────────────────────────────
    let chapter_texts = collect_chapter_texts(&store, &h, &book)?;
    if chapter_texts.is_empty() {
        return Err(Error::Store(format!(
            "audiobook: `{}` has no chapters with prose to read",
            book.title,
        )));
    }

    // Work directory for the per-chapter audio +
    // intermediate files, under the OS temp dir so we
    // don't litter the project.
    let work_dir = std::env::temp_dir().join(format!(
        "inkhaven-audiobook-{}",
        book.slug,
    ));
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).map_err(Error::Io)?;

    // ── synthesise each chapter ───────────────────────
    let total = chapter_texts.len();
    // Backend-specific extension: macOS `say -o` rejects
    // .wav (wants aiff); Piper writes wav.  ffmpeg reads
    // either by content.
    let ext = engine.audio_extension();
    let mut chapters: Vec<ChapterAudio> = Vec::new();
    for (i, (title_text, prose)) in chapter_texts.iter().enumerate() {
        eprintln!("  [{}/{}] synthesising `{}`…", i + 1, total, title_text);
        let audio_path =
            work_dir.join(format!("chapter-{:03}.{ext}", i + 1));
        engine
            .speak_to_file_blocking(
                prose,
                &voice,
                Some(wpm),
                &audio_path,
                // Generous per-chapter cap — a long
                // chapter is minutes of audio.
                std::time::Duration::from_secs(1800),
            )
            .map_err(|e| {
                Error::Store(format!(
                    "audiobook: synthesis failed on `{title_text}` — {e}"
                ))
            })?;
        let duration_secs =
            audiobook::probe_duration_secs("ffprobe", &audio_path)
                .map_err(|e| {
                    Error::Store(format!("audiobook: ffprobe — {e:#}"))
                })?;
        chapters.push(ChapterAudio {
            title: title_text.clone(),
            path: audio_path,
            duration_secs,
        });
    }

    // ── build chapter metadata + mux ──────────────────
    let book_title = title
        .map(str::to_string)
        .unwrap_or_else(|| crate::cli::epub::clean_title(&book.title));
    let author_name = author
        .map(str::to_string)
        .or_else(|| cfg.editor.comment_author.clone())
        .unwrap_or_else(|| "Unknown Author".to_string());

    let meta_chapters: Vec<(String, f64)> = chapters
        .iter()
        .map(|c| (c.title.clone(), c.duration_secs))
        .collect();
    let ffmeta = audiobook::build_ffmetadata(
        &book_title,
        &author_name,
        &meta_chapters,
    );
    let ffmeta_path = work_dir.join("chapters.ffmeta");
    std::fs::write(&ffmeta_path, ffmeta).map_err(Error::Io)?;

    let dest = output
        .map(PathBuf::from)
        .unwrap_or_else(|| layout.root.join(format!("{}.m4b", book.slug)));

    let chapter_paths: Vec<PathBuf> =
        chapters.iter().map(|c| c.path.clone()).collect();
    eprintln!("audiobook: muxing {} chapters → m4b…", chapters.len());
    audiobook::mux_m4b("ffmpeg", &chapter_paths, &ffmeta_path, &dest, &work_dir)
        .map_err(|e| Error::Store(format!("audiobook: {e:#}")))?;

    // Best-effort cleanup of the work dir.
    let _ = std::fs::remove_dir_all(&work_dir);

    let total_secs: f64 = chapters.iter().map(|c| c.duration_secs).sum();
    let bytes = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    println!(
        "Audiobook: {} ({} chapters · {} · {} MB)",
        dest.display(),
        chapters.len(),
        fmt_duration(total_secs),
        bytes / 1_048_576,
    );
    Ok(())
}

fn fmt_duration(secs: f64) -> String {
    let total = secs.round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}h{m:02}m{s:02}s")
    } else {
        format!("{m}m{s:02}s")
    }
}

/// One `(chapter_title, plain_prose)` per top-level
/// Chapter under `book`, in display order.  Empty-prose
/// chapters are skipped.
fn collect_chapter_texts(
    store: &Store,
    h: &Hierarchy,
    book: &Node,
) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for chapter in h.children_of(Some(book.id)) {
        if chapter.kind != NodeKind::Chapter {
            continue;
        }
        let mut prose = String::new();
        append_branch_prose(store, h, chapter, &mut prose)?;
        let prose = prose.trim().to_string();
        if !prose.is_empty() {
            out.push((crate::cli::epub::clean_title(&chapter.title), prose));
        }
    }
    Ok(out)
}

/// Walk a chapter/subchapter's children, appending each
/// paragraph's *plain* prose (newline-separated, empties
/// dropped) for TTS synthesis.
///
/// 1.2.22 D.3.b — intentionally NOT shared with
/// `cli::epub::append_branch_prose`: only the recursion
/// skeleton matches.  epub takes `is_sub`, emits `<h2>` on
/// subchapter entry, and uses `typst_to_xhtml` with no
/// empty-skip or separator; this emits no headings, uses
/// `typst_to_plain`, drops empties, and newline-joins.  A
/// shared higher-order walk would be net-positive LOC for
/// two callers.
fn append_branch_prose(
    store: &Store,
    h: &Hierarchy,
    branch: &Node,
    prose: &mut String,
) -> Result<()> {
    for child in h.children_of(Some(branch.id)) {
        match child.kind {
            NodeKind::Paragraph => {
                let bytes = store
                    .get_content(child.id)
                    .map_err(|e| {
                        Error::Store(format!(
                            "audiobook: read paragraph {}: {e}",
                            child.id
                        ))
                    })?
                    .unwrap_or_default();
                let raw = String::from_utf8_lossy(&bytes);
                let plain = audiobook::typst_to_plain(&raw);
                if !plain.trim().is_empty() {
                    if !prose.is_empty() {
                        prose.push('\n');
                    }
                    prose.push_str(&plain);
                }
            }
            NodeKind::Subchapter => {
                append_branch_prose(store, h, child, prose)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Resolve the user book.  Mirrors `cli::epub` /
/// `cli::build`.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_duration_under_an_hour() {
        assert_eq!(fmt_duration(125.0), "2m05s");
    }

    #[test]
    fn fmt_duration_over_an_hour() {
        assert_eq!(fmt_duration(3725.0), "1h02m05s");
    }

    #[test]
    fn fmt_duration_rounds() {
        assert_eq!(fmt_duration(59.6), "1m00s");
    }
}
