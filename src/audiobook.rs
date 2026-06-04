//! 1.2.18+ R.2 — audiobook (.m4b) export.
//!
//! Turns a user book into a single `.m4b` audiobook with
//! a chapter marker per Chapter node — the resumable,
//! jump-by-chapter format audiobook players expect.
//! Builds on the 1.2.17 TTS engine (`speak_to_file_blocking`):
//! each chapter's prose is synthesised to a temp audio
//! file, durations are probed, and `ffmpeg` concatenates
//! + muxes the lot into an AAC `.m4b` with embedded
//! chapter metadata.
//!
//! ## ffmpeg requirement
//!
//! There is no pure-Rust m4b muxer with chapter-marker
//! support, so `ffmpeg` (+ `ffprobe`) is a required
//! external for `inkhaven audiobook`.  Universal on
//! macOS / Linux / Windows via the usual package
//! managers; the CLI surfaces a clear error when it's
//! absent.
//!
//! ## Wall-clock cost
//!
//! Synthesis is roughly real-time per chapter — a 10K-
//! paragraph book is hours of audio + hours to produce.
//! The CLI reports per-chapter progress on stderr; this
//! is a batch export, not an interactive operation.
//!
//! The pure pieces here — `typst_to_plain` (strip markup
//! for clean TTS input), `build_ffmetadata` (the chapter-
//! marker file), `parse_ffprobe_duration` — are unit-
//! tested directly; the ffmpeg orchestration is exercised
//! end-to-end by the CLI.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

/// One synthesised chapter: its title + audio file +
/// measured duration.
#[derive(Debug, Clone)]
pub struct ChapterAudio {
    pub title: String,
    pub path: PathBuf,
    pub duration_secs: f64,
}

/// Strip typst markup down to clean prose for TTS.
///
/// Drops the leading `= title` (organisational
/// scaffolding), keeps subheading TEXT (a reader of the
/// audio hears the scene label), removes `_` / `*`
/// emphasis markers (keeping the inner words), drops
/// `#footnote[…]` entirely (footnotes are disruptive
/// read aloud), and collapses blank-line blocks into
/// sentence-paced paragraphs separated by newlines (the
/// `say` / piper engines treat a newline as a short
/// pause).
pub fn typst_to_plain(body: &str) -> String {
    let stripped = strip_leading_heading(body);
    let mut out_blocks: Vec<String> = Vec::new();
    for block in split_blocks(&stripped) {
        let trimmed = block.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Subheadings: keep the text, drop the `=` marks.
        let no_heading = trimmed
            .trim_start_matches('=')
            .trim_start();
        // Collapse intra-block newlines to spaces.
        let joined = no_heading
            .split('\n')
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ");
        let plain = strip_inline_markup(&joined);
        if !plain.trim().is_empty() {
            out_blocks.push(plain.trim().to_string());
        }
    }
    out_blocks.join("\n")
}

fn strip_leading_heading(body: &str) -> String {
    let mut lines = body.lines();
    if let Some(first) = lines.clone().next() {
        if first.trim_start().starts_with("= ") {
            lines.next();
            let rest: Vec<&str> = lines.collect();
            return rest.join("\n").trim_start_matches('\n').to_string();
        }
    }
    body.to_string()
}

fn split_blocks(s: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut cur = String::new();
    for line in s.lines() {
        if line.trim().is_empty() {
            if !cur.trim().is_empty() {
                blocks.push(std::mem::take(&mut cur));
            }
        } else {
            if !cur.is_empty() {
                cur.push('\n');
            }
            cur.push_str(line);
        }
    }
    if !cur.trim().is_empty() {
        blocks.push(cur);
    }
    blocks
}

/// Remove `#footnote[…]` blocks + `_` / `*` emphasis
/// delimiters, keeping the spoken words.
fn strip_inline_markup(s: &str) -> String {
    let no_footnotes = drop_footnotes(s);
    no_footnotes
        .chars()
        .filter(|c| *c != '_' && *c != '*')
        .collect()
}

fn drop_footnotes(s: &str) -> String {
    let needle = "#footnote[";
    let mut out = String::new();
    let mut rest = s;
    while let Some(pos) = rest.find(needle) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + needle.len()..];
        if let Some(end) = after.find(']') {
            // Drop the footnote entirely (disruptive in
            // audio).
            rest = &after[end + 1..];
        } else {
            // Unterminated — drop the rest.
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// Build an ffmpeg metadata file (`;FFMETADATA1`) with a
/// `[CHAPTER]` block per chapter.  Pure.  `chapters` is
/// `(title, duration_secs)` in order; START/END are
/// accumulated in milliseconds (TIMEBASE 1/1000).
pub fn build_ffmetadata(
    book_title: &str,
    author: &str,
    chapters: &[(String, f64)],
) -> String {
    let mut out = String::from(";FFMETADATA1\n");
    out.push_str(&format!("title={}\n", escape_ffmeta(book_title)));
    out.push_str(&format!("artist={}\n", escape_ffmeta(author)));
    out.push_str(&format!("album={}\n", escape_ffmeta(book_title)));
    out.push_str("genre=Audiobook\n");

    let mut start_ms: i64 = 0;
    for (title, dur_secs) in chapters {
        let dur_ms = (dur_secs * 1000.0).round() as i64;
        let end_ms = start_ms + dur_ms;
        out.push_str("\n[CHAPTER]\n");
        out.push_str("TIMEBASE=1/1000\n");
        out.push_str(&format!("START={start_ms}\n"));
        out.push_str(&format!("END={end_ms}\n"));
        out.push_str(&format!("title={}\n", escape_ffmeta(title)));
        start_ms = end_ms;
    }
    out
}

/// Escape the characters ffmetadata treats specially
/// (`=`, `;`, `#`, `\`, newline) with a backslash.
fn escape_ffmeta(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '=' | ';' | '#' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\\n"),
            _ => out.push(c),
        }
    }
    out
}

/// Parse the duration (seconds) out of `ffprobe
/// -show_entries format=duration -of csv=p=0` output.
/// Pure.
pub fn parse_ffprobe_duration(stdout: &str) -> Option<f64> {
    stdout.trim().lines().next()?.trim().parse::<f64>().ok()
}

// ── subprocess wrappers ──────────────────────────────

/// Probe an audio file's duration via `ffprobe`.
pub fn probe_duration_secs(ffprobe_bin: &str, path: &Path) -> Result<f64> {
    let output = std::process::Command::new(ffprobe_bin)
        .args([
            "-v",
            "quiet",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .map_err(|e| anyhow!("spawn {ffprobe_bin}: {e}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "ffprobe failed on {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ffprobe_duration(&stdout)
        .ok_or_else(|| anyhow!("couldn't parse ffprobe duration: {stdout:?}"))
}

/// Concatenate the chapter audio files + mux into a
/// `.m4b` (AAC) with the chapter metadata embedded.
///
/// Uses the ffmpeg concat demuxer (all inputs share the
/// engine's output format within a single book) + a
/// second `-i` for the ffmetadata, mapping its metadata
/// into the output.
pub fn mux_m4b(
    ffmpeg_bin: &str,
    chapter_paths: &[PathBuf],
    ffmeta_path: &Path,
    dest: &Path,
    work_dir: &Path,
) -> Result<()> {
    if chapter_paths.is_empty() {
        return Err(anyhow!("no chapter audio to mux"));
    }
    // Write the concat list (ffmpeg concat demuxer
    // format: one `file '<abs-path>'` per line).
    let list_path = work_dir.join("concat-list.txt");
    let mut list = String::new();
    for p in chapter_paths {
        let abs = p
            .canonicalize()
            .unwrap_or_else(|_| p.clone());
        // ffmpeg concat needs single-quote escaping:
        // a `'` becomes `'\''`.
        let escaped = abs.to_string_lossy().replace('\'', "'\\''");
        list.push_str(&format!("file '{escaped}'\n"));
    }
    std::fs::write(&list_path, list)?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let output = std::process::Command::new(ffmpeg_bin)
        .args(["-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list_path)
        .arg("-i")
        .arg(ffmeta_path)
        .args([
            "-map_metadata",
            "1",
            "-c:a",
            "aac",
            "-b:a",
            "64k",
            "-movflags",
            "+faststart",
        ])
        .arg(dest)
        .output()
        .map_err(|e| anyhow!("spawn {ffmpeg_bin}: {e}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "ffmpeg mux failed: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .rev()
                .take(5)
                .collect::<Vec<_>>()
                .join(" | "),
        ));
    }
    Ok(())
}

/// True when `bin` resolves on PATH.
pub fn binary_on_path(bin: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| {
        let p = dir.join(bin);
        std::fs::metadata(&p).map(|m| m.is_file()).unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── typst_to_plain ────────────────────────────────

    #[test]
    fn plain_strips_leading_heading() {
        let body = "= 001. Approach\n\nHelena paused.";
        assert_eq!(typst_to_plain(body), "Helena paused.");
    }

    #[test]
    fn plain_removes_emphasis_markers() {
        let body = "She was _very_ tired and *quite* done.";
        assert_eq!(
            typst_to_plain(body),
            "She was very tired and quite done.",
        );
    }

    #[test]
    fn plain_drops_footnotes() {
        let body = "A claim#footnote[a citation] continues.";
        assert_eq!(typst_to_plain(body), "A claim continues.");
    }

    #[test]
    fn plain_keeps_subheading_text() {
        let body = "Lead.\n\n== A Scene\n\nMore.";
        let got = typst_to_plain(body);
        assert!(got.contains("A Scene"));
        assert!(!got.contains("=="));
    }

    #[test]
    fn plain_collapses_intra_block_newlines() {
        let body = "Line one\nline two";
        assert_eq!(typst_to_plain(body), "Line one line two");
    }

    #[test]
    fn plain_separates_blocks_with_newline() {
        let body = "First.\n\nSecond.";
        assert_eq!(typst_to_plain(body), "First.\nSecond.");
    }

    #[test]
    fn plain_empty_body_is_empty() {
        assert_eq!(typst_to_plain("= Title\n\n"), "");
    }

    // ── build_ffmetadata ──────────────────────────────

    #[test]
    fn ffmeta_accumulates_chapter_timestamps() {
        let chapters = vec![
            ("Arrivals".to_string(), 120.0),
            ("The Wharf".to_string(), 90.5),
        ];
        let meta = build_ffmetadata("My Book", "Author", &chapters);
        assert!(meta.starts_with(";FFMETADATA1"));
        assert!(meta.contains("title=My Book"));
        assert!(meta.contains("artist=Author"));
        // Chapter 1: 0 → 120000ms.
        assert!(meta.contains("START=0\nEND=120000\ntitle=Arrivals"));
        // Chapter 2: 120000 → 210500ms (120000 + 90500).
        assert!(meta.contains("START=120000\nEND=210500\ntitle=The Wharf"));
        assert!(meta.contains("genre=Audiobook"));
    }

    #[test]
    fn ffmeta_escapes_special_chars() {
        let chapters = vec![("A=B; #note".to_string(), 1.0)];
        let meta = build_ffmetadata("T=t", "x", &chapters);
        assert!(meta.contains("title=T\\=t"));
        assert!(meta.contains("title=A\\=B\\; \\#note"));
    }

    #[test]
    fn ffmeta_single_chapter_starts_at_zero() {
        let chapters = vec![("Only".to_string(), 60.0)];
        let meta = build_ffmetadata("B", "A", &chapters);
        assert!(meta.contains("START=0\nEND=60000\ntitle=Only"));
    }

    // ── parse_ffprobe_duration ────────────────────────

    #[test]
    fn parse_duration_reads_float() {
        assert_eq!(parse_ffprobe_duration("123.456\n"), Some(123.456));
    }

    #[test]
    fn parse_duration_rejects_garbage() {
        assert_eq!(parse_ffprobe_duration("N/A\n"), None);
        assert_eq!(parse_ffprobe_duration(""), None);
    }

    #[test]
    fn parse_duration_takes_first_line() {
        assert_eq!(parse_ffprobe_duration("12.5\nextra\n"), Some(12.5));
    }

    // ── binary_on_path ────────────────────────────────

    #[test]
    fn binary_on_path_finds_sh() {
        // `sh` is on PATH on every unix CI runner.
        #[cfg(unix)]
        assert!(binary_on_path("sh"));
    }

    #[test]
    fn binary_on_path_misses_nonsense() {
        assert!(!binary_on_path("definitely-not-a-real-binary-xyzzy"));
    }
}
