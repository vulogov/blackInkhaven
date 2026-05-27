//! `inkhaven doctor` — print a one-shot health report of the
//! inkhaven install and (when run inside a project) the current
//! project. Modelled on `brew doctor`: terse, structured, no
//! questions asked.
//!
//! Three sections, every section optional based on what's
//! discoverable:
//!
//! * **Binary** — version + Rust toolchain it was built with,
//!   typst engine summary, font count (bundled / system), package
//!   cache path + size.
//! * **Project** — when the working directory (or `--project`)
//!   resolves to an initialised inkhaven project: hierarchy
//!   shape, total word count, today's progress, last assembled
//!   PDF + its mtime.
//! * **Notes** — warnings the user might want to act on (no
//!   typst on PATH when engine=external, package cache empty,
//!   font count = 0, etc.).
//!
//! Designed to print plain text — pipe-friendly. No colour, no
//! TTY tricks.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::Store;

/// 1.2.9+ — `inkhaven doctor --voices` — pipe-friendly
/// list of every TTS voice visible through `tts-rs`.  No
/// project context needed (this is a host-OS query),
/// hence no `project: &Path` argument.  Output format
/// for each voice (one per line):
///
///   <name>  ·  <language>  ·  <gender>
///
/// On engine-init failure (Linux without speech-dispatcher,
/// etc.) prints the error to stderr and exits with a
/// non-zero code so a wrapping shell pipeline can branch
/// on the result.
pub fn run_voices() -> Result<()> {
    println!("inkhaven TTS voices — v{}", env!("CARGO_PKG_VERSION"));
    println!("================================================================");
    if !cfg!(target_os = "macos") || !std::path::Path::new("/usr/bin/say").exists() {
        eprintln!(
            "TTS unavailable: 1.2.9 ships TTS via macOS `/usr/bin/say`.  \
             Cross-platform TTS is on the roadmap."
        );
        return Err(crate::error::Error::Config(
            "TTS unavailable on this host".into(),
        ));
    }
    let voices = list_say_voices();
    if voices.is_empty() {
        eprintln!(
            "No voices reported by `say -v \"?\"`.  Open System Settings → \
             Accessibility → Spoken Content → System Voice → Manage Voices \
             and install at least one."
        );
        return Ok(());
    }
    // Stable ordering for diff-friendly output: by locale
    // then by name.
    let mut rows = voices.clone();
    rows.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    let max_name = rows.iter().map(|(n, _, _)| n.chars().count()).max().unwrap_or(0);
    let max_lang = rows.iter().map(|(_, l, _)| l.chars().count()).max().unwrap_or(0);
    println!(
        "{:name_w$}  {:lang_w$}  sample",
        "name",
        "locale",
        name_w = max_name,
        lang_w = max_lang,
    );
    println!(
        "{}  {}  ------",
        "-".repeat(max_name),
        "-".repeat(max_lang),
    );
    for (name, lang, sample) in &rows {
        println!(
            "{:name_w$}  {:lang_w$}  {}",
            name,
            lang,
            sample,
            name_w = max_name,
            lang_w = max_lang,
        );
    }
    println!();
    println!("{} voice(s) total.", rows.len());
    println!();
    println!("Set in inkhaven.hjson:");
    println!("  editor: {{ tts: {{ enabled: true, voice: \"<name fragment>\" }} }}");
    println!("The name field accepts a case-insensitive substring; entries");
    println!("with `Enhanced` or `Premium` in the name are preferred when");
    println!("multiple voices match.");
    Ok(())
}

/// 1.2.9+ — list voices via `say -v "?"`.  Returns
/// `(name, locale, sample)` tuples in the order `say`
/// produced them.
fn list_say_voices() -> Vec<(String, String, String)> {
    let output = match std::process::Command::new("/usr/bin/say")
        .arg("-v")
        .arg("?")
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();
    for line in stdout.lines() {
        let (head, sample) = match line.split_once("# ") {
            Some((a, b)) => (a.trim_end(), b.to_string()),
            None => (line.trim_end(), String::new()),
        };
        let mut parts: Vec<&str> = head.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let locale = parts.pop().unwrap_or("").to_string();
        let name = parts.join(" ");
        if name.is_empty() {
            continue;
        }
        out.push((name, locale, sample));
    }
    out
}

/// 1.2.9+ — emit a copy-paste-ready HJSON snippet of
/// every built-in filter-word list so users can dump
/// them into their `inkhaven.hjson` for visibility +
/// editing.  Output goes under
/// `editor.style_warnings.filter_words`.
pub fn run_filter_words_snippet() -> Result<()> {
    println!("// Paste under editor.style_warnings.filter_words:");
    println!("// (existing values stay; empty arrays already use these built-ins).");
    println!();
    println!("filter_words: {{");
    println!("  enabled: true");
    println!("  use_stemming: true");
    println!("  extra_words: []");
    println!();
    for lang in &["english", "russian", "french", "german", "spanish"] {
        let words = crate::config::built_in_filter_words(lang);
        println!("  // Lemmas — Snowball stemming catches inflections.");
        println!("  {lang}: [");
        // Two-per-line so the dump fits comfortably in a
        // terminal column.  Each entry on its own line
        // would balloon to 30+ rows per language.
        let mut buf = String::from("    ");
        for (i, w) in words.iter().enumerate() {
            if i > 0 {
                buf.push(' ');
            }
            buf.push('"');
            buf.push_str(w);
            buf.push('"');
            if buf.chars().count() > 64 {
                println!("{buf}");
                buf = String::from("    ");
            }
        }
        if buf.trim() != "" {
            println!("{buf}");
        }
        println!("  ]");
        println!();
    }
    println!("}}");
    Ok(())
}

/// 1.2.9+ — `inkhaven doctor --tts-test "<text>"`.
/// Diagnostic for the TTS pipeline.  Initialises the
/// engine, applies the project's configured voice +
/// speed, speaks the given text synchronously, and
/// reports each step on stdout so a user-reported
/// "modal flickers but no audio" can be triaged
/// without instrumenting the TUI.  Exits 0 on success.
/// Loads HJSON config if the path looks like a project;
/// falls back to defaults (Milena, speed 1.0) when no
/// inkhaven.hjson is present at `project`.
pub fn run_tts_test(project: &Path, text: &str) -> Result<()> {
    println!("inkhaven TTS test — v{}", env!("CARGO_PKG_VERSION"));
    println!("project: {}", project.display());
    println!("text:    {text:?}");

    let cfg_path = project.join("inkhaven.hjson");
    let cfg = match Config::load(&cfg_path) {
        Ok(c) => {
            println!("config:  loaded from {}", cfg_path.display());
            c
        }
        Err(e) => {
            println!(
                "config:  {} (using defaults: {})",
                e,
                cfg_path.display()
            );
            Config::default()
        }
    };
    let tts_cfg = &cfg.editor.tts;
    println!(
        "config:  enabled={} voice={:?} speed={}",
        tts_cfg.enabled, tts_cfg.voice, tts_cfg.speed,
    );

    // Platform gate.
    print!("[1/3] platform ... ");
    if !cfg!(target_os = "macos") {
        println!("FAIL — not macOS");
        return Err(crate::error::Error::Config(
            "TTS is macOS-only in 1.2.9".into(),
        ));
    }
    if !std::path::Path::new("/usr/bin/say").exists() {
        println!("FAIL — /usr/bin/say not found");
        return Err(crate::error::Error::Config(
            "/usr/bin/say not found".into(),
        ));
    }
    println!("macOS + /usr/bin/say OK");

    // Voice pick.
    print!("[2/3] pick voice ... ");
    let voices = list_say_voices();
    let needle = tts_cfg.voice.to_lowercase();
    let pick = voices
        .iter()
        .filter(|(n, _, _)| n.to_lowercase().contains(&needle))
        .max_by_key(|(n, _, _)| {
            let lc = n.to_lowercase();
            let enhanced = lc.contains("enhanced") || lc.contains("premium");
            (enhanced as u8, isize::MAX - n.chars().count() as isize)
        })
        .map(|(n, _, _)| n.clone());
    let voice = pick.clone().unwrap_or_else(|| "(system default)".to_string());
    println!("{voice}");

    // Speak via subprocess.  Each round is a fresh
    // `say` process, so engine-reuse bugs don't apply.
    // Run two rounds back-to-back to confirm.
    let wpm = ((180.0 * tts_cfg.speed.max(0.1)).round() as i32).clamp(80, 400);
    println!("[3/3] spawn say (twice, back-to-back at {wpm} wpm):");

    let speak = |label: &str| -> std::io::Result<()> {
        let start = std::time::Instant::now();
        print!("         {label}: spawn ... ");
        let mut cmd = std::process::Command::new("/usr/bin/say");
        if let Some(v) = &pick {
            cmd.arg("-v").arg(v);
        }
        cmd.arg("-r").arg(wpm.to_string());
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(text.as_bytes())?;
        }
        let status = child.wait()?;
        println!(
            "exit={} elapsed={:.2}s",
            status.code().unwrap_or(-1),
            start.elapsed().as_secs_f32(),
        );
        Ok(())
    };
    if let Err(e) = speak("round 1") {
        eprintln!("         round 1 spawn FAIL: {e}");
        return Err(crate::error::Error::Config(
            "TTS subprocess failed".into(),
        ));
    }
    if let Err(e) = speak("round 2") {
        eprintln!("         round 2 spawn FAIL: {e}");
        return Err(crate::error::Error::Config(
            "TTS subprocess failed".into(),
        ));
    }

    println!();
    println!("Both rounds should have produced audible audio.");
    println!("If round 1 played but round 2 didn't, the issue is");
    println!("audio device state — try System Settings → Sound →");
    println!("Output and verify the active device.");
    Ok(())
}

pub fn run(project: &Path) -> Result<()> {
    println!("inkhaven doctor — v{}", env!("CARGO_PKG_VERSION"));
    println!("================================================================");
    println!();

    // ── Binary ────────────────────────────────────────────────
    section("Binary");
    kv("version", env!("CARGO_PKG_VERSION"));
    kv("description", env!("CARGO_PKG_DESCRIPTION"));
    kv("rust-version (min)", env!("CARGO_PKG_RUST_VERSION"));
    kv("repository", env!("CARGO_PKG_REPOSITORY"));
    println!();

    // The engine summary needs a config. Outside-of-project
    // contexts get the compiled defaults; inside-project gets
    // the user's HJSON. Either way the report is meaningful.
    let layout = ProjectLayout::new(project);
    let cfg_opt = if layout.is_initialized() {
        match Config::load(&layout.config_path()) {
            Ok(c) => Some(c),
            Err(e) => {
                println!("  config-load: ERROR {e}");
                None
            }
        }
    } else {
        None
    };
    let cfg_for_engine = cfg_opt.clone().unwrap_or_default();

    section("Typst engine");
    kv("engine", &crate::typst_compile::engine_summary(&cfg_for_engine));
    kv(
        "external typst path",
        &crate::typst_compile::typst_external_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "NOT FOUND on PATH".to_owned()),
    );
    kv(
        "bundle_fonts (HJSON)",
        &cfg_for_engine.typst_compile.bundle_fonts.to_string(),
    );
    kv(
        "use_system_fonts (HJSON)",
        &cfg_for_engine.typst_compile.use_system_fonts.to_string(),
    );
    kv(
        "packages_enabled (HJSON)",
        &cfg_for_engine.typst_compile.packages_enabled.to_string(),
    );
    kv(
        "semantic_diagnostics (HJSON)",
        &cfg_for_engine.typst_compile.semantic_diagnostics.to_string(),
    );
    println!();

    section("Package cache");
    if let Some(cache) = default_package_cache_path() {
        kv("path", &cache.display().to_string());
        let (count, bytes) = dir_count_size(&cache);
        kv("entries", &count.to_string());
        kv("size", &humanise_bytes(bytes));
    } else {
        println!("  (platform default unknown — no `cache_dir`)");
    }
    println!();

    // ── Project ───────────────────────────────────────────────
    section("Project");
    kv("root", &project.display().to_string());
    if !layout.is_initialized() {
        println!("  status: not an inkhaven project (no inkhaven.hjson)");
        println!();
    } else {
        kv("status", "initialised");
        kv("config", &layout.config_path().display().to_string());
        kv("metadata.db", &layout.metadata_db_path().display().to_string());
        kv("vectors/", &layout.vecstore_path().display().to_string());
        kv("books/", &layout.books_path().display().to_string());

        // Walk the hierarchy — paragraph + word counts. Word
        // counts come from each node's persisted `word_count`
        // field (kept fresh by save / reindex), so this is a
        // pure metadata read; no need to slurp file bodies.
        if let Some(cfg) = cfg_opt.as_ref() {
            match Store::open(layout.clone(), cfg) {
                Ok(store) => match Hierarchy::load(&store) {
                    Ok(hierarchy) => {
                        let mut paragraphs = 0usize;
                        let mut user_paragraphs = 0usize;
                        let mut user_books = 0usize;
                        let mut system_books = 0usize;
                        let mut total_words: u64 = 0;
                        for (n, _) in hierarchy.flatten() {
                            match n.kind {
                                NodeKind::Book => {
                                    if n.system_tag.is_some() {
                                        system_books += 1;
                                    } else {
                                        user_books += 1;
                                    }
                                }
                                NodeKind::Paragraph => {
                                    paragraphs += 1;
                                    let in_system = hierarchy
                                        .ancestors(n)
                                        .into_iter()
                                        .any(|a| {
                                            a.kind == NodeKind::Book
                                                && a.system_tag.is_some()
                                        });
                                    if !in_system {
                                        user_paragraphs += 1;
                                        total_words += n.word_count;
                                    }
                                }
                                _ => {}
                            }
                        }
                        kv("user books", &user_books.to_string());
                        kv("system books", &system_books.to_string());
                        kv("paragraphs (total)", &paragraphs.to_string());
                        kv("paragraphs (user)", &user_paragraphs.to_string());
                        kv(
                            "words (user paragraphs)",
                            &total_words.to_string(),
                        );
                    }
                    Err(e) => {
                        println!("  hierarchy: ERROR {e}");
                    }
                },
                Err(e) => {
                    println!("  store: ERROR {e}");
                }
            }
        } else {
            println!("  (no config loaded — skipping hierarchy walk)");
        }
        println!();
    }

    // ── Notes ─────────────────────────────────────────────────
    section("Notes");
    let mut notes: Vec<String> = Vec::new();
    if !cfg_for_engine.typst_compile.use_inprocess_engine()
        && crate::typst_compile::typst_external_path().is_none()
    {
        notes.push(
            "engine = external but `typst` is NOT on PATH — Ctrl+B B / O will fail. \
             Install Typst from typst.app/docs/install, or set \
             typst_compile.engine = \"inprocess\" in inkhaven.hjson."
                .into(),
        );
    }
    if !cfg_for_engine.typst_compile.bundle_fonts
        && !cfg_for_engine.typst_compile.use_system_fonts
    {
        notes.push(
            "engine has BOTH bundle_fonts AND use_system_fonts disabled — the in-process \
             compiler will report `font not found` for every manuscript. Enable one of them."
                .into(),
        );
    }
    if notes.is_empty() {
        println!("  no warnings");
    } else {
        for n in notes {
            // Soft-wrap at 76 cols so paragraph hints stay readable on
            // an 80-col terminal.
            print_wrapped("  ⚠ ", "    ", &n, 76);
        }
    }
    println!();

    Ok(())
}

fn section(title: &str) {
    println!("─── {title} ───");
}

fn kv(key: &str, value: &str) {
    println!("  {key:32} {value}");
}

fn print_wrapped(first_prefix: &str, cont_prefix: &str, text: &str, width: usize) {
    let mut current = String::new();
    let mut first_line = true;
    for word in text.split_whitespace() {
        let prefix_len = if first_line {
            first_prefix.len()
        } else {
            cont_prefix.len()
        };
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() + prefix_len <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            if first_line {
                println!("{first_prefix}{current}");
                first_line = false;
            } else {
                println!("{cont_prefix}{current}");
            }
            current = word.to_owned();
        }
    }
    if !current.is_empty() {
        if first_line {
            println!("{first_prefix}{current}");
        } else {
            println!("{cont_prefix}{current}");
        }
    }
}

fn default_package_cache_path() -> Option<PathBuf> {
    typst_kit::package::default_package_cache_path()
}

fn dir_count_size(root: &Path) -> (u64, u64) {
    if !root.is_dir() {
        return (0, 0);
    }
    let mut count = 0u64;
    let mut bytes = 0u64;
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let it = match std::fs::read_dir(&d) {
            Ok(it) => it,
            Err(_) => continue,
        };
        for entry in it.flatten() {
            let path = entry.path();
            match entry.metadata() {
                Ok(m) if m.is_file() => {
                    count += 1;
                    bytes += m.len();
                }
                Ok(m) if m.is_dir() => {
                    stack.push(path);
                }
                _ => {}
            }
        }
    }
    (count, bytes)
}

fn humanise_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut idx = 0;
    while value >= 1024.0 && idx + 1 < UNITS.len() {
        value /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[idx])
    }
}
