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
    let engine = match tts::Tts::default() {
        Ok(e) => e,
        Err(err) => {
            eprintln!(
                "TTS engine unavailable on this host: {err}\n\
                 \n\
                 Platform notes:\n  \
                 · macOS:   voices ship with the OS; ensure System Settings → Accessibility → Spoken Content has voices downloaded.\n  \
                 · Linux:   install `speech-dispatcher` (`apt install speech-dispatcher`); configure speechd to use RHVoice / piper for natural Russian.\n  \
                 · Windows: voices ship with the OS; nothing to do."
            );
            return Err(crate::error::Error::Config(
                "TTS engine init failed".into(),
            ));
        }
    };
    let voices = engine.voices().unwrap_or_default();
    if voices.is_empty() {
        eprintln!(
            "No TTS voices installed.  On macOS / Windows, install voices through System Settings.  On Linux, ensure your speech-dispatcher backend has voices configured."
        );
        return Ok(());
    }
    // Stable ordering for diff-friendly output: by language
    // then by name.  Each voice ID is platform-specific so
    // we skip it from the rendered output; users pick by
    // name (substring match against `editor.tts.voice`).
    let mut rows: Vec<(String, String, String)> = voices
        .iter()
        .map(|v| {
            let lang = v.language().to_string();
            let gender = match v.gender() {
                Some(tts::Gender::Male) => "male".to_string(),
                Some(tts::Gender::Female) => "female".to_string(),
                None => "—".to_string(),
            };
            (v.name(), lang, gender)
        })
        .collect();
    rows.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    let max_name = rows.iter().map(|(n, _, _)| n.chars().count()).max().unwrap_or(0);
    let max_lang = rows.iter().map(|(_, l, _)| l.chars().count()).max().unwrap_or(0);
    println!(
        "{:name_w$}  {:lang_w$}  gender",
        "name",
        "language",
        name_w = max_name,
        lang_w = max_lang,
    );
    println!(
        "{}  {}  ------",
        "-".repeat(max_name),
        "-".repeat(max_lang),
    );
    for (name, lang, gender) in &rows {
        println!(
            "{:name_w$}  {:lang_w$}  {}",
            name,
            lang,
            gender,
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

    // Load config (best-effort — we want the test to
    // work outside a real project too).  Config::load
    // takes the FILE PATH (inkhaven.hjson), not the
    // project root, so we resolve here.
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

    // Engine init.
    print!("[1/4] init engine ... ");
    let mut engine = match tts::Tts::default() {
        Ok(e) => {
            println!("OK");
            e
        }
        Err(err) => {
            println!("FAIL");
            eprintln!("\nEngine init error: {err}");
            return Err(crate::error::Error::Config(
                "TTS engine init failed".into(),
            ));
        }
    };

    // Voice selection.
    print!("[2/4] pick voice ... ");
    let voices = engine.voices().unwrap_or_default();
    let needle = tts_cfg.voice.to_lowercase();
    let picked = voices
        .iter()
        .filter(|v| v.name().to_lowercase().contains(&needle))
        .max_by_key(|v| {
            let n = v.name().to_lowercase();
            let enhanced = n.contains("enhanced") || n.contains("premium");
            (enhanced as u8, v.name().len() as isize)
        });
    match picked {
        Some(v) => {
            print!("{} — set_voice ... ", v.name());
            match engine.set_voice(v) {
                Ok(_) => println!("OK"),
                Err(e) => println!("set_voice FAIL: {e:?}"),
            }
        }
        None => {
            println!("no match for {:?} — using engine default", tts_cfg.voice);
        }
    }

    // Rate.
    print!("[3/4] set rate ... ");
    let speed = tts_cfg.speed.max(0.1);
    let target = (engine.normal_rate() * speed)
        .clamp(engine.min_rate(), engine.max_rate());
    println!(
        "normal={:.3} target={:.3} (clamped to [{:.3}, {:.3}])",
        engine.normal_rate(),
        target,
        engine.min_rate(),
        engine.max_rate(),
    );
    let _ = engine.set_rate(target);

    // Helper closure: speak, wait min_hold, poll until idle.
    let speak_and_wait = |engine: &mut tts::Tts,
                          label: &str,
                          payload: &str|
     -> bool {
        let start = std::time::Instant::now();
        print!("         {label}: speak ... ");
        let result = engine.speak(payload.to_string(), true);
        match &result {
            Ok(id) => println!("Ok({:?})", id),
            Err(e) => {
                println!("FAIL: {e:?}");
                return false;
            }
        }
        let chars = payload.chars().count() as u64;
        let min_hold_ms = (400 + chars * 80).min(10_000);
        std::thread::sleep(std::time::Duration::from_millis(min_hold_ms));
        let hard = start + std::time::Duration::from_secs(10);
        let mut polls = 0;
        while std::time::Instant::now() < hard {
            polls += 1;
            match engine.is_speaking() {
                Ok(false) => break,
                Ok(true) => {}
                Err(_) => break,
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        println!(
            "         {label}: elapsed={:.2}s polls={polls}",
            start.elapsed().as_secs_f32(),
        );
        true
    };

    // [4/4] First speak — fresh engine.
    println!("[4/4] speak (fresh engine):");
    if !speak_and_wait(&mut engine, "round 1", text) {
        return Err(crate::error::Error::Config(
            "TTS speak failed".into(),
        ));
    }

    // [5/5] Second speak — REUSE the same engine.  This
    // mirrors what the TUI does: greeting at startup
    // uses the engine, then Ctrl+B S / goodbye reuse it.
    // If audio plays on round 1 but not round 2, the
    // bug is engine reuse on this platform.
    println!("[5/5] speak (reused engine):");
    let _ = speak_and_wait(&mut engine, "round 2", text);

    // Round 3 — drop the previous engine, init fresh,
    // speak again.  If audio plays here when round 2
    // didn't, the platform's AVFoundation backend doesn't
    // tolerate back-to-back speak() calls on the same Tts
    // instance — fix is to drop+recreate the engine per
    // call in the TUI path.
    drop(engine);
    println!("[6/6] speak (fresh engine, third try):");
    let mut engine3 = match tts::Tts::default() {
        Ok(e) => e,
        Err(err) => {
            println!("         re-init FAIL: {err}");
            return Ok(());
        }
    };
    if let Some(v) = engine3
        .voices()
        .unwrap_or_default()
        .iter()
        .filter(|v| {
            v.name()
                .to_lowercase()
                .contains(&tts_cfg.voice.to_lowercase())
        })
        .max_by_key(|v| {
            let n = v.name().to_lowercase();
            let enhanced = n.contains("enhanced") || n.contains("premium");
            (enhanced as u8, v.name().len() as isize)
        })
    {
        let _ = engine3.set_voice(v);
    }
    let _ = engine3.set_rate(target);
    let _ = speak_and_wait(&mut engine3, "round 3", text);
    println!();
    println!("If you heard NO audio during the run above, the engine");
    println!("path is broken on this host.  Common causes:");
    println!("  - macOS: voice not yet downloaded.  Open System Settings");
    println!("    → Accessibility → Spoken Content → System Voice → Manage");
    println!("    Voices and ensure the language for {:?} is installed.",
        tts_cfg.voice,
    );
    println!("  - macOS: audio output muted or routed to a sink that's");
    println!("    not playing (HDMI, headphones unplugged but still");
    println!("    selected, etc.).");
    println!("  - Linux: speech-dispatcher running but no audio output");
    println!("    backend configured.");
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
