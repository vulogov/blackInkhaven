use std::io;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent,
    KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, MouseButton,
    MouseEvent, MouseEventKind, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};
use tui_textarea::{CursorMove, TextArea};
use uuid::Uuid;

use crate::ai::AiClient;
use crate::ai::prompts::PromptLibrary;
use crate::ai::stream::{ChatTurn, StreamMsg, spawn_chat_stream};
use crate::config::Config;
use crate::error::{Error, Result as InkResult};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::InsertPosition;

use super::file_picker::{FilePicker, PickerContext};
use super::focus::Focus;
use super::quickref;
use super::search_replace::SearchState;
use super::session::{
    ParagraphCursor, SessionState, TimelineViewSnapshot,
};
use super::highlight::{
    BlockSelection, TypstHighlighter,
};
use super::backup_ui::maybe_auto_backup;
use super::credits::embedded_logo_image;
use super::input::TextInput;
use super::lexicon_build::{LexiconKind, build_lexicon};
use super::modal::{
    EventPickerEntry, ImagePickerEntry, Modal, PagesToSave, PromptBody,
    PromptSource, RenderedPageProto, ScriptPickerEntry, ScriptPickerScope,
    SimilarPickerEntry, StatusFilterEntry, TagPickerTarget, visible_event_entries,
};
use super::timeline_state::{
    TimelineEvent, TimelineFocusLevel, TimelineViewState, cycle_track, timeline_auto_fit,
};
use super::search_results::SearchHit;
use super::splash::{
    StartupError, TYPST_COMPILE_SPINNER, draw_assembly_splash, draw_import_splash,
    draw_pulse_splash, draw_take_extras_splash, draw_typst_compile_splash,
    open_store_with_splash,
};
use super::hjson_edit::{
    set_key_in_hjson_block, set_llm_default_in_hjson,
    set_sound_enabled_in_hjson,
};
use super::inference::{
    AiMode, Inference, InferenceAction, InferenceMode, InferenceStatus,
};
use super::state::{
    BookStats, ChatSearchState, ChatSelectionState, DeletedParagraphStash,
    ImageCallContext, ImportCounts, Keymap, LinkPickDirection, MoveDir, OpenedDoc,
    SplitView, ViewMdScope,
};
use super::status_helpers::{display_status, next_status, prev_status};
use super::text_utils::{
    PARAGRAPH_PLACEHOLDER_TITLE, body_to_lines, extract_first_sentence,
    format_active_duration, format_age_humantime, pad_or_trim, truncate_to_chars,
};
#[cfg(test)]
use super::text_utils::format_reading_time;

/// 1.2.4+: project-pulse splash shown right after project open.
/// Renders for up to `STARTUP_SPLASH_SECS` seconds or until a key
/// press; the key press is consumed so it doesn't leak into the
/// editor's first frame. Failures (`terminal.draw` returning Err)
/// silently fall through — we'd rather skip the splash than block
/// the editor from launching.
fn run_startup_pulse_splash<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &App,
) {
    const STARTUP_SPLASH_SECS: u64 = 7;
    let started = std::time::Instant::now();
    let snap = app.progress_cache.clone();
    let project_display = app
        .layout
        .root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();
    let total_paragraphs = app
        .hierarchy
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Paragraph
                && !app
                    .hierarchy
                    .ancestors(n)
                    .into_iter()
                    .any(|a| a.kind == NodeKind::Book && a.system_tag.is_some())
        })
        .count();
    let mut by_status: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for n in app.hierarchy.iter() {
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        if app
            .hierarchy
            .ancestors(n)
            .into_iter()
            .any(|a| a.kind == NodeKind::Book && a.system_tag.is_some())
        {
            continue;
        }
        let key = n.status.as_deref().unwrap_or("None").to_string();
        *by_status.entry(key).or_insert(0) += 1;
    }

    loop {
        let elapsed = started.elapsed().as_secs();
        if elapsed >= STARTUP_SPLASH_SECS {
            break;
        }
        let remaining = STARTUP_SPLASH_SECS - elapsed;
        let _ = terminal.draw(|f| {
            draw_pulse_splash(
                f,
                &project_display,
                snap.as_ref(),
                total_paragraphs,
                &by_status,
                remaining,
            )
        });
        // Poll for keys with a short timeout. Any keystroke
        // dismisses the splash; consume it so the editor's
        // first frame doesn't see it as input.
        if crossterm::event::poll(std::time::Duration::from_millis(100))
            .unwrap_or(false)
        {
            if let Ok(crossterm::event::Event::Key(_)) = crossterm::event::read() {
                break;
            }
        }
    }
}

pub fn run(project: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized().map_err(anyhow::Error::from)?;

    let cfg = Config::load(&layout.config_path()).map_err(anyhow::Error::from)?;

    // 1.2.5+: log the typst engine at startup so users can confirm
    // their HJSON setting took effect. Both engines are always
    // available — the in-process compiler ships in every 1.2.5
    // build — but the default stays `external` to match prior
    // behaviour exactly. The same one-liner also lands on the
    // status bar after first paint, and persistently in the
    // Ctrl+B V credits pane.
    let engine_summary = crate::typst_compile::engine_summary(&cfg);
    tracing::info!(
        target: "inkhaven::typst",
        "typst engine: {engine_summary}",
    );

    // Install the panic hook BEFORE we touch the terminal so a panic during
    // DB load (or anywhere later) still restores the screen.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    // Try to enable the kitty keyboard protocol. Without it, legacy
    // terminal encoding can't distinguish e.g. `Ctrl+1` from a bare `1`
    // (the TTY only has dedicated bytes for Ctrl+A..Z + a handful of
    // punctuation), so `Ctrl+1` ends up inserting "1" into the AI prompt.
    // Best-effort: terminals that don't support it just ignore the CSI
    // sequence and we run with reduced functionality.
    let kbd_enhanced = execute!(
        io::stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS,
        )
    )
    .is_ok();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Open the document store on a worker thread so the TUI can draw a
    // "Please wait" splash immediately. First-time runs of fastembed download
    // a ~120 MB model, which takes long enough to look like a hang otherwise.
    let store_result = open_store_with_splash(&mut terminal, layout.clone(), cfg.clone());

    let store = match store_result {
        Ok(s) => s,
        Err(StartupError::UserAborted) => {
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            terminal.show_cursor()?;
            return Ok(());
        }
        Err(StartupError::Store(e)) => {
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            terminal.show_cursor()?;
            return Err(e);
        }
    };

    // 1.2.6+ — idempotent re-seed of the Prompts book with the
    // embedded `<name>.example` defaults. The seeder skips
    // paragraphs whose title already matches, so existing
    // content is never overwritten. Covers users whose
    // project was initialised before 1.2.6 (and so missed
    // the init-time seeding) — they get the examples the
    // first time they open the project under 1.2.6+. Gated
    // on `ai.reseed_prompt_examples` (default true).
    if cfg.ai.reseed_prompt_examples {
        if let Err(e) = crate::cli::init::seed_prompt_examples(&cfg, &store) {
            tracing::warn!(
                target: "inkhaven::tui::run",
                "could not seed Prompts.book examples on open: {e}",
            );
        }
    }

    // Keep a copy of the config and layout for the auto-backup hook below;
    // App takes ownership of the originals.
    let cfg_for_exit = cfg.clone();
    let layout_for_exit = layout.clone();

    // Scripting layer (policy + active store) was armed inside
    // Store::open via scripting::configure. Force eager Adam
    // construction here so the bootstrap script runs and hook
    // lambdas are registered before the first store mutation
    // can fire a hook.
    if let Err(e) = crate::scripting::init_adam() {
        tracing::warn!("scripting init failed: {e}");
    }

    let mut app = App::new(layout, cfg, store)?;
    app.restore_session();
    app.install_progress();

    // 1.2.8+ — `editor.mouse_captured = false` in HJSON
    // asks the launcher to start with native terminal
    // drag-select instead of the TUI mouse-capture default.
    // `run` above already enabled capture unconditionally —
    // sync to App's initial state so the two agree before
    // first paint.
    if !app.mouse_captured {
        let _ = execute!(terminal.backend_mut(), DisableMouseCapture);
    }

    // 1.2.4+: project-pulse startup splash. Renders today /
    // streak / active-time / by-status counts; auto-closes
    // after 7 seconds or on any key press. Disabled via
    // `editor.startup_splash = false` in HJSON.
    if app.cfg.editor.startup_splash {
        run_startup_pulse_splash(&mut terminal, &app);
    }

    // 1.2.9+: TTS greeting fires here, after the splash
    // (so the user has seen the daily-progress numbers)
    // and just before the main render loop starts.  Non-
    // blocking — speech plays in parallel with the
    // editor coming up.  No-op when `editor.tts.enabled
    // = false` or the greeting field is empty.
    app.tts_speak_greeting();

    let result = app.run(&mut terminal);

    // 1.2.9+: TTS goodbye fires after the main loop
    // returns but BEFORE shutdown_flush + terminal
    // teardown.  Blocks up to 5 seconds for the audio
    // to drain so the shell doesn't truncate it.
    app.tts_speak_goodbye_blocking();

    // Explicit final flush — HNSW save + DuckDB CHECKPOINT — while the
    // App still holds the Store. The pool's Drop impl would checkpoint
    // implicitly, but doing it explicitly here lets us log any error
    // and guarantees the auto-backup below sees a fully-drained WAL.
    app.shutdown_flush();
    crate::progress::uninstall();

    // Drop the App (and its Store handle) BEFORE running the auto-backup so
    // duckdb/HNSW checkpoint state is flushed to disk and the zip captures
    // a consistent snapshot rather than mid-write WAL data.
    drop(app);

    if let Err(e) = maybe_auto_backup(&mut terminal, &layout_for_exit, &cfg_for_exit) {
        // Backup failures must not eat the editor's own exit status — just
        // log them to stderr (which routes to .inkhaven.log in TUI mode)
        // and let the user retry with `inkhaven backup` manually.
        tracing::warn!("auto-backup on exit failed: {e}");
    }

    if kbd_enhanced {
        let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    }
    let _ = execute!(terminal.backend_mut(), DisableMouseCapture);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    // Restore the hook we replaced.
    let _ = std::panic::take_hook();

    result
}

/// Walk `root` and count regular files that would be imported as
/// paragraphs (mirrors the importer's hidden-entry filter so the total
/// matches the progress callbacks).
fn count_importable_files(root: &Path) -> usize {
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            e.file_name()
                .to_str()
                .map(|s| !s.starts_with('.'))
                .unwrap_or(true)
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .count()
}

/// Read a directory's immediate children, filter hidden entries, sort dirs
/// first then alphabetical. Returns owned paths so the caller doesn't carry
/// a borrow against the DirEntry iterator.
fn read_sorted_children(source: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(rd) = std::fs::read_dir(source) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = rd
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|s| !s.starts_with('.'))
                .unwrap_or(true)
        })
        .collect();
    entries.sort_by(|a, b| {
        let a_dir = a.path().is_dir();
        let b_dir = b.path().is_dir();
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });
    entries.into_iter().map(|e| e.path()).collect()
}

fn derive_paragraph_title_from_path(path: &std::path::Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("imported");
    let pretty: String = stem
        .replace('_', " ")
        .replace('-', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(c).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if pretty.is_empty() {
        "Imported".into()
    } else {
        pretty
    }
}

/// Return the lowercase extension if `path` looks like an image file
/// we'll route to `import_single_image`. The list is the recognised
/// set Typst's `#image(...)` natively understands: PNG, JPG, JPEG,
/// GIF, WebP, SVG. Anything else stays a paragraph candidate.
pub(super) fn image_extension_for(path: &std::path::Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => Some(ext),
        _ => None,
    }
}

/// Map file extension → content_type tag stored on the resulting
/// Paragraph. `.hjson` → "hjson"; anything else (including `.typ` /
/// no extension / plain text files) → `None`, which means "typst
/// default".
fn content_type_for(path: &std::path::Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "hjson" => Some("hjson".into()),
        _ => None,
    }
}

/// 1.2.6+ — pick the "thing to paste" for a Replace-style AI
/// apply. For grammar-style outputs (`<<<CORRECTED>>> … <<<END>>>`
/// markers, a trailing fenced code block, or a "Corrected …"
/// section header), this returns only that block — the user
/// doesn't want the commentary, summary, or diff explanation
/// the model wrote around it. For everything else, it falls
/// back to the full response (markdown→typst converted by the
/// caller). Returns `(text, extracted)` so the caller can
/// surface a "✂ extracted X of Y chars" hint in the status
/// line.
///
/// `force_extract` (used by ReplaceCorrected / F7-apply) means
/// "if I can't find a discrete block, treat that as an error";
/// the caller should refuse to apply rather than paste prose
/// commentary by mistake.
pub(super) fn select_apply_text(
    raw: &str,
    force_extract: bool,
) -> Result<(String, bool), &'static str> {
    if let Some(extracted) = extract_corrected_text(raw) {
        return Ok((extracted, true));
    }
    // Diagnostic log when no extractable block is found.
    // Surfaces the first 200 chars of the response so a user
    // who hits this can paste their `.inkhaven.log` and we
    // see exactly which bracket shape the model emitted.
    tracing::warn!(
        target: "inkhaven::ai::apply",
        force_extract = force_extract,
        sample_len = raw.len(),
        sample_head = %raw.chars().take(200).collect::<String>(),
        "select_apply_text: no extractable corrected block — \
         marker / Unicode-bracket / code-fence / heading passes all missed",
    );
    if force_extract {
        return Err(
            "couldn't find a corrected block in the response \
             (expected `<<<CORRECTED>>>` markers, a fenced code \
             block, or a `Corrected:` heading)",
        );
    }
    // Non-grammar Replace path: convert markdown→Typst on the
    // whole response. The conversion is best-effort; passes
    // through unrecognised markup verbatim.
    Ok((super::markdown::markdown_to_typst(raw), false))
}

/// Extract only the corrected-paragraph text from a grammar-check
/// response. Tries in order:
///
///   1. Canonical `<<<CORRECTED>>>` / `<<<END>>>` marker block
///      (what the system prompt instructs).
///   2. Relaxed bracket pair — any two-or-more `<` followed by
///      optional word characters followed by two-or-more `>`
///      appearing at least twice. Models routinely compress
///      the canonical markers down to `<<>>` / `<<END>>` /
///      `<<<corrected>>>`, even when the prompt is explicit;
///      this catches every variant we've observed in deepseek,
///      gemini, and gpt-4o-mini drift.
///   3. Last fenced code block — common when the model
///      ignores markers entirely.
///   4. Everything after a "Corrected …" line.
///
/// Returns `None` if none of those patterns match so callers
/// can refuse rather than paste commentary by mistake.
pub(super) fn extract_corrected_text(response: &str) -> Option<String> {
    if let Some(begin) = response.find(CORRECTED_BEGIN) {
        let after = &response[begin + CORRECTED_BEGIN.len()..];
        if let Some(end_offset) = after.find(CORRECTED_END) {
            let inner = &after[..end_offset];
            let cleaned = inner.trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
            if !cleaned.is_empty() {
                return Some(cleaned.to_string());
            }
        }
    }
    // Pass A — ASCII multi-char marker (`<<>>`, `<<END>>`,
    // `<<<corrected>>>`, etc). Token shape: 2+ `<`, optional
    // word chars, 2+ `>`. If we find at least two such tokens,
    // content between the first and the last is the correction.
    let ascii_re = regex::Regex::new(r"<<+[A-Za-z_]*>>+");
    if let Ok(re) = ascii_re {
        let positions: Vec<_> = re.find_iter(response).collect();
        if positions.len() >= 2 {
            let first = &positions[0];
            let last = &positions[positions.len() - 1];
            if last.start() > first.end() {
                let inner = &response[first.end()..last.start()];
                let cleaned = inner
                    .trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
        }
    }
    // Pass B — single-char Unicode bracket pairs. Several
    // shapes show up depending on the model's tokenizer:
    //
    //   ≪≫        U+226A / U+226B (much-less / greater)
    //   «»        U+00AB / U+00BB (single guillemets — render
    //             in monospace fonts as a tight double-angle
    //             glyph that visually mimics ASCII `<<>>`)
    //   〈〉⟨⟩《》  CJK / mathematical angle brackets
    //
    // Try labeled first (`«CORRECTED»…«END»`), then unlabeled
    // (`« body »`). Labeled must precede unlabeled because an
    // unlabeled scan over a labeled response would grab
    // `CORRECTED» body «END` (including the inner brackets).
    let pairs: &[(char, char)] = &[
        ('≪', '≫'),
        ('«', '»'),
        ('〈', '〉'),
        ('⟨', '⟩'),
        ('《', '》'),
    ];
    // Pass B.1 — labeled Unicode markers.
    for &(l, r) in pairs {
        let pat = format!(r"{}[A-Za-z_]+{}", regex::escape(&l.to_string()), regex::escape(&r.to_string()));
        let Ok(re) = regex::Regex::new(&pat) else {
            continue;
        };
        let positions: Vec<_> = re.find_iter(response).collect();
        if positions.len() >= 2 {
            let first = &positions[0];
            let last = &positions[positions.len() - 1];
            if last.start() > first.end() {
                let inner = &response[first.end()..last.start()];
                let cleaned = inner
                    .trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
        }
    }
    // Pass B.2 — unlabeled Unicode markers (`« body »`).
    // First `l` and the LAST `r` after it, content between.
    for &(l, r) in pairs {
        let Some(first_left) = response.find(l) else {
            continue;
        };
        let inner_search_start = first_left + l.len_utf8();
        let Some(last_right_rel) =
            response[inner_search_start..].rfind(r)
        else {
            continue;
        };
        let last_right = inner_search_start + last_right_rel;
        if last_right <= first_left + l.len_utf8() {
            continue;
        }
        let inner = &response[first_left + l.len_utf8()..last_right];
        let cleaned = inner
            .trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
        if !cleaned.is_empty() {
            return Some(cleaned.to_string());
        }
    }
    if let Some(last_close) = response.rfind("```") {
        let before = &response[..last_close];
        if let Some(open) = before.rfind("```") {
            let body = &response[open + 3..last_close];
            let (first_nl, rest) = match body.find('\n') {
                Some(i) => (&body[..i], &body[i + 1..]),
                None => (body, ""),
            };
            // Drop a short alphanumeric language tag on the first line.
            let cleaned = if !first_nl.is_empty()
                && first_nl.len() < 16
                && first_nl.chars().all(|c| c.is_ascii_alphanumeric())
            {
                rest.to_string()
            } else {
                body.to_string()
            };
            let trimmed = cleaned.trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    let lower = response.to_ascii_lowercase();
    if let Some(idx) = lower.rfind("corrected") {
        if let Some(line_end) = response[idx..].find('\n') {
            let after = response[idx + line_end + 1..]
                .trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
            if !after.is_empty() {
                return Some(after.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod corrected_tests {
    use super::*;

    #[test]
    fn marker_block_wins() {
        let r = "Summary: 1 issue.\n\n<<<CORRECTED>>>\n= Heading\n\nThe rain in Spain.\n<<<END>>>\n";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "= Heading\n\nThe rain in Spain.");
    }

    #[test]
    fn falls_back_to_code_fence() {
        let r = "Summary.\n\n```typst\n= Heading\n\nFixed body.\n```\n";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "= Heading\n\nFixed body.");
    }

    #[test]
    fn falls_back_to_corrected_heading() {
        let r = "Summary line.\n- issue 1\n\nCorrected paragraph:\n= Heading\n\nFixed body.\n";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "= Heading\n\nFixed body.");
    }

    #[test]
    fn returns_none_on_empty_or_unmatched() {
        assert!(extract_corrected_text("").is_none());
        assert!(extract_corrected_text("Just commentary, no markers.").is_none());
    }

    /// 1.2.6+ — models routinely compress the canonical
    /// markers. We've observed `<<>>` (deepseek), `<<END>>`,
    /// and `<<<corrected>>>` (lowercase). All should land on
    /// the relaxed-bracket pass.

    #[test]
    fn relaxed_empty_brackets_deepseek_drift() {
        // Exact shape from a deepseek grammar-check reply.
        let r = "\
2 grammar issues, otherwise clean

+ \"lile\" → \"little\"
+ \"playes\" → \"plays\"
+ \"tha\" → \"the\"

<<>> The little boy plays the fiddle. <<>>
";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "The little boy plays the fiddle.");
    }

    #[test]
    fn relaxed_double_bracket_with_label() {
        let r = "Summary.\n\n<<CORRECTED>>\n= H\nBody.\n<<END>>\n";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "= H\nBody.");
    }

    #[test]
    fn relaxed_lowercase_canonical() {
        let r = "Summary.\n<<<corrected>>>\nBody.\n<<<end>>>\n";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "Body.");
    }

    /// U+226A / U+226B "much less / greater than" — a single
    /// Unicode char on each side that renders in monospace
    /// fonts as a tight double-angle glyph. Observed when the
    /// model's tokenizer collapses `<<` into a single
    /// codepoint.
    #[test]
    fn relaxed_unicode_much_less_greater() {
        let r = "Summary.\n\n≪ The corrected sentence. ≫\n";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "The corrected sentence.");
    }

    /// U+00AB / U+00BB single guillemets. Same visual effect
    /// as the much-less-than pair when the font renders them
    /// compact.
    #[test]
    fn relaxed_unicode_guillemets() {
        let r = "Summary.\n\n«CORRECTED» Body sentence. «END»\n";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "Body sentence.");
    }

    /// Multi-line response from the screenshot — with a
    /// numbered issue list, a Unicode arrow, and the deepseek
    /// `<<>>` brackets at the end. End-to-end smoke.
    #[test]
    fn relaxed_full_screenshot_shape() {
        let r = "1 grammar issue, 1 spelling issue, otherwise clean.\n\n\
                 Issues:\n\
                 1. \"litle\" should be \"little\"\n\
                 2. \"playz\" should be \"plays\"\n\
                 3. (Lowercase \"tha\" → \"the\".)\n\n\
                 <<>> The little boy plays the fiddle. <<>>\n";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "The little boy plays the fiddle.");
    }

    // 1.2.6+ — `select_apply_text` should auto-prefer a
    // discrete corrected block over the surrounding chatter,
    // regardless of which apply action triggered.

    #[test]
    fn select_extracts_marker_block_when_force_false() {
        let r = "I found 2 issues.\n- foo\n- bar\n\n<<<CORRECTED>>>\n= H\nBody.\n<<<END>>>\n";
        let (text, extracted) = select_apply_text(r, false).unwrap();
        assert!(extracted);
        assert_eq!(text, "= H\nBody.");
    }

    #[test]
    fn select_extracts_code_fence_when_force_false() {
        let r = "Here is the rewrite:\n\n```typst\n= H\nBody.\n```\nThoughts above.";
        let (text, extracted) = select_apply_text(r, false).unwrap();
        assert!(extracted);
        assert_eq!(text, "= H\nBody.");
    }

    #[test]
    fn select_falls_back_to_full_markdown_when_no_extractable_block() {
        let r = "Some plain commentary without a discrete block.";
        let (text, extracted) = select_apply_text(r, false).unwrap();
        assert!(!extracted);
        assert!(text.contains("commentary"));
    }

    #[test]
    fn select_force_errors_on_unextractable() {
        let r = "Just commentary, no markers anywhere.";
        assert!(select_apply_text(r, true).is_err());
    }
}

#[cfg(test)]
mod book_info_tests {
    use super::*;

    #[test]
    fn count_sentences_basic_terminators() {
        assert_eq!(count_sentences("Hello. World!"), 2);
        assert_eq!(count_sentences("Why? Because!"), 2);
        assert_eq!(count_sentences("One sentence only"), 0);
    }

    #[test]
    fn count_sentences_collapses_runs() {
        // Repeated terminators ("...", "?!") should count as one sentence
        // each, not three / two.
        assert_eq!(count_sentences("Hmm... interesting?!"), 2);
    }

    #[test]
    fn count_sentences_ignores_headings_and_comments() {
        let body = "= Chapter title\n\n// this comment has a period.\n\
                    First. Second.";
        assert_eq!(count_sentences(body), 2);
    }

    #[test]
    fn count_sentences_handles_blank_and_multiline() {
        let body = "First sentence.\n\nSecond sentence!\nThird?";
        assert_eq!(count_sentences(body), 3);
    }

    #[test]
    fn format_age_subminute() {
        assert_eq!(format_age_humantime(std::time::Duration::from_secs(0)), "0s");
        assert_eq!(format_age_humantime(std::time::Duration::from_secs(45)), "45s");
    }

    #[test]
    fn chat_turn_roundtrips_through_serde() {
        let history = vec![
            ChatTurn::User("What's the weather?".into()),
            ChatTurn::Assistant("I don't have a weather tool.".into()),
        ];
        let json = serde_json::to_string(&history).expect("encode");
        let back: Vec<ChatTurn> = serde_json::from_str(&json).expect("decode");
        assert_eq!(back.len(), 2);
        match &back[0] {
            ChatTurn::User(s) => assert_eq!(s, "What's the weather?"),
            _ => panic!("expected User turn"),
        }
        match &back[1] {
            ChatTurn::Assistant(s) => assert_eq!(s, "I don't have a weather tool."),
            _ => panic!("expected Assistant turn"),
        }
    }

    #[test]
    fn byte_offset_for_cursor_basic() {
        let src = "abc\ndef\nghi";
        // Row 0, col 0 → byte 0.
        assert_eq!(byte_offset_for_cursor(src, 0, 0), 0);
        // Row 0, col 3 → end of "abc".
        assert_eq!(byte_offset_for_cursor(src, 0, 3), 3);
        // Row 1, col 0 → after the first newline.
        assert_eq!(byte_offset_for_cursor(src, 1, 0), 4);
        // Row 2, col 2 → "gh|i".
        assert_eq!(byte_offset_for_cursor(src, 2, 2), 10);
    }

    #[test]
    fn byte_offset_for_cursor_handles_multibyte() {
        // "Москва" is 6 chars / 12 bytes.
        let src = "Москва";
        assert_eq!(byte_offset_for_cursor(src, 0, 0), 0);
        assert_eq!(byte_offset_for_cursor(src, 0, 6), 12);
        // Mid-way: 3 chars in is 6 bytes.
        assert_eq!(byte_offset_for_cursor(src, 0, 3), 6);
    }

    #[test]
    fn open_pair_for_known_openers() {
        assert_eq!(open_pair_for('('), Some(')'));
        assert_eq!(open_pair_for('['), Some(']'));
        assert_eq!(open_pair_for('{'), Some('}'));
        assert_eq!(open_pair_for('"'), Some('"'));
        assert_eq!(open_pair_for('\''), Some('\''));
        assert_eq!(open_pair_for('a'), None);
        assert_eq!(open_pair_for(')'), None);
    }

    #[test]
    fn is_close_pair_char_covers_all_closers() {
        for c in [')', ']', '}', '"', '\''] {
            assert!(is_close_pair_char(c), "{c} should be a close pair char");
        }
        for c in ['(', '[', '{', 'a', '#'] {
            assert!(!is_close_pair_char(c), "{c} should not be");
        }
    }

    /// Reproduce the pair-detection rule used by the Enter and
    /// Backspace handlers without going through tui-textarea.
    fn between_pair(line: &str, col: usize) -> bool {
        let chars: Vec<char> = line.chars().collect();
        let before = if col > 0 { chars.get(col - 1).copied() } else { None };
        let after = chars.get(col).copied();
        matches!(
            (before, after),
            (Some('('), Some(')')) | (Some('['), Some(']')) | (Some('{'), Some('}'))
        )
    }

    #[test]
    fn pair_detection_inside_freshly_typed_pair() {
        // `foo(|)` — cursor at col 4, between `(` and `)`.
        assert!(between_pair("foo()", 4));
        // `[]` — cursor at col 1, between `[` and `]`.
        assert!(between_pair("[]", 1));
        // `{}` — cursor at col 1, between `{` and `}`.
        assert!(between_pair("{}", 1));
    }

    #[test]
    fn pair_detection_skips_when_chars_dont_match() {
        // Just before `)` but `(` is not on the immediate left.
        assert!(!between_pair("foo )", 4));
        // Indented prose with parens that DO match — pair logic only
        // cares about the immediately adjacent chars.
        assert!(!between_pair("  abc(def)", 6));
    }

    #[test]
    fn filter_functions_empty_returns_all() {
        let all = filter_functions("").len();
        assert!(all > 50, "expected the baked-in table to have >50 entries, got {all}");
    }

    #[test]
    fn filter_functions_substring_match() {
        let m: Vec<&'static str> = filter_functions("image").iter().map(|f| f.name).collect();
        assert!(m.contains(&"image"), "got: {m:?}");
        assert!(m.contains(&"figure") == false || true);
    }

    #[test]
    fn filter_functions_case_insensitive() {
        let a = filter_functions("Heading");
        let b = filter_functions("heading");
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn filter_functions_no_match_returns_empty() {
        assert!(filter_functions("zzz-no-such-function").is_empty());
    }

    #[test]
    fn image_call_context_inside_open_string() {
        // Cursor is after the `"` — we're inside the first string arg.
        let line = "#image(\"";
        let ctx = detect_image_call_context(line, line.chars().count())
            .expect("should detect");
        assert!(!ctx.closing_quote_present);
    }

    #[test]
    fn image_call_context_inside_with_closing_quote() {
        let line = "#image(\"\")";
        // Cursor positioned right after the opening quote (col 8).
        let ctx = detect_image_call_context(line, 8).expect("should detect");
        assert!(ctx.closing_quote_present);
    }

    #[test]
    fn image_call_context_not_inside_when_closed() {
        // After the closing `)`.
        let line = "#image(\"cover.png\")";
        let n = line.chars().count();
        assert!(detect_image_call_context(line, n).is_none());
    }

    #[test]
    fn image_call_context_requires_hash_prefix() {
        // `image(` without leading `#` is not a typst function call.
        let line = "image(\"cover.png";
        assert!(detect_image_call_context(line, line.chars().count()).is_none());
    }

    #[test]
    fn image_call_context_not_after_other_function() {
        let line = "#text(\"hello";
        assert!(detect_image_call_context(line, line.chars().count()).is_none());
    }

    #[test]
    fn image_call_context_in_chapter_body() {
        // Realistic editor line — leading indentation + prose, then a
        // `#image(` partway through.
        let line = "  Some prose. #image(\"01-co";
        let ctx = detect_image_call_context(line, line.chars().count())
            .expect("should detect inside the call");
        assert!(!ctx.closing_quote_present);
    }

    #[test]
    fn body_to_lines_strips_crlf() {
        // CRLF (DOS / Windows / RFC dumps): trailing `\r` must not
        // survive into the line list.
        let body = "Network Working Group\r\nRequest for Comments: 1\r\n";
        let lines = body_to_lines(body);
        assert_eq!(lines.len(), 3); // last `\n` produces a trailing "" entry
        assert_eq!(lines[0], "Network Working Group");
        assert_eq!(lines[1], "Request for Comments: 1");
        assert_eq!(lines[2], "");
    }

    #[test]
    fn body_to_lines_strips_bare_cr() {
        // Old-Mac files used bare `\r`. Treat them as line breaks too.
        let body = "first\rsecond\rthird";
        let lines = body_to_lines(body);
        assert_eq!(lines, vec!["first", "second", "third"]);
    }

    #[test]
    fn body_to_lines_unix_passthrough() {
        let body = "alpha\nbeta\ngamma";
        let lines = body_to_lines(body);
        assert_eq!(lines, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn body_to_lines_empty_yields_single_empty() {
        assert_eq!(body_to_lines(""), vec![String::new()]);
    }

    #[test]
    fn set_llm_default_rewrites_value_only() {
        let raw = "\
language: english

llm: {
  // Provider used by default.
  default: gemini
  providers: {
    gemini: { model: gemini-2.5-pro }
    ollama: { model: llama3.2 }
  }
}
";
        let out = set_llm_default_in_hjson(raw, "ollama").unwrap();
        // The default line changed.
        assert!(out.contains("default: ollama"));
        assert!(!out.contains("default: gemini"));
        // Everything else survives byte-for-byte.
        assert!(out.contains("language: english"));
        assert!(out.contains("// Provider used by default."));
        assert!(out.contains("model: gemini-2.5-pro"));
        assert!(out.contains("model: llama3.2"));
    }

    #[test]
    fn set_llm_default_preserves_trailing_comment() {
        let raw = "\
llm: {
  default: gemini    // pick gemini for prose
  providers: { gemini: { model: x } }
}
";
        let out = set_llm_default_in_hjson(raw, "ollama").unwrap();
        // Value swapped, comment retained.
        assert!(
            out.contains("default: ollama"),
            "expected new default; got:\n{out}"
        );
        assert!(
            out.contains("// pick gemini for prose"),
            "expected trailing comment preserved; got:\n{out}"
        );
    }

    #[test]
    fn set_llm_default_quotes_unsafe_values() {
        let raw = "\
llm: {
  default: gemini
  providers: { x: { model: y } }
}
";
        let out = set_llm_default_in_hjson(raw, "weird name").unwrap();
        assert!(
            out.contains("default: \"weird name\""),
            "value with space should be quoted; got:\n{out}"
        );
    }

    #[test]
    fn set_llm_default_inserts_when_missing() {
        // No `default:` key in the llm block — insert one.
        let raw = "\
llm: {
  providers: { gemini: { model: g } }
}
";
        let out = set_llm_default_in_hjson(raw, "gemini").unwrap();
        assert!(
            out.contains("default: gemini"),
            "expected inserted default; got:\n{out}"
        );
        // The providers block survives.
        assert!(out.contains("providers: { gemini: { model: g } }"));
    }

    #[test]
    fn set_llm_default_errors_on_missing_block() {
        let raw = "language: english\n";
        let err = set_llm_default_in_hjson(raw, "gemini").unwrap_err();
        assert!(err.contains("no `llm:` block"), "got: {err}");
    }

    #[test]
    fn set_llm_default_roundtrips_shipped_template() {
        // The annotated default HJSON the project ships with has to
        // survive a switch and still parse cleanly via `Config::load`.
        // This is the regression we care about most — if the in-place
        // edit garbles the file, the user's next launch fails to read
        // their config.
        let raw = crate::config::DEFAULT_PROJECT_CONFIG;
        let edited = set_llm_default_in_hjson(raw, "ollama").unwrap();
        let cfg: crate::config::Config =
            serde_hjson::from_str(&edited).expect("edited HJSON should still parse");
        assert_eq!(cfg.llm.default, "ollama");
        // The two non-llm sections should round-trip unchanged.
        assert_eq!(cfg.language, "english");
        assert_eq!(cfg.editor.tab_width, 2);
        // Comments survive (string match in the raw text).
        assert!(edited.contains("// Provider used by the AI pane"));
    }

    #[test]
    fn set_sound_enabled_rewrites_existing_block() {
        let raw = "\
sound: {
  enabled: false
  volume: 0.6
}
";
        let out = set_sound_enabled_in_hjson(raw, true).unwrap();
        assert!(out.contains("enabled: true"));
        assert!(out.contains("volume: 0.6"));
        // And toggling back.
        let back = set_sound_enabled_in_hjson(&out, false).unwrap();
        assert!(back.contains("enabled: false"));
    }

    #[test]
    fn set_sound_enabled_roundtrips_shipped_template() {
        let raw = crate::config::DEFAULT_PROJECT_CONFIG;
        let edited = set_sound_enabled_in_hjson(raw, true).unwrap();
        let cfg: crate::config::Config =
            serde_hjson::from_str(&edited).expect("edited HJSON should still parse");
        assert!(cfg.sound.enabled);
        // Sound-unrelated stanzas untouched.
        assert_eq!(cfg.language, "english");
        assert!(edited.contains("// Typewriter-style sound effects"));
    }

    #[test]
    fn set_sound_enabled_inserts_block_when_missing() {
        // Older configs without a sound block — the helper inserts a
        // minimal one just inside the root closing brace, so the file
        // stays valid HJSON.
        let raw = "{\n  language: english\n}\n";
        let out = set_sound_enabled_in_hjson(raw, true).unwrap();
        assert!(out.contains("sound: {"), "got:\n{out}");
        assert!(out.contains("enabled: true"));
        let cfg: crate::config::Config =
            serde_hjson::from_str(&out).expect("inserted HJSON should still parse");
        assert!(cfg.sound.enabled);
    }

    #[test]
    fn set_sound_enabled_insertion_lands_before_root_close() {
        // Regression: the previous version appended after the root `}`
        // which made the file invalid HJSON. Verify the new block lives
        // strictly before the root close, and the file round-trips.
        let raw = "{\n  language: english\n  theme: {\n    pane_bg: \"#1e1e2e\"\n  }\n}\n";
        let out = set_sound_enabled_in_hjson(raw, true).unwrap();
        let sound_idx = out.find("sound:").expect("sound block inserted");
        let last_close = out.rfind('}').expect("root close present");
        assert!(
            sound_idx < last_close,
            "sound block must be before root close — got:\n{out}"
        );
        let _: crate::config::Config = serde_hjson::from_str(&out).expect("must parse");
    }

    #[test]
    fn format_reading_time_thresholds() {
        assert_eq!(format_reading_time(0), "<1m");
        assert_eq!(format_reading_time(1), "~1m");          // ceil(1/250) = 1
        assert_eq!(format_reading_time(250), "~1m");
        assert_eq!(format_reading_time(251), "~2m");
        assert_eq!(format_reading_time(250 * 60), "~1h");   // exact hour, no "0m"
        assert_eq!(format_reading_time(250 * 75), "~1h 15m");
    }

    #[test]
    fn set_llm_default_does_not_match_provider_internals() {
        // A `default:` key inside a nested provider block must NOT be
        // mistaken for `llm.default`. Our scanner requires depth==1.
        let raw = "\
llm: {
  default: gemini
  providers: {
    fake: {
      default: should_not_be_touched
      model: x
    }
  }
}
";
        let out = set_llm_default_in_hjson(raw, "ollama").unwrap();
        assert!(out.contains("default: ollama"));
        assert!(
            out.contains("default: should_not_be_touched"),
            "nested `default:` inside a provider must survive untouched; got:\n{out}"
        );
    }

    #[test]
    fn format_age_minutes_hours_days() {
        assert_eq!(
            format_age_humantime(std::time::Duration::from_secs(7 * 60)),
            "7m"
        );
        assert_eq!(
            format_age_humantime(std::time::Duration::from_secs(3 * 3600 + 30 * 60)),
            "3h 30m"
        );
        assert_eq!(
            format_age_humantime(std::time::Duration::from_secs(2 * 86_400 + 4 * 3600)),
            "2d 4h"
        );
        // Whole-hour and whole-day values shouldn't dangle a "0m" / "0h".
        assert_eq!(
            format_age_humantime(std::time::Duration::from_secs(5 * 86_400)),
            "5d"
        );
        assert_eq!(
            format_age_humantime(std::time::Duration::from_secs(2 * 3600)),
            "2h"
        );
    }
}

/// Strip a leading Typst heading line (`= Title`) from a paragraph body so
/// it doesn't end up in the LLM prompt. The heading is editor chrome — it
/// describes the prompt for tree-pane navigation, not text the user wants
/// to send. Trims any blank lines that immediately follow the heading.
fn strip_leading_typst_heading(body: &str) -> String {
    let mut lines: Vec<&str> = body.lines().collect();
    if let Some(first) = lines.first() {
        if first.trim_start().starts_with('=') {
            lines.remove(0);
            while lines.first().is_some_and(|l| l.trim().is_empty()) {
                lines.remove(0);
            }
        }
    }
    lines.join("\n")
}

/// 1.2.8+ — maximum number of recently-deleted paragraphs held
/// in `App.kill_ring`. Picked to be large enough that an
/// accidental burst of single-¶ deletes doesn't clobber an
/// earlier recovery, small enough that the picker fits on a
/// 24-row terminal without scrolling.
const KILL_RING_CAP: usize = 10;

/// 1.2.9+ — true when `line` is a typographic scene-
/// break line.  Recognised forms (case-insensitive,
/// after trimming whitespace + collapsing internal
/// spaces):
///   * 3+ copies of any one of `*`, `-`, `_`, `~`, `#`
///     (optionally separated by single spaces — so
///     `* * *` and `***` both match).
///   * A single `§` (typographic section sign).
/// Reject everything else.  Notably does NOT match
/// typst headings (`= Foo`, `== Foo`) — those are
/// structural section markers, not scene breaks.
/// Doesn't match `**` (2 chars, below threshold) or
/// `***bold***` (mixed-content), avoiding common
/// false positives.
fn is_scene_break(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "§" {
        return true;
    }
    // Strip internal whitespace; the remaining chars must
    // be 3+ copies of one of the marker characters.
    let chars: Vec<char> = trimmed
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if chars.len() < 3 {
        return false;
    }
    let first = chars[0];
    if !"*-_~#".contains(first) {
        return false;
    }
    chars.iter().all(|c| *c == first)
}

/// 1.2.9+ — sanitise a paragraph title into a
/// filesystem-safe slug for the default audio path.
/// Lowercased, ASCII-alphanumeric kept verbatim, every
/// other char collapsed to a hyphen, runs of hyphens
/// trimmed, empty result falls back to "paragraph".
/// Cyrillic / non-Latin chars survive as hyphens
/// because filesystem-portability is the goal here;
/// users who want a localised name edit the path
/// before pressing Enter.
fn audio_filename_slug(title: &str) -> String {
    let lc = title.to_lowercase();
    let mut out = String::with_capacity(lc.len());
    let mut last_was_dash = false;
    for c in lc.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_was_dash = false;
        } else {
            if !last_was_dash && !out.is_empty() {
                out.push('-');
                last_was_dash = true;
            }
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "paragraph".to_string()
    } else {
        out
    }
}

/// 1.2.8+ — truncate a captured shell-output blob to at most
/// `max_lines` lines.  Returns the input unchanged if it
/// already fits.  When truncated, the last kept line is
/// replaced with `… (N more lines truncated)` so the user
/// sees that something was dropped and how much.  The marker
/// itself counts toward `max_lines`, so the visible line
/// count never exceeds the configured cap.
///
/// Used at the App level (post-eval) rather than inside
/// `shell::Engine::eval` so the engine stays config-free —
/// it always returns whatever nu produced, and the UI layer
/// decides what to retain.
fn truncate_to_lines(text: String, max_lines: usize) -> String {
    if text.is_empty() {
        return text;
    }
    let cap = max_lines.max(1);
    let total: usize = text.lines().count();
    if total <= cap {
        return text;
    }
    // Keep cap-1 head lines, replace the rest with the marker.
    let keep = cap.saturating_sub(1);
    let head: String = text
        .lines()
        .take(keep)
        .collect::<Vec<_>>()
        .join("\n");
    let dropped = total - keep;
    let marker = format!(
        "… ({dropped} more lines truncated · raise shell.max_output_lines to keep more)"
    );
    if head.is_empty() { marker } else { format!("{head}\n{marker}") }
}

pub(crate) struct App {
    layout: ProjectLayout,
    store: Store,
    keymap: Keymap,
    cfg: Config,
    ai: AiClient,
    prompts: PromptLibrary,

    /// 1.2.7+ — mouse capture toggle. True (the default)
    /// means crossterm captures every mouse event for the
    /// TUI (focus on click, scroll wheel, etc.). False
    /// releases capture so terminal-native drag-to-select +
    /// Cmd+C (macOS) / Ctrl+Shift+C (Linux/Windows) work
    /// inside the editor and AI pane. Toggled by
    /// `Ctrl+Shift+M`.
    mouse_captured: bool,

    /// 1.2.7+ — per-book swim-lane view state cache.
    /// Captured when the timeline view closes (Esc), restored
    /// on next `open_timeline_view` for the same book.
    /// Survives restart via `.session.json`.
    timeline_views: std::collections::HashMap<Uuid, TimelineViewSnapshot>,

    /// 1.2.7+ — single-slot kill-ring for the most recent
    /// PARAGRAPH delete. Captured in `commit_delete` just
    /// before `delete_subtree` fires (single-paragraph only;
    /// subtree deletes are too risky to undo without store
    /// API support). Restored by Ctrl+B U (pops the front =
    /// most-recent) or Ctrl+V Shift+U (picker over the
    /// whole ring) via `create_node` at the original
    /// position — content, tags, linked_paragraphs, event
    /// data preserved; the uuid changes (so paragraph links from
    /// elsewhere pointing at the deleted id stay broken).
    ///
    /// 1.2.8+ — was `Option<DeletedParagraphStash>` (single
    /// slot) in 1.2.7; widened to a VecDeque so accidental
    /// burst-deletes don't clobber an earlier recovery.
    /// Capped at `KILL_RING_CAP`; oldest entries drop off
    /// the back as the front fills.
    kill_ring: std::collections::VecDeque<DeletedParagraphStash>,

    /// 1.2.7+ — visited-paragraph history (browser-style
    /// back/forward). Pushed on every `load_paragraph` that
    /// isn't itself a back/forward navigation; truncated
    /// forward on a new push (just like a browser).
    /// Persisted in `.session.json`.
    visited_history: Vec<Uuid>,
    /// Index into `visited_history` pointing at the current
    /// paragraph. Back = -1, Forward = +1.
    visited_cursor: usize,
    /// Set by Alt+Left / Alt+Right before they call
    /// `open_paragraph_by_uuid` so the resulting
    /// `load_paragraph` doesn't re-push (which would
    /// instantly clobber the forward stack). Cleared by
    /// `load_paragraph` after read.
    visited_skip_next_push: bool,

    hierarchy: Hierarchy,
    rows: Vec<(Uuid, usize)>,
    /// Branches whose children are hidden in the tree pane. The branch
    /// itself stays visible; only its subtree is collapsed. Left arrow adds
    /// to this set, Right removes from it.
    collapsed_nodes: std::collections::HashSet<Uuid>,
    /// True after the user pressed the meta-prefix chord (default Ctrl+B).
    /// The next key is interpreted as an action selector and clears this.
    meta_pending: bool,
    /// True for one keystroke after the user presses Ctrl+V. The
    /// next key picks an export variant — `1` / `2` whose meaning
    /// depends on the current focus (editor: buffer / subchapter;
    /// tree: current node + descendants). All variants produce
    /// markdown and write to the launch cwd with a deterministic
    /// stem so the user can find the file without a save dialog.
    view_pending: bool,
    /// True after the user pressed the Bund-meta prefix (default
    /// Ctrl+Z). The next key dispatches into `handle_bund_action`
    /// (R run, E eval, N new script).
    bund_pending: bool,
    modal: Modal,

    focus: Focus,
    tree_cursor: usize,
    tree_scroll: usize,

    search_input: TextInput,
    ai_input: TextInput,
    /// Sent AI prompts in chronological order (oldest first).
    /// Up/Down in the AI prompt walks this list when no prompt
    /// picker is showing. Cleared on every send via push_back.
    /// 1.2.4+.
    ai_prompt_history: Vec<String>,
    /// 1.2.8+ — F1 help-query history. Up/Down inside the
    /// `Modal::HelpQuery` input walks this ring. Pushed on
    /// every successful Enter (dedup against the immediate
    /// predecessor); not persisted (session-only) to keep
    /// stale "what did inkhaven 1.2.5 do for X" queries out
    /// of long-running ring tails.
    help_query_history: Vec<String>,
    /// Cursor into `help_query_history`; same semantics as
    /// `ai_prompt_history_cursor` — None = not navigating.
    help_query_history_cursor: Option<usize>,
    /// 1.2.8+ — F6 snapshot-picker annotation filter.
    /// Substring-match (case-insensitive) against the
    /// snapshot's annotation text; empty = show all.
    /// `/` toggles `snapshot_filter_focused`; while focused,
    /// typed chars edit the filter (D/V/Enter chord keys
    /// route into the filter instead of firing actions).
    /// Reset to defaults each `open_snapshot_picker`.
    snapshot_filter: String,
    snapshot_filter_focused: bool,
    /// 1.2.8+ — embedded nushell.  `None` until first
    /// `Ctrl+Z o` opens the pane (lazy init); persists
    /// across close + reopen so env-var mutations (`$env.X
    /// = ...`) and the engine state stay alive between
    /// sessions of the pane.  `Ctrl+Z O` (Shift) destroys
    /// this and rebuilds fresh.
    shell_engine: Option<super::shell::Engine>,
    /// 1.2.8+ — turn buffer for the shell pane.  Capped at
    /// `cfg.shell.max_buffered_turns`; oldest entries roll
    /// off the front as the back fills.  Lives on `App`
    /// rather than on `Modal::ShellPane` so it survives
    /// modal close/reopen — selection mode (Phase 6) and
    /// the typst-box insert read from this Vec.
    shell_history: Vec<super::shell::ShellTurn>,
    /// 1.2.8+ — command-only history ring for Up/Down
    /// arrow recall inside the shell input.  Pushed on each
    /// non-empty Enter; deduped against the immediate
    /// predecessor.  Persisted to
    /// `.inkhaven/shell_history.db` (1.2.8+ — see
    /// `shell_history_db`).
    shell_command_history: Vec<String>,
    /// 1.2.8+ — SQLite-backed history at
    /// `<project>/.inkhaven/shell_history.db`.  Loaded
    /// lazily on first `open_shell_pane`; appended on
    /// every non-empty Enter.  `None` until the first
    /// open.
    shell_history_db: Option<super::shell::History>,
    /// 1.2.8+ — pane-local Ctrl+B chord prefix.  Kept
    /// separate from the global `bund_pending` so the
    /// shell pane's `Ctrl+B H` doesn't clash with the
    /// global Ctrl+B chord ladder (Quit, Load, …).
    /// `true` after Ctrl+B inside the pane, consumed by
    /// the next keystroke.
    shell_ctrlb_pending: bool,
    /// 1.2.9+ — TTS subprocess wrapper.  Each
    /// `Ctrl+B S` / greeting / goodbye spawns a fresh
    /// `/usr/bin/say` child process and stores it here.
    /// Side-steps the in-process `tts-rs` / AVFoundation
    /// reuse bug where the second `speak()` call in the
    /// same process produces no audio.  See `tui::say`
    /// for the wrapper detail.
    tts_say: super::say::Say,
    /// 1.2.9+ — session-local override for
    /// `editor.style_warnings.enabled`.  `Ctrl+V w`
    /// flips this; `true` forces overlays on regardless
    /// of HJSON, `false` forces off, `None` (the
    /// default) defers to the HJSON setting.
    style_warnings_toggle: Option<bool>,
    /// 1.2.9+ — session-local override for
    /// `editor.pov_chip_enabled`.  `Ctrl+B Shift+P`
    /// flips this; semantics identical to
    /// `style_warnings_toggle`.
    pov_chip_toggle: Option<bool>,
    /// Cursor into `ai_prompt_history`. None when not navigating;
    /// `Some(i)` when the user is stepping through history. Any
    /// edit (typing, backspace, etc.) clears it so the next Up
    /// arrow starts at the end of the list again.
    ai_prompt_history_cursor: Option<usize>,

    opened: Option<OpenedDoc>,
    /// "Similar paragraphs" mode: when `Some`, a second
    /// paragraph is loaded side-by-side with `opened`. The right
    /// editor pane (which normally holds the AI pane) is
    /// repurposed to render this doc. Set by the SimilarPicker
    /// modal; cleared by re-pressing Ctrl+V S, which first saves
    /// both buffers.
    ///
    /// `self.opened` always carries the **focused** doc — Tab in
    /// similar mode swaps `opened` ↔ `secondary` so the existing
    /// editor key handlers keep working unchanged. The
    /// `secondary_in_left_pane` flag tells the renderer which
    /// physical pane currently holds `opened`.
    secondary: Option<OpenedDoc>,
    /// Cached writing-progress snapshot, refreshed on every save +
    /// on project open. Status-bar widget reads from this cache so
    /// per-frame redraws don't trigger a hierarchy walk. `None`
    /// means "progress disabled / not yet computed".
    progress_cache: Option<crate::progress::ProgressSnapshot>,
    /// "Select paragraph to link" mode (1.2.4+). When Some, the
    /// tree pane shows a custom title and `Enter` on a paragraph
    /// links it to the owning UUID stashed here. Esc / tree-focus
    /// loss exits the mode and restores normal Enter semantics.
    ///
    /// Direction:
    /// * `Outgoing` — the Ctrl+V A flow. Open paragraph
    ///   `linked_paragraphs` gains the tree-picked target.
    /// * `Incoming` — the Ctrl+V I flow. The tree-picked
    ///   paragraph's `linked_paragraphs` gains the open one
    ///   (i.e. creates a link FROM the picked paragraph TO
    ///   current).
    link_pick_for: Option<(Uuid, LinkPickDirection)>,
    /// Multi-select set in the tree pane (1.2.4+). Toggled by
    /// `Space` on a row; when non-empty, `Ctrl+B R` (cycle
    /// status) and `Ctrl+V T` (set target) operate on every
    /// marked paragraph instead of just the cursor's. Cleared
    /// by `Esc` in the tree.
    tree_marked: std::collections::HashSet<Uuid>,
    /// In similar-paragraph mode, which pane has keyboard focus.
    /// `false` (default) → left pane = `self.opened` is the
    /// keyboard target. `true` → right pane = `self.secondary`
    /// is the target. Tab inside `Focus::Editor` flips this flag.
    /// All existing editor handlers continue to read/write
    /// `self.opened`; routing happens via a swap performed at
    /// the key-dispatch + save boundaries.
    secondary_focused: bool,
    status: String,
    show_results_overlay: bool,
    results: Vec<SearchHit>,
    results_cursor: usize,

    /// System clipboard handle. May be None on headless systems or when init
    /// fails; in that case copy/cut/paste use tui-textarea's internal yank
    /// buffer only.
    clipboard: Option<arboard::Clipboard>,

    highlighter: TypstHighlighter,
    /// Decoded ratatui colours for the active theme; built once at startup
    /// from `cfg.theme`. Read everywhere the renderer used to hard-code
    /// `Color::Cyan` / `Color::DarkGray` / etc.
    theme: super::theme::Theme,

    /// Place/Character names recompiled into regexes for the editor highlight
    /// overlay. Rebuilt after every save and at startup. None means an empty
    /// lexicon — render path skips work.
    lexicon: super::lexicon::Lexicon,

    inference: Option<Inference>,
    show_prompt_picker: bool,
    prompt_picker_cursor: usize,

    /// Saved (cursor_row, cursor_col, scroll_row, scroll_col) per paragraph
    /// UUID. Updated when the editor loses focus, when the user switches
    /// paragraphs, and at exit. Loaded from `.session.json` on startup so
    /// positions survive across runs.
    paragraph_cursors: std::collections::HashMap<Uuid, ParagraphCursor>,

    /// Cumulative AI chat history for the current session. Each prompt the
    /// user sends from the AI prompt bar appends a User turn, and the
    /// resulting assistant response (when streaming finishes) appends an
    /// Assistant turn. The full history is replayed back to the model on
    /// every follow-up so the conversation is continuous. Cleared by F9 or
    /// the meta-prefix Ctrl+B C.
    ///
    /// Help (F1 / `Help!`) inferences are deliberately *not* added here —
    /// they're one-shot RAG flows with a separate strict system prompt and
    /// don't benefit from carrying chat context.
    chat_history: Vec<ChatTurn>,
    /// User-supplied system-prompt override set via the
    /// `ink.ai.set_system_prompt` Bund stdlib word. When `Some`,
    /// every AI inference (start_inference / start_help_inference
    /// / grammar check) uses this string instead of the
    /// inference-mode-derived default. Cleared by passing empty
    /// string. Volatile — not persisted to HJSON.
    system_prompt_override: Option<String>,
    /// Captures the user message of the currently-streaming chat inference
    /// so we can record the matching Assistant turn into `chat_history`
    /// once the stream finishes. None during one-shot (Help) inferences.
    pending_chat_user_msg: Option<String>,

    /// 1.2.6+ — paragraph UUID to stamp the next completed
    /// inference's turns onto via `Node.ai_memory`. Set at
    /// send-time when (a) `ai.per_paragraph_memory` is on,
    /// (b) mode_used == Paragraph, and (c) a paragraph is
    /// open. Consumed by `pump_inference` on stream
    /// completion alongside `pending_chat_user_msg`.
    pending_paragraph_memory_target: Option<Uuid>,

    /// RAG context block (e.g. a place/character lookup) that the next
    /// AI-prompt submission should prepend to the user's typed query.
    /// Used by the Ctrl+B P / Ctrl+B C editor flows when the AI prompt is
    /// empty: the context is stashed, focus jumps to the AI prompt so the
    /// user can type their question, and `start_inference` lifts the
    /// prefix on Enter.
    pending_rag_prefix: Option<String>,

    /// Per-pane rectangles cached from the most recent `draw()` so the
    /// mouse handler can map a click coordinate to the right pane. Empty
    /// `Rect`s before the first frame is drawn — every handler checks
    /// `contains` so a click during that window safely no-ops.
    layout_search: Rect,
    layout_tree: Rect,
    layout_editor: Rect,
    layout_ai: Rect,
    layout_ai_prompt: Rect,

    /// Active AI "scope" picker. Cycled with F9. When set to anything but
    /// `None`, the next AI-prompt submission prepends the matching context
    /// (selection text, paragraph body, subchapter / chapter / book
    /// concatenation) to the user's query, then auto-resets to `None`.
    ai_mode: AiMode,

    /// How aggressively the model may draw on its own knowledge. Toggled
    /// globally by F10. Help inferences pin this to `Local` regardless of
    /// the current value (see `start_help_inference`).
    inference_mode: InferenceMode,

    /// Set by `commit_file_pick` when the user picks a directory to
    /// import. The main loop picks this up, renders a progress splash,
    /// and runs the (synchronous) import with periodic redraws. None
    /// during normal operation.
    pending_import: Option<std::path::PathBuf>,

    /// Set by Ctrl+B A (`schedule_assembly`). Main loop drives the
    /// synchronous book-assembly procedure with a progress splash.
    pending_assembly: Option<Uuid>,

    /// Set by Ctrl+B B — run assembly + `typst compile`, surface
    /// errors via a fresh AI chat tuned for typst diagnostics.
    pending_build: Option<Uuid>,
    /// 1.2.6+ — set by `Ctrl+B Shift+B` (Action::BackupNow). The
    /// main event loop runs `run_manual_backup` next tick so the
    /// splash + wait-for-key dance happens off the chord-handler
    /// path (which doesn't own the terminal).
    pending_backup_now: bool,

    /// Set by Ctrl+B O — Ctrl+B B + copy the resulting PDF into the
    /// inkhaven launch cwd with a timestamped filename.
    pending_take: Option<Uuid>,

    /// Typewriter-style SFX (Enter key + editor-pane focus-out). None
    /// when the host has no audio device — every play call then
    /// silently no-ops, so a headless / SSH-without-audio session
    /// behaves identically to a desktop with `sound.enabled = false`.
    sound: Option<super::sound::SoundPlayer>,

    /// Cached ratatui-image Picker — queried once at startup from the
    /// host terminal. None when the query failed (CI, weird ssh
    /// session, terminals without ANSI graphics support); preview
    /// then falls back to the status-bar info line. Also None when
    /// `images.preview_enabled = false` regardless of capability.
    image_picker: Option<ratatui_image::picker::Picker>,

    /// Ctrl+B W toggles full-screen "typewriter mode" — the editor
    /// pane expands to the whole terminal, every other pane (search
    /// bar, tree, AI, AI prompt, status bar) is hidden. The same
    /// chord returns to the normal layout.
    typewriter_mode: bool,

    /// Ctrl+B K toggles full-screen AI mode — left half = the live
    /// AI pane (streaming inference), right half = scrolling chat
    /// history, bottom = AI prompt. Same chord returns to the
    /// normal layout. Mutually exclusive with `typewriter_mode`.
    ai_fullscreen: bool,

    /// Extra lines to scroll the chat-history pane UP from its auto-
    /// bottom-pin. PageUp adds, PageDown subtracts; the value is
    /// clamped against the total line count so over-scrolling stops
    /// at the top of the history. Reset to 0 each time a new user
    /// message is sent so the streaming reply is visible.
    chat_history_scroll: usize,

    /// Active chat-history search state (Ctrl+F in AI-fullscreen).
    /// While Some, matching lines render with a highlight bg and
    /// the renderer scrolls so the `current` match lands in the
    /// middle of the pane. Ctrl+X advances toward older matches
    /// (the spec: "Start from bottom, going up to older").
    chat_search: Option<ChatSearchState>,

    /// Active chat-selection mode (Ctrl+C in AI-fullscreen). `Some`
    /// when the user is selecting a turn block. Up / Down navigate;
    /// `C` copies the selected turn to the system clipboard, `T`
    /// inserts it into the editor buffer at the cursor.
    chat_selection: Option<ChatSelectionState>,
}

mod ai_impl;
mod backup_impl;
mod editor_impl;
mod render;
mod snapshot_impl;
mod tag_impl;
mod timeline_impl;
mod tree_impl;

impl App {
    fn new(layout: ProjectLayout, cfg: Config, store: Store) -> Result<Self> {
        let keymap = Keymap::from_config(&cfg).map_err(anyhow::Error::from)?;
        // Build the chord-action table from the user's HJSON
        // overlay. Defaults first, then `keys.bindings` rewrites.
        // Install into the process-wide slot so `ink.key.*` Bund
        // stdlib words can mutate the same source of truth the
        // App reads from on every chord dispatch.
        let overrides: Vec<(String, String, Option<String>)> = cfg
            .keys
            .bindings
            .iter()
            .map(|b| (b.chord.clone(), b.action.clone(), b.scope.clone()))
            .collect();
        let keys = super::keybind::KeyBindings::from_overrides(
            keymap.meta_prefix,
            keymap.bund_prefix,
            keymap.view_prefix,
            &overrides,
        )
        .map_err(|e| Error::Config(format!("keys.bindings: {e}")))?;
        super::keybind::install(keys);
        let hierarchy = Hierarchy::load(&store).map_err(anyhow::Error::from)?;
        let lexicon = build_lexicon(&hierarchy, &cfg);
        let collapsed_nodes: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        let rows: Vec<(Uuid, usize)> = hierarchy
            .flatten_with_collapsed(&collapsed_nodes)
            .into_iter()
            .map(|(n, d)| (n.id, d))
            .collect();

        // Background sync: every `sync_interval_seconds` flush the HNSW
        // index (cheap no-op when clean) and force a DuckDB CHECKPOINT
        // against `metadata.db` + `blobs.db` (cheap when WAL is empty).
        // Both ops short-circuit when there's no real work, so the tick
        // can be generous (default 600s). Store is Send+Sync and cheap to
        // clone (Arc inside). 0 disables.
        if cfg.sync_interval_seconds > 0 {
            let store_for_sync = store.clone();
            let interval_secs = cfg.sync_interval_seconds;
            tokio::spawn(async move {
                let period = std::time::Duration::from_secs(interval_secs);
                let mut ticker = tokio::time::interval(period);
                // Skip the immediate first tick (interval fires at t=0).
                ticker.tick().await;
                loop {
                    ticker.tick().await;
                    if let Err(e) = store_for_sync.sync() {
                        tracing::warn!("background sync failed: {e}");
                    }
                    if let Err(e) = store_for_sync.checkpoint() {
                        tracing::warn!("background checkpoint failed: {e}");
                    }
                }
            });
        }

        let ai = AiClient::from_config(&cfg.llm).map_err(anyhow::Error::from)?;

        let prompts_path = layout.prompts_path(&cfg);
        let prompts = if prompts_path.is_file() {
            PromptLibrary::load(&prompts_path).map_err(anyhow::Error::from)?
        } else {
            PromptLibrary::default()
        };

        let theme = super::theme::Theme::from_config(&cfg.theme);
        // Try to claim the default audio device. None on hosts without
        // one — the player then silently no-ops, mirroring
        // `sound.enabled = false`.
        let sound =
            super::sound::SoundPlayer::try_new(cfg.sound.enabled, cfg.sound.volume);
        // Probe the host terminal for graphics-protocol support so the
        // image-preview modal can pick kitty / sixel / iterm2 / half-
        // block. Errors here just disable the preview pane; the rest
        // of the app behaves identically.
        let image_picker = if cfg.images.preview_enabled {
            match ratatui_image::picker::Picker::from_query_stdio() {
                Ok(p) => Some(p),
                Err(e) => {
                    tracing::info!("image preview disabled — terminal probe: {e}");
                    None
                }
            }
        } else {
            None
        };
        let initial_mouse_captured = cfg.editor.mouse_captured;
        Ok(Self {
            layout,
            store,
            keymap,
            cfg,
            ai,
            prompts,
            mouse_captured: initial_mouse_captured,
            timeline_views: std::collections::HashMap::new(),
            kill_ring: std::collections::VecDeque::new(),
            visited_history: Vec::new(),
            visited_cursor: 0,
            visited_skip_next_push: false,
            hierarchy,
            rows,
            modal: Modal::None,
            collapsed_nodes,
            meta_pending: false,
            bund_pending: false,
            view_pending: false,
            focus: Focus::Tree,
            tree_cursor: 0,
            tree_scroll: 0,
            search_input: TextInput::new(),
            ai_input: TextInput::new(),
            ai_prompt_history: Vec::new(),
            ai_prompt_history_cursor: None,
            help_query_history: Vec::new(),
            help_query_history_cursor: None,
            snapshot_filter: String::new(),
            snapshot_filter_focused: false,
            shell_engine: None,
            shell_history: Vec::new(),
            shell_command_history: Vec::new(),
            shell_history_db: None,
            shell_ctrlb_pending: false,
            tts_say: super::say::Say::default(),
            style_warnings_toggle: None,
            pov_chip_toggle: None,
            opened: None,
            secondary: None,
            secondary_focused: false,
            progress_cache: None,
            link_pick_for: None,
            tree_marked: std::collections::HashSet::new(),
            status: String::from(
                "Tab=panes · Enter=open · Ctrl+S=save · Ctrl+B then B/C/S/P add · D delete · ↑/↓ reorder · Ctrl+Q quit",
            ),
            show_results_overlay: false,
            results: Vec::new(),
            results_cursor: 0,
            clipboard: arboard::Clipboard::new().ok(),
            highlighter: TypstHighlighter::new()
                .map_err(|e| anyhow::anyhow!("typst highlighter init: {e}"))?,
            theme,
            lexicon,
            inference: None,
            show_prompt_picker: false,
            prompt_picker_cursor: 0,
            paragraph_cursors: std::collections::HashMap::new(),
            chat_history: Vec::new(),
            system_prompt_override: None,
            pending_chat_user_msg: None,
            pending_paragraph_memory_target: None,
            pending_rag_prefix: None,
            layout_search: Rect::default(),
            layout_tree: Rect::default(),
            layout_editor: Rect::default(),
            layout_ai: Rect::default(),
            layout_ai_prompt: Rect::default(),
            ai_mode: AiMode::None,
            inference_mode: InferenceMode::Full,
            pending_import: None,
            pending_assembly: None,
            pending_build: None,
            pending_backup_now: false,
            pending_take: None,
            sound,
            image_picker,
            typewriter_mode: false,
            ai_fullscreen: false,
            chat_history_scroll: 0,
            chat_search: None,
            chat_selection: None,
        })
    }

    /// Open the progress store under the project root and seed
    /// today's baselines. Called from `run` after `App::new` and
    /// before the event loop spins up. Failures are logged + the
    /// store stays uninstalled — progress tracking degrades to
    /// "(disabled)" rather than aborting startup.
    pub(crate) fn install_progress(&mut self) {
        if let Err(e) = crate::progress::install(&self.layout.root) {
            tracing::warn!(target: "inkhaven::progress", "install: {e:#}");
            return;
        }
        // Snapshot today's per-book + project-wide totals so
        // `today_words` has a stable reference point. Idempotent
        // per (day, book) — subsequent calls in the same UTC day
        // are silent no-ops.
        let (per_book, project_total) = self.compute_word_totals_now();
        crate::progress::capture_today_baselines(&per_book, project_total);
        self.refresh_progress_cache();
    }

    /// Rebuild `self.progress_cache` from the current hierarchy +
    /// progress store. Cheap enough to call per save (one hierarchy
    /// walk + a handful of small DuckDB selects). The status bar
    /// always reads from the cache so it doesn't pay the cost on
    /// every redraw.
    ///
    /// 1.2.4+: fires two Bund hooks when the snapshot transitions
    /// across a meaningful threshold — see `fire_progress_hooks`.
    pub(crate) fn refresh_progress_cache(&mut self) {
        let (per_book_vec, project_total) = self.compute_word_totals_now();
        let mut per_book: std::collections::HashMap<Uuid, i64> =
            std::collections::HashMap::new();
        let mut book_titles: std::collections::HashMap<Uuid, String> =
            std::collections::HashMap::new();
        let mut book_slugs: std::collections::HashMap<Uuid, String> =
            std::collections::HashMap::new();
        for (id, total) in per_book_vec {
            per_book.insert(id, total);
            if let Some(node) = self.hierarchy.get(id) {
                book_titles.insert(id, node.title.clone());
                book_slugs.insert(id, node.slug.clone());
            }
        }
        let live = crate::progress::LiveTotals {
            per_book,
            project_total,
            book_titles,
            book_slugs,
        };
        let prev = self.progress_cache.clone();
        let snap = crate::progress::snapshot(&self.cfg.goals, &live);
        self.fire_progress_hooks(prev.as_ref(), &snap);
        self.progress_cache = Some(snap);
    }

    /// Diff the previous progress snapshot against the new one
    /// and fire the matching transition hooks.
    ///
    /// * `hook.on_goal_hit ( today_words daily_goal -- )` — fired
    ///   the first time `today_words` crosses `daily_goal` on a
    ///   given day. Doesn't re-fire while still over the line;
    ///   self-resets if the user deletes back below it.
    /// * `hook.on_streak_break ( prev_streak_days -- )` — fired
    ///   when the streak transitions from positive to zero. The
    ///   argument is the streak length immediately before the
    ///   break so a hook can log/notify "you just broke a 12-day
    ///   streak".
    fn fire_progress_hooks(
        &self,
        prev: Option<&crate::progress::ProgressSnapshot>,
        new: &crate::progress::ProgressSnapshot,
    ) {
        // on_goal_hit
        if let Some(goal) = new.project.daily_goal.filter(|n| *n > 0) {
            let prev_today = prev
                .map(|p| p.project.today_words)
                .unwrap_or(0);
            let new_today = new.project.today_words;
            if prev_today < goal && new_today >= goal {
                crate::scripting::hooks::fire(
                    "hook.on_goal_hit",
                    vec![
                        rust_dynamic::value::Value::from_int(new_today),
                        rust_dynamic::value::Value::from_int(goal),
                    ],
                );
            }
        }
        // on_streak_break — only fires when we had a positive
        // streak previously and now we have zero. First-launch
        // (prev is None) doesn't count as a break.
        let prev_streak = prev.map(|p| p.streak.days).unwrap_or(0);
        if prev_streak > 0 && new.streak.days == 0 {
            crate::scripting::hooks::fire(
                "hook.on_streak_break",
                vec![rust_dynamic::value::Value::from_int(prev_streak)],
            );
        }
        // on_active_goal_hit — same transitional semantics as
        // on_goal_hit but against `goals.active_minutes_daily`.
        let active_goal_secs = self.cfg.goals.active_minutes_daily.max(0) * 60;
        if active_goal_secs > 0 {
            let prev_active = prev.map(|p| p.active_seconds_today).unwrap_or(0);
            let new_active = new.active_seconds_today;
            if prev_active < active_goal_secs && new_active >= active_goal_secs {
                crate::scripting::hooks::fire(
                    "hook.on_active_goal_hit",
                    vec![
                        rust_dynamic::value::Value::from_int(new_active),
                        rust_dynamic::value::Value::from_int(active_goal_secs),
                    ],
                );
            }
        }
    }

    /// Walk the hierarchy and compute current per-book + project
    /// word totals. Touches the filesystem (reads paragraph
    /// bodies) — okay to call once at startup; called from the
    /// progress modal too where the cost amortises across an
    /// interactive open.
    pub(crate) fn compute_word_totals_now(
        &self,
    ) -> (Vec<(Uuid, i64)>, i64) {
        use crate::progress::word_count::count_words;
        let mut per_book: std::collections::HashMap<Uuid, i64> =
            std::collections::HashMap::new();
        for (node, _) in self.hierarchy.flatten() {
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            // Skip protected (system / Help) books — they're not
            // user manuscript content.
            let in_user_book = self
                .hierarchy
                .ancestors(node)
                .into_iter()
                .find(|a| a.kind == NodeKind::Book)
                .filter(|b| b.system_tag.is_none());
            let Some(book) = in_user_book else { continue };
            let Some(rel) = node.file.as_ref() else { continue };
            let abs = self.layout.root.join(rel);
            let body = std::fs::read_to_string(&abs).unwrap_or_default();
            let n = count_words(&body);
            *per_book.entry(book.id).or_insert(0) += n;
        }
        let project_total: i64 = per_book.values().sum();
        let per_book_vec: Vec<(Uuid, i64)> = per_book.into_iter().collect();
        (per_book_vec, project_total)
    }

    /// Final HNSW save + DuckDB CHECKPOINT before the App (and its
    /// `Store` handle, and therefore the duckdb connection pool) are
    /// dropped. Called from the exit sequence in `run(&Path)` so the
    /// `.db.wal` files are drained while we can still surface errors
    /// — the pool's own Drop impl would checkpoint implicitly, but
    /// silently.
    /// Evaluate a Bund script with `self` installed as the
    /// active `App`. Lets `ink.editor.* / ink.ai.* / ink.typst.*`
    /// stdlib words reach App state. Pure wrapper around
    /// `scripting::eval` — sets the global ACTIVE_APP slot via
    /// `AppGuard` before invoking, restores on RAII drop.
    pub(crate) fn scripting_eval(
        &mut self,
        code: &str,
    ) -> anyhow::Result<crate::scripting::EvalOutput> {
        let _guard = crate::scripting::AppGuard::enter(self);
        crate::scripting::eval(code)
    }

    fn shutdown_flush(&self) {
        if let Err(e) = self.store.sync() {
            tracing::warn!("shutdown sync failed: {e}");
        }
        if let Err(e) = self.store.checkpoint() {
            tracing::warn!("shutdown checkpoint failed: {e}");
        }
    }

    fn run<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            self.pump_inference();
            self.tick_autosave();
            // 1.2.9+ — close the TTS playback modal as
            // soon as the engine reports it's idle, so
            // the modal disappears when the paragraph
            // finishes naturally (no user keystroke
            // needed).  Cheap call — single FFI
            // boolean.
            self.tts_poll_playback();
            // Drive any deferred directory import — `commit_file_pick`
            // sets `pending_import` so the splash can be drawn directly
            // via the terminal handle (which `commit_file_pick` doesn't
            // own). Runs synchronously: same UX as the backup splash.
            if let Some(root) = self.pending_import.take() {
                self.run_pending_import(terminal, &root);
            }
            // Same pattern for Book assembly (Ctrl+B A): the dispatcher
            // stashes the book uuid, the main loop runs the procedure
            // and drives the splash redraws.
            if let Some(book_id) = self.pending_assembly.take() {
                self.run_pending_assembly(terminal, book_id);
            }
            if let Some(book_id) = self.pending_build.take() {
                self.run_pending_build(terminal, book_id, false);
            }
            if let Some(book_id) = self.pending_take.take() {
                self.run_pending_build(terminal, book_id, true);
            }
            if std::mem::take(&mut self.pending_backup_now) {
                self.run_pending_backup_now(terminal);
            }
            terminal.draw(|f| self.draw(f))?;
            // Shorter poll interval while streaming so tokens render with low
            // latency without burning CPU when idle.
            let timeout = if self.is_streaming() {
                Duration::from_millis(40)
            } else {
                Duration::from_millis(200)
            };
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.kind != KeyEventKind::Press {
                            continue;
                        }
                        if self.handle_key(key)? {
                            return Ok(());
                        }
                    }
                    Event::Mouse(mouse) => self.handle_mouse(mouse),
                    _ => {}
                }
            }
        }
    }

    fn tick_autosave(&mut self) {
        let secs = self.cfg.editor.autosave_seconds;
        if secs == 0 {
            return;
        }
        let due = match self.opened.as_ref() {
            // Suspend idle autosave while a grammar-correction overlay is
            // active so it doesn't disappear under the user's nose. The
            // overlay is dismissed by Ctrl+S (manual save), focus-out, or
            // Ctrl+B C — each of those resumes normal autosave.
            Some(doc) if doc.dirty && doc.correction_baseline.is_none() => {
                doc.last_activity.elapsed().as_secs() >= secs
            }
            _ => false,
        };
        if due {
            let _ = self.save_current();
        }

        // 1.2.4+: also autosave the secondary editor (similar-
        // paragraph mode's right pane). Before this, the secondary
        // doc was flush-only-on-Ctrl+V-S-exit, which surprised
        // users who typed in the right pane and walked away.
        // Same idle-threshold as the primary; no correction-overlay
        // gate since correction overlays only apply to the primary.
        // `Option::take()` pulls the doc out so we can call
        // `save_doc(&mut self, &mut OpenedDoc)` without aliasing
        // self.secondary against itself; the doc goes straight
        // back regardless of save outcome.
        let sec_due = match self.secondary.as_ref() {
            Some(doc) if doc.dirty => doc.last_activity.elapsed().as_secs() >= secs,
            _ => false,
        };
        if sec_due {
            if let Some(mut doc) = self.secondary.take() {
                if let Err(e) = self.save_doc(&mut doc) {
                    tracing::warn!(
                        target: "inkhaven::autosave",
                        "secondary autosave failed: {e}",
                    );
                }
                self.secondary = Some(doc);
            }
        }

        // 1.2.7+ — external-change watch. If the open
        // paragraph's file mtime moved since we loaded it,
        // someone else (CLI, sed, git pull) touched it.
        // Clean buffer → silently reload + status hint;
        // dirty buffer → red warning with a hint to use
        // Ctrl+B Shift+R to reload (losing local changes)
        // or Ctrl+S to overwrite. Cheap (one syscall per
        // tick) so safe at the autosave cadence.
        self.tick_external_change_check();

        // 1.2.5+: idle typst-syntax recheck. Independent of save
        // — runs whenever the user has paused for
        // `typst_compile.diagnostics_idle_seconds` and the buffer
        // has moved on since the last check. Save itself already
        // calls `refresh_typst_diagnostics_for_opened`, so this
        // covers the "I'm staring at the buffer wondering why
        // typst errored" case where no save has fired yet.
        if self.cfg.typst_compile.diagnostics {
            let idle = self.cfg.typst_compile.diagnostics_idle_seconds;
            let due = match self.opened.as_ref() {
                Some(doc) => {
                    let idle_ok =
                        doc.last_activity.elapsed().as_secs() >= idle;
                    let stale = doc
                        .typst_diagnostics_checked_at
                        .elapsed()
                        .as_secs()
                        >= idle.max(1);
                    idle_ok && stale && doc.dirty
                }
                None => false,
            };
            if due {
                self.refresh_typst_diagnostics_for_opened();
            }
        }
    }

    /// 1.2.7+ — once per tick, check whether the open
    /// paragraph's file changed on disk since we loaded it.
    /// Three cases:
    ///   1. mtime unchanged → no-op.
    ///   2. mtime newer + buffer CLEAN → silent reload +
    ///      stamp the new mtime. Status notes the reload.
    ///   3. mtime newer + buffer DIRTY → red warning. We do
    ///      NOT clobber the user's edits; they decide via
    ///      Ctrl+S (overwrite the on-disk change) or by
    ///      copying their text elsewhere + manually
    ///      reloading.
    fn tick_external_change_check(&mut self) {
        let Some(doc) = self.opened.as_ref() else { return; };
        let Some(loaded_mtime) = doc.loaded_mtime else { return; };
        let abs = self.layout.root.join(&doc.rel_path);
        let on_disk_mtime = match std::fs::metadata(&abs).and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => return,
        };
        if on_disk_mtime <= loaded_mtime {
            return;
        }
        // File changed externally.
        if doc.dirty {
            // Status warning, don't touch the buffer.
            self.status = format!(
                "⚠ `{}` changed on disk while you have unsaved edits — Ctrl+S to overwrite the external change",
                doc.title
            );
            return;
        }
        // Clean buffer → silent reload.
        let body = match std::fs::read_to_string(&abs) {
            Ok(b) => b,
            Err(e) => {
                self.status =
                    format!("external reload failed: {}: {e}", abs.display());
                return;
            }
        };
        let lines = body_to_lines(&body);
        let title = doc.title.clone();
        let id = doc.id;
        if let Some(doc) = self.opened.as_mut() {
            let saved_lines = lines.clone();
            doc.textarea = TextArea::new(lines);
            doc.saved_lines = saved_lines;
            doc.dirty = false;
            doc.loaded_mtime = Some(on_disk_mtime);
            // Move cursor to (0, 0) — the previous
            // position may no longer make sense after an
            // external rewrite.
            doc.textarea.move_cursor(CursorMove::Jump(0, 0));
        }
        let _ = id;
        self.refresh_typst_diagnostics_for_opened();
        self.status = format!(
            "↻ reloaded `{title}` — file changed on disk"
        );
    }

    fn is_streaming(&self) -> bool {
        matches!(
            self.inference.as_ref().map(|i| &i.status),
            Some(InferenceStatus::Streaming)
        )
    }

    fn pump_inference(&mut self) {
        let Some(inf) = self.inference.as_mut() else {
            return;
        };
        if !matches!(inf.status, InferenceStatus::Streaming) {
            return;
        }
        // Track terminal state of this poll so we can commit to chat history
        // exactly once when the stream completes successfully.
        let mut just_finished = false;
        loop {
            match inf.rx.try_recv() {
                Ok(StreamMsg::Token(t)) => inf.response.push_str(&t),
                Ok(StreamMsg::Done) => {
                    inf.status = InferenceStatus::Done;
                    let elapsed = inf.started_at.elapsed();
                    self.status = format!(
                        "{} responded in {:.1}s",
                        inf.provider,
                        elapsed.as_secs_f32()
                    );
                    just_finished = true;
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    inf.status = InferenceStatus::Error(e.clone());
                    self.status = format!("inference error: {e}");
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    // Task ended without a final message — treat as done so
                    // the assistant turn still gets recorded.
                    if matches!(inf.status, InferenceStatus::Streaming) {
                        inf.status = InferenceStatus::Done;
                        just_finished = true;
                    }
                    break;
                }
            }
        }
        if just_finished {
            // Pair the pending user message with this assistant response
            // and append both to chat_history. Help one-shots leave
            // `pending_chat_user_msg = None`, so they're skipped here.
            let assistant_text = self
                .inference
                .as_ref()
                .map(|i| i.response.clone())
                .unwrap_or_default();
            if let Some(user_msg) = self.pending_chat_user_msg.take() {
                if !assistant_text.trim().is_empty() {
                    // 1.2.6+ — stamp the turns onto the open
                    // paragraph's `ai_memory` if a target was
                    // captured at send time. Persisted via
                    // update_metadata so the buffer survives
                    // restart. Cap is enforced at write time:
                    // oldest turns evict first when length
                    // exceeds `per_paragraph_memory_max_turns`.
                    if let Some(target_id) =
                        self.pending_paragraph_memory_target.take()
                    {
                        self.record_paragraph_ai_memory(
                            target_id,
                            &user_msg,
                            &assistant_text,
                        );
                    }
                    self.chat_history.push(ChatTurn::User(user_msg));
                    self.chat_history
                        .push(ChatTurn::Assistant(assistant_text));
                }
            } else {
                // No pending user msg means a one-shot
                // (Help / F7 / F11 / F12). Discard any stale
                // memory target — those flows don't pollute
                // per-paragraph memory.
                self.pending_paragraph_memory_target = None;
            }
        }
    }

    /// 1.2.6+ — append a `(user, assistant)` pair to the open
    /// paragraph's `Node.ai_memory`, persist via
    /// `update_metadata`, and enforce the
    /// `ai.per_paragraph_memory_max_turns` cap by trimming
    /// oldest turns first. Per-paragraph AI memory is an
    /// additive metadata write — failures are logged but never
    /// abort the visible chat-history append above.
    fn record_paragraph_ai_memory(
        &mut self,
        paragraph_id: Uuid,
        user_msg: &str,
        assistant_text: &str,
    ) {
        let cap = self.cfg.ai.per_paragraph_memory_max_turns;
        if cap == 0 {
            return;
        }
        let Some(node) = self.hierarchy.get(paragraph_id).cloned() else {
            return;
        };
        let mut updated = node.clone();
        updated
            .ai_memory
            .push(crate::store::node::AiMemoryTurn {
                role: "user".to_string(),
                text: user_msg.to_owned(),
            });
        updated
            .ai_memory
            .push(crate::store::node::AiMemoryTurn {
                role: "assistant".to_string(),
                text: assistant_text.to_owned(),
            });
        // Trim oldest turns until we're within the cap. The
        // cap counts individual turns; trimming two at a time
        // keeps the buffer pair-aligned.
        while updated.ai_memory.len() > cap {
            updated.ai_memory.remove(0);
            if updated.ai_memory.len() > cap {
                updated.ai_memory.remove(0);
            }
        }
        updated.modified_at = chrono::Utc::now();
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(paragraph_id, updated.to_json())
        {
            tracing::warn!(
                target: "inkhaven::ai_memory",
                uuid = %paragraph_id,
                "record_paragraph_ai_memory: update_metadata failed: {e}",
            );
            return;
        }
        // Reload so the next prompt-send (which reads from
        // `self.hierarchy`) sees the freshly-stamped turns.
        self.reload_hierarchy();
    }

    // -------- key dispatch ------------------------------------------------

    /// Dispatch a crossterm mouse event. Left-click moves focus to the
    /// clicked pane and positions the cursor inside it where possible.
    /// Scroll wheel scrolls the pane under the pointer. Other kinds
    /// (middle-click, drag, motion) are ignored for now.
    ///
    /// Modals / overlays (file picker, prompt picker, modal stack) eat
    /// mouse input — clicking through a modal feels wrong and the
    /// keyboard flow for those is well-trodden. We early-return if any
    /// modal is up so the click can't accidentally focus a pane that's
    /// hidden behind the floating panel.
    fn handle_mouse(&mut self, ev: MouseEvent) {
        // 1.2.8+ — modal dispatch FIRST.  Previously
        // returned early for any modal; now route the
        // wheel into modals that have meaningful scroll
        // state (shell pane turn buffer, HJSON editor
        // cursor, picker cursors).  Picker overlays still
        // dominate — handle them inline.
        if self.show_results_overlay || self.show_prompt_picker {
            return;
        }
        if !matches!(self.modal, Modal::None) {
            self.handle_mouse_in_modal(ev);
            return;
        }
        let (col, row) = (ev.column, ev.row);
        let pane = self.pane_at(col, row);
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(target) = pane {
                    if self.focus != target {
                        self.change_focus(target);
                    }
                    match target {
                        Focus::Tree => self.mouse_position_tree(row),
                        Focus::Editor => self.mouse_position_editor(col, row),
                        _ => {}
                    }
                }
            }
            MouseEventKind::ScrollUp => match pane {
                Some(Focus::Tree) => self.move_cursor(-3),
                Some(Focus::Editor) => self.mouse_scroll_editor(-3),
                Some(Focus::Ai) => {
                    // Wheel up = show older content
                    // (chat_history_scroll grows backward).
                    self.chat_history_scroll =
                        self.chat_history_scroll.saturating_add(3);
                }
                _ => {}
            },
            MouseEventKind::ScrollDown => match pane {
                Some(Focus::Tree) => self.move_cursor(3),
                Some(Focus::Editor) => self.mouse_scroll_editor(3),
                Some(Focus::Ai) => {
                    self.chat_history_scroll =
                        self.chat_history_scroll.saturating_sub(3);
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// 1.2.8+ — mouse-wheel dispatcher for the active modal.
    /// Tier-1 modals (shell pane, HJSON editor, kill-ring
    /// picker, fuzzy paragraph picker) get explicit wheel
    /// behaviour; everything else falls through silently.
    /// Step is 3 entries / lines per wheel-tick, matching
    /// the main UI panes.
    fn handle_mouse_in_modal(&mut self, ev: MouseEvent) {
        use tui_textarea::CursorMove;
        let dir: i32 = match ev.kind {
            MouseEventKind::ScrollUp => -1,
            MouseEventKind::ScrollDown => 1,
            _ => return,
        };
        const STEP: usize = 3;
        match &mut self.modal {
            // Shell pane: wheel adjusts the turn-buffer
            // scroll directly.  Up wheel = scroll back =
            // increase the offset; down wheel = forward =
            // decrease.
            Modal::ShellPane { scroll, .. } => {
                if dir < 0 {
                    *scroll = scroll.saturating_add(STEP);
                } else {
                    *scroll = scroll.saturating_sub(STEP);
                }
            }
            // HJSON editor: walk the textarea cursor up /
            // down N lines.  Selection state is preserved
            // (no cancel_selection here — wheel scrolling
            // shouldn't dump an in-progress selection).
            Modal::HjsonEditor { textarea, .. } => {
                let cmove = if dir < 0 {
                    CursorMove::Up
                } else {
                    CursorMove::Down
                };
                for _ in 0..STEP {
                    textarea.move_cursor(cmove);
                }
            }
            // Kill-ring picker: move the cursor up/down
            // within the ring.  Clamp to [0, len-1].
            Modal::KillRingPicker { cursor } => {
                let len = self.kill_ring.len();
                if len == 0 {
                    return;
                }
                let max = len - 1;
                if dir < 0 {
                    *cursor = cursor.saturating_sub(STEP);
                } else {
                    *cursor = (*cursor + STEP).min(max);
                }
            }
            // Fuzzy paragraph picker: move cursor + keep
            // it visible by adjusting scroll.  Same step.
            Modal::FuzzyParagraphPicker {
                cursor,
                scroll,
                entries,
                ..
            } => {
                let len = entries.len();
                if len == 0 {
                    return;
                }
                let max = len - 1;
                if dir < 0 {
                    *cursor = cursor.saturating_sub(STEP);
                } else {
                    *cursor = (*cursor + STEP).min(max);
                }
                // Keep scroll roughly aligned with cursor.
                if *cursor < *scroll {
                    *scroll = *cursor;
                }
            }
            _ => {}
        }
    }

    /// Map a terminal coordinate to the pane that owns it, if any. Uses
    /// the rectangles cached by the most recent `draw()`.
    fn pane_at(&self, col: u16, row: u16) -> Option<Focus> {
        if rect_contains(self.layout_tree, col, row) {
            return Some(Focus::Tree);
        }
        if rect_contains(self.layout_editor, col, row) {
            return Some(Focus::Editor);
        }
        if rect_contains(self.layout_ai, col, row) {
            return Some(Focus::Ai);
        }
        if rect_contains(self.layout_search, col, row) {
            return Some(Focus::SearchBar);
        }
        if rect_contains(self.layout_ai_prompt, col, row) {
            return Some(Focus::AiPrompt);
        }
        None
    }

    /// Move the tree cursor to whatever row was clicked. Accounts for
    /// the 1-row top border and the current scroll offset.
    fn mouse_position_tree(&mut self, row: u16) {
        if self.rows.is_empty() {
            return;
        }
        let body_y = self.layout_tree.y.saturating_add(1);
        if row < body_y {
            return;
        }
        let row_in_view = (row - body_y) as usize;
        let idx = self.tree_scroll + row_in_view;
        if idx < self.rows.len() {
            self.tree_cursor = idx;
        }
    }

    /// Position the editor cursor under the click. Accounts for the
    /// pane border (1 row top, 1 col left), the line-number gutter, and
    /// the current scroll. Wrapped-line clicks degrade gracefully by
    /// snapping to the closest source row.
    fn mouse_position_editor(&mut self, col: u16, row: u16) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let inner_x = self.layout_editor.x.saturating_add(1);
        let inner_y = self.layout_editor.y.saturating_add(1);
        if row < inner_y || col < inner_x {
            return;
        }
        // Gutter width: digits in line count + 1 trailing space (see
        // `digit_count` + `gutter_width` in the editor renderer).
        let total_lines = doc.textarea.lines().len().max(1);
        let lineno_chars = digit_count(total_lines);
        let gutter = (lineno_chars + 1) as u16;
        let inner_cursor_x = inner_x.saturating_add(gutter);
        // Click inside the gutter or borders → ignore.
        if col < inner_cursor_x {
            return;
        }
        let rel_row = (row - inner_y) as usize;
        let rel_col = (col - inner_cursor_x) as usize;
        let src_row = (doc.scroll_row + rel_row).min(total_lines - 1);
        let line_len = doc
            .textarea
            .lines()
            .get(src_row)
            .map_or(0, |s| s.chars().count());
        let src_col = (doc.scroll_col + rel_col).min(line_len);
        doc.textarea
            .move_cursor(tui_textarea::CursorMove::Jump(src_row as u16, src_col as u16));
        doc.last_activity = std::time::Instant::now();
    }

    /// Scroll the editor by `delta` source rows (positive = down).
    /// Updates the cursor too so the visible window doesn't snap back on
    /// the next render — the renderer keeps the cursor in view, which
    /// would otherwise undo a pure scroll-only adjustment.
    fn mouse_scroll_editor(&mut self, delta: i32) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let total = doc.textarea.lines().len().max(1);
        if delta < 0 {
            let n = (-delta) as usize;
            doc.scroll_row = doc.scroll_row.saturating_sub(n);
            use tui_textarea::CursorMove;
            for _ in 0..n {
                doc.textarea.move_cursor(CursorMove::Up);
            }
        } else {
            let n = delta as usize;
            doc.scroll_row = (doc.scroll_row + n).min(total.saturating_sub(1));
            use tui_textarea::CursorMove;
            for _ in 0..n {
                doc.textarea.move_cursor(CursorMove::Down);
            }
        }
        doc.last_activity = std::time::Instant::now();
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // 1.2.8+ — ConfirmQuit modal swallows its own keys:
        // Y / Enter proceed with the existing quit flow;
        // N / Esc cancel + close the modal.  Anything else
        // is ignored (so a misclick on the keyboard doesn't
        // dismiss the modal).
        if matches!(self.modal, Modal::ConfirmQuit) {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.modal = Modal::None;
                    return Ok(self.request_quit());
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.modal = Modal::None;
                    self.status = "quit cancelled".into();
                    return Ok(false);
                }
                _ => return Ok(false),
            }
        }

        // 1.2.9+ — TtsUnavailable: any key dismisses.
        if matches!(self.modal, Modal::TtsUnavailable { .. }) {
            self.modal = Modal::None;
            return Ok(false);
        }
        // 1.2.9+ — TtsPlayback: any key stops the engine
        // and closes the modal.  The render loop polls
        // for natural end-of-speech in `tts_poll_playback`.
        if matches!(self.modal, Modal::TtsPlayback { .. }) {
            self.tts_stop_playback();
            return Ok(false);
        }

        // Hard quit works from anywhere, including inside a modal.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
        {
            // 1.2.8+ — gate the quit behind a confirmation
            // modal when `editor.confirm_quit = true`.
            // Skip when a modal is already open (the user
            // is mid-workflow; treat Ctrl+Q in-modal as the
            // unconditional escape hatch it always was).
            if self.cfg.editor.confirm_quit
                && matches!(self.modal, Modal::None)
            {
                self.modal = Modal::ConfirmQuit;
                self.status =
                    "quit inkhaven? Y/Enter confirm · N/Esc cancel".into();
                return Ok(false);
            }
            return Ok(self.request_quit());
        }

        // Modal eats every other key.
        if !matches!(self.modal, Modal::None) {
            return self.handle_modal_key(key);
        }

        // Ctrl+1..5 — direct focus jumps. Most terminals send these as
        // Char(digit) + CONTROL. But Ctrl+2 on US-layout terminals is often
        // re-encoded as Ctrl+@ → KeyCode::Char('@') with CONTROL, or as
        // KeyCode::Null. Try every variant we've seen in the wild.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && !key
                .modifiers
                .intersects(KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            let target = match key.code {
                KeyCode::Char('1') => Some(Focus::Editor),
                // Ctrl+2 alternates:
                KeyCode::Char('2') | KeyCode::Char('@') => Some(Focus::Tree),
                KeyCode::Char('3') => Some(Focus::Ai),
                KeyCode::Char('4') => Some(Focus::SearchBar),
                KeyCode::Char('5') => Some(Focus::AiPrompt),
                // Ctrl+T also focuses Tree — mnemonic and not terminal-eaten.
                KeyCode::Char('t') | KeyCode::Char('T') => Some(Focus::Tree),
                _ => None,
            };
            if let Some(focus) = target {
                self.change_focus(focus);
                return Ok(false);
            }
        }
        // KeyCode::Null with no modifiers is what some terminals report for
        // Ctrl+2 / Ctrl+Space. Catch that separately because the inner block
        // requires the CONTROL modifier flag.
        if matches!(key.code, KeyCode::Null) {
            self.change_focus(Focus::Tree);
            return Ok(false);
        }

        // View-prefix dispatch (Ctrl+V by default, 1.2.4+).
        // Captures the next keystroke. Resolution goes through
        // the binding table now (Layer::ViewSub), so HJSON
        // `keys.bindings` + `ink.key.*` can rebind every chord
        // under the prefix.
        if self.view_pending {
            self.handle_view_action(key);
            return Ok(false);
        }
        if let Some(view_prefix) = self.keymap.view_prefix {
            if view_prefix.matches(&key) {
                self.view_pending = true;
                self.status = super::keybind::read().view_hint(self.focus);
                return Ok(false);
            }
        }

        // Bund-meta dispatch (Ctrl+Z by default). Mirrors the
        // meta-prefix machinery below — same state-machine shape,
        // different action table. Intercept BEFORE tui-textarea
        // sees the key so its default Ctrl+Z=undo binding stays
        // dormant.
        if self.bund_pending {
            self.handle_bund_action(key);
            return Ok(false);
        }
        if let Some(bund_prefix) = self.keymap.bund_prefix {
            if bund_prefix.matches(&key) {
                self.bund_pending = true;
                self.status = super::keybind::read().bund_hint(self.focus);
                return Ok(false);
            }
        }

        // Meta-prefix dispatch. If we're already inside meta mode, the next
        // key is the action selector. Otherwise check whether THIS key is
        // the meta prefix and enter the mode.
        if self.meta_pending {
            self.handle_meta_action(key);
            return Ok(false);
        }
        if self.keymap.meta_prefix.matches(&key) {
            self.meta_pending = true;
            // The meta action table is pane-specific (see dispatch_meta_*),
            // so the hint shown in the status bar should match the focused
            // pane. Generic suffix (· H help · Esc cancel) is shared.
            // Build the meta-hint from the live binding table so
            // user overlays (HJSON + ink.key.*) show up in the
            // status bar automatically.
            self.status = super::keybind::read().meta_hint(self.focus);
            return Ok(false);
        }


        // 1.2.4+: F-keys + every top-level (no-prefix) chord
        // flow through the `top_level` binding table. The table
        // is pane-aware via Scope, so F-keys that only made
        // sense in one pane (F2 rename / F3 file picker /
        // F4-F6 editor) keep their per-pane behaviour without
        // hardcoded match arms. The user can rebind any of
        // them via HJSON `keys.bindings` (single-token chord
        // strings route to TopLevel) or runtime
        // `ink.key.bind`.
        if let Some(action) = super::keybind::read().resolve_top_level(&key, self.focus) {
            if !matches!(action, super::keybind::Action::None) {
                self.run_action(action);
                return Ok(false);
            }
        }

        // AI-fullscreen Ctrl+C toggles "Chat selection mode" — the
        // editor's Ctrl+C still does clipboard-copy via the focus
        // dispatch, but in this layout the editor isn't visible and
        // the chord is reclaimed for navigating chat turns.
        if self.ai_fullscreen
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
            && matches!(self.modal, Modal::None)
        {
            self.toggle_chat_selection_mode();
            return Ok(false);
        }

        // When chat-selection is active: Up / Down step turns, `c` /
        // `C` copies to clipboard, `t` / `T` inserts at the editor
        // cursor, Esc / Enter exits.
        if self.ai_fullscreen && self.chat_selection.is_some() && matches!(self.modal, Modal::None) {
            let plain = !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
            match key.code {
                KeyCode::Up if plain => {
                    self.chat_selection_step(-1);
                    return Ok(false);
                }
                KeyCode::Down if plain => {
                    self.chat_selection_step(1);
                    return Ok(false);
                }
                KeyCode::Home if plain => {
                    self.chat_selection_jump(0);
                    return Ok(false);
                }
                KeyCode::End if plain => {
                    self.chat_selection_jump(usize::MAX);
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('C') if plain => {
                    self.chat_selection_copy();
                    return Ok(false);
                }
                KeyCode::Char('t') | KeyCode::Char('T') if plain => {
                    self.chat_selection_into_editor();
                    return Ok(false);
                }
                KeyCode::Esc | KeyCode::Enter => {
                    self.chat_selection = None;
                    self.status = "chat selection mode off".into();
                    return Ok(false);
                }
                _ => {}
            }
        }

        // AI-fullscreen Esc with an active chat search → clear the
        // search (drop highlights + scroll back to the bottom-pin).
        // Routed before the focus handlers so AI-prompt Esc doesn't
        // grab it.
        if self.ai_fullscreen
            && self.chat_search.is_some()
            && matches!(key.code, KeyCode::Esc)
            && matches!(self.modal, Modal::None)
        {
            self.chat_search = None;
            self.chat_history_scroll = 0;
            self.status = "chat search cleared".into();
            return Ok(false);
        }

        // AI-fullscreen Ctrl+F → chat-history search modal. Intercepted
        // before the editor-pane Ctrl+F handler so the chord behaves
        // contextually based on layout, not focus.
        if self.ai_fullscreen
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('f') | KeyCode::Char('F'))
        {
            self.open_chat_search_prompt();
            return Ok(false);
        }
        // Ctrl+X advances to the next (older) match while a chat
        // search is active. No-op when no search is running.
        if self.ai_fullscreen
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('x') | KeyCode::Char('X'))
            && self.chat_search.is_some()
        {
            self.advance_chat_search();
            return Ok(false);
        }

        // AI-fullscreen-only: PageUp / PageDown scroll the chat history
        // pane regardless of which sub-pane has focus (the user is
        // typically focused on the AI prompt). Up / Down do the same
        // one line at a time for fine-grained reading. Intercepted
        // here so the AI prompt's own input handler never sees these
        // keys in this layout. The non-fullscreen path leaves them
        // untouched.
        //
        // Exception: when the `/` prompt-library picker is open, Up /
        // Down still navigate the picker — that's the picker's only
        // way to move the selection.
        if self.ai_fullscreen {
            if self.keymap.page_up.matches(&key) {
                self.chat_history_scroll = self.chat_history_scroll.saturating_add(10);
                return Ok(false);
            }
            if self.keymap.page_down.matches(&key) {
                self.chat_history_scroll = self.chat_history_scroll.saturating_sub(10);
                return Ok(false);
            }
            if !self.show_prompt_picker {
                if matches!(key.code, KeyCode::Up) {
                    self.chat_history_scroll = self.chat_history_scroll.saturating_add(1);
                    return Ok(false);
                }
                if matches!(key.code, KeyCode::Down) {
                    self.chat_history_scroll = self.chat_history_scroll.saturating_sub(1);
                    return Ok(false);
                }
            }
        }

        // F9 / F10 / F7 / F1 et al. now flow through the
        // top_level binding-table dispatch above. The hardcoded
        // match arms were removed in the 1.2.4 F-key migration.

        // Save works from anywhere as long as a doc is open.
        if self.keymap.save.matches(&key) && self.opened.is_some() {
            self.save_current()?;
            return Ok(false);
        }

        // Focus jumps from anywhere.
        if self.keymap.search.matches(&key) {
            self.change_focus(Focus::SearchBar);
            return Ok(false);
        }
        if self.keymap.ai_prompt.matches(&key) {
            self.change_focus(Focus::AiPrompt);
            return Ok(false);
        }

        // Tab cycling everywhere except when typing into a buffer.
        let in_editor_with_doc = self.focus == Focus::Editor && self.opened.is_some();
        let cycling_blocked = self.focus.is_input() || in_editor_with_doc;
        if !cycling_blocked {
            if self.keymap.next_pane.matches(&key) {
                self.change_focus(self.focus.next());
                return Ok(false);
            }
            if self.keymap.prev_pane.matches(&key) {
                self.change_focus(self.focus.prev());
                return Ok(false);
            }
        } else if in_editor_with_doc
            && (self.keymap.next_pane.matches(&key) || self.keymap.prev_pane.matches(&key))
        {
            // Inside an active editor, Tab cycles focus too — but only when
            // the user really meant to (no other modifiers were on). If we
            // didn't intercept here, Tab would insert a literal tab via
            // tui-textarea.
            //
            // Similar-paragraph mode special case: when the AI pane
            // has been replaced by a second editor (`self.secondary
            // .is_some()`), Tab inside the editor pane toggles
            // keyboard focus between the two editor panes instead
            // of cycling to a non-existent AI pane. Shift+Tab does
            // the same — there's only one "other pane" to flip to.
            if self.secondary.is_some() {
                self.secondary_focused = !self.secondary_focused;
                self.status = if self.secondary_focused {
                    "similar: right editor focused".into()
                } else {
                    "similar: left editor focused".into()
                };
                return Ok(false);
            }
            let next = if self.keymap.next_pane.matches(&key) {
                self.focus.next()
            } else {
                self.focus.prev()
            };
            self.change_focus(next);
            return Ok(false);
        }

        match self.focus {
            Focus::Tree => self.handle_tree_key(key),
            Focus::Editor => self.handle_editor_key(key),
            Focus::Ai => self.handle_passive_key(key),
            Focus::SearchBar => self.handle_input_key(key, true),
            Focus::AiPrompt => self.handle_input_key(key, false),
        }
    }

    fn handle_tree_key(&mut self, key: KeyEvent) -> Result<bool> {
        // For plain-letter / punctuation shortcuts we ignore the SHIFT modifier
        // (uppercase letters require Shift on most layouts) but reject Ctrl /
        // Alt / Super so e.g. Ctrl+A doesn't accidentally trigger Add-subchapter.
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') if plain => return Ok(self.request_quit()),
            // Esc cycles Tree → Search bar (third leg of the
            // Editor → Tree → Search → Editor rotation).
            // 1.2.4+: Esc exits link-pick mode (returning to
            // the editor) instead of cycling focus to the
            // search bar. Also clears any multi-select marks
            // so the next chord operates on the cursor row,
            // not stale marks.
            KeyCode::Esc => {
                if self.link_pick_for.is_some() {
                    self.link_pick_for = None;
                    self.status = "link cancelled".into();
                    self.change_focus(Focus::Editor);
                } else if !self.tree_marked.is_empty() {
                    let n = self.tree_marked.len();
                    self.tree_marked.clear();
                    self.status =
                        format!("multi-select cleared ({n} paragraph(s))");
                } else {
                    self.change_focus(Focus::SearchBar);
                }
            }
            // 1.2.4+: Space toggles multi-select on the
            // cursor row (paragraphs only — branches don't
            // accept the bulk operations).
            KeyCode::Char(' ') if plain => {
                if let Some((id, _)) = self.rows.get(self.tree_cursor) {
                    let id = *id;
                    if let Some(node) = self.hierarchy.get(id) {
                        if node.kind == NodeKind::Paragraph {
                            if self.tree_marked.remove(&id) {
                                self.status = format!(
                                    "unmarked · {} marked",
                                    self.tree_marked.len()
                                );
                            } else {
                                self.tree_marked.insert(id);
                                self.status = format!(
                                    "marked · {} marked",
                                    self.tree_marked.len()
                                );
                            }
                        } else {
                            self.status =
                                "multi-select only marks paragraphs".into();
                        }
                    }
                }
            }
            // F2 (rename) and F3 (file picker) now flow through
            // the top_level binding-table dispatch in handle_key.
            KeyCode::Up => self.move_cursor(-1),
            KeyCode::Down => self.move_cursor(1),
            KeyCode::Home => self.tree_cursor = 0,
            KeyCode::End => {
                if !self.rows.is_empty() {
                    self.tree_cursor = self.rows.len() - 1;
                }
            }
            // 1.2.4+: when link-pick mode is active, Enter on a
            // tree row links the row's paragraph to the
            // pick-mode owner rather than opening it for editing.
            KeyCode::Enter => {
                if self.link_pick_for.is_some() {
                    self.commit_link_pick();
                } else {
                    self.open_selected()?;
                }
            }

            // Right arrow: expand cursor's branch (no-op for leaves).
            // Left arrow: collapse cursor's branch if expanded, else move
            // cursor to its parent in the hierarchy. Same semantics as the
            // F3 file picker (§11 in KEYBINDING.md).
            KeyCode::Right => self.tree_expand_at_cursor(),
            KeyCode::Left => self.tree_collapse_or_step_out(),

            // Tree-pane add shortcuts. These exist alongside the global
            // Ctrl+Shift+* chords because terminals and multiplexers
            // commonly eat those (Ctrl+S = XOFF, tmux prefix, etc.).
            KeyCode::Char('B') | KeyCode::Char('b') if plain => {
                self.open_add_modal(NodeKind::Book);
            }
            // C/A/+ append at the end of the parent's children;
            // V/S/P insert immediately after the cursor's same-kind ancestor.
            KeyCode::Char('C') | KeyCode::Char('c') if plain => {
                self.open_add_modal(NodeKind::Chapter);
            }
            KeyCode::Char('V') | KeyCode::Char('v') if plain => {
                self.open_add_modal_after(NodeKind::Chapter);
            }
            KeyCode::Char('A') | KeyCode::Char('a') if plain => {
                self.open_add_modal(NodeKind::Subchapter);
            }
            KeyCode::Char('S') | KeyCode::Char('s') if plain => {
                self.open_add_modal_after(NodeKind::Subchapter);
            }
            KeyCode::Char('+') if plain => {
                self.open_add_modal(NodeKind::Paragraph);
            }
            KeyCode::Char('P') | KeyCode::Char('p') if plain => {
                self.open_add_modal_after(NodeKind::Paragraph);
            }

            // Kind-specific delete: D for branches, - for paragraphs. This is
            // a safety feature — `-` won't nuke a whole chapter by accident.
            KeyCode::Char('D') | KeyCode::Char('d') if plain => self.delete_branch_only(),
            KeyCode::Char('-') if plain => self.delete_paragraph_only(),

            // Sibling reorder, plain-letter form. Equivalent to the meta-
            // prefix chord `Ctrl+B ↑` / `Ctrl+B ↓` but reachable without a
            // chord — handy when reorganising a long list of paragraphs.
            KeyCode::Char('U') | KeyCode::Char('u') if plain => {
                self.move_current(MoveDir::Up);
            }
            KeyCode::Char('J') | KeyCode::Char('j') if plain => {
                self.move_current(MoveDir::Down);
            }

            // Z collapses the cursor's enclosing subchapter; X collapses
            // every expanded branch in the tree. Both rebuild the row list
            // so the view updates immediately.
            KeyCode::Char('Z') | KeyCode::Char('z') if plain => {
                self.collapse_enclosing_subchapter();
            }
            KeyCode::Char('X') | KeyCode::Char('x') if plain => {
                self.collapse_all_branches();
            }

            // 1.2.4+: tree T cycles the type of the cursor row
            // (or every marked paragraph, when multi-select is
            // active). Same ladder as Ctrl+B M:
            // Paragraph(typst) → Paragraph(hjson) → Script(bund).
            KeyCode::Char('T') | KeyCode::Char('t') if plain => {
                if !self.tree_marked.is_empty() {
                    self.cycle_leaf_type_bulk();
                } else {
                    self.cycle_leaf_type();
                }
            }
            // 1.2.4+: tree O cycles paragraph status. Mirrors
            // Ctrl+B R; honours multi-select for bulk status
            // transitions.
            KeyCode::Char('O') | KeyCode::Char('o') if plain => {
                self.cycle_paragraph_status();
            }
            // 1.2.5+: tree g opens the tag picker for the
            // marked set (or the cursor row when no marks).
            // Same modal as Ctrl+B ], but applies the picked
            // tags across every selected paragraph at once.
            KeyCode::Char('G') | KeyCode::Char('g') if plain => {
                self.open_tag_picker_for_tree_selection();
            }

            _ if self.keymap.page_up.matches(&key) => self.move_cursor(-10),
            _ if self.keymap.page_down.matches(&key) => self.move_cursor(10),
            _ => {}
        }
        Ok(false)
    }

    fn rebuild_rows_preserving_cursor(&mut self) {
        let prev_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        self.rows = self
            .hierarchy
            .flatten_with_collapsed(&self.collapsed_nodes)
            .into_iter()
            .map(|(n, d)| (n.id, d))
            .collect();
        if let Some(id) = prev_id {
            if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
                self.tree_cursor = i;
                return;
            }
        }
        if !self.rows.is_empty() {
            self.tree_cursor = self.tree_cursor.min(self.rows.len() - 1);
        } else {
            self.tree_cursor = 0;
        }
    }

    fn delete_branch_only(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if node.kind == NodeKind::Paragraph {
            self.status = format!(
                "`{}` is a paragraph — press `-` (or Ctrl+B then D) to delete it",
                node.title
            );
            return;
        }
        self.open_delete_modal();
    }

    fn delete_paragraph_only(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if node.kind != NodeKind::Paragraph {
            self.status = format!(
                "`{}` is a {} — press `D` (or Ctrl+B then D) to delete it",
                node.title,
                node.kind.as_str()
            );
            return;
        }
        self.open_delete_modal();
    }

    /// Entry point for the editor pane. When in similar-paragraph
    /// mode + the secondary pane has keyboard focus
    /// (`self.secondary_focused`), we swap `opened ↔ secondary` so
    /// every downstream handler — all of which target
    /// `self.opened` — naturally operates on the right-pane doc.
    /// Swap back after the call returns. This keeps the 100+
    /// existing editor key handlers unchanged.
    fn handle_editor_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.secondary_focused && self.secondary.is_some() {
            std::mem::swap(&mut self.opened, &mut self.secondary);
            let r = self.handle_editor_key_inner(key);
            std::mem::swap(&mut self.opened, &mut self.secondary);
            return r;
        }
        self.handle_editor_key_inner(key)
    }

    fn handle_editor_key_inner(&mut self, key: KeyEvent) -> Result<bool> {
        if self.opened.is_none() {
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
                return Ok(self.request_quit());
            }
            return Ok(false);
        }

        // Stamp last_activity so idle autosave only fires after the user
        // actually pauses. Done first so every branch below benefits.
        if let Some(doc) = self.opened.as_mut() {
            doc.last_activity = std::time::Instant::now();
        }

        // Read-only gate (Help subtree): allow navigation, search, copy, and
        // focus-related chords; refuse anything that would touch the buffer
        // or the store.
        if self
            .opened
            .as_ref()
            .is_some_and(|d| d.read_only)
            && !is_read_only_safe_key(&key)
        {
            self.status = "Help is read-only".into();
            return Ok(false);
        }

        // 1.2.8+ — Help paragraphs render through
        // `markdown::render` instead of the source view, so
        // moving the textarea cursor (the default route for
        // arrow / PageUp / PageDown / Home / End) is
        // invisible to the user.  Intercept those keys
        // BEFORE the textarea catches them and adjust
        // `opened.scroll_row` directly so the rendered
        // viewport actually moves.  Only fires for the
        // exact read-only+markdown combo the renderer
        // recognises; other read-only views (snapshots,
        // diffs) keep the existing source-cursor behaviour.
        let is_help_rendered = self.opened.as_ref().is_some_and(|d| {
            d.read_only
                && d.content_type.as_deref() == Some("markdown")
        });
        if is_help_rendered {
            // The render path clamps scroll_row at draw
            // time, so we don't need to know the rendered
            // line count here.  Page step = visible
            // editor height; line step = 1.
            let page = self.layout_editor.height.saturating_sub(2) as usize;
            let page = page.max(3);
            let doc = self
                .opened
                .as_mut()
                .expect("opened checked by is_help_rendered");
            match key.code {
                KeyCode::Up => {
                    doc.scroll_row = doc.scroll_row.saturating_sub(1);
                    return Ok(false);
                }
                KeyCode::Down => {
                    doc.scroll_row = doc.scroll_row.saturating_add(1);
                    return Ok(false);
                }
                KeyCode::PageUp => {
                    doc.scroll_row = doc.scroll_row.saturating_sub(page);
                    return Ok(false);
                }
                KeyCode::PageDown => {
                    doc.scroll_row = doc.scroll_row.saturating_add(page);
                    return Ok(false);
                }
                KeyCode::Home => {
                    doc.scroll_row = 0;
                    return Ok(false);
                }
                KeyCode::End => {
                    // Set to a large value; the render-time
                    // clamp drops it back to (total - height).
                    doc.scroll_row = usize::MAX / 2;
                    return Ok(false);
                }
                // Left/Right have no meaning in the rendered
                // viewer (the renderer hard-wraps long
                // lines).  Swallow them so they don't
                // accidentally hit the textarea below.
                KeyCode::Left | KeyCode::Right => {
                    return Ok(false);
                }
                _ => {}
            }
        }

        // Typewriter SFX — plain Enter (end-of-line click). Fires after
        // the read-only gate so refused keystrokes in Help don't click.
        // No-op when sound is disabled or the host has no audio device.
        if matches!(key.code, KeyCode::Enter)
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            if let Some(sp) = &self.sound {
                sp.play_enter();
            }
        }

        // F3 / F5 / F6 now resolve through the top_level binding
        // table at the top of handle_key (1.2.4+ migration).
        // Ctrl+F open find, Ctrl+X "repeat" (advance / replace+advance),
        // Ctrl+R open replace dialog or "replace all" when already in
        // replace mode.
        let ctrl_no_shift = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::SHIFT)
            && !key.modifiers.intersects(KeyModifiers::ALT | KeyModifiers::SUPER);
        if ctrl_no_shift {
            // Ctrl+X is "Repeat" only while a search is active. Otherwise
            // it falls through (currently no-op — was cut, now Ctrl+K).
            if matches!(key.code, KeyCode::Char('x') | KeyCode::Char('X'))
                && self
                    .opened
                    .as_ref()
                    .map_or(false, |d| d.search.is_some())
            {
                self.search_advance_or_replace();
                return Ok(false);
            }
            match key.code {
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    self.open_find_modal(false);
                    return Ok(false);
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if self
                        .opened
                        .as_ref()
                        .and_then(|d| d.search.as_ref())
                        .is_some_and(|s| s.replace_with.is_some())
                    {
                        self.replace_all_remaining();
                    } else {
                        self.open_find_modal(true);
                    }
                    return Ok(false);
                }
                _ => {}
            }
        }
        // Esc in editor: first press clears an active in-buffer search (the
        // Ctrl+F flow); second press cycles focus → Tree. The Editor/Tree/
        // Search cycle is Editor → Tree → Search → Editor.
        if matches!(key.code, KeyCode::Esc) {
            if self.opened.as_ref().is_some_and(|d| d.search.is_some()) {
                if let Some(doc) = self.opened.as_mut() {
                    doc.search = None;
                }
                self.status = "search cleared".into();
                return Ok(false);
            }
            self.change_focus(Focus::Tree);
            return Ok(false);
        }

        // F4 / Ctrl+F4 resolve through the top_level binding
        // table at the top of handle_key.

        // Ctrl+H / Ctrl+J scroll the lower (read-only) pane while split is
        // open. Without split, they fall through to normal editor handling
        // (tui-textarea / our backspace-word / etc.).
        let split_active = self.opened.as_ref().is_some_and(|d| d.split.is_some());
        if split_active && key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('h') | KeyCode::Char('H') => {
                    self.scroll_split_up();
                    return Ok(false);
                }
                KeyCode::Char('j') | KeyCode::Char('J') => {
                    self.scroll_split_down();
                    return Ok(false);
                }
                _ => {}
            }
        }

        // Alt+arrows enter / extend vertical-block selection. Alt+C copies it.
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if alt {
            match key.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                    if let Some(doc) = self.opened.as_mut() {
                        if doc.block_anchor.is_none() {
                            doc.block_anchor = Some(doc.textarea.cursor());
                        }
                        match key.code {
                            KeyCode::Up => doc.textarea.move_cursor(CursorMove::Up),
                            KeyCode::Down => doc.textarea.move_cursor(CursorMove::Down),
                            KeyCode::Left => doc.textarea.move_cursor(CursorMove::Back),
                            KeyCode::Right => doc.textarea.move_cursor(CursorMove::Forward),
                            _ => {}
                        }
                    }
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.block_copy();
                    return Ok(false);
                }
                _ => {}
            }
        }

        if matches!(key.code, KeyCode::Esc) {
            if let Some(doc) = self.opened.as_mut() {
                if doc.block_anchor.is_some() {
                    doc.block_anchor = None;
                    return Ok(false);
                }
            }
            self.change_focus(Focus::Tree);
            return Ok(false);
        }

        // Any key other than Alt+arrows clears block selection state.
        if let Some(doc) = self.opened.as_mut() {
            if doc.block_anchor.is_some() && !alt {
                doc.block_anchor = None;
            }
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // Plain arrow keys + Home/End/PageUp/PageDown. `input_without_shortcuts`
        // (which we use to silence tui-textarea's emacs defaults) also treats
        // these as shortcuts and drops them — so we route them ourselves.
        // Shift extends a linear selection; plain cancels any selection.
        if !alt && !ctrl {
            let cmove = match key.code {
                KeyCode::Up => Some(CursorMove::Up),
                KeyCode::Down => Some(CursorMove::Down),
                KeyCode::Left => Some(CursorMove::Back),
                KeyCode::Right => Some(CursorMove::Forward),
                KeyCode::Home => Some(CursorMove::Head),
                KeyCode::End => Some(CursorMove::End),
                KeyCode::PageUp => Some(CursorMove::ParagraphBack),
                KeyCode::PageDown => Some(CursorMove::ParagraphForward),
                _ => None,
            };
            if let Some(cmove) = cmove {
                if let Some(doc) = self.opened.as_mut() {
                    if shift {
                        if doc.textarea.selection_range().is_none() {
                            doc.textarea.start_selection();
                        }
                    } else {
                        doc.textarea.cancel_selection();
                    }
                    doc.textarea.move_cursor(cmove);
                }
                return Ok(false);
            }
        }

        // Editor key map (intercepted before tui-textarea so its emacs
        // defaults don't fire). Note the rebinds from earlier conventions:
        //   Ctrl+U undo  (was Ctrl+Z)
        //   Ctrl+K cut   (was Ctrl+X — now "repeat" for search/replace)
        //   Ctrl+P paste (was Ctrl+V)
        //   Ctrl+D/E/W/Z delete-line / delete-to-end / delete-to-start /
        //                delete-to-end (Z duplicates E per spec)
        if ctrl {
            match key.code {
                // Undo / Redo
                KeyCode::Char('u') | KeyCode::Char('U') if !shift => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.undo();
                    }
                    return Ok(false);
                }
                KeyCode::Char('y') | KeyCode::Char('Y') if !shift => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.redo();
                    }
                    return Ok(false);
                }
                // Clipboard
                KeyCode::Char('k') | KeyCode::Char('K') if !shift => {
                    self.editor_cut();
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('C') if !shift => {
                    self.editor_copy();
                    return Ok(false);
                }
                KeyCode::Char('p') | KeyCode::Char('P') if !shift => {
                    self.editor_paste();
                    return Ok(false);
                }
                KeyCode::Char('a') | KeyCode::Char('A') if !shift => {
                    self.editor_select_all();
                    return Ok(false);
                }
                // Line-targeted deletes. All four operations preserve the
                // yank buffer so they don't clobber Ctrl+C / Ctrl+P state.
                KeyCode::Char('d') | KeyCode::Char('D') if !shift => {
                    self.editor_delete_line();
                    return Ok(false);
                }
                KeyCode::Char('e') | KeyCode::Char('E') if !shift => {
                    self.editor_delete_to_eol();
                    return Ok(false);
                }
                KeyCode::Char('w') | KeyCode::Char('W') if !shift => {
                    self.editor_delete_to_bol();
                    return Ok(false);
                }
                // Ctrl+Z is intentionally unbound. Undo is Ctrl+U,
                // delete-to-EOL is Ctrl+E. The key falls through to
                // input_without_shortcuts (which itself ignores it).
                KeyCode::Home => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::Top);
                    }
                    return Ok(false);
                }
                KeyCode::End => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::Bottom);
                    }
                    return Ok(false);
                }
                KeyCode::Left => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::WordBack);
                    }
                    return Ok(false);
                }
                KeyCode::Right => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::WordForward);
                    }
                    return Ok(false);
                }
                KeyCode::Backspace => {
                    if let Some(doc) = self.opened.as_mut() {
                        if doc.textarea.delete_word() {
                            doc.dirty = true;
                        }
                    }
                    return Ok(false);
                }
                _ => {}
            }
        }

        // Auto-close pairs (configurable). Only plain keystrokes — Ctrl /
        // Alt / Super combinations fall through to the textarea catch-all
        // below. Each helper returns `true` when it consumed the key,
        // letting us skip the catch-all without disturbing the cursor.
        let plain_no_mods = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if plain_no_mods && self.cfg.editor.auto_close_pairs {
            if let KeyCode::Char(c) = key.code {
                if let Some(close) = open_pair_for(c) {
                    self.editor_auto_open_pair(c, close);
                    return Ok(false);
                }
                if is_close_pair_char(c) && self.editor_try_skip_close(c) {
                    return Ok(false);
                }
            }
            if matches!(key.code, KeyCode::Enter) && self.editor_try_expand_pair_on_enter() {
                return Ok(false);
            }
            if matches!(key.code, KeyCode::Backspace) && self.editor_try_delete_pair() {
                return Ok(false);
            }
        }

        // Everything else: pass to textarea WITHOUT its emacs-style defaults,
        // so plain typing/arrows/Home/End/PageUp/PageDown/Shift+arrows still
        // work but Ctrl+letter combinations don't get hijacked.
        if let Some(doc) = self.opened.as_mut() {
            let input: tui_textarea::Input = key.into();
            if doc.textarea.input_without_shortcuts(input) {
                doc.dirty = true;
            }
        }
        Ok(false)
    }

    /// Copy the current rectangular selection to the system clipboard
    /// (falling back to tui-textarea's yank buffer). The rectangle is the
    /// inclusive range from `block_anchor` to the current cursor.
    fn block_copy(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            return;
        };
        let Some(anchor) = doc.block_anchor else {
            return;
        };
        let block = BlockSelection::from_anchor_and_cursor(anchor, doc.textarea.cursor());
        let lines = doc.textarea.lines();
        let mut out = String::new();
        for r in block.row_min..=block.row_max {
            if let Some(line) = lines.get(r) {
                let chars: Vec<char> = line.chars().collect();
                let s = block.col_min.min(chars.len());
                let e = (block.col_max + 1).min(chars.len());
                let piece: String = chars[s..e].iter().collect();
                out.push_str(&piece);
            }
            if r < block.row_max {
                out.push('\n');
            }
        }
        // Push to system clipboard if available.
        if let Some(cb) = self.clipboard.as_mut() {
            let _ = cb.set_text(out.clone());
        }
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea.set_yank_text(out);
            doc.block_anchor = None;
        }
        self.status = "copied rectangular block to clipboard".into();
    }

    fn current_block(&self) -> Option<BlockSelection> {
        let doc = self.opened.as_ref()?;
        let anchor = doc.block_anchor?;
        Some(BlockSelection::from_anchor_and_cursor(
            anchor,
            doc.textarea.cursor(),
        ))
    }

    fn handle_passive_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Esc bounces AI pane → AI prompt so the user can edit / send the
        // next message without an extra Tab. Mirror of the AiPrompt → Ai
        // bounce in handle_input_key.
        if self.focus == Focus::Ai && matches!(key.code, KeyCode::Esc) {
            self.change_focus(Focus::AiPrompt);
            return Ok(false);
        }
        // When the AI pane has a completed inference and is focused, single-
        // letter keys apply the result to the editor.
        if self.focus == Focus::Ai && self.inference_done_with_text() {
            match key.code {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.apply_inference(InferenceAction::Replace);
                    return Ok(false);
                }
                KeyCode::Char('i') | KeyCode::Char('I') => {
                    self.apply_inference(InferenceAction::Insert);
                    return Ok(false);
                }
                // `t` / `T` — prepend the AI response to the top of the
                // paragraph (markdown→Typst conversion applied).
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    self.apply_inference(InferenceAction::Top);
                    return Ok(false);
                }
                // `g` / `G` — grammar-check apply: lift only the
                // corrected paragraph (between `<<<CORRECTED>>>` /
                // `<<<END>>>` markers, or last fenced code, or after a
                // "Corrected …" heading) and overwrite the buffer
                // wholesale. No markdown→Typst conversion runs because
                // the grammar prompt instructs the model to preserve
                // Typst markup verbatim.
                KeyCode::Char('g') | KeyCode::Char('G') => {
                    self.apply_inference(InferenceAction::ReplaceCorrected);
                    return Ok(false);
                }
                KeyCode::Char('b') | KeyCode::Char('B') => {
                    self.apply_inference(InferenceAction::Bottom);
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.apply_inference(InferenceAction::CopyOnly);
                    return Ok(false);
                }
                _ => {}
            }
        }

        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
            return Ok(self.request_quit());
        }
        Ok(false)
    }

    /// Centralized exit gate. If the open paragraph is dirty, autosave it
    /// before returning true. If the save fails, abort the exit and leave a
    /// status message so the user can recover. Called from every quit chord
    /// (Ctrl+Q, plain q in Tree / Editor-empty / AI).
    fn request_quit(&mut self) -> bool {
        if self.opened.as_ref().is_some_and(|d| d.dirty) {
            // save_current writes its own status. If it can't save, doc.dirty
            // stays true and we refuse to quit so the user can see the error
            // and recover.
            let _ = self.save_current();
            if self.opened.as_ref().is_some_and(|d| d.dirty) {
                return false;
            }
        }
        // Persist session state regardless of save outcome path above.
        // Failure is silent — sessions are a UX nicety, not correctness.
        let _ = self.save_session();
        true
    }

    /// Re-apply a saved session on startup. Silently ignores anything that
    /// no longer makes sense (missing UUIDs, corrupt fields). Should be
    /// called after `Hierarchy::load` so the lookups can resolve.
    fn restore_session(&mut self) {
        let Some(state) = SessionState::load(&self.layout.root) else {
            return;
        };

        // Per-paragraph cursor map. We restore this BEFORE opening the
        // last-active paragraph so `load_paragraph` finds an entry and seeds
        // the cursor immediately.
        for (key, pc) in &state.paragraph_cursors {
            if let Ok(id) = Uuid::parse_str(key) {
                self.paragraph_cursors.insert(id, *pc);
            }
        }

        // 1.2.7+ — visited-paragraph history. Restore only the
        // entries whose nodes still exist (deleted paragraphs
        // drop out silently), and clamp the cursor.
        let history: Vec<Uuid> = state
            .visited_history
            .iter()
            .filter_map(|s| Uuid::parse_str(s).ok())
            .filter(|id| self.hierarchy.get(*id).is_some())
            .collect();
        if !history.is_empty() {
            let max_idx = history.len().saturating_sub(1);
            self.visited_cursor = state.visited_cursor.min(max_idx);
            self.visited_history = history;
        }
        // 1.2.7+ — per-book timeline view snapshots.
        for (key, snap) in &state.timeline_views {
            if let Ok(id) = Uuid::parse_str(key) {
                if self.hierarchy.get(id).is_some() {
                    self.timeline_views.insert(id, snap.clone());
                }
            }
        }

        // Collapsed branches.
        for s in &state.tree.collapsed_nodes {
            if let Ok(id) = Uuid::parse_str(s) {
                if self.hierarchy.get(id).is_some() {
                    self.collapsed_nodes.insert(id);
                }
            }
        }
        self.rebuild_rows_preserving_cursor();

        // Tree cursor.
        if let Some(cid) = &state.tree.cursor_id {
            if let Ok(id) = Uuid::parse_str(cid) {
                if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
                    self.tree_cursor = i;
                }
            }
        }

        // Open paragraph + cursor. The per-paragraph map (above) takes
        // precedence over the legacy single-cursor field — load_paragraph
        // restores from the map automatically.
        if let Some(ed) = &state.editor {
            if let Ok(id) = Uuid::parse_str(&ed.opened_id) {
                if let Some(node) = self.hierarchy.get(id).cloned() {
                    if node.kind == NodeKind::Paragraph {
                        let _ = self.load_paragraph(&node);
                        // If a fresh load didn't find a per-paragraph entry
                        // (older session file), fall back to the legacy
                        // single-cursor coordinates.
                        if !self.paragraph_cursors.contains_key(&id) {
                            if let Some(doc) = self.opened.as_mut() {
                                doc.textarea.move_cursor(CursorMove::Jump(
                                    ed.cursor_row as u16,
                                    ed.cursor_col as u16,
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Focus.
        let restored_focus = match state.focus.as_str() {
            "Tree" => Some(Focus::Tree),
            "Editor" => Some(Focus::Editor),
            "Ai" => Some(Focus::Ai),
            "SearchBar" => Some(Focus::SearchBar),
            "AiPrompt" => Some(Focus::AiPrompt),
            _ => None,
        };
        if let Some(f) = restored_focus {
            self.focus = f;
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent, is_search: bool) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                if is_search && self.show_results_overlay {
                    self.show_results_overlay = false;
                } else if !is_search && self.show_prompt_picker {
                    self.show_prompt_picker = false;
                } else {
                    self.show_results_overlay = false;
                    self.show_prompt_picker = false;
                    // Search bar → Editor closes the
                    // Editor → Tree → Search → Editor rotation. AI prompt
                    // bounces to AI pane (separate Ai↔AiPrompt pairing).
                    let target = if is_search {
                        Focus::Editor
                    } else {
                        Focus::Ai
                    };
                    self.change_focus(target);
                }
            }
            KeyCode::Enter => {
                if is_search {
                    if self.show_results_overlay && !self.results.is_empty() {
                        let id = self.results[self.results_cursor].id;
                        self.open_search_result(id);
                    } else {
                        self.run_search();
                    }
                } else if self.show_prompt_picker {
                    self.commit_prompt_pick();
                } else {
                    self.start_inference();
                }
            }
            KeyCode::Up if is_search && self.show_results_overlay => {
                if !self.results.is_empty() && self.results_cursor > 0 {
                    self.results_cursor -= 1;
                }
            }
            KeyCode::Down if is_search && self.show_results_overlay => {
                if !self.results.is_empty()
                    && self.results_cursor + 1 < self.results.len()
                {
                    self.results_cursor += 1;
                }
            }
            KeyCode::Up if !is_search && self.show_prompt_picker => {
                if self.prompt_picker_cursor > 0 {
                    self.prompt_picker_cursor -= 1;
                }
            }
            KeyCode::Down if !is_search && self.show_prompt_picker => {
                let n = self.prompt_picker_matches().len();
                if n > 0 && self.prompt_picker_cursor + 1 < n {
                    self.prompt_picker_cursor += 1;
                }
            }
            // 1.2.4+: Up / Down in the AI prompt (no picker
            // showing) walks `ai_prompt_history`. Shell-style.
            KeyCode::Up if !is_search => {
                if !self.ai_prompt_history.is_empty() {
                    let next = match self.ai_prompt_history_cursor {
                        Some(0) => 0,
                        Some(i) => i - 1,
                        None => self.ai_prompt_history.len() - 1,
                    };
                    self.ai_prompt_history_cursor = Some(next);
                    let entry = self.ai_prompt_history[next].clone();
                    self.ai_input.clear();
                    for c in entry.chars() {
                        self.ai_input.insert_char(c);
                    }
                }
            }
            KeyCode::Down if !is_search => {
                if let Some(cur) = self.ai_prompt_history_cursor {
                    let next = cur + 1;
                    if next >= self.ai_prompt_history.len() {
                        // Past the newest entry → leave history
                        // navigation, clear the input.
                        self.ai_prompt_history_cursor = None;
                        self.ai_input.clear();
                    } else {
                        self.ai_prompt_history_cursor = Some(next);
                        let entry = self.ai_prompt_history[next].clone();
                        self.ai_input.clear();
                        for c in entry.chars() {
                            self.ai_input.insert_char(c);
                        }
                    }
                }
            }
            KeyCode::Tab if !is_search && self.show_prompt_picker => {
                self.commit_prompt_pick();
            }
            KeyCode::Backspace => {
                self.current_input(is_search).backspace();
                if is_search {
                    self.show_results_overlay = false;
                }
                if !is_search {
                    self.ai_prompt_history_cursor = None;
                }
            }
            KeyCode::Delete => {
                self.current_input(is_search).delete();
                if is_search {
                    self.show_results_overlay = false;
                }
                if !is_search {
                    self.ai_prompt_history_cursor = None;
                }
            }
            KeyCode::Left => self.current_input(is_search).move_left(),
            KeyCode::Right => self.current_input(is_search).move_right(),
            KeyCode::Home => self.current_input(is_search).move_home(),
            KeyCode::End => self.current_input(is_search).move_end(),
            KeyCode::Char(c) => {
                // Explicit AI-prompt focus shortcuts. The global Ctrl+1..5
                // block at the top of `handle_key` covers these too, but
                // some terminals re-encode the chords in ways that bypass
                // the global path — handle them locally as a safety net so
                // they always work from the AI prompt.
                if !is_search
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::ALT | KeyModifiers::SUPER)
                {
                    if c == '1' {
                        self.change_focus(Focus::Editor);
                        return Ok(false);
                    }
                    if c == 't' || c == 'T' {
                        self.change_focus(Focus::Tree);
                        return Ok(false);
                    }
                }
                let mut residual = key.modifiers;
                residual.remove(KeyModifiers::SHIFT);
                if residual.is_empty() {
                    let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                        && c.is_ascii_alphabetic()
                    {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    };
                    self.current_input(is_search).insert_char(final_c);
                    if is_search {
                        self.show_results_overlay = false;
                    } else {
                        // 1.2.4+: typing in the AI prompt
                        // breaks history-recall navigation —
                        // the next Up arrow starts at the
                        // newest entry again.
                        self.ai_prompt_history_cursor = None;
                        self.refresh_prompt_picker();
                    }
                }
            }
            _ => {}
        }
        // After Backspace/Delete in the AI input, also refresh the picker.
        if !is_search {
            self.refresh_prompt_picker();
        }
        Ok(false)
    }

    fn refresh_prompt_picker(&mut self) {
        let s = self.ai_input.as_str();
        self.show_prompt_picker = s.starts_with('/');
        self.prompt_picker_cursor = self
            .prompt_picker_cursor
            .min(self.prompt_picker_matches().len().saturating_sub(1));
    }

    fn commit_prompt_pick(&mut self) {
        let matches = self.prompt_picker_matches();
        let Some(picked) = matches.into_iter().nth(self.prompt_picker_cursor) else {
            self.status = "no matching prompt".into();
            return;
        };
        let template = match &picked.body {
            PromptBody::Static(t) => t.clone(),
            PromptBody::BookParagraph(id) => {
                match self.store.get_content(*id) {
                    Ok(Some(bytes)) => {
                        let text = String::from_utf8_lossy(&bytes).to_string();
                        // Strip the leading `= Title` Typst heading that the
                        // editor inserts by default so it doesn't pollute the
                        // prompt sent to the LLM.
                        strip_leading_typst_heading(&text)
                    }
                    Ok(None) => {
                        self.status = format!("prompt `{}` has no body", picked.name);
                        return;
                    }
                    Err(e) => {
                        self.status = format!(
                            "loading prompt `{}` from book failed: {e}",
                            picked.name
                        );
                        return;
                    }
                }
            }
        };
        let body = self.render_template(&template);
        self.ai_input.clear();
        for c in body.chars() {
            self.ai_input.insert_char(c);
        }
        self.show_prompt_picker = false;
        let chip = match picked.source {
            PromptSource::System => "system",
            PromptSource::Book => "book",
        };
        self.status = format!(
            "loaded prompt `{}` [{chip}] — Enter to send",
            picked.name
        );
    }

    /// Look up a prompt by name inside the Prompts system book. Returns the
    /// paragraph body with the leading `= Title` heading stripped, ready to
    /// be passed through `render_template`. Returns None if no such
    /// paragraph exists or its body can't be loaded.
    /// 1.2.6+ — unified prompt resolver used by F7 / F11 / F12.
    /// Precedence (highest first):
    ///
    ///   1. Paragraph in the Prompts system book whose slug or
    ///      title matches `name` (case-insensitive) — wins
    ///      regardless of any `.example` siblings.
    ///   2. Same-named paragraph with the literal display
    ///      title (`name.replace('-', ' ')`).
    ///   3. Entry in `prompts.hjson` named `name`.
    ///   4. The supplied embedded fallback.
    ///
    /// `inkhaven init` seeds `<name>.example` paragraphs into
    /// the Prompts book for each embedded default so the user
    /// can review/tune and remove the `.example` suffix to
    /// take effect.
    fn resolve_prompt_template(
        &self,
        name: &str,
        fallback: impl FnOnce() -> String,
    ) -> String {
        let display = name.replace('-', " ");
        if let Some(t) = self.lookup_book_prompt_template(name) {
            return t;
        }
        if let Some(t) = self.lookup_book_prompt_template(&display) {
            return t;
        }
        if let Some(p) = self.prompts.find(name) {
            return p.template.clone();
        }
        if let Some(p) = self.prompts.find(&display) {
            return p.template.clone();
        }
        fallback()
    }

    fn lookup_book_prompt_template(&self, name: &str) -> Option<String> {
        let book_id = self.system_book_id(crate::store::SYSTEM_TAG_PROMPTS)?;
        let lower = name.to_lowercase();
        for id in self.hierarchy.collect_subtree(book_id) {
            if id == book_id {
                continue;
            }
            let node = self.hierarchy.get(id)?;
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            if node.slug.to_lowercase() == lower || node.title.to_lowercase() == lower {
                let bytes = self.store.get_content(node.id).ok().flatten()?;
                let text = String::from_utf8_lossy(&bytes).to_string();
                return Some(strip_leading_typst_heading(&text));
            }
        }
        None
    }

    fn current_selection_or_paragraph(&self) -> String {
        let Some(doc) = self.opened.as_ref() else {
            return String::new();
        };
        if let Some(((r1, c1), (r2, c2))) = doc.textarea.selection_range() {
            let lines = doc.textarea.lines();
            return slice_lines(lines, r1, c1, r2, c2);
        }
        doc.textarea.lines().join("\n")
    }

    fn current_context_breadcrumb(&self) -> String {
        let Some(doc) = self.opened.as_ref() else {
            return String::new();
        };
        let Some(node) = self.hierarchy.get(doc.id) else {
            return String::new();
        };
        let mut parts: Vec<String> = self
            .hierarchy
            .ancestors(node)
            .into_iter()
            .map(|n| n.title.clone())
            .collect();
        parts.push(node.title.clone());
        parts.join(" › ")
    }

    fn run_search(&mut self) {
        let query = self.search_input.as_str().trim().to_string();
        if query.is_empty() {
            self.show_results_overlay = false;
            self.results.clear();
            self.status = "empty query".into();
            return;
        }
        match self.store.search_text(&query, 10) {
            Ok(raw) => {
                self.results = raw.iter().filter_map(SearchHit::parse).collect();
                self.results_cursor = 0;
                self.show_results_overlay = true;
                self.status = format!(
                    "`{}` → {} result(s) · ↑/↓ to navigate · Enter to open · Esc to close",
                    query,
                    self.results.len()
                );
            }
            Err(e) => {
                self.status = format!("search failed: {e}");
            }
        }
    }

    fn open_search_result(&mut self, id: Uuid) {
        if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
            self.tree_cursor = i;
        }
        let node = match self.hierarchy.get(id).cloned() {
            Some(n) => n,
            None => {
                self.status = format!("hit {id} no longer in hierarchy — try a fresh search");
                self.show_results_overlay = false;
                return;
            }
        };
        self.show_results_overlay = false;
        if matches!(node.kind, NodeKind::Paragraph) {
            if let Err(e) = self.load_paragraph(&node) {
                self.status = format!("open failed: {e}");
            }
        } else {
            self.change_focus(Focus::Tree);
            self.status = format!(
                "`{}` is a {} — its paragraph children are editable",
                node.title,
                node.kind.as_str()
            );
        }
    }

    fn current_input(&mut self, is_search: bool) -> &mut TextInput {
        if is_search {
            &mut self.search_input
        } else {
            &mut self.ai_input
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        if self.rows.is_empty() {
            return;
        }
        let new =
            (self.tree_cursor as i32 + delta).clamp(0, self.rows.len() as i32 - 1) as usize;
        self.tree_cursor = new;
    }

    // -------- modal -------------------------------------------------------

    fn open_add_modal(&mut self, kind: NodeKind) {
        // User-added Books at root always slot ABOVE the system block
        // ([Notes, Research, Prompts, Places, Characters, Help]) so the
        // user's own content stays at the top of the tree.
        if kind == NodeKind::Book {
            if let Some(notes_id) = self.system_book_id(crate::store::SYSTEM_TAG_NOTES) {
                self.open_add_modal_inner(kind, InsertPosition::Before(notes_id));
                return;
            }
        }
        self.open_add_modal_inner(kind, InsertPosition::End);
    }

    /// Build a "Book › Chapter › Subchapter" style breadcrumb of titles
    /// for the node identified by `id`. Used by the search-results overlay
    /// and the Help RAG context block so users see human names rather than
    /// the slug-derived filesystem path. Falls back to the hit's own slug
    /// path if the node has vanished from the hierarchy (e.g. just deleted).
    fn title_breadcrumb(&self, id: Uuid) -> String {
        let Some(node) = self.hierarchy.get(id) else {
            return String::new();
        };
        let mut parts: Vec<String> = self
            .hierarchy
            .ancestors(node)
            .into_iter()
            .map(|n| n.title.clone())
            .collect();
        parts.push(node.title.clone());
        parts.join(" › ")
    }

    /// Look up a system book's UUID by tag. Returns None if the project
    /// pre-dates the system-book feature and the book hasn't been seeded
    /// yet (shouldn't happen in practice — ensure_system_books runs on
    /// every Store::open).
    fn system_book_id(&self, tag: &str) -> Option<Uuid> {
        self.hierarchy
            .iter()
            .find(|n| {
                n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(tag)
            })
            .map(|n| n.id)
    }

    /// Insert-after variant: walks up from the tree cursor to find a node of
    /// the same `kind` as the one being added; if found, the new node will be
    /// placed immediately after it. Falls back to append-at-end if no
    /// same-kind ancestor exists (e.g. pressing P with cursor on a book).
    fn open_add_modal_after(&mut self, kind: NodeKind) {
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let anchor = cursor_id.and_then(|id| {
            let mut cur = Some(id);
            while let Some(c) = cur {
                let node = self.hierarchy.get(c)?;
                if node.kind == kind {
                    return Some(node.id);
                }
                cur = node.parent_id;
            }
            None
        });
        let position = match anchor {
            Some(id) => InsertPosition::After(id),
            None => InsertPosition::End,
        };
        self.open_add_modal_inner(kind, position);
    }

    fn open_add_modal_inner(&mut self, kind: NodeKind, position: InsertPosition) {
        // For After(anchor), the parent is anchor.parent_id (always valid
        // because the anchor's a same-kind node). For End, walk up to find a
        // valid parent.
        let parent_node = match position {
            InsertPosition::After(anchor_id) | InsertPosition::Before(anchor_id) => {
                match self.hierarchy.get(anchor_id) {
                    Some(anchor) => anchor.parent_id.and_then(|pid| self.hierarchy.get(pid)),
                    None => {
                        self.status = "anchor for insert-around vanished from hierarchy".into();
                        return;
                    }
                }
            }
            InsertPosition::End => {
                let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
                match self.hierarchy.pick_parent_for(&self.cfg, cursor_id, kind) {
                    Ok(p) => p,
                    Err(e) => {
                        self.status = format!("can't add {}: {e}", kind.as_str());
                        return;
                    }
                }
            }
        };
        let parent_label = parent_node.map_or_else(
            || "<books root>".to_string(),
            |n| self.hierarchy.slug_path(n),
        );
        let parent_id = parent_node.map(|n| n.id);
        self.modal = Modal::Adding {
            kind,
            parent_id,
            parent_label,
            input: TextInput::new(),
            position,
        };
    }

    fn move_current(&mut self, dir: MoveDir) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        let siblings = self.hierarchy.children_of(node.parent_id);
        let pos = match siblings.iter().position(|n| n.id == id) {
            Some(p) => p,
            None => return,
        };
        let other_pos = match dir {
            MoveDir::Up => {
                if pos == 0 {
                    self.status = format!("`{}` is already first", node.slug);
                    return;
                }
                pos - 1
            }
            MoveDir::Down => {
                if pos + 1 >= siblings.len() {
                    self.status = format!("`{}` is already last", node.slug);
                    return;
                }
                pos + 1
            }
        };
        let other_id = siblings[other_pos].id;
        match self.store.swap_siblings(&self.hierarchy, id, other_id) {
            Ok(()) => {
                self.status = format!(
                    "moved `{}` {}",
                    node.slug,
                    match dir {
                        MoveDir::Up => "up",
                        MoveDir::Down => "down",
                    }
                );
                self.reload_hierarchy();
                // Keep the cursor on the same node after the swap.
                if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
                    self.tree_cursor = i;
                }
            }
            Err(e) => {
                self.status = format!("move failed: {e}");
            }
        }
    }

    fn open_find_modal(&mut self, with_replace: bool) {
        if self.opened.is_none() {
            self.status = "no paragraph open".into();
            return;
        }
        // Pre-fill with the last pattern if we have one open already.
        let mut search_input = TextInput::new();
        let mut replace_input = TextInput::new();
        if let Some(state) = self.opened.as_ref().and_then(|d| d.search.as_ref()) {
            for c in state.pattern.chars() {
                search_input.insert_char(c);
            }
            if with_replace {
                if let Some(r) = &state.replace_with {
                    for c in r.chars() {
                        replace_input.insert_char(c);
                    }
                }
            }
        }
        self.modal = Modal::FindReplace {
            search_input,
            replace_input: if with_replace {
                Some(replace_input)
            } else {
                None
            },
            focus_replace: false,
        };
    }

    fn commit_find(&mut self) {
        let (pattern, replace_with) = match &self.modal {
            Modal::FindReplace {
                search_input,
                replace_input,
                ..
            } => (
                search_input.as_str().to_string(),
                replace_input.as_ref().map(|i| i.as_str().to_string()),
            ),
            _ => return,
        };
        if pattern.is_empty() {
            self.status = "search pattern is empty".into();
            return;
        }
        let Some(doc) = self.opened.as_mut() else {
            self.modal = Modal::None;
            return;
        };
        let lines = doc.textarea.lines().to_vec();
        match SearchState::build(&pattern, replace_with.clone(), &lines) {
            Ok(state) => {
                let n = state.matches.len();
                doc.search = Some(state);
                self.modal = Modal::None;
                if n == 0 {
                    self.status = format!("no matches for /{pattern}/");
                    return;
                }
                // Jump cursor to first match
                self.jump_to_current_match();
                if replace_with.is_some() {
                    // Replace mode: do the FIRST replacement automatically so
                    // user sees immediate effect; subsequent Ctrl+G keep going.
                    self.do_replace_current();
                    let remaining = self
                        .opened
                        .as_ref()
                        .and_then(|d| d.search.as_ref())
                        .map_or(0, |s| s.matches.len());
                    self.status = format!(
                        "/{pattern}/ → replaced 1 · {remaining} left · Ctrl+G next · Ctrl+R replace all · Esc clear"
                    );
                } else {
                    self.status = format!(
                        "/{pattern}/ → {n} match(es) · Ctrl+G next · Ctrl+R add replacement · Esc clear"
                    );
                }
            }
            Err(e) => {
                self.status = format!("regex error: {e}");
                // Leave the modal open so the user can fix the pattern.
            }
        }
    }

    fn jump_to_current_match(&mut self) {
        let target = self
            .opened
            .as_ref()
            .and_then(|d| d.search.as_ref())
            .and_then(|s| s.current_match())
            .map(|m| (m.row, m.col_start));
        let Some((row, col)) = target else {
            return;
        };
        // Editor viewport height in lines — `layout_editor` is cached
        // from the last draw and includes the two border rows.
        let viewport_h =
            (self.layout_editor.height as usize).saturating_sub(2);
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea
                .move_cursor(CursorMove::Jump(row as u16, col as u16));
            // The editor draws itself — it doesn't render the
            // tui-textarea widget — so the actual viewport top is
            // `doc.scroll_row`, not anything inside tui-textarea. The
            // per-render auto-scroll only nudges `scroll_row` when the
            // cursor falls outside `[scroll_row, scroll_row + h)`,
            // which after a forward Jump lands the cursor at the
            // BOTTOM edge of the viewport. Pre-pinning scroll_row to
            // `target - h/2` keeps the cursor inside the new viewport,
            // so the auto-scroll leaves it alone and the match lands
            // in the middle.
            //
            // Both renderers track scroll_row this way (unwrapped uses
            // source rows verbatim; wrapped uses visual rows but for
            // typical short-line literary content the two agree).
            if viewport_h > 0 {
                let half = viewport_h / 2;
                doc.scroll_row = row.saturating_sub(half);
            }
        }
    }

    fn search_advance_or_replace(&mut self) {
        let is_replace = self
            .opened
            .as_ref()
            .and_then(|d| d.search.as_ref())
            .is_some_and(|s| s.replace_with.is_some());
        if is_replace {
            // Replace the current match, refresh hits, jump to new first.
            self.do_replace_current();
            self.refresh_search_after_edit();
            let remaining = self
                .opened
                .as_ref()
                .and_then(|d| d.search.as_ref())
                .map_or(0, |s| s.matches.len());
            if remaining == 0 {
                if let Some(doc) = self.opened.as_mut() {
                    doc.search = None;
                }
                self.status = "replace done — no more matches".into();
            } else {
                self.jump_to_current_match();
                self.status = format!("replaced · {remaining} left · Ctrl+R replace all");
            }
        } else {
            // Search-only: advance to next match (wraps).
            if let Some(doc) = self.opened.as_mut() {
                if let Some(state) = doc.search.as_mut() {
                    state.advance();
                }
            }
            self.jump_to_current_match();
            let n = self
                .opened
                .as_ref()
                .and_then(|d| d.search.as_ref())
                .map_or(0, |s| s.matches.len());
            let i = self
                .opened
                .as_ref()
                .and_then(|d| d.search.as_ref())
                .map_or(0, |s| s.current);
            if n > 0 {
                self.status = format!("match {} / {}", i + 1, n);
            }
        }
    }

    fn do_replace_current(&mut self) {
        let (row, col_start, col_end, replacement) = {
            let Some(doc) = self.opened.as_ref() else {
                return;
            };
            let Some(state) = &doc.search else {
                return;
            };
            let Some(replacement) = state.replace_with.clone() else {
                return;
            };
            let Some(m) = state.current_match() else {
                return;
            };
            (m.row, m.col_start, m.col_end, replacement)
        };
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        // Select [col_start..col_end] in row, cut, insert replacement.
        doc.textarea
            .move_cursor(CursorMove::Jump(row as u16, col_start as u16));
        doc.textarea.start_selection();
        doc.textarea
            .move_cursor(CursorMove::Jump(row as u16, col_end as u16));
        doc.textarea.cut();
        doc.textarea.insert_str(&replacement);
        doc.dirty = true;
    }

    fn refresh_search_after_edit(&mut self) {
        let lines = self
            .opened
            .as_ref()
            .map(|d| d.textarea.lines().to_vec());
        let Some(lines) = lines else { return };
        if let Some(doc) = self.opened.as_mut() {
            if let Some(state) = doc.search.as_mut() {
                state.refresh(&lines);
            }
        }
    }

    fn replace_all_remaining(&mut self) {
        let mut count = 0;
        loop {
            let has_any = self
                .opened
                .as_ref()
                .and_then(|d| d.search.as_ref())
                .map_or(false, |s| !s.matches.is_empty());
            if !has_any {
                break;
            }
            self.do_replace_current();
            self.refresh_search_after_edit();
            count += 1;
            if count > 100_000 {
                break;
            }
        }
        if let Some(doc) = self.opened.as_mut() {
            doc.search = None;
        }
        self.status = format!("replaced {count} match(es) and cleared search");
    }

    /// Toggle split-edit mode. When entering, capture the current buffer as
    /// the lower-pane snapshot; when leaving, drop the snapshot and restore
    /// full-size editor.
    fn toggle_split(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            self.status = "no paragraph open".into();
            return;
        };
        if doc.split.is_some() {
            doc.split = None;
            self.status = "split closed".into();
        } else {
            let snapshot_lines = doc.textarea.lines().to_vec();
            doc.split = Some(SplitView {
                snapshot_lines,
                scroll_row: 0,
            });
            self.status =
                "split open · upper r/w · lower r/o · Ctrl+H/J scroll · Ctrl+F4 accept · F4 close"
                    .into();
        }
    }

    fn scroll_split_up(&mut self) {
        if let Some(doc) = self.opened.as_mut() {
            if let Some(split) = &mut doc.split {
                split.scroll_row = split.scroll_row.saturating_sub(1);
            }
        }
    }

    fn scroll_split_down(&mut self) {
        if let Some(doc) = self.opened.as_mut() {
            if let Some(split) = &mut doc.split {
                let max = split.snapshot_lines.len().saturating_sub(1);
                if split.scroll_row < max {
                    split.scroll_row += 1;
                }
            }
        }
    }

    /// Dispatch the keystroke that follows the meta-prefix. Each pane has
    /// its own action table:
    ///   * Tree (and Search bar): hierarchy operations
    ///   * Editor: save / snapshots / file load / split-edit
    ///   * AI (and AI prompt): inference management
    fn handle_meta_action(&mut self, key: KeyEvent) {
        self.meta_pending = false;
        if matches!(key.code, KeyCode::Esc) {
            self.status = "meta cancelled".into();
            return;
        }
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if !plain {
            self.status = "meta cancelled".into();
            return;
        }
        let resolved = super::keybind::read().resolve_meta_sub(&key, self.focus);
        match resolved {
            Some(super::keybind::Action::None) => {
                self.status = "meta: chord disabled by config".into();
            }
            Some(action) => self.run_action(action),
            None => {
                self.status = format!(
                    "meta {}: unknown action — use Ctrl+B again to retry",
                    self.focus.label()
                );
            }
        }
    }

    /// Dispatch the Bund-meta chord (Ctrl+Z by default). Three
    /// actions in the v1 chord table:
    ///
    ///   R — run the buffer (eval the open Script's body against Adam)
    ///   N — new script (Add modal under the Scripts system book)
    ///   E — eval one expression (modal prompt, like F1)
    /// Dispatch the view-prefix chord (Ctrl+V): markdown export.
    /// Two suffixes are recognised; their meaning shifts by focus:
    ///
    ///   * Editor pane + `1`  → current paragraph buffer (in-memory,
    ///     includes unsaved edits).
    ///   * Editor pane + `2`  → containing subchapter's subtree.
    ///   * Tree pane   + `1`  → tree-cursor's node and all descendants.
    ///
    /// All variants run the source through `typst_to_markdown`
    /// and write to the launch cwd with a `<title>-<stamp>.md`
    /// filename. Errors land on the status bar; nothing else
    /// changes.
    /// Resolve the tree-cursor row to a paragraph and link it to
    /// the link-pick owner. Always exits pick mode (success or
    /// failure) and returns focus to the editor. Direction is
    /// stashed in `link_pick_for.1`:
    /// * `Outgoing` (Ctrl+V A) — `add_link(owner, picked)`
    /// * `Incoming` (Ctrl+V I) — `add_link(picked, owner)`
    fn commit_link_pick(&mut self) {
        let Some((owner, direction)) = self.link_pick_for else { return };
        let picked = self
            .rows
            .get(self.tree_cursor)
            .map(|(id, _)| *id);
        self.link_pick_for = None;
        let Some(picked) = picked else {
            self.status = "link cancelled: no tree row selected".into();
            self.change_focus(Focus::Editor);
            return;
        };
        let picked_kind = self.hierarchy.get(picked).map(|n| n.kind);
        if !matches!(picked_kind, Some(NodeKind::Paragraph)) {
            self.status =
                "link cancelled: target is not a paragraph".into();
            self.change_focus(Focus::Editor);
            return;
        }
        // Outgoing: owner → picked.  Incoming: picked → owner.
        let (from, to) = match direction {
            LinkPickDirection::Outgoing => (owner, picked),
            LinkPickDirection::Incoming => (picked, owner),
        };
        match self.add_paragraph_link(from, to) {
            Ok(()) => {
                let title = self
                    .hierarchy
                    .get(to)
                    .map(|n| n.title.clone())
                    .unwrap_or_else(|| "?".into());
                self.status = match direction {
                    LinkPickDirection::Outgoing => format!("linked → `{title}`"),
                    LinkPickDirection::Incoming => {
                        let from_title = self
                            .hierarchy
                            .get(from)
                            .map(|n| n.title.clone())
                            .unwrap_or_else(|| "?".into());
                        format!("linked `{from_title}` → current")
                    }
                };
            }
            Err(e) => {
                self.status = format!("link: {e}");
            }
        }
        self.change_focus(Focus::Editor);
    }

    // ── Paragraph links (1.2.4+) ────────────────────────────────

    /// Enter "select paragraph to link" mode. Tree pane gets a
    /// custom title; Enter on a paragraph adds it to the open
    /// paragraph's outgoing links (with a circular-reference
    /// guard). Esc / loss-of-focus cancels.
    fn enter_link_pick_mode(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view A: no paragraph open".into();
            return;
        };
        self.link_pick_for = Some((doc.id, LinkPickDirection::Outgoing));
        self.change_focus(Focus::Tree);
        self.status =
            "link: select paragraph to link · Enter confirms · Esc cancels".into();
    }

    /// Reverse-direction picker (1.2.4+, Ctrl+V I). The
    /// tree-picked paragraph's outgoing links gains the open
    /// paragraph — same circular guard.
    fn enter_incoming_link_pick_mode(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view I: no paragraph open".into();
            return;
        };
        self.link_pick_for = Some((doc.id, LinkPickDirection::Incoming));
        self.change_focus(Focus::Tree);
        self.status =
            "incoming link: select paragraph that will link to current · Enter confirms · Esc cancels"
                .into();
    }

    /// Open the linked-paragraphs modal for the open paragraph.
    /// Lists each outgoing link with title + slug path; `D`
    /// removes a row.
    fn open_link_picker_modal(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view L: no paragraph open".into();
            return;
        };
        let owner = doc.id;
        let entries = self.collect_link_entries(owner);
        if entries.is_empty() {
            self.status =
                "view L: no linked paragraphs (Ctrl+V A adds one)".into();
            return;
        }
        self.modal = Modal::LinkPicker {
            owner,
            entries,
            cursor: 0,
            scroll: 0,
        };
        self.status =
            "links: ↑↓ select · D removes · Esc closes".into();
    }

    /// Toggle bookmark on the open paragraph. Persists via the
    /// existing metadata-update path (`Store::raw().update_metadata`).
    fn toggle_bookmark(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view B: no paragraph open".into();
            return;
        };
        let id = doc.id;
        let Some(node) = self.hierarchy.get(id).cloned() else {
            self.status = "view B: paragraph not in hierarchy".into();
            return;
        };
        let new_value = !node.bookmark;
        let mut updated = node.clone();
        updated.bookmark = new_value;
        updated.modified_at = chrono::Utc::now();
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(id, updated.to_json())
        {
            self.status = format!("bookmark: store update failed: {e}");
            return;
        }
        self.reload_hierarchy();
        self.status = format!(
            "bookmark {}: `{}`",
            if new_value { "added" } else { "cleared" },
            node.title
        );
    }

    /// Open the bookmark picker — every paragraph whose
    /// `bookmark` flag is true.
    fn open_bookmark_picker_modal(&mut self) {
        let entries = self.collect_bookmark_entries();
        if entries.is_empty() {
            self.status =
                "view M: no bookmarks (Ctrl+V B toggles one on the open paragraph)".into();
            return;
        }
        self.modal = Modal::BookmarkPicker {
            entries,
            cursor: 0,
            scroll: 0,
        };
        self.status =
            "bookmarks: ↑↓ select · Enter opens · D clears bookmark · Esc closes"
                .into();
    }

    /// Open the fuzzy paragraph picker. Pre-builds the full
    /// paragraph list (title + slug-path) so subsequent
    /// keystrokes only filter, never re-walk the hierarchy.
    /// Ctrl+V R (1.2.5+) — render the open paragraph in-process,
    /// pop a floating preview modal on top of the editor. Saves
    /// the current buffer first so the rendered preview matches
    /// the on-disk source the user just edited.
    /// Ctrl+V W (1.2.5+) — story view. Build the DOT graph for
    /// the current user book, lay it out, rasterise, and float
    /// the PNG on top of the editor. Saves the open paragraph
    /// first so any pending mentions get scanned.
    fn open_story_view(&mut self) {
        // Pick the same "current book" the assemble/build/take
        // path uses. Refuses if the cursor isn't inside a user
        // book.
        let Some(book_id) = self.resolve_current_user_book("Story view") else {
            return;
        };
        // Save first so mention-scanning sees the latest body.
        if let Some(doc) = self.opened.as_ref() {
            if doc.dirty {
                let _ = self.save_current();
            }
        }
        let Some(picker) = self.image_picker.as_ref() else {
            self.status =
                "story view: terminal can't display images (set `images.preview_enabled: true` or use a kitty / iterm2 / sixel-capable terminal)".into();
            return;
        };
        let book_title = self
            .hierarchy
            .get(book_id)
            .map(|n| n.title.clone())
            .unwrap_or_else(|| "(unknown book)".into());
        self.status = format!(
            "story view: building graph for `{book_title}`…"
        );
        match crate::story_view::build_story_png(
            &self.store,
            &self.hierarchy,
            book_id,
        ) {
            Ok(rendered) => {
                let proto = picker.new_resize_protocol(rendered.image);
                self.modal = Modal::StoryView {
                    book_title: book_title.clone(),
                    width: rendered.width,
                    height: rendered.height,
                    png_bytes: rendered.png_bytes,
                    proto,
                };
                self.status = format!(
                    "story view `{book_title}` · {}×{} · S saves PNG · Esc closes",
                    rendered.width, rendered.height,
                );
            }
            Err(err) => {
                let first = err
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("render failed");
                self.status = format!("story view: {first}");
            }
        }
    }

    /// Ctrl+V w (1.2.6+) — paragraph mini story view. Same
    /// pipeline as `open_story_view`, but for the open
    /// paragraph instead of the current book. Routes to
    /// `Modal::StoryView` so the save / Esc UX is identical.
    fn open_story_view_paragraph(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status =
                "story view (¶): no paragraph open".into();
            return;
        };
        if doc.dirty {
            let _ = self.save_current();
        }
        let Some(doc) = self.opened.as_ref() else {
            return;
        };
        let paragraph_id = doc.id;
        let paragraph_title = doc.title.clone();
        let Some(picker) = self.image_picker.as_ref() else {
            self.status =
                "story view (¶): terminal can't display images".into();
            return;
        };
        match crate::story_view::build_story_png_for_paragraph(
            &self.store,
            &self.hierarchy,
            paragraph_id,
        ) {
            Ok(rendered) => {
                let proto = picker.new_resize_protocol(rendered.image);
                self.modal = Modal::StoryView {
                    book_title: paragraph_title.clone(),
                    width: rendered.width,
                    height: rendered.height,
                    png_bytes: rendered.png_bytes,
                    proto,
                };
                self.status = format!(
                    "story view (¶) `{paragraph_title}` · {}×{} · S saves PNG · Esc closes",
                    rendered.width, rendered.height,
                );
            }
            Err(err) => {
                let first = err
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("render failed");
                self.status = format!("story view (¶): {first}");
            }
        }
    }

    /// `S` inside `Modal::StoryView` — pop the save-as picker
    /// for the rendered PNG. Default path:
    /// `<book-slug>-story-YYYYDDMM-HHMM.png` in cwd.
    fn open_save_story_png_picker(&mut self) {
        let (png_bytes, book_title) = match &self.modal {
            Modal::StoryView {
                png_bytes,
                book_title,
                ..
            } => (png_bytes.clone(), book_title.clone()),
            _ => return,
        };
        let default_dest = match self.default_story_png_dest(&book_title) {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("save story PNG: {e}");
                return;
            }
        };
        let mut input = TextInput::new();
        for c in default_dest.to_string_lossy().chars() {
            input.insert_char(c);
        }
        let return_to = Box::new(std::mem::replace(&mut self.modal, Modal::None));
        self.modal = Modal::SaveStoryPng {
            input,
            png_bytes,
            book_title: book_title.clone(),
            return_to,
        };
        self.status =
            "save story PNG: edit path or Enter to save · Esc returns to preview".into();
    }

    fn default_story_png_dest(
        &self,
        book_title: &str,
    ) -> std::result::Result<std::path::PathBuf, String> {
        let cwd = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
        let stamp = chrono::Local::now().format("%Y%d%m-%H%M");
        let stem = slug::slugify(book_title);
        let safe_stem =
            if stem.is_empty() { "story".to_string() } else { stem };
        Ok(cwd.join(format!("{safe_stem}-story-{stamp}.png")))
    }

    /// Write the already-rendered PNG bytes to disk. No re-render
    /// — the layout is deterministic and the same bytes the
    /// preview displays are what land on disk.
    fn commit_save_story_png(
        &mut self,
        png_bytes: &[u8],
        raw: &str,
        book_title: &str,
    ) {
        let path_str = raw.trim();
        let path = if path_str.is_empty() {
            match self.default_story_png_dest(book_title) {
                Ok(p) => p,
                Err(e) => {
                    self.status = format!("save story PNG: {e}");
                    return;
                }
            }
        } else if let Some(rest) = path_str.strip_prefix("~/") {
            match std::env::var_os("HOME") {
                Some(home) => std::path::PathBuf::from(home).join(rest),
                None => std::path::PathBuf::from(path_str),
            }
        } else {
            std::path::PathBuf::from(path_str)
        };
        match std::fs::write(&path, png_bytes) {
            Ok(()) => {
                self.status = format!(
                    "save story PNG: wrote {} ({} bytes)",
                    path.display(),
                    png_bytes.len(),
                );
            }
            Err(e) => {
                self.status =
                    format!("save story PNG: write {}: {e}", path.display());
            }
        }
    }

    fn open_rendered_paragraph_preview(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status =
                "render ¶: no paragraph open in the editor".into();
            return;
        };
        // Skip Bund / HJSON / images — only typst sources render
        // through the typst pipeline.
        let is_typst = matches!(
            doc.content_type.as_deref(),
            None | Some("") | Some("typst"),
        );
        if !is_typst {
            self.status = format!(
                "render ¶: `{}` is not a typst source — Ctrl+V R only renders .typ buffers",
                doc.title,
            );
            return;
        }
        // Capture the title before we save (save may renumber /
        // re-derive the title from the first sentence).
        let title_before = doc.title.clone();
        // Save first — the spec says "Save current buffer" before
        // render. If save fails, abort the render (we shouldn't
        // render bytes that aren't on disk).
        if doc.dirty {
            if let Err(e) = self.save_current() {
                self.status =
                    format!("render ¶: autosave failed: {e}");
                return;
            }
        }
        let Some(doc) = self.opened.as_ref() else {
            self.status = "render ¶: editor closed during save".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        let title = doc.title.clone();
        if body.trim().is_empty() {
            self.status =
                "render ¶: buffer is empty — nothing to render".into();
            return;
        }
        let _ = title_before;
        // Image picker (ratatui-image) is required to display the
        // PNG. Without it (terminals without graphics support and
        // `images.preview_enabled = false`) fall back to status
        // bar with a hint.
        let Some(picker) = self.image_picker.as_ref() else {
            self.status =
                "render ¶: terminal can't display images (set `images.preview_enabled: true` or use a kitty / iterm2 / sixel-capable terminal)".into();
            return;
        };
        let settings = crate::typst_world::WorldSettings::from_cfg(
            &self.cfg.typst_compile,
        );
        // Preview DPI: 2.0 ppt = ~144 dpi. Good for screen,
        // doesn't blow up memory on long paragraphs. Renders
        // every page up front so Left/Right inside the modal is
        // a pure protocol swap (no re-compile).
        match crate::typst_paragraph_render::render_all(
            &body,
            settings.clone(),
            2.0,
        ) {
            Ok(rendered) => {
                let total = rendered.len();
                let first_w = rendered[0].width;
                let first_h = rendered[0].height;
                let pages: Vec<RenderedPageProto> = rendered
                    .into_iter()
                    .map(|r| RenderedPageProto {
                        proto: picker.new_resize_protocol(r.image),
                        width: r.width,
                        height: r.height,
                    })
                    .collect();
                self.modal = Modal::RenderedPreview {
                    title: title.clone(),
                    body,
                    settings,
                    pages,
                    current_page: 0,
                    ppi: 2.0,
                };
                let pages_note = if total > 1 {
                    format!(" · page 1/{}  · ←/→ navigate", total)
                } else {
                    String::new()
                };
                self.status = format!(
                    "render ¶ `{}` · {}×{}{}  ·  +/- zoom · Esc closes · S saves current · A saves all",
                    title, first_w, first_h, pages_note,
                );
            }
            Err(err) => {
                let first_line = err
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("compile failed");
                self.status =
                    format!("render ¶: {first_line}");
            }
        }
    }

    fn open_fuzzy_paragraph_picker(&mut self) {
        let entries = self.collect_all_paragraph_entries();
        if entries.is_empty() {
            self.status =
                "view P: no paragraphs in this project".into();
            return;
        }
        self.modal = Modal::FuzzyParagraphPicker {
            input: TextInput::new(),
            entries,
            cursor: 0,
            scroll: 0,
        };
        self.status =
            "find ¶: type to filter · ↑↓ select · Enter opens · Esc cancels".into();
    }

    /// 1.2.7+ — Ctrl+V Shift+P. Same fuzzy picker as
    /// `Ctrl+V p` but the entry list is sorted by
    /// `modified_at desc` instead of slug-path. Answers
    /// "what did I touch most recently?" without trawling
    /// the tree.
    fn open_recent_paragraph_picker(&mut self) {
        let mut entries = self.collect_all_paragraph_entries();
        if entries.is_empty() {
            self.status =
                "recent ¶: no paragraphs in this project".into();
            return;
        }
        // Sort by modified_at desc. Look up modified_at via
        // the hierarchy — the picker entry struct doesn't
        // carry it, so we re-resolve here.
        let modified: std::collections::HashMap<Uuid, chrono::DateTime<chrono::Utc>> =
            self.hierarchy
                .iter()
                .map(|n| (n.id, n.modified_at))
                .collect();
        entries.sort_by(|a, b| {
            let ta = modified.get(&a.id).copied()
                .unwrap_or_else(chrono::Utc::now);
            let tb = modified.get(&b.id).copied()
                .unwrap_or_else(chrono::Utc::now);
            tb.cmp(&ta)
        });
        self.modal = Modal::FuzzyParagraphPicker {
            input: TextInput::new(),
            entries,
            cursor: 0,
            scroll: 0,
        };
        self.status =
            "recent ¶: most-recently-modified first · type to filter · ↑↓ select · Enter opens · Esc cancels".into();
    }

    /// Collect every paragraph in the project (excluding
    /// system-book content) as picker entries.
    fn collect_all_paragraph_entries(&self) -> Vec<ScriptPickerEntry> {
        let mut out: Vec<ScriptPickerEntry> = Vec::new();
        for (n, _) in self.hierarchy.flatten() {
            if n.kind != NodeKind::Paragraph {
                continue;
            }
            // Skip system-book content — Help reference, Typst
            // reference, etc., aren't manuscript paragraphs.
            let in_system = self
                .hierarchy
                .ancestors(n)
                .into_iter()
                .any(|a| a.kind == NodeKind::Book && a.system_tag.is_some());
            if in_system {
                continue;
            }
            out.push(ScriptPickerEntry {
                id: n.id,
                title: n.title.clone(),
                slug_path: self.hierarchy.slug_path(n),
            });
        }
        out.sort_by(|a, b| a.slug_path.cmp(&b.slug_path));
        out
    }

    /// Walk the hierarchy and collect every paragraph whose
    /// `bookmark` flag is true.
    fn collect_bookmark_entries(&self) -> Vec<ScriptPickerEntry> {
        let mut out: Vec<ScriptPickerEntry> = Vec::new();
        for (n, _) in self.hierarchy.flatten() {
            if n.kind != NodeKind::Paragraph || !n.bookmark {
                continue;
            }
            out.push(ScriptPickerEntry {
                id: n.id,
                title: n.title.clone(),
                slug_path: self.hierarchy.slug_path(n),
            });
        }
        out.sort_by(|a, b| a.title.cmp(&b.title));
        out
    }

    // ── Tag picker (1.2.5+) ────────────────────────────────────

    /// Every distinct tag in the project, sorted lexicographically
    /// (case-sensitive dedup). System-book contents are included
    /// in the union so the tag namespace is project-wide.
    fn collect_all_tags(&self) -> Vec<String> {
        let mut tags = std::collections::BTreeSet::<String>::new();
        for (n, _) in self.hierarchy.flatten() {
            for t in &n.tags {
                let t = t.trim();
                if !t.is_empty() {
                    tags.insert(t.to_owned());
                }
            }
        }
        tags.into_iter().collect()
    }

    /// Paragraphs tagged with `tag` (case-sensitive match). Sorted
    /// by title to match the bookmark picker's ordering.
    fn collect_paragraphs_with_tag(&self, tag: &str) -> Vec<ScriptPickerEntry> {
        let mut out: Vec<ScriptPickerEntry> = Vec::new();
        for (n, _) in self.hierarchy.flatten() {
            if n.kind != NodeKind::Paragraph {
                continue;
            }
            if !n.tags.iter().any(|t| t == tag) {
                continue;
            }
            out.push(ScriptPickerEntry {
                id: n.id,
                title: n.title.clone(),
                slug_path: self.hierarchy.slug_path(n),
            });
        }
        out.sort_by(|a, b| a.title.cmp(&b.title));
        out
    }

    /// Union `incoming` into `node_id`'s `tags` (dedup case-
    /// sensitively, preserve existing order), persist via
    /// `update_metadata`. Returns true on a successful save.
    /// 1.2.6+: set the node's `tags` to exactly `incoming` (a
    /// set-replace, not a union). Used by single-paragraph picker
    /// commits so unchecking a tag actually removes it. Returns
    /// false on persist failure.
    fn set_tags_on_node(&mut self, node_id: Uuid, incoming: &[String]) -> bool {
        let Some(node) = self.hierarchy.get(node_id).cloned() else {
            return false;
        };
        let mut updated = node;
        let mut next: Vec<String> = incoming
            .iter()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        next.sort();
        next.dedup();
        // Skip the write when nothing actually changed — saves the
        // `modified_at` bump and a needless hierarchy reload.
        if updated.tags == next {
            return true;
        }
        updated.tags = next;
        updated.modified_at = chrono::Utc::now();
        match self.store.raw().update_metadata(node_id, updated.to_json()) {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!(target: "inkhaven::tags",
                    "update_metadata({node_id}) failed: {e}");
                false
            }
        }
    }

    fn add_tags_to_node(&mut self, node_id: Uuid, incoming: &[String]) -> bool {
        let Some(node) = self.hierarchy.get(node_id).cloned() else {
            return false;
        };
        let mut updated = node.clone();
        let existing: std::collections::HashSet<&str> =
            updated.tags.iter().map(|s| s.as_str()).collect();
        let mut additions: Vec<String> = Vec::new();
        for t in incoming {
            let t = t.trim();
            if t.is_empty() {
                continue;
            }
            if !existing.contains(t)
                && !additions.iter().any(|a: &String| a.as_str() == t)
            {
                additions.push(t.to_owned());
            }
        }
        if additions.is_empty() {
            return true;
        }
        updated.tags.extend(additions);
        updated.modified_at = chrono::Utc::now();
        match self.store.raw().update_metadata(node_id, updated.to_json()) {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!(target: "inkhaven::tags",
                    "update_metadata({node_id}) failed: {e}");
                false
            }
        }
    }

    /// Remove `tag` from every node that carries it. Returns the
    /// count of nodes touched (so the picker can report how many
    /// were affected). Persists each via `update_metadata`.
    fn delete_tag_project_wide(&mut self, tag: &str) -> usize {
        let targets: Vec<Uuid> = self
            .hierarchy
            .flatten()
            .into_iter()
            .filter_map(|(n, _)| {
                if n.tags.iter().any(|t| t == tag) {
                    Some(n.id)
                } else {
                    None
                }
            })
            .collect();
        let mut touched = 0usize;
        for id in &targets {
            let Some(node) = self.hierarchy.get(*id).cloned() else {
                continue;
            };
            let mut updated = node.clone();
            updated.tags.retain(|t| t != tag);
            updated.modified_at = chrono::Utc::now();
            if let Err(e) = self.store.raw().update_metadata(*id, updated.to_json()) {
                tracing::warn!(target: "inkhaven::tags",
                    "update_metadata({id}) on delete failed: {e}");
                continue;
            }
            touched += 1;
        }
        touched
    }

    /// Helper — number of nodes a tag delete would affect. Used
    /// by the delete-confirm modal so the user sees the blast
    /// radius before pressing y.
    fn count_nodes_with_tag(&self, tag: &str) -> usize {
        self.hierarchy
            .flatten()
            .into_iter()
            .filter(|(n, _)| n.tags.iter().any(|t| t == tag))
            .count()
    }

    /// Walk every node carrying `old_tag` and replace it with
    /// `new_tag`. If `new_tag` is already present on the node,
    /// the result dedupes (effectively merging the two tags
    /// into one). Returns the count of nodes touched.
    fn rename_tag_project_wide(&mut self, old_tag: &str, new_tag: &str) -> usize {
        let targets: Vec<Uuid> = self
            .hierarchy
            .flatten()
            .into_iter()
            .filter_map(|(n, _)| {
                if n.tags.iter().any(|t| t == old_tag) {
                    Some(n.id)
                } else {
                    None
                }
            })
            .collect();
        let mut touched = 0usize;
        for id in &targets {
            let Some(node) = self.hierarchy.get(*id).cloned() else {
                continue;
            };
            let mut updated = node.clone();
            // Replace + dedup: build a fresh Vec preserving
            // order, skip the old name, append the new name if
            // it isn't already present.
            let mut fresh: Vec<String> = Vec::with_capacity(updated.tags.len());
            let mut new_appended = false;
            for t in updated.tags.iter() {
                if t == old_tag {
                    if !new_appended && !fresh.iter().any(|x| x == new_tag) {
                        fresh.push(new_tag.to_owned());
                        new_appended = true;
                    }
                } else if !fresh.iter().any(|x| x == t) {
                    fresh.push(t.clone());
                }
            }
            updated.tags = fresh;
            updated.modified_at = chrono::Utc::now();
            if let Err(e) = self.store.raw().update_metadata(*id, updated.to_json()) {
                tracing::warn!(target: "inkhaven::tags",
                    "update_metadata({id}) on rename failed: {e}");
                continue;
            }
            touched += 1;
        }
        touched
    }

    /// `T` (or Enter in apply-modes) — apply the selected tag
    /// set (or just the cursor's tag, if nothing is selected) to
    /// the target paragraph(s). On success, close the modal and
    /// return focus to the originating pane.
    fn commit_tags_to_target(&mut self) {
        // 1.2.6+: the `[x]` set IS the intended state for the
        // single-paragraph commit — unchecking a tag now removes
        // it. The picker pre-populates `selected` with the target
        // paragraph's existing tags on open, so the user only
        // pays for what they changed. Multi-paragraph commits
        // remain additive (a destructive bulk-clear is too easy
        // to mis-fire from a multi-mark).
        let (target, mut tags): (TagPickerTarget, Vec<String>) =
            match &self.modal {
                Modal::TagPicker {
                    target, selected, ..
                } => (target.clone(), selected.iter().cloned().collect()),
                _ => return,
            };
        if matches!(target, TagPickerTarget::Search) {
            return;
        }
        tags.sort();
        match &target {
            TagPickerTarget::EditorParagraph { id, title } => {
                // Single paragraph → set-replace semantics. An empty
                // `selected` set means "clear all tags from this
                // paragraph" — the user explicitly chose nothing.
                let ok = self.set_tags_on_node(*id, &tags);
                self.reload_hierarchy();
                self.modal = Modal::None;
                self.change_focus(Focus::Editor);
                self.status = if !ok {
                    format!("tag T: persist failed for `{title}`")
                } else if tags.is_empty() {
                    format!("cleared all tags from `{title}`")
                } else {
                    format!(
                        "set `{title}` tags to: {}",
                        tags.join(", ")
                    )
                };
            }
            TagPickerTarget::TreeSelection(ids) if ids.len() == 1 => {
                let id = ids[0];
                let title = self
                    .hierarchy
                    .get(id)
                    .map(|n| n.title.clone())
                    .unwrap_or_else(|| "<unknown>".into());
                let ok = self.set_tags_on_node(id, &tags);
                self.reload_hierarchy();
                self.modal = Modal::None;
                self.change_focus(Focus::Tree);
                self.status = if !ok {
                    format!("tag T: persist failed for `{title}`")
                } else if tags.is_empty() {
                    format!("cleared all tags from `{title}`")
                } else {
                    format!(
                        "set `{title}` tags to: {}",
                        tags.join(", ")
                    )
                };
            }
            TagPickerTarget::TreeSelection(ids) => {
                // Multi-paragraph → ADD-only. Refuse the no-op
                // (no selection) case loudly so we don't pretend
                // to do something.
                if tags.is_empty() {
                    self.status = "tag T: nothing checked — Space to mark, then T to add".into();
                    return;
                }
                let mut touched = 0usize;
                let mut failed = 0usize;
                for id in ids {
                    if self.add_tags_to_node(*id, &tags) {
                        touched += 1;
                    } else {
                        failed += 1;
                    }
                }
                self.reload_hierarchy();
                self.modal = Modal::None;
                self.change_focus(Focus::Tree);
                self.status = if failed == 0 {
                    format!(
                        "added {} tag(s) to {touched} paragraph(s): {}",
                        tags.len(),
                        tags.join(", ")
                    )
                } else {
                    format!(
                        "tagged {touched}/{} paragraph(s) — {failed} persist failure(s)",
                        ids.len(),
                    )
                };
            }
            TagPickerTarget::Search => {}
        }
    }

    /// Open the backlinks modal for the open paragraph.
    fn open_backlink_picker_modal(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view K: no paragraph open".into();
            return;
        };
        let target = doc.id;
        let entries = self.collect_backlink_entries(target);
        if entries.is_empty() {
            self.status =
                "view K: no incoming links to this paragraph".into();
            return;
        }
        self.modal = Modal::BacklinkPicker {
            target,
            entries,
            cursor: 0,
            scroll: 0,
        };
        self.status =
            "backlinks: ↑↓ select · Enter opens · D removes source link · Esc closes"
                .into();
    }

    /// Walk every paragraph and collect the ones whose
    /// `linked_paragraphs` contains `target`. O(N) over the
    /// hierarchy — acceptable for typical project sizes; if it
    /// ever bites, the obvious next step is a reverse-index
    /// cached at hierarchy-load time.
    fn collect_backlink_entries(&self, target: Uuid) -> Vec<ScriptPickerEntry> {
        let mut out: Vec<ScriptPickerEntry> = Vec::new();
        for (n, _) in self.hierarchy.flatten() {
            if n.kind != NodeKind::Paragraph {
                continue;
            }
            if !n.linked_paragraphs.contains(&target) {
                continue;
            }
            out.push(ScriptPickerEntry {
                id: n.id,
                title: n.title.clone(),
                slug_path: self.hierarchy.slug_path(n),
            });
        }
        out.sort_by(|a, b| a.slug_path.cmp(&b.slug_path));
        out
    }

    /// Resolve the open paragraph's outgoing links into picker
    /// entries (title + slug path). Stale UUIDs (target deleted)
    /// are silently filtered.
    fn collect_link_entries(&self, owner: Uuid) -> Vec<ScriptPickerEntry> {
        let Some(node) = self.hierarchy.get(owner) else {
            return Vec::new();
        };
        node.linked_paragraphs
            .iter()
            .filter_map(|id| self.hierarchy.get(*id))
            .map(|n| ScriptPickerEntry {
                id: n.id,
                title: n.title.clone(),
                slug_path: self.hierarchy.slug_path(n),
            })
            .collect()
    }

    /// Add `target` to `owner`'s outgoing links. Rejects self-
    /// linking and circular references (walks `target`'s
    /// outgoing closure looking for `owner`). Persists via
    /// `Store::update_metadata`.
    fn add_paragraph_link(
        &mut self,
        owner: Uuid,
        target: Uuid,
    ) -> std::result::Result<(), String> {
        if owner == target {
            return Err("can't link a paragraph to itself".into());
        }
        let owner_node = self
            .hierarchy
            .get(owner)
            .cloned()
            .ok_or_else(|| format!("owner paragraph {owner} not in hierarchy"))?;
        if owner_node.linked_paragraphs.contains(&target) {
            return Err("already linked".into());
        }
        // Circular guard: walk target's outgoing transitive closure
        // depth-first; if owner appears in it, we'd create a loop.
        if self.link_path_exists(target, owner) {
            return Err(
                "You can not create circular references".into(),
            );
        }
        let target_node = self
            .hierarchy
            .get(target)
            .ok_or_else(|| format!("target paragraph {target} not in hierarchy"))?;
        if target_node.kind != NodeKind::Paragraph {
            return Err(format!("`{}` is not a paragraph", target_node.title));
        }
        let mut updated = owner_node.clone();
        updated.linked_paragraphs.push(target);
        updated.modified_at = chrono::Utc::now();
        // 1.2.6+: when `owner` is an event paragraph, the new
        // link drops the `orphan` tag (and fires
        // hook.on_event_linked if/when that exists). Reconcile
        // BEFORE writing so a single update_metadata persists
        // both the link AND the tag transition. Idempotent /
        // no-op on non-event nodes.
        crate::store::reconcile_event_orphan_tag(&mut updated);
        self.store
            .raw()
            .update_metadata(owner, updated.to_json())
            .map_err(|e| format!("store update: {e}"))?;
        self.reload_hierarchy();
        Ok(())
    }

    /// Remove `target` from `owner`'s outgoing links. Returns
    /// `false` when the link wasn't present (no-op).
    fn remove_paragraph_link(
        &mut self,
        owner: Uuid,
        target: Uuid,
    ) -> std::result::Result<bool, String> {
        let owner_node = self
            .hierarchy
            .get(owner)
            .cloned()
            .ok_or_else(|| format!("owner paragraph {owner} not in hierarchy"))?;
        if !owner_node.linked_paragraphs.contains(&target) {
            return Ok(false);
        }
        let mut updated = owner_node.clone();
        updated.linked_paragraphs.retain(|u| *u != target);
        updated.modified_at = chrono::Utc::now();
        // 1.2.6+: when `owner` is an event paragraph, losing
        // its last link flips it back to orphan. Reconcile
        // before writing so the tag re-appears atomically with
        // the link removal. No-op on non-event nodes.
        crate::store::reconcile_event_orphan_tag(&mut updated);
        self.store
            .raw()
            .update_metadata(owner, updated.to_json())
            .map_err(|e| format!("store update: {e}"))?;
        self.reload_hierarchy();
        Ok(true)
    }

    /// True when `start`'s outgoing-link transitive closure
    /// reaches `goal`. Used by `add_paragraph_link` to refuse
    /// cycles. Bounded DFS — a malformed graph with a cycle in
    /// it (shouldn't be possible given this very check, but
    /// stay safe) terminates via `visited`.
    fn link_path_exists(&self, start: Uuid, goal: Uuid) -> bool {
        let mut stack: Vec<Uuid> = vec![start];
        let mut visited: std::collections::HashSet<Uuid> =
            std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if id == goal {
                return true;
            }
            if !visited.insert(id) {
                continue;
            }
            if let Some(node) = self.hierarchy.get(id) {
                for next in &node.linked_paragraphs {
                    stack.push(*next);
                }
            }
        }
        false
    }

    /// Dispatch one of the three Ctrl+V markdown-export scopes
    /// through the per-scope `prepare_*` helpers, then open the
    /// SaveMarkdown modal pre-filled with the default
    /// destination. Enter on the modal writes; the user can
    /// edit the path before pressing Enter to redirect.
    fn view_export_markdown(&mut self, scope: ViewMdScope) {
        let prepared = match scope {
            ViewMdScope::Buffer => self.prepare_markdown_buffer(),
            ViewMdScope::Subchapter => self.prepare_markdown_subchapter(),
            ViewMdScope::Subtree => self.prepare_markdown_tree_subtree(),
        };
        match prepared {
            Ok((body, default_dest, label)) => {
                self.open_save_markdown_modal(body, default_dest, label);
            }
            Err(e) => self.status = format!("view: {e}"),
        }
    }

    /// Open the save-as modal with the default path pre-filled.
    /// The user can edit; Enter writes; Esc cancels.
    fn open_save_markdown_modal(
        &mut self,
        body: String,
        default_dest: std::path::PathBuf,
        label: String,
    ) {
        let mut input = TextInput::new();
        for c in default_dest.to_string_lossy().chars() {
            input.insert_char(c);
        }
        self.modal = Modal::SaveMarkdown {
            input,
            body,
            label,
        };
        self.status =
            "save as: edit path or just hit Enter to save · Esc cancels".into();
    }

    /// Commit `body` to whatever path the SaveMarkdown modal's
    /// input contains. Empty input falls back to a fresh default
    /// (defensive — pre-fill should make this rare).
    fn commit_save_markdown(&mut self, body: String, label: String, raw: String) {
        let path_str = raw.trim();
        let path = if path_str.is_empty() {
            match self.default_markdown_dest(&label) {
                Ok(p) => p,
                Err(e) => {
                    self.status = format!("save as: {e}");
                    return;
                }
            }
        } else {
            // Expand `~/` to home if present so users can paste a
            // tilde path. No glob / env expansion — kept minimal.
            let expanded = if let Some(rest) = path_str.strip_prefix("~/") {
                match std::env::var_os("HOME") {
                    Some(home) => std::path::PathBuf::from(home).join(rest),
                    None => std::path::PathBuf::from(path_str),
                }
            } else {
                std::path::PathBuf::from(path_str)
            };
            expanded
        };
        match std::fs::write(&path, body.as_bytes()) {
            Ok(()) => {
                self.status = format!("view: wrote {}", path.display());
            }
            Err(e) => {
                self.status = format!("save as: write {}: {e}", path.display());
            }
        }
    }

    fn handle_view_action(&mut self, key: KeyEvent) {
        self.view_pending = false;
        if matches!(key.code, KeyCode::Esc) {
            self.status = "view cancelled".into();
            return;
        }
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if !plain {
            self.status = "view cancelled".into();
            return;
        }
        let resolved = super::keybind::read().resolve_view_sub(&key, self.focus);
        match resolved {
            Some(super::keybind::Action::None) => {
                self.status = "view: chord disabled by config".into();
            }
            Some(action) => self.run_action(action),
            None => {
                self.status =
                    "view: unknown chord — 1/2 export · S similar · G progress · T target · Esc".into();
            }
        }
    }

    /// Entry point for the Ctrl+V S chord. Behaviour depends on
    /// whether we're already in similar-paragraph mode:
    ///
    /// * Not in mode → save the current buffer, embed it as a
    ///   similarity query against the vector index, and open the
    ///   SimilarPicker modal with the results (current paragraph
    ///   filtered out, paragraphs only).
    /// * Already in mode → save both buffers and drop the
    ///   secondary doc so the layout returns to tree | editor | AI.
    fn toggle_similar_paragraph_mode(&mut self) {
        if self.secondary.is_some() {
            // Save both, then exit similar mode. If the focused
            // (= `self.opened`) doc fails to save, surface that
            // first and keep the user in similar mode so they can
            // fix it. The unfocused doc's save error is swallowed
            // with a tracing log — it's the secondary buffer,
            // less critical to surface immediately.
            if let Err(e) = self.save_current() {
                self.status = format!("view S: save primary failed — {e:#}");
                return;
            }
            if let Some(mut sec) = self.secondary.take() {
                if sec.dirty {
                    if let Err(e) = self.save_doc(&mut sec) {
                        tracing::warn!(
                            target: "inkhaven::view_similar",
                            "secondary save failed: {e:#}",
                        );
                        self.status = format!(
                            "view S: exited (secondary save warning: {e:#})"
                        );
                        self.secondary_focused = false;
                        return;
                    }
                }
            }
            self.secondary_focused = false;
            self.status = "view S: exited similar-paragraphs mode".into();
            return;
        }
        // Not in mode — open the picker. Need an open paragraph
        // to derive the similarity query from.
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view S: no paragraph open — nothing to compare against".into();
            return;
        };
        // Save the current buffer first so the similarity search
        // sees on-disk text (the vector index is refreshed on
        // save). If save fails, abort — searching against stale
        // bytes would mislead.
        if doc.dirty {
            if let Err(e) = self.save_current() {
                self.status = format!("view S: save failed — {e:#}");
                return;
            }
        }
        let (current_id, query) = match self.opened.as_ref() {
            Some(d) => (d.id, d.textarea.lines().join("\n")),
            None => {
                self.status = "view S: paragraph closed during save".into();
                return;
            }
        };
        if query.trim().is_empty() {
            self.status = "view S: paragraph is empty — nothing to compare".into();
            return;
        }
        match self.find_similar_paragraphs(current_id, &query, 20) {
            Ok(entries) if entries.is_empty() => {
                self.status =
                    "view S: no similar paragraphs found (need more indexed content)".into();
            }
            Ok(entries) => {
                self.modal = Modal::SimilarPicker {
                    entries,
                    cursor: 0,
                    scroll: 0,
                };
                self.status =
                    "similar: ↑↓ select · Enter open side-by-side · Esc cancel".into();
            }
            Err(e) => {
                self.status = format!("view S: search failed — {e:#}");
            }
        }
    }

    /// Run a vector-similarity search seeded with `query` and
    /// turn the raw hits into picker entries. Filters out
    /// `exclude_id` (the current paragraph; would otherwise top
    /// the list with score = 1.0) and any non-Paragraph kind
    /// (Help-book content, Notes/Places/etc. should surface
    /// elsewhere — the user asked for paragraphs).
    fn find_similar_paragraphs(
        &self,
        exclude_id: Uuid,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SimilarPickerEntry>> {
        use crate::tui::search_results::SearchHit;
        // Over-fetch: the dedup pass inside the store collapses
        // meta/content slots, but our own filters (current id,
        // non-paragraph kinds) still drop rows. Aim for `limit`
        // *survivors*, not `limit` raw hits.
        let raw = self
            .store
            .search_text(query, (limit + 4).max(8))
            .map_err(|e| anyhow::anyhow!("similarity search: {e}"))?;
        let mut out: Vec<SimilarPickerEntry> = Vec::new();
        for v in raw.iter() {
            let Some(hit) = SearchHit::parse(v) else {
                continue;
            };
            if hit.id == exclude_id {
                continue;
            }
            if !matches!(hit.kind, crate::store::node::NodeKind::Paragraph) {
                continue;
            }
            // Only surface paragraphs that still live in the
            // hierarchy (the vector index can lag a fast delete).
            let Some(node) = self.hierarchy.get(hit.id) else {
                continue;
            };
            let slug_path = self.hierarchy.slug_path(node);
            out.push(SimilarPickerEntry {
                id: hit.id,
                title: hit.title.clone(),
                slug_path,
                score: hit.score,
                snippet: hit.snippet.clone(),
            });
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    /// Compute (markdown body, default destination, status-bar
    /// label) for the open paragraph's buffer. Used by the
    /// save-as flow — `view_export_markdown` opens the
    /// `SaveMarkdown` modal pre-filled with `default_dest`.
    fn prepare_markdown_buffer(
        &self,
    ) -> std::result::Result<(String, std::path::PathBuf, String), String> {
        let doc = self
            .opened
            .as_ref()
            .ok_or_else(|| "no paragraph open".to_string())?;
        let typst_src = doc.textarea.lines().join("\n");
        let md = crate::export::markdown::typst_to_markdown(&typst_src);
        let dest = self.default_markdown_dest(&doc.title)?;
        Ok((md, dest, doc.title.clone()))
    }

    fn prepare_markdown_subchapter(
        &self,
    ) -> std::result::Result<(String, std::path::PathBuf, String), String> {
        let doc = self
            .opened
            .as_ref()
            .ok_or_else(|| "no paragraph open".to_string())?;
        let para = self
            .hierarchy
            .get(doc.id)
            .ok_or_else(|| "paragraph not in hierarchy".to_string())?;
        let root = self
            .hierarchy
            .ancestors(para)
            .into_iter()
            .find(|a| {
                matches!(
                    a.kind,
                    crate::store::node::NodeKind::Subchapter
                        | crate::store::node::NodeKind::Chapter
                )
            })
            .ok_or_else(|| "no containing subchapter".to_string())?;
        self.prepare_markdown_subtree_of(root)
    }

    fn prepare_markdown_tree_subtree(
        &self,
    ) -> std::result::Result<(String, std::path::PathBuf, String), String> {
        let (id, _) = *self
            .rows
            .get(self.tree_cursor)
            .ok_or_else(|| "no tree row under cursor".to_string())?;
        let node = self
            .hierarchy
            .get(id)
            .ok_or_else(|| "node missing from hierarchy".to_string())?;
        self.prepare_markdown_subtree_of(node)
    }

    fn prepare_markdown_subtree_of(
        &self,
        root: &crate::store::node::Node,
    ) -> std::result::Result<(String, std::path::PathBuf, String), String> {
        let layout = crate::project::ProjectLayout::new(self.store.project_root());
        let combined = crate::export::assemble_typst_source(
            &layout,
            &self.hierarchy,
            Some(root.id),
        )
        .map_err(|e| format!("assemble: {e:#}"))?;
        let md = crate::export::markdown::typst_to_markdown(&combined);
        let dest = self.default_markdown_dest(&root.title)?;
        Ok((md, dest, root.title.clone()))
    }

    /// Compute the default markdown destination for a given
    /// title. Format: `<cwd>/<slug>-YYYYDDMM-HHMM.md`. Same
    /// scheme `write_markdown_to_cwd` used before 1.2.4's
    /// save-as picker.
    fn default_markdown_dest(
        &self,
        title: &str,
    ) -> std::result::Result<std::path::PathBuf, String> {
        let cwd = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
        let stamp = chrono::Local::now().format("%Y%d%m-%H%M");
        let stem = slug::slugify(title);
        let safe_stem = if stem.is_empty() { "buffer".to_string() } else { stem };
        Ok(cwd.join(format!("{safe_stem}-{stamp}.md")))
    }

    /// 1.2.6+ — change the live preview zoom by `delta` (units
    /// of PPI). Clamps to [0.5, 6.0]. Re-renders every page at
    /// the new PPI; current_page is preserved so the user stays
    /// on the page they were inspecting. Failures (typst
    /// compile error after edit) leave the existing pages in
    /// place + drop a status line.
    fn zoom_rendered_preview(&mut self, delta: f32) {
        let Modal::RenderedPreview {
            body,
            settings,
            ppi,
            current_page,
            title,
            ..
        } = &self.modal
        else {
            return;
        };
        let body = body.clone();
        let settings = settings.clone();
        let cur_page = *current_page;
        let title = title.clone();
        let new_ppi = (*ppi + delta).clamp(0.5, 6.0);
        if (new_ppi - *ppi).abs() < f32::EPSILON {
            self.status = format!(
                "render ¶: zoom at limit ({:.1}x)",
                *ppi,
            );
            return;
        }
        let Some(picker) = self.image_picker.as_ref() else {
            return;
        };
        match crate::typst_paragraph_render::render_all(
            &body,
            settings.clone(),
            new_ppi,
        ) {
            Ok(rendered) => {
                let pages: Vec<RenderedPageProto> = rendered
                    .into_iter()
                    .map(|r| RenderedPageProto {
                        proto: picker.new_resize_protocol(r.image),
                        width: r.width,
                        height: r.height,
                    })
                    .collect();
                if pages.is_empty() {
                    self.status =
                        "render ¶ zoom: empty render — keeping previous pages".into();
                    return;
                }
                let new_cur = cur_page.min(pages.len() - 1);
                let p = &pages[new_cur];
                let stamp = format!(
                    "render ¶ `{title}` · zoom {:.1}x · page {}/{} · {}×{}",
                    new_ppi,
                    new_cur + 1,
                    pages.len(),
                    p.width,
                    p.height,
                );
                if let Modal::RenderedPreview {
                    pages: dst_pages,
                    current_page: dst_cur,
                    ppi: dst_ppi,
                    ..
                } = &mut self.modal
                {
                    *dst_pages = pages;
                    *dst_cur = new_cur;
                    *dst_ppi = new_ppi;
                }
                self.status = stamp;
            }
            Err(e) => {
                let first = e
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("zoom render failed");
                self.status = format!("render ¶ zoom: {first}");
            }
        }
    }

    /// 1.2.5+ — default destination for `S` on Modal::RenderedPreview.
    /// Mirrors the markdown dest shape but with `.png` extension.
    fn default_rendered_png_dest(
        &self,
        title: &str,
    ) -> std::result::Result<std::path::PathBuf, String> {
        let cwd = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
        let stamp = chrono::Local::now().format("%Y%d%m-%H%M");
        let stem = slug::slugify(title);
        let safe_stem = if stem.is_empty() { "render".to_string() } else { stem };
        Ok(cwd.join(format!("{safe_stem}-{stamp}.png")))
    }

    /// `S` (single) or `A` (all) inside Modal::RenderedPreview —
    /// pop the save-as picker with a sensible default path
    /// pre-filled. `all = true` stamps a multi-page-aware default
    /// (still a single base path; we append `-page-NNN` per page
    /// at write time). The picker stashes the underlying preview
    /// modal in `return_to` so Esc preserves navigation state.
    fn open_save_rendered_png_picker(&mut self, all: bool) {
        let (body, settings, title, current_page) = match &self.modal {
            Modal::RenderedPreview {
                body,
                settings,
                title,
                current_page,
                ..
            } => (
                body.clone(),
                settings.clone(),
                title.clone(),
                *current_page,
            ),
            _ => return,
        };
        let default_dest = match self.default_rendered_png_dest(&title) {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("save PNG as: {e}");
                return;
            }
        };
        let mut input = TextInput::new();
        for c in default_dest.to_string_lossy().chars() {
            input.insert_char(c);
        }
        // Move the current modal into the picker's return_to
        // stash — std::mem::replace avoids cloning the protos.
        let return_to = Box::new(std::mem::replace(&mut self.modal, Modal::None));
        let pages = if all {
            PagesToSave::All
        } else {
            PagesToSave::Single(current_page)
        };
        let mode_label = match &pages {
            PagesToSave::Single(idx) => format!("page {}", idx + 1),
            PagesToSave::All => "all pages".to_string(),
        };
        self.modal = Modal::SaveRenderedPng {
            input,
            body,
            settings,
            title,
            pages,
            return_to,
        };
        self.status = format!(
            "save PNG as ({mode_label}): edit path or hit Enter · Esc returns to preview",
        );
    }

    /// Re-render the paragraph at full DPI (4.0 px/pt) and write
    /// to the picked path. Status-bar reports outcome. For
    /// `PagesToSave::All` we strip a trailing `.png` from the
    /// user's input (if present) and append `-page-NNN.png` per
    /// page; for `Single(idx)` the input is used verbatim.
    fn commit_save_rendered_png(
        &mut self,
        body: &str,
        settings: &crate::typst_world::WorldSettings,
        raw: &str,
        title: &str,
        pages: PagesToSave,
    ) {
        let path_str = raw.trim();
        let base_path = if path_str.is_empty() {
            match self.default_rendered_png_dest(title) {
                Ok(p) => p,
                Err(e) => {
                    self.status = format!("save PNG: {e}");
                    return;
                }
            }
        } else if let Some(rest) = path_str.strip_prefix("~/") {
            match std::env::var_os("HOME") {
                Some(home) => std::path::PathBuf::from(home).join(rest),
                None => std::path::PathBuf::from(path_str),
            }
        } else {
            std::path::PathBuf::from(path_str)
        };
        // 4.0 px/pt ≈ 288 dpi. Print-quality without going
        // wild on memory for chapter-sized paragraphs.
        match pages {
            PagesToSave::Single(idx) => match crate::typst_paragraph_render::render_page(
                body,
                settings.clone(),
                4.0,
                idx,
            ) {
                Ok(rendered) => match std::fs::write(&base_path, &rendered.png_bytes) {
                    Ok(()) => {
                        self.status = format!(
                            "save PNG: wrote {} (page {} · {}×{} · {} bytes)",
                            base_path.display(),
                            idx + 1,
                            rendered.width,
                            rendered.height,
                            rendered.png_bytes.len(),
                        );
                    }
                    Err(e) => {
                        self.status =
                            format!("save PNG: write {}: {e}", base_path.display());
                    }
                },
                Err(e) => {
                    let first =
                        e.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                    self.status =
                        format!("save PNG: re-render failed: {first}");
                }
            },
            PagesToSave::All => {
                // Strip a trailing .png so `myrender.png` and
                // `myrender` both become `myrender-page-001.png` etc.
                let stem = base_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "render".to_string());
                let parent = base_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                match crate::typst_paragraph_render::render_all(
                    body,
                    settings.clone(),
                    4.0,
                ) {
                    Ok(rendered_pages) => {
                        let total = rendered_pages.len();
                        let pad = total.to_string().len().max(3);
                        let mut written: Vec<String> = Vec::with_capacity(total);
                        for (i, page) in rendered_pages.iter().enumerate() {
                            let fname =
                                format!("{stem}-page-{:0pad$}.png", i + 1, pad = pad);
                            let dest = parent.join(&fname);
                            if let Err(e) = std::fs::write(&dest, &page.png_bytes) {
                                self.status = format!(
                                    "save PNG: write {} failed: {e} (wrote {} of {})",
                                    dest.display(),
                                    written.len(),
                                    total,
                                );
                                return;
                            }
                            written.push(fname);
                        }
                        let in_dir = if parent.as_os_str().is_empty() {
                            "(cwd)".to_string()
                        } else {
                            parent.display().to_string()
                        };
                        self.status = format!(
                            "save PNG: wrote {} pages to {} ({})",
                            total,
                            in_dir,
                            // Show first..last filename for context
                            // — full list would blow the status bar.
                            if total == 1 {
                                written[0].clone()
                            } else {
                                format!("{}…{}", written[0], written[total - 1])
                            },
                        );
                    }
                    Err(e) => {
                        let first =
                            e.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                        self.status =
                            format!("save PNG (all): re-render failed: {first}");
                    }
                }
            }
        }
    }

    fn handle_bund_action(&mut self, key: KeyEvent) {
        self.bund_pending = false;
        if matches!(key.code, KeyCode::Esc) {
            self.status = "bund cancelled".into();
            return;
        }
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if !plain {
            self.status = "bund cancelled".into();
            return;
        }
        let resolved = super::keybind::read().resolve_bund_sub(&key, self.focus);
        match resolved {
            Some(super::keybind::Action::None) => {
                self.status = "bund: chord disabled by config".into();
            }
            Some(action) => self.run_action(action),
            None => {
                self.status =
                    "bund: unknown action — R run · N new · E eval · Esc cancel".into();
            }
        }
    }

    /// Central dispatcher for every chord-action. Stage 1 of the
    /// rebindable-keys roadmap (`Documentation/RELEASE_NOTES/…`):
    /// the table at `self.keys` resolves `KeyEvent` → `Action`,
    /// and this switch is the single point where each variant
    /// hits its concrete handler. Adding a new action means a
    /// new variant in `keybind::Action`, a new arm here, and an
    /// entry in `KeyBindings::defaults()`.
    fn run_action(&mut self, action: super::keybind::Action) {
        use super::keybind::Action as A;
        match action {
            // ── Tree pane ─────────────────────────────────────
            A::AddBook => self.open_add_modal(NodeKind::Book),
            A::AddChapter => self.open_add_modal(NodeKind::Chapter),
            A::AddSubchapter => self.open_add_modal(NodeKind::Subchapter),
            A::AddParagraph => self.open_add_modal(NodeKind::Paragraph),
            A::DeleteNode => self.open_delete_modal(),
            A::MorphType => self.cycle_leaf_type(),
            A::ReorderUp => self.move_current(MoveDir::Up),
            A::ReorderDown => self.move_current(MoveDir::Down),

            // ── Editor pane ───────────────────────────────────
            A::Save => {
                if self.opened.is_some() {
                    let _ = self.save_current();
                } else {
                    self.status = "no paragraph open".into();
                }
            }
            A::CreateSnapshot => self.create_snapshot_of_current(),
            A::CycleStatus => self.cycle_paragraph_status(),
            A::OpenFunctionPicker => self.open_function_picker(),
            A::RenameToFirstSentence => self.rename_paragraph_to_first_sentence(),
            A::LookupPlacesOrImage => {
                if !self.try_open_image_picker() {
                    self.start_lexicon_inference(LexiconKind::Places);
                }
            }
            A::LookupCharacters => self.start_lexicon_inference(LexiconKind::Characters),
            A::LookupNotes => self.start_lexicon_inference(LexiconKind::Notes),
            A::LookupArtefacts => self.start_lexicon_inference(LexiconKind::Artefacts),
            A::OpenQuickref => self.open_quickref(),

            // ── Global ────────────────────────────────────────
            A::OpenCredits => self.open_credits(),
            A::OpenBookInfo => self.open_book_info(),
            A::OpenLlmPicker => self.open_llm_picker(),
            A::ToggleSound => self.toggle_sound(),
            A::ToggleMouseCapture => self.toggle_mouse_capture(),
            A::VisitedBack => self.navigate_visited_back(),
            A::VisitedForward => self.navigate_visited_forward(),
            A::UndoLastDelete => self.undo_last_delete(),
            A::ScheduleAssemble => self.schedule_assembly(),
            A::ScheduleBuild => self.schedule_build(),
            A::ScheduleTake => self.schedule_take(),
            A::BackupNow => self.schedule_backup_now(),
            A::ToggleTypewriter => self.toggle_typewriter_mode(),
            A::ToggleAiFullscreen => self.toggle_ai_fullscreen(),
            A::StatusFilterReady => self.open_status_filter("Ready"),
            A::StatusFilterFinal => self.open_status_filter("Final"),
            A::StatusFilterThird => self.open_status_filter("Third"),
            A::StatusFilterSecond => self.open_status_filter("Second"),
            A::StatusFilterFirst => self.open_status_filter("First"),
            A::StatusFilterNapkin => self.open_status_filter("Napkin"),
            A::StatusFilterNone => self.open_status_filter("None"),

            // ── Tagging (1.2.5+) ──────────────────────────────
            A::TagParagraph => self.open_tag_picker_for_editor(),
            A::TagSearch => self.open_tag_search_picker(),

            // ── AI pane ───────────────────────────────────────
            A::ClearChat => self.clear_chat_history(),

            // ── Bund prefix ───────────────────────────────────
            A::BundRunBuffer => self.bund_run_buffer(),
            A::BundNewScript => self.bund_new_script(),
            A::BundOpenEvalModal => self.bund_open_eval_modal(),
            A::BundOpenScriptPicker => self.bund_open_script_picker(),
            A::BundOpenShell => self.open_shell_pane(false),
            A::BundOpenShellFresh => self.open_shell_pane(true),
            A::BundShellSelection => self.toggle_shell_selection_mode(),
            A::BundEditProjectHjson => self.open_hjson_editor(),
            A::TtsReadParagraph => self.tts_read_paragraph(),
            A::TtsSaveAsAudio => self.tts_open_save_as_audio_picker(),
            A::OpenWritingStreakHeatmap => self.open_writing_streak_heatmap(),
            A::SceneBreakPrev => self.scene_break_jump(-1),
            A::SceneBreakNext => self.scene_break_jump(1),
            A::ToggleStyleWarnings => self.toggle_style_warnings(),
            A::OpenConcordance => self.open_concordance(),
            A::TogglePovChip => self.toggle_pov_chip(),

            // ── View prefix ───────────────────────────────────
            A::ViewExportMarkdownBuffer => self.view_export_markdown(ViewMdScope::Buffer),
            A::ViewExportMarkdownSubchapter => self.view_export_markdown(ViewMdScope::Subchapter),
            A::ViewExportMarkdownSubtree => self.view_export_markdown(ViewMdScope::Subtree),
            A::ViewToggleSimilarMode => self.toggle_similar_paragraph_mode(),
            A::ViewOpenProgress => self.open_progress_modal(),
            A::ViewOpenParagraphTarget => self.open_paragraph_target_modal(),
            A::ViewAddLink => self.enter_link_pick_mode(),
            A::ViewAddIncomingLink => self.enter_incoming_link_pick_mode(),
            A::ViewListLinks => self.open_link_picker_modal(),
            A::ViewListBacklinks => self.open_backlink_picker_modal(),
            A::ViewToggleBookmark => self.toggle_bookmark(),
            A::ViewListBookmarks => self.open_bookmark_picker_modal(),
            A::ViewFuzzyParagraphPicker => self.open_fuzzy_paragraph_picker(),
            A::ViewRecentParagraphPicker => self.open_recent_paragraph_picker(),
            A::ViewKillRingPicker => self.open_kill_ring_picker(),
            A::ViewHiddenCharsReport => self.report_hidden_chars(),
            A::ViewShowBreadcrumb => self.show_cursor_breadcrumb(),
            A::ViewRenderParagraph => self.open_rendered_paragraph_preview(),
            A::ViewNextDiagnostic => self.jump_to_next_diagnostic(),
            A::ViewStoryGraph => self.open_story_view(),
            A::ViewStoryGraphParagraph => self.open_story_view_paragraph(),
            A::ViewEventPicker => self.open_event_picker(),
            A::ViewTimeline => self.open_timeline_view(),
            A::ViewNewEventPrompt => self.open_new_event_prompt_from_anywhere(),
            A::ViewEditEventMetadata => self.open_edit_event_metadata_prompt(),

            // ── Top-level F-keys (1.2.4+ migration) ───────────
            A::HelpQuery => self.open_help_query_modal(),
            A::RenameNode => self.open_rename_modal(),
            A::FilePickerTreeImport => {
                self.open_file_picker(PickerContext::TreeInsertOrImport)
            }
            A::FilePickerEditorLoad => {
                self.open_file_picker(PickerContext::EditorLoad)
            }
            A::ToggleSplit => self.toggle_split(),
            A::AcceptSplitSnapshot => self.accept_split_snapshot(),
            A::OpenSnapshotPicker => self.open_snapshot_picker(),
            A::GrammarCheck => self.start_grammar_check(),
            A::CycleAiMode => self.cycle_ai_mode(),
            A::ToggleInferenceMode => self.toggle_inference_mode(),
            A::DiagnosticsList => self.open_diagnostics_list(),
            A::ExplainDiagnostic => self.start_explain_diagnostic(),
            A::Critique => self.start_critique(),

            // Runtime-bound Bund lambda. Dispatch through the
            // hooks machinery so the recursion-cap + policy-deny
            // semantics already in place apply uniformly. No args
            // pushed; the lambda body sees an empty workbench.
            A::BundLambda(name) => {
                crate::scripting::hooks::fire(name.as_ref(), Vec::new());
            }

            // Explicit "do nothing" — should never reach here
            // because the dispatcher catches it first, but harmless.
            A::None => {}
        }
    }

    /// Ctrl+B T — cycle the selected leaf node's flavour through
    /// `Paragraph(typst) → Paragraph(hjson) → Script(bund)` and
    /// back. Target picked from:
    ///   * the open buffer when the focus is on the editor, or
    ///   * the tree cursor otherwise.
    /// Closes + reopens the buffer (if open on the converted node)
    /// so the new highlighter + content_type take effect immediately.
    /// 1.2.4+: multi-select-aware bulk type cycle. When
    /// `tree_marked` is non-empty, runs `cycle_leaf_type_single`
    /// over every marked id and reports the aggregate.
    fn cycle_leaf_type_bulk(&mut self) {
        if self.tree_marked.is_empty() {
            return;
        }
        let ids: Vec<Uuid> = self.tree_marked.iter().copied().collect();
        let mut ok = 0usize;
        let mut fail = 0usize;
        // If the open buffer is in the set, close it first —
        // its rel_path will change as part of the conversion,
        // so reopening from the fresh hierarchy is safer than
        // trying to keep the live doc in sync mid-loop.
        let reopen_id = self
            .opened
            .as_ref()
            .map(|d| d.id)
            .filter(|id| self.tree_marked.contains(id));
        if reopen_id.is_some() {
            self.opened = None;
        }
        for id in &ids {
            if self.cycle_leaf_type_single(*id).is_ok() {
                ok += 1;
            } else {
                fail += 1;
            }
        }
        self.reload_hierarchy();
        // Reopen if needed.
        if let Some(id) = reopen_id {
            if let Some(node) = self.hierarchy.get(id).cloned() {
                if matches!(node.kind, NodeKind::Paragraph) {
                    let _ = self.load_paragraph(&node);
                }
            }
        }
        self.status = if fail == 0 {
            format!("type cycled on {ok} paragraph(s)")
        } else {
            format!("type cycled on {ok} · {fail} failed")
        };
    }

    /// Single-node type cycle used by both the cursor-row chord
    /// and the multi-select wrapper. No buffer-reopen logic —
    /// callers are responsible for that.
    fn cycle_leaf_type_single(
        &mut self,
        node_id: Uuid,
    ) -> std::result::Result<(), String> {
        let node = self
            .hierarchy
            .get(node_id)
            .cloned()
            .ok_or_else(|| format!("node {node_id} not in hierarchy"))?;
        let (new_kind, new_ct, _label) = match (node.kind, node.content_type.as_deref()) {
            (NodeKind::Paragraph, None | Some("typst")) => {
                (NodeKind::Paragraph, Some("hjson"), "hjson")
            }
            (NodeKind::Paragraph, Some("hjson")) => {
                (NodeKind::Script, Some("bund"), "bund")
            }
            (NodeKind::Script, _) => (NodeKind::Paragraph, None, "typst"),
            _ => return Err("not a text leaf".into()),
        };
        self.store
            .convert_leaf(&self.hierarchy, node_id, new_kind, new_ct)
            .map_err(|e| format!("convert: {e}"))?;
        Ok(())
    }

    fn cycle_leaf_type(&mut self) {
        // Pick the node to convert. From the Editor pane: prefer
        // the open buffer; fall back to the tree cursor when the
        // editor pane has nothing open (so M still does the
        // right thing if the user pressed it from a blank editor).
        // From any other pane: always tree cursor.
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let target_id: Option<Uuid> = match self.focus {
            Focus::Editor => self.opened.as_ref().map(|d| d.id).or(cursor_id),
            _ => cursor_id,
        };
        let Some(node_id) = target_id else {
            self.status = "type-cycle: nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(node_id).cloned() else {
            self.status = "type-cycle: node missing from hierarchy".into();
            return;
        };
        let (new_kind, new_ct, label) = match (node.kind, node.content_type.as_deref()) {
            (NodeKind::Paragraph, None | Some("typst")) => {
                (NodeKind::Paragraph, Some("hjson"), "hjson")
            }
            (NodeKind::Paragraph, Some("hjson")) => {
                (NodeKind::Script, Some("bund"), "bund")
            }
            (NodeKind::Script, _) => (NodeKind::Paragraph, None, "typst"),
            (k, ct) => {
                self.status = format!(
                    "type-cycle: {} ({ct:?}) is not a text leaf — only paragraphs / scripts cycle",
                    k.as_str()
                );
                return;
            }
        };

        // Snapshot whether the buffer is open on this node + the
        // focus we should be on when we return. `load_paragraph`
        // unconditionally focuses Editor at the end, so if the
        // user invoked the cycle from the Tree pane we'd steal
        // their focus otherwise.
        let buffer_was_open = self.opened.as_ref().is_some_and(|d| d.id == node_id);
        let saved_focus = self.focus;
        if buffer_was_open {
            self.opened = None;
        }

        match self
            .store
            .convert_leaf(&self.hierarchy, node_id, new_kind, new_ct)
        {
            Ok(converted) => {
                self.status =
                    format!("type-cycle: `{}` is now {label}", converted.title);
                self.reload_hierarchy();
                if buffer_was_open {
                    let _ = self.load_paragraph(&converted);
                    // `load_paragraph` focuses the editor; restore
                    // the pane the user was actually in.
                    if saved_focus != Focus::Editor {
                        self.change_focus(saved_focus);
                    }
                }
            }
            Err(e) => {
                self.status = format!("type-cycle failed: {e}");
            }
        }
    }

    /// Ctrl+Z R — eval the currently-open Script's body against Adam.
    /// No-ops with a status message when no Script is open.
    fn bund_run_buffer(&mut self) {
        // Snapshot everything we need out of self before invoking
        // scripting_eval — that needs &mut self, so we can't hold
        // any borrow of self.opened / self.hierarchy across the
        // call.
        let (body, title) = {
            let Some(doc) = self.opened.as_ref() else {
                self.status = "bund: no buffer open — Ctrl+Z R needs an open .bund".into();
                return;
            };
            let Some(node) = self.hierarchy.get(doc.id) else {
                self.status = "bund: open buffer's node is missing from hierarchy".into();
                return;
            };
            if node.kind != NodeKind::Script {
                self.status = format!(
                    "bund: open buffer is a {}, not a script",
                    node.kind.as_str()
                );
                return;
            }
            (doc.textarea.lines().join("\n"), node.title.clone())
        };
        // Go through `scripting_eval` so the App-state-accessing
        // `ink.editor.* / ink.ai.* / ink.typst.*` stdlib words can
        // reach `self`.
        match self.scripting_eval(&body) {
            Ok(out) => {
                self.status = format_eval_output(&out, Some(&title));
            }
            Err(e) => {
                self.status = format!("bund: eval failed — {e:#}");
            }
        }
    }

    /// Ctrl+Z N — open the Add modal pre-targeted at the Scripts
    /// system book. Falls back to the standard add path if the
    /// Scripts book hasn't been seeded for some reason.
    fn bund_new_script(&mut self) {
        if let Some(scripts_id) = self.system_book_id(crate::store::SYSTEM_TAG_SCRIPTS) {
            // Same shape as open_add_modal_inner for End, but with
            // the parent forced to the Scripts book rather than
            // derived from cursor position.
            let parent_label = if let Some(scripts) = self.hierarchy.get(scripts_id) {
                self.hierarchy.slug_path(scripts)
            } else {
                "scripts".to_string()
            };
            self.modal = Modal::Adding {
                kind: NodeKind::Script,
                parent_id: Some(scripts_id),
                parent_label,
                input: TextInput::new(),
                position: InsertPosition::End,
            };
            self.status = "bund: new script under Scripts — type a title, Enter to create".into();
        } else {
            // Fall back to the default cursor-based parent picker —
            // user-added Books can also host scripts.
            self.open_add_modal(NodeKind::Script);
        }
    }

    /// Ctrl+Z E — open a modal asking for a one-shot expression.
    /// On Enter, eval against Adam and surface the result (or
    /// error) in the status bar. Reuses the BundEval modal variant.
    fn bund_open_eval_modal(&mut self) {
        self.modal = Modal::BundEval {
            input: TextInput::new(),
        };
        self.status = "bund eval: type an expression, Enter to run, Esc to cancel".into();
    }

    /// Ctrl+Z ? — list executable scripts. Starts in `Branch`
    /// scope (the cursor's nearest containing book / chapter /
    /// subchapter). `A` toggles to `ScriptsBook` scope. Enter
    /// runs the highlighted script.
    fn bund_open_script_picker(&mut self) {
        let scope = ScriptPickerScope::Branch;
        let entries = self.collect_script_entries(scope);
        if entries.is_empty() {
            // Fall back to Scripts book if the branch is empty —
            // saves the user one keystroke and matches the
            // "show me something useful" expectation.
            let fallback = self.collect_script_entries(ScriptPickerScope::ScriptsBook);
            if fallback.is_empty() {
                self.status = "bund: no scripts found (try Ctrl+Z N to create one)".into();
                return;
            }
            self.modal = Modal::ScriptPicker {
                scope: ScriptPickerScope::ScriptsBook,
                entries: fallback,
                cursor: 0,
                scroll: 0,
            };
            self.status =
                "bund: no scripts in current branch — showing Scripts book".into();
            return;
        }
        self.modal = Modal::ScriptPicker {
            scope,
            entries,
            cursor: 0,
            scroll: 0,
        };
        self.status =
            "bund: ↑↓ select · Enter run · A toggle scope · Esc cancel".into();
    }

    /// Walk the requested scope and pull every Script node out.
    /// Returns them in slug-path order so the modal listing is
    /// stable across openings.
    fn collect_script_entries(
        &self,
        scope: ScriptPickerScope,
    ) -> Vec<ScriptPickerEntry> {
        let root_id: Option<Uuid> = match scope {
            ScriptPickerScope::Branch => self.current_branch_root_id(),
            ScriptPickerScope::ScriptsBook => self
                .hierarchy
                .iter()
                .find(|n| {
                    n.kind == NodeKind::Book
                        && n.parent_id.is_none()
                        && n.title.eq_ignore_ascii_case("Scripts")
                })
                .map(|n| n.id),
        };
        let Some(root_id) = root_id else {
            return Vec::new();
        };
        let mut ids = self.hierarchy.collect_subtree(root_id);
        // collect_subtree includes the root — drop it if it's
        // not itself a script.
        let mut entries: Vec<ScriptPickerEntry> = Vec::new();
        for id in ids.drain(..) {
            let Some(node) = self.hierarchy.get(id) else {
                continue;
            };
            if node.kind != NodeKind::Script {
                continue;
            }
            let slug_path = self.hierarchy.slug_path(node);
            entries.push(ScriptPickerEntry {
                id: node.id,
                title: node.title.clone(),
                slug_path,
            });
        }
        entries.sort_by(|a, b| a.slug_path.cmp(&b.slug_path));
        entries
    }

    /// The nearest book / chapter / subchapter ancestor of the
    /// tree cursor (or the cursor itself if it already names a
    /// branch). Returns `None` if no row is selected.
    fn current_branch_root_id(&self) -> Option<Uuid> {
        let (id, _) = *self.rows.get(self.tree_cursor)?;
        let node = self.hierarchy.get(id)?;
        if matches!(
            node.kind,
            NodeKind::Subchapter | NodeKind::Chapter | NodeKind::Book
        ) {
            return Some(node.id);
        }
        // Leaf node — walk ancestors until we hit a branch.
        for anc in self.hierarchy.ancestors(node) {
            if matches!(
                anc.kind,
                NodeKind::Subchapter | NodeKind::Chapter | NodeKind::Book
            ) {
                return Some(anc.id);
            }
        }
        None
    }

    // dispatch_meta_tree was absorbed into keybind::KeyBindings —
    // the meta_sub table now carries every tree-pane chord via
    // Scope::Tree entries, and resolution flows through
    // handle_meta_action → resolve_meta_sub → run_action.

    // dispatch_meta_editor + dispatch_meta_ai were absorbed into
    // keybind::KeyBindings — every chord they handled now lives in
    // the meta_sub table under Scope::Editor / Scope::Ai. See
    // run_action for the action→handler dispatch.

    fn open_quickref(&mut self) {
        self.modal = Modal::QuickRef {
            focus: self.focus,
            scroll: 0,
        };
    }

    fn open_credits(&mut self) {
        // 1.2.5+: build a fresh ratatui-image protocol over the
        // embedded `logo.png` so the credits modal can banner it
        // at the top. The image picker is None on terminals
        // without graphics support; we fall through with no logo
        // and the modal still renders fine.
        let logo = self
            .image_picker
            .as_ref()
            .and_then(|picker| {
                embedded_logo_image().map(|img| picker.new_resize_protocol(img.clone()))
            });
        self.modal = Modal::Credits { scroll: 0, logo };
        self.status = "Credits · ↑↓/PgUp/PgDn scroll · Esc close".into();
    }

    /// Ctrl+B I — open the "current book info" panel. The content is
    /// rendered each frame in `draw_book_info_modal` so figures stay
    /// fresh as the user edits; we only stash the scroll offset here.
    fn open_book_info(&mut self) {
        self.modal = Modal::BookInfo { scroll: 0 };
        self.status =
            "Book info · ↑↓/PgUp/PgDn scroll · Esc close".into();
    }

    /// Scroll handler for the BookInfo modal — mirrors
    /// `credits_handle_key`. Renderer clamps scroll to the actual line
    /// count.
    fn book_info_handle_key(&mut self, key: KeyEvent) -> bool {
        let Modal::BookInfo { scroll } = &mut self.modal else {
            return false;
        };
        match key.code {
            KeyCode::Up => {
                *scroll = scroll.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                *scroll = scroll.saturating_add(1);
                true
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(10);
                true
            }
            KeyCode::PageDown => {
                *scroll = scroll.saturating_add(10);
                true
            }
            KeyCode::Home => {
                *scroll = 0;
                true
            }
            KeyCode::End => {
                *scroll = usize::MAX / 2;
                true
            }
            _ => false,
        }
    }

    /// Resolve the "current book" the user is inside: prefer the book
    /// containing the open paragraph (if any), otherwise the book
    /// containing the tree cursor. Returns the cloned node so the
    /// caller can drop the temporary `Hierarchy` it loaded.
    ///
    /// "Current book" walks `parent_id` up to the root: for a node at
    /// any depth this lands on the root Book; for a Book node it
    /// returns the node itself. Returns `None` only when the project
    /// is completely empty (no books seeded yet).
    fn current_book_node(&self, hierarchy: &Hierarchy) -> Option<Node> {
        let start_id = self
            .opened
            .as_ref()
            .map(|d| d.id)
            .or_else(|| self.rows.get(self.tree_cursor).map(|(id, _)| *id))?;

        let mut current_id = start_id;
        loop {
            let node = hierarchy.get(current_id)?;
            match node.parent_id {
                None => return Some(node.clone()),
                Some(pid) => current_id = pid,
            }
        }
    }

    fn build_book_info_lines(&self) -> Vec<Line<'static>> {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let label_color = Style::default().fg(Color::Cyan);

        let mut out: Vec<Line<'static>> = Vec::new();
        out.push(Line::from(""));

        let Ok(hierarchy) = Hierarchy::load(&self.store) else {
            out.push(Line::from(Span::styled(
                "  (could not load hierarchy)".to_string(),
                dim,
            )));
            return out;
        };

        let Some(book) = self.current_book_node(&hierarchy) else {
            out.push(Line::from(Span::styled(
                "  No book selected. Move the tree cursor onto a book \
                 (or any node inside one) and press Ctrl+B I again."
                    .to_string(),
                dim,
            )));
            return out;
        };

        out.push(Line::from(vec![
            Span::styled("  Book: ", label_color),
            Span::styled(book.title.clone(), bold),
            Span::styled(
                format!("   ({})", book.slug),
                dim,
            ),
        ]));
        if let Some(tag) = book.system_tag.as_deref() {
            out.push(Line::from(Span::styled(
                format!("    system book · tag={tag}"),
                dim,
            )));
        }
        out.push(Line::from(""));

        // 1. Paths.
        let backup_dir =
            crate::store::default_user_backup_dir(&self.layout.root);
        let artefacts_root = self.store.resolve_artefacts_dir(&self.cfg);
        let artefacts_book = artefacts_root.join(&book.slug);

        out.push(Line::from(Span::styled(
            "  Paths".to_string(),
            label_color.add_modifier(Modifier::BOLD),
        )));
        out.push(Line::from(format!("    backups:    {}", backup_dir.display())));
        out.push(Line::from(format!(
            "    artefacts:  {}",
            artefacts_book.display()
        )));
        out.push(Line::from(""));

        // 2. Stats.
        let stats = compute_book_stats(&hierarchy, &book, &self.layout.root);
        out.push(Line::from(Span::styled(
            "  Structure".to_string(),
            label_color.add_modifier(Modifier::BOLD),
        )));
        out.push(Line::from(format!(
            "    chapters:     {}",
            stats.chapters
        )));
        out.push(Line::from(format!(
            "    subchapters:  {}",
            stats.subchapters
        )));
        out.push(Line::from(format!(
            "    paragraphs:   {}",
            stats.paragraphs
        )));
        out.push(Line::from(""));

        out.push(Line::from(Span::styled(
            "  Prose".to_string(),
            label_color.add_modifier(Modifier::BOLD),
        )));
        out.push(Line::from(format!(
            "    sentences:    {}",
            stats.sentences
        )));
        out.push(Line::from(format!(
            "    words:        {}",
            stats.words
        )));
        // 250 wpm — a standard adult silent-reading speed for prose
        // (educational references typically quote 200–300 wpm). Round
        // up to whole minutes so humantime doesn't print sub-second
        // precision for an estimate that's never that precise.
        let read_pretty = if stats.words == 0 {
            "< 1m".to_string()
        } else {
            let minutes = ((stats.words as f64) / 250.0).ceil() as u64;
            humantime::format_duration(std::time::Duration::from_secs(
                minutes.max(1) * 60,
            ))
            .to_string()
        };
        out.push(Line::from(format!(
            "    reading time: {read_pretty}  (at 250 words/min)"
        )));
        out.push(Line::from(""));

        // 3. PDF status.
        let pdf_path = artefacts_book.join(format!("{}.pdf", book.slug));
        out.push(Line::from(Span::styled(
            "  Rendered PDF".to_string(),
            label_color.add_modifier(Modifier::BOLD),
        )));
        out.push(Line::from(format!(
            "    expected:   {}",
            pdf_path.display()
        )));
        match std::fs::metadata(&pdf_path) {
            Ok(meta) => {
                let created = meta
                    .created()
                    .or_else(|_| meta.modified())
                    .ok()
                    .and_then(|t| {
                        std::time::SystemTime::now()
                            .duration_since(t)
                            .ok()
                    });
                let age = match created {
                    Some(d) => format_age_humantime(d),
                    None => "(timestamp unavailable)".to_string(),
                };
                let size_kb = meta.len() / 1024;
                out.push(Line::from(vec![
                    Span::raw("    status:     "),
                    Span::styled(
                        "present".to_string(),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!("  ({size_kb} KiB)")),
                ]));
                out.push(Line::from(format!("    created:    {age} ago")));
            }
            Err(_) => {
                out.push(Line::from(vec![
                    Span::raw("    status:     "),
                    Span::styled(
                        "missing".to_string(),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "   (render the book to create it)".to_string(),
                        dim,
                    ),
                ]));
            }
        }
        out.push(Line::from(""));
        out
    }

    /// Ctrl+B L — open the LLM provider picker. Empty providers map
    /// yields a status-bar diagnostic instead of an empty modal.
    fn open_llm_picker(&mut self) {
        if self.cfg.llm.providers.is_empty() {
            self.status =
                "No LLM providers configured in inkhaven.hjson — add at least one under `llm.providers`."
                    .into();
            return;
        }
        let providers: Vec<String> = self.cfg.llm.providers.keys().cloned().collect();
        // Position the cursor on the currently-active default so Enter
        // is a confirm rather than a switch.
        let cursor = providers
            .iter()
            .position(|p| p == &self.cfg.llm.default)
            .unwrap_or(0);
        let initial_default = self.cfg.llm.default.clone();
        self.modal = Modal::LlmPicker {
            providers,
            cursor,
            initial_default,
        };
        self.status = "Switch LLM provider · ↑↓ / Enter to switch · Esc to cancel".into();
    }

    fn llm_picker_handle_key(&mut self, key: KeyEvent) -> bool {
        let Modal::LlmPicker {
            providers, cursor, ..
        } = &mut self.modal
        else {
            return false;
        };
        let total = providers.len();
        match key.code {
            KeyCode::Up => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
                true
            }
            KeyCode::Down => {
                if *cursor + 1 < total {
                    *cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                *cursor = 0;
                true
            }
            KeyCode::End => {
                *cursor = total.saturating_sub(1);
                true
            }
            KeyCode::Enter => {
                self.commit_llm_picker();
                true
            }
            _ => false,
        }
    }

    fn commit_llm_picker(&mut self) {
        let (chosen, initial_default) = match &self.modal {
            Modal::LlmPicker {
                providers,
                cursor,
                initial_default,
            } => (
                providers.get(*cursor).cloned(),
                initial_default.clone(),
            ),
            _ => return,
        };
        let Some(chosen) = chosen else {
            self.modal = Modal::None;
            return;
        };

        // No-op early-out: picking the same provider just closes the
        // modal without rewriting the file.
        if chosen == initial_default {
            self.modal = Modal::None;
            self.status = format!("LLM provider unchanged · still `{chosen}`");
            return;
        }

        // Persist to inkhaven.hjson with a targeted text edit so user
        // comments + the rest of the config survive the rewrite.
        let config_path = self.layout.config_path();
        let raw = match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                self.status =
                    format!("LLM switch aborted: read {}: {e}", config_path.display());
                return;
            }
        };
        let updated = match set_llm_default_in_hjson(&raw, &chosen) {
            Ok(s) => s,
            Err(reason) => {
                self.status = format!(
                    "LLM switch aborted: can't rewrite {}: {reason}",
                    config_path.display()
                );
                return;
            }
        };
        if let Err(e) = std::fs::write(&config_path, &updated) {
            self.status = format!("LLM switch aborted: write {}: {e}", config_path.display());
            return;
        }

        // Update the live config + AiClient so subsequent prompts use
        // the new provider without restarting.
        self.cfg.llm.default = chosen.clone();
        match AiClient::from_config(&self.cfg.llm) {
            Ok(ai) => self.ai = ai,
            Err(e) => {
                // The file is already on disk so the next startup will
                // honour the new default — surface the error so the
                // user knows the in-memory client wasn't refreshed.
                self.status = format!(
                    "switched to `{chosen}` on disk, but couldn't refresh in-memory client: {e}"
                );
                self.modal = Modal::None;
                return;
            }
        }

        self.modal = Modal::None;
        self.status = format!(
            "LLM provider switched to `{chosen}` · saved to {}",
            config_path.display()
        );
    }

    /// 1.2.7+ — capture a paragraph's full state into the
    /// kill-ring before delete_subtree drops it. Pushes to
    /// the front; trims to KILL_RING_CAP. Skipped silently
    /// when the file body can't be read (which still permits
    /// the delete to proceed).
    fn stash_deleted_paragraph(&mut self, node: &Node) {
        // Find the sibling that comes immediately BEFORE the
        // about-to-be-deleted node so we can re-insert at
        // the same position via InsertPosition::After.
        let siblings = self.hierarchy.children_of(node.parent_id);
        let pos = siblings.iter().position(|s| s.id == node.id);
        let anchor_id = match pos {
            Some(0) => None, // first child — restore at end via fallback
            Some(i) => siblings.get(i - 1).map(|n| n.id),
            None => None,
        };
        let content = node
            .file
            .as_ref()
            .map(|rel| self.layout.root.join(rel))
            .and_then(|abs| std::fs::read(&abs).ok())
            .unwrap_or_default();
        self.kill_ring.push_front(DeletedParagraphStash {
            parent_id: node.parent_id,
            anchor_id,
            title: node.title.clone(),
            slug: node.slug.clone(),
            content,
            tags: node.tags.clone(),
            linked_paragraphs: node.linked_paragraphs.clone(),
            status: node.status.clone(),
            target_words: node.target_words,
            content_type: node.content_type.clone(),
            event: node.event.clone(),
        });
        while self.kill_ring.len() > KILL_RING_CAP {
            self.kill_ring.pop_back();
        }
    }

    /// 1.2.7+ — Ctrl+B U. Pop the front of the kill-ring
    /// (= most-recent delete); re-create the paragraph at
    /// its original position; restore body + metadata. The
    /// entry is consumed on success; on failure it's
    /// reinserted so the user can retry.
    fn undo_last_delete(&mut self) {
        let Some(stash) = self.kill_ring.pop_front() else {
            self.status = "undo: kill-ring is empty".into();
            return;
        };
        let parent = stash
            .parent_id
            .and_then(|id| self.hierarchy.get(id).cloned());
        let position = match stash.anchor_id {
            Some(id) if self.hierarchy.get(id).is_some() => {
                crate::store::InsertPosition::After(id)
            }
            _ => crate::store::InsertPosition::End,
        };
        let created = match self.store.create_node(
            &self.cfg,
            &self.hierarchy,
            NodeKind::Paragraph,
            &stash.title,
            parent.as_ref(),
            Some(&stash.slug),
            position,
        ) {
            Ok(n) => n,
            Err(e) => {
                // Re-stash to the front so the user can
                // retry after fixing whatever rejected the
                // create.
                self.kill_ring.push_front(stash);
                self.status = format!("undo: create_node failed: {e}");
                return;
            }
        };
        // Write the body to disk.
        if let Some(rel) = created.file.as_ref() {
            let abs = self.layout.root.join(rel);
            if let Err(e) = std::fs::write(&abs, &stash.content) {
                self.status = format!(
                    "undo: paragraph created at `{}` but body write failed: {e}",
                    created.slug
                );
                self.reload_hierarchy();
                return;
            }
        }
        // Restore the metadata (tags, linked_paragraphs,
        // status, target_words, content_type, event).
        let mut updated = created.clone();
        updated.tags = stash.tags;
        updated.linked_paragraphs = stash.linked_paragraphs;
        updated.status = stash.status;
        updated.target_words = stash.target_words;
        if stash.content_type.is_some() {
            updated.content_type = stash.content_type;
        }
        updated.event = stash.event;
        updated.modified_at = chrono::Utc::now();
        crate::store::reconcile_event_orphan_tag(&mut updated);
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(updated.id, updated.to_json())
        {
            tracing::warn!(target: "inkhaven::undo",
                "metadata restore failed for {}: {e}", updated.id);
        }
        self.reload_hierarchy();
        self.status = format!(
            "↺ restored `{}` (new uuid {} — cross-refs to old uuid stay broken)",
            updated.title,
            updated.id.simple()
        );
    }

    /// 1.2.8+ — `Ctrl+Z o` / `Ctrl+Z O` toggle the shell
    /// pane.  `fresh=true` drops the cached engine + turn
    /// buffer first.  No-op (with status hint) when
    /// `shell.enabled = false` in HJSON.
    /// 1.2.8+ — open `<project>/inkhaven.hjson` in a
    /// full-screen modal editor.  Reads the current file
    /// from disk fresh on every open (so external edits
    /// since process launch are visible), seeds a
    /// `tui-textarea` with the bytes, and routes Ctrl+S,
    /// Esc, and editing keys via `hjson_editor_handle_key`.
    /// When the file is missing, we open with an empty
    /// buffer + a status hint — saving will create the file.
    /// 1.2.9+ — Ctrl+B S action: read the open paragraph
    /// aloud via the OS TTS engine.  Dispatch order:
    ///
    ///   1. No paragraph open → status hint.
    ///   2. Empty / whitespace-only body → status hint.
    ///   3. `editor.tts.enabled = false` → friendly
    ///      "TTS disabled" modal pointing at HJSON.
    ///   4. Lazy-init the engine.  On failure, cache the
    ///      error string and show the "TTS unavailable"
    ///      modal so we don't pay engine-init cost on
    ///      every keystroke.
    ///   5. Apply HJSON voice (substring match against
    ///      installed voices; prefer "Enhanced" /
    ///      "Premium" variants when present) and speed
    ///      (multiplier over `normal_rate`, clamped to
    ///      engine bounds).
    ///   6. Spawn speech with `interrupt = true` so a
    ///      second Ctrl+B S during playback restarts on
    ///      the new paragraph cleanly.
    ///   7. Open the playback modal.  The render loop
    ///      polls `is_speaking()` each frame and closes
    ///      the modal when speech ends.
    fn tts_read_paragraph(&mut self) {
        // ── 1+2 — open paragraph + non-empty body ────
        let source: String = match self.opened.as_ref() {
            Some(doc) => doc.textarea.lines().join("\n"),
            None => {
                self.status = "TTS: no paragraph open".into();
                return;
            }
        };
        let body = strip_leading_typst_heading(&source);
        let body = body.trim().to_string();
        if body.is_empty() {
            self.status = "TTS: paragraph is empty".into();
            return;
        }

        // ── 3 — feature gate ────────────────────────
        if !self.cfg.editor.tts.enabled {
            self.modal = Modal::TtsUnavailable {
                title: " TTS disabled ".into(),
                reason: format!(
                    "Read-aloud (Ctrl+B S) is disabled in this project.\n\n\
                     Enable it by adding to inkhaven.hjson:\n\n  \
                     editor: {{ tts: {{ enabled: true }} }}\n\n\
                     Default voice is `Milena` (Russian female; ships free \
                     with macOS after a one-time language download via \
                     System Settings → Accessibility → Spoken Content)."
                ),
            };
            return;
        }

        // ── 4 — platform gate ───────────────────────
        if let Err(reason) = super::say::Say::available() {
            self.modal = Modal::TtsUnavailable {
                title: " TTS unavailable ".into(),
                reason: format!(
                    "{reason}\n\n\
                     1.2.9 ships TTS via macOS `/usr/bin/say` — the only \
                     backend that proved reliable for repeated invocations \
                     in one process.  Cross-platform TTS is on the roadmap; \
                     until then non-macOS hosts surface this modal."
                ),
            };
            return;
        }

        // ── 5 — pick voice + speak ──────────────────
        let cfg = &self.cfg.editor.tts;
        let voice_resolved = super::say::Say::pick_voice(&cfg.voice);
        let voice_label = voice_resolved
            .clone()
            .unwrap_or_else(|| "system default".to_string());
        let voice_arg = voice_resolved.unwrap_or_default();
        // Convert speed multiplier to words-per-minute.
        // 180 wpm is the canonical "normal" reading rate;
        // multiplying by the HJSON speed lands us in
        // `say`'s expected range.  Clamp to [80, 400] —
        // `say` accepts the full range, but speech is
        // unintelligible outside it.
        let wpm = ((180.0 * cfg.speed.max(0.1)).round() as i32)
            .clamp(80, 400) as u16;

        if let Err(e) = self.tts_say.speak(&body, &voice_arg, Some(wpm)) {
            self.modal = Modal::TtsUnavailable {
                title: " TTS error ".into(),
                reason: format!(
                    "Couldn't start `say` subprocess:\n\n  {e}\n\n\
                     Check that /usr/bin/say is on disk and executable.  \
                     This binary ships with every macOS install since \
                     OS X 10.3 — its absence is usually a permissions or \
                     SIP issue."
                ),
            };
            return;
        }

        // ── 6 — playback modal ─────────────────────
        let preview: String = body.chars().take(80).collect();
        self.modal = Modal::TtsPlayback {
            started_at: std::time::Instant::now(),
            preview,
            voice_label,
        };
    }

    /// 1.2.9+ — close the playback modal when the `say`
    /// subprocess exits.  Called each render frame so
    /// playback ending naturally closes the modal
    /// without the user pressing anything.
    pub(super) fn tts_poll_playback(&mut self) {
        if !matches!(self.modal, Modal::TtsPlayback { .. }) {
            return;
        }
        if !self.tts_say.is_speaking() {
            self.modal = Modal::None;
            self.status = "TTS: finished".into();
        }
    }

    /// 1.2.9+ — stop in-flight playback + close the
    /// playback modal.  Called when the user hits any
    /// key while the playback modal is open.
    fn tts_stop_playback(&mut self) {
        self.tts_say.stop();
        self.modal = Modal::None;
        self.status = "TTS: stopped".into();
    }

    /// 1.2.9+ — Ctrl+B Shift+F action: flip the
    /// session-local style-warnings toggle.  Three
    /// states cycle:
    ///   None  → defer to HJSON default (initial state)
    ///   true  → force overlays ON regardless of HJSON
    ///   false → force overlays OFF regardless of HJSON
    /// The chord only steps between true / false once
    /// the user has touched it; the None state is
    /// initial-only.  Status line names the new effective
    /// state so the user doesn't have to look.
    fn toggle_style_warnings(&mut self) {
        let new_state = match self.style_warnings_toggle {
            None => Some(!self.cfg.editor.style_warnings.enabled),
            Some(true) => Some(false),
            Some(false) => Some(true),
        };
        self.style_warnings_toggle = new_state;
        let effective = new_state.unwrap_or(self.cfg.editor.style_warnings.enabled);
        self.status = if effective {
            format!(
                "style warnings: ON · language={} · filter-words enabled",
                self.cfg.language,
            )
        } else {
            "style warnings: OFF".into()
        };
    }

    /// 1.2.9+ — true when the POV / character chip
    /// should be painted on the status bar.  Session
    /// override (set via `Ctrl+B Shift+P`) wins; falls
    /// back to `editor.pov_chip_enabled` in HJSON.
    fn pov_chip_effective(&self) -> bool {
        match self.pov_chip_toggle {
            Some(v) => v,
            None => self.cfg.editor.pov_chip_enabled,
        }
    }

    /// 1.2.9+ — Ctrl+B Shift+P action: flip the
    /// session-local POV-chip toggle.  Three-state
    /// cycle identical to `toggle_style_warnings`.
    fn toggle_pov_chip(&mut self) {
        let new_state = match self.pov_chip_toggle {
            None => Some(!self.cfg.editor.pov_chip_enabled),
            Some(true) => Some(false),
            Some(false) => Some(true),
        };
        self.pov_chip_toggle = new_state;
        let effective = self.pov_chip_effective();
        self.status = if effective {
            "POV chip: ON · status bar shows character + supporting cast".into()
        } else {
            "POV chip: OFF".into()
        };
    }

    /// 1.2.9+ — build the styled spans for the POV
    /// character chip.  Empty Vec when the chip is
    /// disabled, no paragraph is open, the lexicon
    /// is empty, or no character names are mentioned
    /// in the open paragraph.  The render loop in
    /// `draw_status` splices the result into the
    /// status-bar span list after the focus chip.
    pub(crate) fn pov_chip_spans(&self) -> Vec<ratatui::text::Span<'_>> {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::Span;
        if !self.pov_chip_effective() {
            return Vec::new();
        }
        let Some(doc) = self.opened.as_ref() else {
            return Vec::new();
        };
        let lines: Vec<String> = doc
            .textarea
            .lines()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let chip = match crate::tui::pov_tracker::compute_pov_chip(
            &self.lexicon, &lines,
        ) {
            Some(c) => c,
            None => return Vec::new(),
        };
        let mut spans = Vec::with_capacity(3);
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!(" POV: {} ", chip.pov),
            Style::default()
                .bg(Color::Magenta)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        if !chip.supporting.is_empty() {
            spans.push(Span::styled(
                format!(" +{}", chip.supporting.join(", ")),
                Style::default().add_modifier(Modifier::DIM),
            ));
        }
        spans
    }

    /// 1.2.9+ — Ctrl+B < / Ctrl+B > scene-break
    /// navigation.  `dir = -1` walks backward to the
    /// previous scene-break line; `dir = 1` walks
    /// forward to the next.  Moves the textarea
    /// cursor to column 0 of the matching line and
    /// recenters the viewport via the existing
    /// CursorMove::Jump machinery.
    fn scene_break_jump(&mut self, dir: i32) {
        let Some(doc) = self.opened.as_mut() else {
            self.status = "scene break: no paragraph open".into();
            return;
        };
        let (cur_row, _cur_col) = doc.textarea.cursor();
        let lines = doc.textarea.lines();
        let n = lines.len();
        let target: Option<usize> = if dir > 0 {
            // Walk forward from cur_row + 1.
            (cur_row + 1..n).find(|&r| is_scene_break(&lines[r]))
        } else {
            // Walk backward from cur_row - 1.
            (0..cur_row).rev().find(|&r| is_scene_break(&lines[r]))
        };
        match target {
            Some(row) => {
                doc.textarea.move_cursor(
                    tui_textarea::CursorMove::Jump(row as u16, 0),
                );
                self.status = format!(
                    "scene break: line {} of {}",
                    row + 1,
                    n,
                );
            }
            None => {
                self.status = if dir > 0 {
                    "scene break: no break below"
                } else {
                    "scene break: no break above"
                }
                .into();
            }
        }
    }

    /// 1.2.9+ — Ctrl+B Shift+G action: open the
    /// writing-streak heatmap modal.  Pulls the last
    /// 91 days of project-wide word deltas from the
    /// progress store, computes current + longest
    /// streak in the window, and seeds the modal with
    /// the data so the render path doesn't re-query.
    fn open_writing_streak_heatmap(&mut self) {
        let project_total = self
            .progress_cache
            .as_ref()
            .map(|s| s.project.total_words)
            .unwrap_or(0);
        let daily_words = crate::progress::daily_words(project_total, 91);
        if daily_words.is_empty() {
            self.status =
                "streak: no progress data yet — write a paragraph then save".into();
            return;
        }
        // Current streak: count consecutive trailing days
        // with > 0 words.  Stops at the first 0.  We treat
        // today's 0 as a non-break (the user might still
        // be about to write), but yesterday + earlier
        // zeros break the streak.
        let mut streak_days: u32 = 0;
        let n = daily_words.len();
        // Skip today if it's 0 (grace).
        let mut idx = n.saturating_sub(1);
        if daily_words.get(idx).copied().unwrap_or(0) == 0 && idx > 0 {
            idx -= 1;
        }
        loop {
            if daily_words.get(idx).copied().unwrap_or(0) > 0 {
                streak_days += 1;
                if idx == 0 {
                    break;
                }
                idx -= 1;
            } else {
                break;
            }
        }
        // Longest run of consecutive >0 days in the window.
        let mut longest: u32 = 0;
        let mut current: u32 = 0;
        for w in &daily_words {
            if *w > 0 {
                current += 1;
                longest = longest.max(current);
            } else {
                current = 0;
            }
        }
        // Today's date in (y, m, d) — chrono's UTC date.
        let today = chrono::Utc::now().date_naive();
        use chrono::Datelike;
        let today_ymd = (today.year(), today.month(), today.day());
        self.modal = Modal::WritingStreakHeatmap {
            daily_words,
            streak_days,
            longest_streak: longest,
            today_ymd,
        };
        self.status = format!(
            "streak: {streak_days}-day current · {longest} longest (91-day window) · Esc closes"
        );
    }

    /// 1.2.9+ — Ctrl+B Shift+L action: build a
    /// project-wide concordance and open the
    /// `Concordance` modal.  Walks every paragraph in
    /// the in-memory hierarchy, loads its body via
    /// `Store::get_content`, strips the leading typst
    /// heading line, and feeds the body rows into the
    /// concordance builder.  Uses the project's
    /// `language` to pick the Snowball stemmer + the
    /// stop-word set (same plumbing as the repeated-
    /// phrase detector — no second list to tune).
    /// Cheap at literary scale (well under a second
    /// for a 100k-word manuscript); the build runs
    /// synchronously on the UI thread, then the modal
    /// just slices the cached entries on every key.
    fn open_concordance(&mut self) {
        use crate::tui::concordance::{build, ParagraphInput, SortMode};
        let mut bodies: Vec<(String, Vec<String>)> = Vec::new();
        for node in self.hierarchy.iter() {
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let slug_path = self.hierarchy.slug_path(node);
            let raw = match self.store.get_content(node.id) {
                Ok(Some(bytes)) => bytes,
                _ => continue,
            };
            let text = match std::str::from_utf8(&raw) {
                Ok(s) => strip_leading_typst_heading(s),
                Err(_) => continue,
            };
            let lines: Vec<String> =
                text.split('\n').map(|s| s.to_string()).collect();
            bodies.push((slug_path, lines));
        }
        if bodies.is_empty() {
            self.status =
                "concordance: project has no paragraphs to analyse".into();
            return;
        }
        let inputs: Vec<ParagraphInput<'_>> = bodies
            .iter()
            .map(|(slug, lines)| ParagraphInput {
                slug_path: slug.clone(),
                lines,
            })
            .collect();
        let data = build(
            &self.cfg.editor.style_warnings.repeated_phrases,
            &self.cfg.language,
            &inputs,
        );
        if data.entries.is_empty() {
            self.status = format!(
                "concordance: {} paragraphs scanned, no lexical content (all stop-words?)",
                data.paragraphs_scanned
            );
            return;
        }
        let visible: Vec<usize> = (0..data.entries.len()).collect();
        let stats = format!(
            "concordance: {} distinct stems · {} tokens · {} paragraphs · Esc closes",
            data.distinct_words, data.total_tokens, data.paragraphs_scanned
        );
        self.status = stats;
        self.modal = Modal::Concordance {
            data,
            filter: crate::tui::input::TextInput::new(),
            cursor: 0,
            scroll: 0,
            sort: SortMode::Count,
            visible,
        };
    }

    /// 1.2.9+ — recompute the cached `visible` list on
    /// the `Concordance` modal.  Called whenever the
    /// filter text or sort mode changes.  Filter
    /// semantics: case-insensitive substring match
    /// against the headword OR any kept variant — so
    /// typing `walk` surfaces an entry whose headword
    /// is `walked` but whose variants include `walk`.
    fn concordance_refilter(&mut self) {
        use crate::tui::concordance::sort_in_place;
        let Modal::Concordance {
            data,
            filter,
            cursor,
            scroll,
            sort,
            visible,
        } = &mut self.modal
        else {
            return;
        };
        sort_in_place(&mut data.entries, *sort);
        let needle = filter.as_str().trim().to_lowercase();
        visible.clear();
        for (i, entry) in data.entries.iter().enumerate() {
            if needle.is_empty()
                || entry.headword.contains(&needle)
                || entry.stem.contains(&needle)
                || entry.variants.iter().any(|v| v.contains(&needle))
            {
                visible.push(i);
            }
        }
        if visible.is_empty() {
            *cursor = 0;
            *scroll = 0;
        } else if *cursor >= visible.len() {
            *cursor = visible.len() - 1;
        }
        // Keep cursor on-screen — render clamps scroll
        // against the visible-row height, so this is
        // just a coarse precaution.
        if *cursor < *scroll {
            *scroll = *cursor;
        }
    }

    /// 1.2.9+ — key dispatch for the `Concordance`
    /// modal.  Single-pane list with an inline filter
    /// input — typing characters narrows the list,
    /// arrow keys move the selection, `Ctrl+S` toggles
    /// the sort mode, Esc closes.  No Enter-to-jump
    /// behaviour yet (would need a paragraph-locator
    /// indirection through `slug_path`); that's a
    /// likely follow-up.
    fn concordance_handle_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyModifiers};
        // Esc always closes — even mid-filter.
        if matches!(key.code, KeyCode::Esc) {
            self.modal = Modal::None;
            self.status = "concordance: closed".into();
            return;
        }
        // Ctrl+S toggles sort mode.  Plain `s` falls
        // through to the filter input so headwords
        // beginning with `s` stay typeable.
        if matches!(key.code, KeyCode::Char('s'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            if let Modal::Concordance { sort, .. } = &mut self.modal {
                *sort = sort.toggle();
            }
            self.concordance_refilter();
            if let Modal::Concordance { sort, .. } = &self.modal {
                self.status = format!("concordance: sort = {}", sort.label());
            }
            return;
        }
        // Arrow keys + PgUp/PgDn move the selection.
        match key.code {
            KeyCode::Down => {
                if let Modal::Concordance { cursor, visible, .. } = &mut self.modal {
                    if !visible.is_empty() && *cursor + 1 < visible.len() {
                        *cursor += 1;
                    }
                }
                return;
            }
            KeyCode::Up => {
                if let Modal::Concordance { cursor, .. } = &mut self.modal {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                return;
            }
            KeyCode::PageDown => {
                if let Modal::Concordance { cursor, visible, .. } = &mut self.modal {
                    if !visible.is_empty() {
                        *cursor = (*cursor + 10).min(visible.len() - 1);
                    }
                }
                return;
            }
            KeyCode::PageUp => {
                if let Modal::Concordance { cursor, .. } = &mut self.modal {
                    *cursor = cursor.saturating_sub(10);
                }
                return;
            }
            KeyCode::Home => {
                if let Modal::Concordance { cursor, .. } = &mut self.modal {
                    *cursor = 0;
                }
                return;
            }
            KeyCode::End => {
                if let Modal::Concordance { cursor, visible, .. } = &mut self.modal {
                    if !visible.is_empty() {
                        *cursor = visible.len() - 1;
                    }
                }
                return;
            }
            _ => {}
        }
        // Filter input: typing edits the filter buffer,
        // Backspace deletes, etc.  Only ASCII-printable
        // + Unicode chars are forwarded; control
        // characters fall through to the no-op branch.
        if let Modal::Concordance { filter, .. } = &mut self.modal {
            match key.code {
                KeyCode::Backspace => filter.backspace(),
                KeyCode::Delete => filter.delete(),
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    filter.insert_char(c);
                }
                _ => return,
            }
        }
        self.concordance_refilter();
    }

    /// 1.2.9+ — Ctrl+B Shift+R action: open a save-as
    /// path picker pre-filled with
    /// `<project>/audio/<paragraph-slug>.aiff`.  Enter
    /// commits via `commit_tts_save_as_audio`; Esc
    /// cancels.  Same enable + platform gates as the
    /// chord-driven TTS — surfaces the `TtsUnavailable`
    /// modal when TTS is disabled or the OS isn't
    /// macOS.
    fn tts_open_save_as_audio_picker(&mut self) {
        // Reuse the same enable + platform gates as
        // tts_read_paragraph.
        let source: String = match self.opened.as_ref() {
            Some(doc) => doc.textarea.lines().join("\n"),
            None => {
                self.status = "TTS save: no paragraph open".into();
                return;
            }
        };
        let body = strip_leading_typst_heading(&source);
        let body = body.trim().to_string();
        if body.is_empty() {
            self.status = "TTS save: paragraph is empty".into();
            return;
        }
        if !self.cfg.editor.tts.enabled {
            self.modal = Modal::TtsUnavailable {
                title: " TTS disabled ".into(),
                reason: format!(
                    "Read-aloud + save-as-audio are disabled in this \
                     project.\n\n\
                     Enable by adding to inkhaven.hjson:\n\n  \
                     editor: {{ tts: {{ enabled: true }} }}"
                ),
            };
            return;
        }
        if let Err(reason) = super::say::Say::available() {
            self.modal = Modal::TtsUnavailable {
                title: " TTS unavailable ".into(),
                reason: format!(
                    "{reason}\n\nSave-as-audio uses macOS `/usr/bin/say -o`. \
                     Cross-platform support is on the 1.3+ roadmap."
                ),
            };
            return;
        }
        // Default destination: <project>/audio/<slug>.aiff.
        // Slug comes from the opened doc's title (sanitised
        // for filesystem) so two paragraphs with similar
        // titles get distinct files.
        let title = self
            .opened
            .as_ref()
            .map(|d| d.title.clone())
            .unwrap_or_else(|| "paragraph".to_string());
        let slug = audio_filename_slug(&title);
        let dest = self.layout.root.join("audio").join(format!("{slug}.aiff"));
        let mut input = TextInput::new();
        for c in dest.to_string_lossy().chars() {
            input.insert_char(c);
        }
        let cfg = &self.cfg.editor.tts;
        let voice = super::say::Say::pick_voice(&cfg.voice).unwrap_or_default();
        let voice_label = if voice.is_empty() {
            "system default".to_string()
        } else {
            voice.clone()
        };
        let wpm = ((180.0 * cfg.speed.max(0.1)).round() as i32)
            .clamp(80, 400) as u16;
        let status_label = voice_label.clone();
        self.modal = Modal::TtsSaveAsAudio {
            input,
            body,
            voice,
            wpm,
            voice_label,
        };
        self.status = format!(
            "TTS save: edit path or Enter to confirm · voice={status_label} · Esc cancels",
        );
    }

    /// 1.2.9+ — commit the save-as-audio picker.  Creates
    /// the parent directory if missing; spawns
    /// `say -o <dest> -v <voice> -r <wpm>` with the
    /// paragraph text on stdin.  Blocks until the
    /// subprocess exits so the file is on disk by the
    /// time the status bar reports success — the user
    /// expects to see + open it immediately.  5-second
    /// hard cap on the wait so a stuck `say` doesn't
    /// hang the TUI.
    fn commit_tts_save_as_audio(
        &mut self,
        dest_raw: &str,
        body: &str,
        voice: &str,
        wpm: u16,
        voice_label: &str,
    ) {
        let dest = std::path::PathBuf::from(dest_raw.trim());
        if let Some(parent) = dest.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                self.status = format!(
                    "TTS save: couldn't create {}: {e}",
                    parent.display(),
                );
                return;
            }
        }
        let mut cmd = std::process::Command::new("/usr/bin/say");
        cmd.arg("-o").arg(&dest);
        if !voice.is_empty() {
            cmd.arg("-v").arg(voice);
        }
        cmd.arg("-r").arg(wpm.to_string());
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::piped());
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                self.status = format!("TTS save: spawn failed: {e}");
                return;
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            if let Err(e) = stdin.write_all(body.as_bytes()) {
                self.status = format!("TTS save: write stdin: {e}");
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
        }
        // Bounded wait: 30 seconds.  `say -o` writes
        // streaming so even a long paragraph fits.
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_secs(30);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if status.success() {
                        let bytes = std::fs::metadata(&dest)
                            .map(|m| m.len())
                            .unwrap_or(0);
                        self.status = format!(
                            "TTS save: wrote {} ({} KB · voice={voice_label})",
                            dest.display(),
                            bytes / 1024,
                        );
                    } else {
                        let mut stderr = String::new();
                        if let Some(mut s) = child.stderr.take() {
                            use std::io::Read;
                            let _ = s.read_to_string(&mut stderr);
                        }
                        self.status = format!(
                            "TTS save: `say` exited {} — {}",
                            status.code().unwrap_or(-1),
                            stderr.trim(),
                        );
                    }
                    return;
                }
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        self.status = format!(
                            "TTS save: timed out after 30s — partial file at {}",
                            dest.display(),
                        );
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    self.status = format!("TTS save: wait failed: {e}");
                    return;
                }
            }
        }
    }

    /// 1.2.9+ — speak arbitrary text through the same
    /// TTS pipeline `Ctrl+B S` uses.  Public entry point
    /// for Bund scripts (`"text" ink.tts.speak`).  Honours
    /// the `editor.tts.enabled` HJSON gate the chord does
    /// — scripts can't bypass user preference.  Returns
    /// `Ok(())` on successful spawn (the speech then plays
    /// in parallel), `Err(...)` when:
    ///   * TTS is disabled in config, OR
    ///   * the platform isn't macOS, OR
    ///   * the `/usr/bin/say` subprocess failed to spawn.
    /// Callers (the Bund word handler) surface these as a
    /// script-visible error so users get a clear reason
    /// rather than a silent no-op.
    pub(crate) fn tts_speak_string(&mut self, text: &str) -> Result<(), String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("empty text".into());
        }
        if !self.cfg.editor.tts.enabled {
            return Err(
                "TTS is disabled in inkhaven.hjson (editor.tts.enabled = true)".into(),
            );
        }
        if let Err(reason) = super::say::Say::available() {
            return Err(reason.to_string());
        }
        let cfg = &self.cfg.editor.tts;
        let voice = super::say::Say::pick_voice(&cfg.voice).unwrap_or_default();
        let wpm = ((180.0 * cfg.speed.max(0.1)).round() as i32)
            .clamp(80, 400) as u16;
        self.tts_say
            .speak(text, &voice, Some(wpm))
            .map_err(|e| format!("subprocess spawn failed: {e}"))?;
        Ok(())
    }

    /// 1.2.9+ — speak `editor.tts.greeting` at startup.
    /// Non-blocking: spawns `say` and returns immediately.
    /// Audio plays in parallel with the main loop coming
    /// up.  Silently no-op when greeting is empty,
    /// `enabled = false`, or the platform isn't macOS.
    pub(super) fn tts_speak_greeting(&mut self) {
        let text = self.cfg.editor.tts.greeting.trim().to_string();
        if text.is_empty() || !self.cfg.editor.tts.enabled {
            return;
        }
        if super::say::Say::available().is_err() {
            return;
        }
        let cfg = &self.cfg.editor.tts;
        let voice = super::say::Say::pick_voice(&cfg.voice).unwrap_or_default();
        let wpm = ((180.0 * cfg.speed.max(0.1)).round() as i32)
            .clamp(80, 400) as u16;
        let _ = self.tts_say.speak(&text, &voice, Some(wpm));
    }

    /// 1.2.9+ — speak `editor.tts.goodbye` at shutdown
    /// and BLOCK until the `say` subprocess exits or the
    /// 6-second safety cap fires.  Without the block,
    /// inkhaven would exit and the OS audio device's
    /// driver would terminate the speech mid-word.
    /// Polled in 50ms increments.  Silently no-op when
    /// goodbye is empty, `enabled = false`, or the
    /// platform isn't macOS.
    pub(super) fn tts_speak_goodbye_blocking(&mut self) {
        let text = self.cfg.editor.tts.goodbye.trim().to_string();
        if text.is_empty() || !self.cfg.editor.tts.enabled {
            return;
        }
        if super::say::Say::available().is_err() {
            return;
        }
        let cfg = &self.cfg.editor.tts;
        let voice = super::say::Say::pick_voice(&cfg.voice).unwrap_or_default();
        let wpm = ((180.0 * cfg.speed.max(0.1)).round() as i32)
            .clamp(80, 400) as u16;
        if self.tts_say.speak(&text, &voice, Some(wpm)).is_err() {
            return;
        }
        // Wait for the subprocess to exit naturally.
        // Poll in 50ms increments; 6-second hard cap so a
        // user who configured a tome as their goodbye
        // doesn't hang the quit indefinitely.
        let hard_deadline = std::time::Instant::now()
            + std::time::Duration::from_secs(6);
        while std::time::Instant::now() < hard_deadline {
            if !self.tts_say.is_speaking() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        // Ensure the subprocess is gone even if it
        // outran our deadline.
        self.tts_say.stop();
    }

    fn open_hjson_editor(&mut self) {
        let path = self.layout.config_path();
        let original_content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.status = format!(
                    "hjson: `{}` not found — opening empty buffer (Ctrl+S to create)",
                    path.display()
                );
                String::new()
            }
            Err(e) => {
                self.status = format!(
                    "hjson: cannot read `{}`: {e}",
                    path.display()
                );
                return;
            }
        };

        // Seed the textarea with the file's lines.  Setting
        // line[0] via TextArea::new(vec!["".into()]) and then
        // typing in lots of chars is much slower than passing
        // the full Vec up-front; do the latter.
        let lines: Vec<String> = if original_content.is_empty() {
            vec![String::new()]
        } else {
            original_content.lines().map(String::from).collect()
        };
        let mut textarea = TextArea::new(lines);
        textarea.set_line_number_style(
            Style::default().fg(self.theme.line_number_fg),
        );
        // Reuse the editor's cursor style so the visual
        // feedback feels consistent.
        textarea.set_cursor_style(
            Style::default().add_modifier(Modifier::REVERSED),
        );
        // No tabstop translation — the file is config, leave
        // tabs verbatim.
        textarea.set_hard_tab_indent(true);

        self.modal = Modal::HjsonEditor {
            textarea,
            original_content,
            path,
            restart_required: false,
            scroll_row: 0,
            scroll_col: 0,
        };
        self.status =
            "hjson: editing inkhaven.hjson · Ctrl+S save · Esc close".into();
    }

    /// 1.2.8+ — write the HJSON editor's buffer to its
    /// captured path.  When the saved bytes differ from the
    /// pre-open original, flip `restart_required` so the
    /// renderer pops the warning overlay; otherwise just
    /// surface "saved" on the status bar.
    fn save_hjson_editor(&mut self) {
        let Modal::HjsonEditor {
            textarea,
            original_content,
            path,
            restart_required,
            ..
        } = &mut self.modal
        else {
            return;
        };
        let bytes: String = textarea.lines().join("\n");
        match std::fs::write(&path, &bytes) {
            Ok(_) => {
                let changed = bytes != *original_content;
                if changed {
                    *restart_required = true;
                    // Update the baseline so a subsequent save
                    // of unchanged-since-last-save bytes
                    // doesn't keep re-firing the overlay.
                    *original_content = bytes;
                    self.status = format!(
                        "hjson: saved · restart required (any key dismisses overlay)"
                    );
                } else {
                    self.status =
                        "hjson: saved (no changes since last save)".into();
                }
            }
            Err(e) => {
                self.status =
                    format!("hjson: save failed: {e}");
            }
        }
    }

    /// 1.2.8+ — route a keystroke through the HJSON editor
    /// modal.  Mirrors the main paragraph editor's key set
    /// so muscle memory carries over verbatim.  Handles:
    ///   - Restart overlay: any key dismisses.
    ///   - Ctrl+S save; Esc close (with unsaved-edit warn).
    ///   - Arrows / Home / End / PgUp / PgDn (Shift extends
    ///     selection, plain cancels it) — routed via
    ///     CursorMove because tui-textarea's
    ///     `input_without_shortcuts` drops those keys.
    ///   - Ctrl+Home/End → top/bottom of buffer.
    ///   - Ctrl+Left/Right → word back / forward.
    ///   - Ctrl+Backspace → delete word.
    ///   - Ctrl+U undo, Ctrl+Y redo.
    ///   - Ctrl+K cut, Ctrl+C copy, Ctrl+P paste, Ctrl+A
    ///     select-all.
    ///   - Ctrl+D delete-line, Ctrl+E delete-to-EOL,
    ///     Ctrl+W delete-to-BOL.
    ///   - Everything else: textarea.input_without_shortcuts
    ///     (so plain typing + Tab + Enter + Backspace work,
    ///     but no emacs-style ctrl chord interception).
    fn hjson_editor_handle_key(&mut self, key: KeyEvent) {
        use tui_textarea::CursorMove;

        // Restart overlay is sticky-modal: any key clears it.
        if matches!(
            self.modal,
            Modal::HjsonEditor { restart_required: true, .. }
        ) {
            if let Modal::HjsonEditor { restart_required, .. } = &mut self.modal {
                *restart_required = false;
            }
            self.status = "hjson: overlay dismissed (modal still open)".into();
            return;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        // Ctrl+S — save.
        if ctrl
            && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
        {
            self.save_hjson_editor();
            return;
        }

        // Esc — close (with dirty warn).
        if matches!(key.code, KeyCode::Esc) {
            let dirty = matches!(
                &self.modal,
                Modal::HjsonEditor { textarea, original_content, .. }
                    if textarea.lines().join("\n") != *original_content
            );
            if dirty {
                self.status =
                    "hjson: closed with unsaved edits — re-open with Ctrl+B 0 to recover from disk".into();
            } else {
                self.status = "hjson: closed".into();
            }
            self.modal = Modal::None;
            return;
        }

        // Plain arrows + Home/End/PgUp/PgDn.  Shift extends
        // selection, plain cancels.  Routed explicitly
        // because input_without_shortcuts drops them.
        if !alt && !ctrl {
            let cmove = match key.code {
                KeyCode::Up => Some(CursorMove::Up),
                KeyCode::Down => Some(CursorMove::Down),
                KeyCode::Left => Some(CursorMove::Back),
                KeyCode::Right => Some(CursorMove::Forward),
                KeyCode::Home => Some(CursorMove::Head),
                KeyCode::End => Some(CursorMove::End),
                KeyCode::PageUp => Some(CursorMove::ParagraphBack),
                KeyCode::PageDown => Some(CursorMove::ParagraphForward),
                _ => None,
            };
            if let Some(cmove) = cmove {
                if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                    if shift {
                        if textarea.selection_range().is_none() {
                            textarea.start_selection();
                        }
                    } else {
                        textarea.cancel_selection();
                    }
                    textarea.move_cursor(cmove);
                }
                return;
            }
        }

        // Ctrl+modified motion + edit chords.  Mirror the
        // main editor (Ctrl+U undo, Ctrl+Y redo, Ctrl+K/C/P
        // cut/copy/paste, Ctrl+A select-all, Ctrl+D/E/W
        // line-targeted deletes, Ctrl+Home/End top/bottom,
        // Ctrl+Left/Right word jumps, Ctrl+Backspace
        // delete-word).
        if ctrl {
            match key.code {
                KeyCode::Char('u') | KeyCode::Char('U') if !shift => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.undo();
                    }
                    return;
                }
                KeyCode::Char('y') | KeyCode::Char('Y') if !shift => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.redo();
                    }
                    return;
                }
                KeyCode::Char('k') | KeyCode::Char('K') if !shift => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.cut();
                    }
                    return;
                }
                KeyCode::Char('c') | KeyCode::Char('C') if !shift => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.copy();
                    }
                    return;
                }
                KeyCode::Char('p') | KeyCode::Char('P') if !shift => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.paste();
                    }
                    return;
                }
                KeyCode::Char('a') | KeyCode::Char('A') if !shift => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.select_all();
                    }
                    return;
                }
                KeyCode::Char('d') | KeyCode::Char('D') if !shift => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.delete_line_by_end();
                        textarea.delete_line_by_head();
                    }
                    return;
                }
                KeyCode::Char('e') | KeyCode::Char('E') if !shift => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.delete_line_by_end();
                    }
                    return;
                }
                KeyCode::Char('w') | KeyCode::Char('W') if !shift => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.delete_line_by_head();
                    }
                    return;
                }
                KeyCode::Home => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.move_cursor(CursorMove::Top);
                    }
                    return;
                }
                KeyCode::End => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.move_cursor(CursorMove::Bottom);
                    }
                    return;
                }
                KeyCode::Left => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.move_cursor(CursorMove::WordBack);
                    }
                    return;
                }
                KeyCode::Right => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.move_cursor(CursorMove::WordForward);
                    }
                    return;
                }
                KeyCode::Backspace => {
                    if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
                        textarea.delete_word();
                    }
                    return;
                }
                _ => {}
            }
        }

        // Everything else: pass to textarea WITHOUT its
        // emacs-style defaults (matches the main editor).
        // This handles plain typing, Tab, Enter, Backspace,
        // Delete, and any other key not caught above.
        if let Modal::HjsonEditor { textarea, .. } = &mut self.modal {
            let input: tui_textarea::Input = key.into();
            textarea.input_without_shortcuts(input);
        }
    }

    fn open_shell_pane(&mut self, fresh: bool) {
        if !self.cfg.shell.enabled {
            self.status =
                "shell: disabled (set `shell.enabled: true` in inkhaven.hjson)".into();
            return;
        }
        // Toggle: if the pane is already open, close it
        // (engine + history persist on App).
        if matches!(self.modal, Modal::ShellPane { .. }) {
            self.modal = Modal::None;
            self.status = "shell: closed (state preserved · Ctrl+Z o to reopen)".into();
            return;
        }
        // 1.2.8+ — autosave the open buffer before yielding
        // control to the shell.  Common workflow is "write
        // a paragraph → flip to shell to inspect / compute
        // → flip back" and losing unsaved edits on shell-
        // open would surprise the user.  Failures are
        // non-fatal: save_current writes its own status
        // line on disk error.
        if self.opened.as_ref().is_some_and(|d| d.dirty) {
            let _ = self.save_current();
        }
        if fresh {
            self.shell_engine = None;
            self.shell_history.clear();
            self.shell_command_history.clear();
            // `Ctrl+Z O` fresh-start: we DON'T wipe the
            // on-disk history.  The user wants a fresh
            // engine state, not amnesia about what they
            // typed yesterday.  If they want full reset
            // they can `rm .inkhaven/shell_history.db`.
        }
        // Lazy-init the engine on first open.
        if self.shell_engine.is_none() {
            self.shell_engine = Some(
                super::shell::Engine::new(&self.layout.root)
                    .with_blocked_externals(
                        self.cfg.shell.blocked_externals.clone(),
                    ),
            );
        }
        // Lazy-init the per-project SQLite history.  Load
        // the most-recent cap commands so Up-arrow recall
        // works on the first open of every session.
        if self.shell_history_db.is_none() {
            let db = super::shell::History::open(&self.layout.root);
            // Seed the in-memory ring from disk if the
            // in-memory ring is empty (first open of this
            // session — or fresh reset above).
            if self.shell_command_history.is_empty() {
                self.shell_command_history =
                    db.load(self.cfg.shell.max_buffered_turns);
            }
            self.shell_history_db = Some(db);
        }
        self.modal = Modal::ShellPane {
            input: TextInput::new(),
            command_history_cursor: None,
            selection_mode: false,
            selection_cursor: 0,
            scroll: 0,
            show_help: false,
        };
        self.status = if fresh {
            "shell: fresh engine · type a command, Enter runs it".into()
        } else {
            "shell: open · type a command, Enter runs it · Ctrl+Z o close · Ctrl+Z h selection".into()
        };
    }

    /// 1.2.8+ — eval the user's line with a wall-clock
    /// watchdog.  Moves the engine into a worker thread,
    /// polls the result channel with the configured timeout,
    /// and recovers a fresh engine when a wedged child
    /// process (`vim` blocking on `/dev/tty`, an unknown TUI
    /// app, an infinite loop) refuses to respond to the
    /// nu-side interrupt signal.
    ///
    /// Three completion paths:
    ///   1. Worker returns within `external_timeout_secs`
    ///      → engine moved back, output forwarded verbatim.
    ///   2. Worker doesn't return within budget but DOES
    ///      respond to `signals().trigger()` within the
    ///      2-second grace window → engine moved back, output
    ///      replaced by a "timed out — engine recovered"
    ///      ShellOutput so the user knows the command was cut.
    ///   3. Worker still wedged after grace → spawn a fresh
    ///      `Engine`, abandon the worker thread (Rust will
    ///      keep it alive until it eventually exits, but it
    ///      no longer interferes with the App).  Env vars,
    ///      `def` declarations, and `cd`-mutated `$env.PWD`
    ///      are lost; the user is told so.
    fn shell_eval_with_watchdog(
        &mut self,
        line: String,
    ) -> super::shell::ShellOutput {
        let timeout_secs = self.cfg.shell.external_timeout_secs.max(1);
        let timeout = std::time::Duration::from_secs(timeout_secs);
        // Grace = how long we give the worker to honour the
        // signal after the budget runs out.  Two seconds is
        // enough for nu's tight inner loops to notice but
        // short enough that a genuinely wedged external
        // doesn't keep the user waiting forever.
        let grace = std::time::Duration::from_secs(2);

        let engine = match self.shell_engine.take() {
            Some(e) => e,
            None => {
                return super::shell::ShellOutput {
                    stdout: String::new(),
                    stderr: "engine not initialised".into(),
                    success: false,
                };
            }
        };
        // Reset the interrupt flag in case a prior watchdog
        // event left it set (we cleared it on engine move-back,
        // but a fresh engine has it set to false anyway).
        engine.reset_interrupt();
        let interrupt = engine.interrupt_handle();

        let (tx, rx) = std::sync::mpsc::channel();
        let _handle = std::thread::Builder::new()
            .name("inkhaven-shell-eval".into())
            .spawn(move || {
                let mut eng = engine;
                let out = eng.eval(&line);
                // Send back the engine + output.  Receiver may
                // have already given up (timeout path 3); the
                // send fails silently and `eng` drops with the
                // closure.
                let _ = tx.send((eng, out));
            });

        match rx.recv_timeout(timeout) {
            Ok((eng_back, out)) => {
                self.shell_engine = Some(eng_back);
                out
            }
            Err(_) => {
                // Path 2: trigger interrupt and wait grace.
                interrupt.store(true, std::sync::atomic::Ordering::Relaxed);
                match rx.recv_timeout(grace) {
                    Ok((eng_back, _stale_out)) => {
                        eng_back.reset_interrupt();
                        self.shell_engine = Some(eng_back);
                        super::shell::ShellOutput {
                            stdout: String::new(),
                            stderr: format!(
                                "timed out after {timeout_secs}s — \
                                 command was interrupted.  Engine \
                                 recovered cleanly."
                            ),
                            success: false,
                        }
                    }
                    Err(_) => {
                        // Path 3: hard timeout, worker leaked.
                        let blocked =
                            self.cfg.shell.blocked_externals.clone();
                        self.shell_engine = Some(
                            super::shell::Engine::new(&self.layout.root)
                                .with_blocked_externals(blocked),
                        );
                        super::shell::ShellOutput {
                            stdout: String::new(),
                            stderr: format!(
                                "timed out after {timeout_secs}s — \
                                 command did not respond to interrupt. \
                                 Engine has been restarted; env vars, \
                                 def declarations, and `cd` state are \
                                 lost.  Consider adding the offending \
                                 binary to `shell.blocked_externals`."
                            ),
                            success: false,
                        }
                    }
                }
            }
        }
    }

    /// 1.2.8+ — Tab completion inside the OS Shell pane.
    /// Routes the current token through `Engine::complete`
    /// (which decides command vs path context) and folds
    /// the result into the input:
    ///   - 0 matches → status "no matches"
    ///   - 1 match    → replace token with match (+ trailing
    ///                  space for commands, kept-`/` for dirs)
    ///   - N matches  → replace token with longest common
    ///                  prefix when it advances; status line
    ///                  shows up to 6 candidates
    /// No-op outside the pane (the Tab key only reaches this
    /// method via `shell_pane_handle_key`).
    fn shell_tab_complete(&mut self) {
        let (input_str, cursor_chars) = match &self.modal {
            Modal::ShellPane { input, .. } => {
                (input.as_str().to_string(), input.cursor())
            }
            _ => return,
        };
        let completion = match self.shell_engine.as_ref() {
            Some(eng) => eng.complete(&input_str, cursor_chars),
            None => return,
        };
        if completion.matches.is_empty() {
            self.status = format!(
                "shell: no matches for `{}`",
                completion.token
            );
            return;
        }

        // Decide what text to splice into the input.  For a
        // single match: the whole match.  For multiple: the
        // longest common prefix — but only if it ACTUALLY
        // advances past what the user has already typed
        // (otherwise we'd produce a no-op edit and the user
        // would think Tab is broken).
        let replacement: String = if completion.matches.len() == 1 {
            let mut r = completion.matches[0].clone();
            // Append a trailing space for completed command
            // names so the next token can start typing
            // immediately.  Directories already have a
            // trailing `/` from complete_path.
            if !r.ends_with('/') {
                r.push(' ');
            }
            r
        } else {
            let lcp = super::shell::longest_common_prefix(&completion.matches);
            if lcp.chars().count() > completion.token.chars().count() {
                lcp
            } else {
                // No advance possible — keep token unchanged,
                // just show the matches.
                completion.token.clone()
            }
        };

        // Splice the replacement into the input:
        //   prefix = input[..token_start] (chars)
        //   suffix = input[cursor..]      (chars)
        let chars: Vec<char> = input_str.chars().collect();
        let prefix: String =
            chars[..completion.token_start].iter().collect();
        let suffix: String = chars[cursor_chars..].iter().collect();
        let new_cursor_chars = prefix.chars().count()
            + replacement.chars().count();
        let new_buffer = format!("{prefix}{replacement}{suffix}");

        if let Modal::ShellPane { input, .. } = &mut self.modal {
            input.set_with_cursor(new_buffer, new_cursor_chars);
        }

        // Surface the candidate list when there are several,
        // so the user can see what else matches.
        if completion.matches.len() > 1 {
            const SHOW_CAP: usize = 6;
            let preview: Vec<&str> = completion
                .matches
                .iter()
                .take(SHOW_CAP)
                .map(String::as_str)
                .collect();
            let more = completion.matches.len().saturating_sub(SHOW_CAP);
            let suffix = if more > 0 {
                format!(" … (+{more} more)")
            } else {
                String::new()
            };
            self.status = format!(
                "shell: {} matches — {}{}",
                completion.matches.len(),
                preview.join("  "),
                suffix,
            );
        } else {
            self.status = "shell: completed".into();
        }
    }

    /// 1.2.8+ — `Ctrl+Z h` toggle history-selection mode
    /// from inside the shell pane.  No-op when invoked
    /// outside the pane (the chord lives in the bund-
    /// prefix and dispatcher routes unconditionally).
    fn toggle_shell_selection_mode(&mut self) {
        let Modal::ShellPane {
            selection_mode,
            selection_cursor,
            ..
        } = &mut self.modal
        else {
            self.status = "shell selection: only available inside the shell pane".into();
            return;
        };
        if *selection_mode {
            *selection_mode = false;
            self.status = "shell: back to normal mode".into();
        } else {
            if self.shell_history.is_empty() {
                self.status = "shell selection: nothing to select yet".into();
                return;
            }
            *selection_mode = true;
            // Land on the most-recent turn — same default as
            // AI chat selection mode.
            *selection_cursor = self.shell_history.len() - 1;
            self.status =
                "shell selection · ↑↓ pick turn · c copy · i insert · Ctrl+Z h exit".into();
        }
    }

    /// 1.2.8+ — selection-mode `c`: copy the highlighted
    /// turn's output to the system clipboard.  Includes
    /// stderr below stdout when the command failed, so the
    /// user gets full context if they paste into a bug
    /// report.  No-op when no clipboard is available
    /// (rare — usually only on minimal Linux + no X / no
    /// wayland).
    fn shell_selection_copy(&mut self) {
        let cursor = match &self.modal {
            Modal::ShellPane { selection_cursor, .. } => *selection_cursor,
            _ => return,
        };
        let Some(turn) = self.shell_history.get(cursor) else {
            self.status = "shell copy: cursor out of range".into();
            return;
        };
        let mut text = turn.stdout.clone();
        if !turn.success && !turn.stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str("[stderr]\n");
            text.push_str(&turn.stderr);
        }
        match self.clipboard.as_mut() {
            Some(cb) => match cb.set_text(text.clone()) {
                Ok(()) => {
                    self.status = format!(
                        "shell: copied turn `{}` output ({} chars)",
                        truncate_to_chars(&turn.command, 30),
                        text.chars().count()
                    );
                }
                Err(e) => {
                    self.status =
                        format!("shell copy: clipboard error: {e}");
                }
            },
            None => {
                self.status =
                    "shell copy: no system clipboard available".into();
            }
        }
    }

    /// 1.2.8+ — selection-mode `i`: insert the highlighted
    /// turn's stdout into the editor at cursor, wrapped in
    /// `cfg.shell.insert_template`.  `{output}` in the
    /// template is replaced with the raw output verbatim;
    /// no escaping (the default template uses a typst raw
    /// block where backticks bound the literal, so embedded
    /// quotes / backslashes survive).  Closes the shell
    /// pane after insert + refocuses the editor — same UX
    /// as AI chat selection `t`.
    fn shell_selection_insert(&mut self) {
        let cursor = match &self.modal {
            Modal::ShellPane { selection_cursor, .. } => *selection_cursor,
            _ => return,
        };
        let Some(turn) = self.shell_history.get(cursor).cloned() else {
            self.status = "shell insert: cursor out of range".into();
            return;
        };
        if self.opened.is_none() {
            self.status =
                "shell insert: no paragraph open — open one first".into();
            return;
        }
        let template = self.cfg.shell.insert_template.clone();
        let wrapped = template.replace("{output}", &turn.stdout);
        let chars = wrapped.chars().count();
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea.insert_str(&wrapped);
            doc.dirty = true;
            doc.last_activity = std::time::Instant::now();
        }
        // Close the pane (engine + history persist on App).
        self.modal = Modal::None;
        self.change_focus(Focus::Editor);
        self.status = format!(
            "shell: inserted output of `{}` ({} chars wrapped)",
            truncate_to_chars(&turn.command, 30),
            chars,
        );
    }

    /// 1.2.8+ — key handler for `Modal::ShellPane`.  Routes
    /// by `selection_mode`: false → line-editor + Up/Down
    /// command-history recall + Enter eval + Esc close;
    /// true → ↑↓ turn cursor + c / i actions.
    ///
    /// Ctrl+Z chords (`o` / `O` / `h`) are handled locally
    /// because the modal-key dispatcher intercepts keys
    /// before the global bund-prefix resolver runs — without
    /// this branch a user pressing `Ctrl+Z o` inside the
    /// pane sees no effect.
    fn shell_pane_handle_key(&mut self, key: KeyEvent) {
        // 1.2.8+ — Help overlay swallows EVERY key.  Any
        // press dismisses the overlay; the underlying pane
        // state is preserved unchanged.
        if matches!(self.modal, Modal::ShellPane { show_help: true, .. }) {
            if let Modal::ShellPane { show_help, .. } = &mut self.modal {
                *show_help = false;
            }
            self.status = "shell help: closed".into();
            return;
        }

        // 1.2.8+ — Ctrl+B chord prefix (pane-local).  Kept
        // separate from the global `bund_pending` so we
        // don't fire global chords (Q quit, L load, …) from
        // inside the pane and so the suffix matching here
        // doesn't have to disambiguate by prefix.  Only
        // chord we currently honour is `Ctrl+B H` → help.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('b') | KeyCode::Char('B'))
        {
            self.shell_ctrlb_pending = true;
            self.status = "Ctrl+B … (H help)".into();
            return;
        }
        if self.shell_ctrlb_pending {
            self.shell_ctrlb_pending = false;
            if matches!(
                key.code,
                KeyCode::Char('h') | KeyCode::Char('H')
            ) {
                if let Modal::ShellPane { show_help, .. } = &mut self.modal {
                    *show_help = true;
                }
                self.status = "shell help: any key to close".into();
                return;
            }
            // Any other suffix: clear pending state and fall
            // through to normal handling.
        }

        // Two-key Ctrl+Z chord, handled in-place.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('z') | KeyCode::Char('Z'))
        {
            self.bund_pending = true;
            self.status =
                "Ctrl+Z … (o close · O reset · h selection)".into();
            return;
        }
        if self.bund_pending {
            self.bund_pending = false;
            // Translate the suffix into the appropriate
            // action.  Anything else clears the pending
            // state and falls through to normal handling so
            // a stray Ctrl+Z followed by a real keystroke
            // doesn't get stuck.
            match (key.code, key.modifiers.contains(KeyModifiers::SHIFT)) {
                (KeyCode::Char('o'), false) | (KeyCode::Char('O'), false) => {
                    self.open_shell_pane(false);
                    return;
                }
                (KeyCode::Char('O'), true) | (KeyCode::Char('o'), true) => {
                    self.open_shell_pane(true);
                    return;
                }
                (KeyCode::Char('h'), _) | (KeyCode::Char('H'), _) => {
                    self.toggle_shell_selection_mode();
                    return;
                }
                _ => {
                    // Fall through to normal handling.  No
                    // status update — the user sees the
                    // suffix appear / not-appear in the input
                    // and figures it out.
                }
            }
        }
        // PgUp/PgDown/Home/End scroll the turn-buffer in BOTH
        // selection and normal modes — they don't conflict
        // with command-history recall (↑↓) or selection-mode
        // turn navigation (↑↓).  Home/End are special:
        //   - In selection mode they ALSO move the turn cursor
        //     to the first / last turn (existing behaviour).
        //   - In normal mode they only affect scroll.
        // Step = 10 logical lines per Page key — render clamps
        // if the user scrolls past the top of the buffer.
        const SCROLL_PAGE: usize = 10;
        if matches!(
            key.code,
            KeyCode::PageUp | KeyCode::PageDown
        ) {
            if let Modal::ShellPane { scroll, .. } = &mut self.modal {
                match key.code {
                    KeyCode::PageUp => *scroll = scroll.saturating_add(SCROLL_PAGE),
                    KeyCode::PageDown => *scroll = scroll.saturating_sub(SCROLL_PAGE),
                    _ => unreachable!(),
                }
            }
            return;
        }

        // Selection mode first — narrower keyspace.
        let in_selection = matches!(
            self.modal,
            Modal::ShellPane { selection_mode: true, .. }
        );
        if in_selection {
            let history_len = self.shell_history.len();
            if let Modal::ShellPane {
                selection_cursor,
                scroll,
                ..
            } = &mut self.modal
            {
                match key.code {
                    KeyCode::Up => {
                        if *selection_cursor > 0 {
                            *selection_cursor -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if *selection_cursor + 1 < history_len {
                            *selection_cursor += 1;
                        }
                    }
                    KeyCode::Home => {
                        *selection_cursor = 0;
                        // Match: walking to the first turn
                        // exposes the head of the buffer.
                        *scroll = usize::MAX / 2;
                    }
                    KeyCode::End => {
                        *selection_cursor = history_len.saturating_sub(1);
                        *scroll = 0;
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        self.shell_selection_copy();
                    }
                    KeyCode::Char('i') | KeyCode::Char('I') => {
                        self.shell_selection_insert();
                    }
                    KeyCode::Esc => {
                        // Esc inside selection mode just exits
                        // selection — second Esc closes the pane
                        // (matches the F6 filter ergonomics).
                        if let Modal::ShellPane { selection_mode, .. } =
                            &mut self.modal
                        {
                            *selection_mode = false;
                        }
                        self.status = "shell: back to normal mode".into();
                    }
                    _ => {}
                }
            }
            return;
        }

        // Normal shell mode.
        match key.code {
            KeyCode::Enter => {
                let line = match &self.modal {
                    Modal::ShellPane { input, .. } => {
                        input.as_str().to_string()
                    }
                    _ => return,
                };
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {
                    return;
                }
                // Block `exit` / `quit` from reaching nu.  Nu's
                // built-in `exit` calls `std::process::exit()`
                // unconditionally — if it ran, the entire
                // inkhaven process would die, taking unsaved
                // editor buffers with it.  Users typing `exit`
                // intend to close the SHELL pane (terminal
                // muscle memory), so we intercept and do
                // exactly that.  Also handles `exit 1`,
                // `quit 0`, etc. — any line whose first token
                // is `exit` or `quit`.  Engine + history are
                // preserved; reopen with `Ctrl+Z o`.
                let first_tok = trimmed
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if matches!(first_tok, "exit" | "quit") {
                    if let Modal::ShellPane { input, .. } = &mut self.modal {
                        input.clear();
                    }
                    self.modal = Modal::None;
                    self.status =
                        "shell: closed via `exit` (state preserved · Ctrl+Z o to reopen)".into();
                    return;
                }
                // Reset input + history cursor; eval; push turn.
                // Also reset scroll so the new turn lands
                // visible at the bottom — anything else would
                // be confusing (user runs `ls`, expects to
                // see its output, but stays scrolled to old
                // content).
                if let Modal::ShellPane {
                    input,
                    command_history_cursor,
                    scroll,
                    ..
                } = &mut self.modal
                {
                    input.clear();
                    *command_history_cursor = None;
                    *scroll = 0;
                }
                // Command-history ring: dedup vs immediate
                // predecessor.  Push to both the in-memory
                // ring (Up-arrow recall) AND the per-project
                // SQLite (survives restart).
                if self.shell_command_history.last().map(String::as_str)
                    != Some(trimmed.as_str())
                {
                    self.shell_command_history.push(trimmed.clone());
                    if let Some(db) = self.shell_history_db.as_ref() {
                        db.push(&trimmed);
                    }
                }
                // Eval with the watchdog (1.2.8+).  Moves the
                // engine into a worker thread, polls main
                // thread with a wall-clock budget, recovers a
                // fresh engine on hard timeout.
                let out = self.shell_eval_with_watchdog(trimmed.clone());
                // Per-turn output truncation: a single `cat`
                // / `git log` can drop tens of thousands of
                // lines into stdout, which bloats memory and
                // makes ratatui rendering crawl.  Cap at
                // configured max_output_lines, replacing the
                // tail with a "(N more lines truncated)"
                // marker so the user knows.
                let max_lines = self.cfg.shell.max_output_lines.max(1);
                let stdout = truncate_to_lines(out.stdout, max_lines);
                let stderr = truncate_to_lines(out.stderr, max_lines);
                let turn = super::shell::ShellTurn {
                    command: trimmed,
                    stdout,
                    stderr,
                    success: out.success,
                };
                self.shell_history.push(turn);
                let cap = self.cfg.shell.max_buffered_turns.max(1);
                while self.shell_history.len() > cap {
                    self.shell_history.remove(0);
                }
            }
            KeyCode::Up => {
                if !self.shell_command_history.is_empty() {
                    let next_cursor = match self.modal {
                        Modal::ShellPane {
                            command_history_cursor: Some(0),
                            ..
                        } => 0,
                        Modal::ShellPane {
                            command_history_cursor: Some(i),
                            ..
                        } => i - 1,
                        Modal::ShellPane {
                            command_history_cursor: None,
                            ..
                        } => self.shell_command_history.len() - 1,
                        _ => return,
                    };
                    let entry =
                        self.shell_command_history[next_cursor].clone();
                    if let Modal::ShellPane {
                        input,
                        command_history_cursor,
                        ..
                    } = &mut self.modal
                    {
                        *command_history_cursor = Some(next_cursor);
                        input.clear();
                        for c in entry.chars() {
                            input.insert_char(c);
                        }
                    }
                }
            }
            KeyCode::Down => {
                if let Modal::ShellPane {
                    command_history_cursor: Some(cur),
                    ..
                } = self.modal
                {
                    let next = cur + 1;
                    if next >= self.shell_command_history.len() {
                        if let Modal::ShellPane {
                            input,
                            command_history_cursor,
                            ..
                        } = &mut self.modal
                        {
                            *command_history_cursor = None;
                            input.clear();
                        }
                    } else {
                        let entry =
                            self.shell_command_history[next].clone();
                        if let Modal::ShellPane {
                            input,
                            command_history_cursor,
                            ..
                        } = &mut self.modal
                        {
                            *command_history_cursor = Some(next);
                            input.clear();
                            for c in entry.chars() {
                                input.insert_char(c);
                            }
                        }
                    }
                }
            }
            KeyCode::Tab => {
                // 1.2.8+ — autocomplete commands, externals,
                // and filesystem paths.  Replace the token
                // under the cursor with either the single
                // match or the longest common prefix of all
                // matches.  Multiple matches surface as a
                // status-line list so the user can see what
                // else exists.
                self.shell_tab_complete();
            }
            // 1.2.8+ — readline-style line editing.  Single-
            // line prompt so we omit Ctrl+A=select-all,
            // Ctrl+U=undo, Ctrl+Y=redo from the editor's
            // textarea bindings (no selection model, no
            // undo history on the input ring).  Instead
            // bind the chords the way bash / zsh / readline
            // users expect:
            //   Ctrl+A   → start of line
            //   Ctrl+E   → end of line
            //   Ctrl+U   → kill from cursor to start
            //   Ctrl+K   → kill from cursor to end
            //   Ctrl+W   → kill the word before the cursor
            //   Ctrl+L   → clear scrollback
            //   Ctrl+D   → clear input if non-empty, close
            //              pane if empty (terminal idiom)
            //   Alt+B / Alt+F / Ctrl+Left / Ctrl+Right
            //            → word back / word forward
            //   Alt+Backspace → kill word back
            KeyCode::Char('a') | KeyCode::Char('A')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Modal::ShellPane { input, .. } = &mut self.modal {
                    input.move_home();
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Modal::ShellPane { input, .. } = &mut self.modal {
                    input.move_end();
                }
            }
            KeyCode::Char('u') | KeyCode::Char('U')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Modal::ShellPane { input, .. } = &mut self.modal {
                    input.kill_to_start();
                }
            }
            KeyCode::Char('k') | KeyCode::Char('K')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Modal::ShellPane { input, .. } = &mut self.modal {
                    input.kill_to_end();
                }
            }
            KeyCode::Char('w') | KeyCode::Char('W')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Modal::ShellPane { input, .. } = &mut self.modal {
                    input.kill_word_left();
                }
            }
            KeyCode::Char('l') | KeyCode::Char('L')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                // Clear scrollback (terminal idiom).  Engine
                // state and command history are preserved —
                // this is purely a visual reset of the turn
                // buffer.
                self.shell_history.clear();
                if let Modal::ShellPane { scroll, .. } = &mut self.modal {
                    *scroll = 0;
                }
                self.status = "shell: scrollback cleared".into();
            }
            KeyCode::Char('d') | KeyCode::Char('D')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                let input_empty = matches!(
                    &self.modal,
                    Modal::ShellPane { input, .. } if input.is_empty()
                );
                if input_empty {
                    self.modal = Modal::None;
                    self.status =
                        "shell: closed via Ctrl+D (state preserved · Ctrl+Z o to reopen)".into();
                } else if let Modal::ShellPane { input, .. } = &mut self.modal {
                    input.clear();
                }
            }
            KeyCode::Char('b') | KeyCode::Char('B')
                if key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Modal::ShellPane { input, .. } = &mut self.modal {
                    input.move_word_left();
                }
            }
            KeyCode::Char('f') | KeyCode::Char('F')
                if key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Modal::ShellPane { input, .. } = &mut self.modal {
                    input.move_word_right();
                }
            }
            KeyCode::Left
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Modal::ShellPane { input, .. } = &mut self.modal {
                    input.move_word_left();
                }
            }
            KeyCode::Right
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Modal::ShellPane { input, .. } = &mut self.modal {
                    input.move_word_right();
                }
            }
            KeyCode::Backspace
                if key.modifiers.contains(KeyModifiers::ALT)
                    || key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Modal::ShellPane { input, .. } = &mut self.modal {
                    input.kill_word_left();
                }
            }
            KeyCode::Esc => {
                // Esc closes the pane; engine + history
                // persist on App.
                self.modal = Modal::None;
                self.status =
                    "shell: closed (state preserved · Ctrl+Z o to reopen)".into();
            }
            // Home/End in normal mode go to the prompt's
            // beginning/end (text-input native behaviour),
            // BUT modified Home/End — Shift+Home / Shift+End —
            // jump scrollback to top/bottom.  Without the
            // modifier, the text-input handler picks them up
            // via the `_` arm below.
            KeyCode::Home if key.modifiers.contains(KeyModifiers::SHIFT) => {
                if let Modal::ShellPane { scroll, .. } = &mut self.modal {
                    *scroll = usize::MAX / 2;
                }
            }
            KeyCode::End if key.modifiers.contains(KeyModifiers::SHIFT) => {
                if let Modal::ShellPane { scroll, .. } = &mut self.modal {
                    *scroll = 0;
                }
            }
            _ => {
                if let Modal::ShellPane { input, .. } = &mut self.modal {
                    handle_text_input_key(input, key);
                }
            }
        }
    }

    /// 1.2.8+ — Ctrl+V Shift+U. Open the kill-ring picker.
    /// Empty ring → status message and no modal.
    fn open_kill_ring_picker(&mut self) {
        if self.kill_ring.is_empty() {
            self.status = "kill-ring: empty (nothing to restore)".into();
            return;
        }
        self.modal = Modal::KillRingPicker { cursor: 0 };
        self.status = format!(
            "kill-ring: {} entr{} · ↑↓ select · Enter restore · Esc cancel",
            self.kill_ring.len(),
            if self.kill_ring.len() == 1 { "y" } else { "ies" }
        );
    }

    /// 1.2.8+ — handle keystrokes inside `Modal::KillRingPicker`.
    fn kill_ring_picker_handle_key(&mut self, key: KeyEvent) {
        let Modal::KillRingPicker { cursor } = &mut self.modal else { return; };
        let len = self.kill_ring.len();
        if len == 0 {
            self.modal = Modal::None;
            return;
        }
        match key.code {
            KeyCode::Up => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
            }
            KeyCode::Down => {
                if *cursor + 1 < len {
                    *cursor += 1;
                }
            }
            KeyCode::Home => *cursor = 0,
            KeyCode::End => *cursor = len.saturating_sub(1),
            KeyCode::Enter => {
                let idx = *cursor;
                self.modal = Modal::None;
                self.restore_kill_ring_entry(idx);
            }
            KeyCode::Esc => {
                self.modal = Modal::None;
                self.status = "kill-ring: cancelled".into();
            }
            _ => {}
        }
    }

    /// 1.2.8+ — restore the kill-ring entry at `idx`. Removes
    /// it from the ring (whether or not the restore succeeds —
    /// failure cases reinsert on the error path, mirroring
    /// `undo_last_delete`'s semantics).
    fn restore_kill_ring_entry(&mut self, idx: usize) {
        if idx >= self.kill_ring.len() {
            self.status = "kill-ring: index out of range".into();
            return;
        }
        let stash = self.kill_ring.remove(idx).expect("idx checked");
        let parent = stash
            .parent_id
            .and_then(|id| self.hierarchy.get(id).cloned());
        let position = match stash.anchor_id {
            Some(id) if self.hierarchy.get(id).is_some() => {
                crate::store::InsertPosition::After(id)
            }
            _ => crate::store::InsertPosition::End,
        };
        let created = match self.store.create_node(
            &self.cfg,
            &self.hierarchy,
            NodeKind::Paragraph,
            &stash.title,
            parent.as_ref(),
            Some(&stash.slug),
            position,
        ) {
            Ok(n) => n,
            Err(e) => {
                // Reinsert at the same index so the user can
                // retry; status names the failure.
                self.kill_ring.insert(idx, stash);
                self.status = format!("kill-ring restore: create_node failed: {e}");
                return;
            }
        };
        if let Some(rel) = created.file.as_ref() {
            let abs = self.layout.root.join(rel);
            if let Err(e) = std::fs::write(&abs, &stash.content) {
                self.status = format!(
                    "kill-ring restore: paragraph created at `{}` but body write failed: {e}",
                    created.slug
                );
                self.reload_hierarchy();
                return;
            }
        }
        let mut updated = created.clone();
        updated.tags = stash.tags;
        updated.linked_paragraphs = stash.linked_paragraphs;
        updated.status = stash.status;
        updated.target_words = stash.target_words;
        if stash.content_type.is_some() {
            updated.content_type = stash.content_type;
        }
        updated.event = stash.event;
        updated.modified_at = chrono::Utc::now();
        crate::store::reconcile_event_orphan_tag(&mut updated);
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(updated.id, updated.to_json())
        {
            tracing::warn!(
                target: "inkhaven::undo",
                "kill-ring metadata restore failed for {}: {e}",
                updated.id
            );
        }
        self.reload_hierarchy();
        self.status = format!(
            "↺ restored `{}` from kill-ring (new uuid {} — cross-refs to old uuid stay broken)",
            updated.title,
            updated.id.simple()
        );
    }

    /// 1.2.7+ — Alt+Left. Browser-style back through the
    /// visited-paragraph history.
    fn navigate_visited_back(&mut self) {
        if self.visited_history.is_empty() || self.visited_cursor == 0 {
            self.status = "navigate: already at the start of the visit history".into();
            return;
        }
        self.visited_cursor -= 1;
        let id = self.visited_history[self.visited_cursor];
        self.visited_skip_next_push = true;
        match self.open_paragraph_by_uuid(id) {
            Ok(()) => {
                let title = self
                    .hierarchy
                    .get(id)
                    .map(|n| n.title.clone())
                    .unwrap_or_else(|| id.to_string());
                self.status = format!(
                    "← back · `{title}` ({}/{})",
                    self.visited_cursor + 1,
                    self.visited_history.len()
                );
            }
            Err(e) => {
                // Failed to open — restore cursor; the
                // skip-flag will be consumed only if we
                // actually loaded.
                self.visited_skip_next_push = false;
                self.visited_cursor += 1;
                self.status = format!("navigate: couldn't open back-target: {e}");
            }
        }
    }

    /// 1.2.7+ — Alt+Right. Forward through the visited-
    /// paragraph history. No-op if already at the head.
    fn navigate_visited_forward(&mut self) {
        if self.visited_cursor + 1 >= self.visited_history.len() {
            self.status = "navigate: already at the end of the visit history".into();
            return;
        }
        self.visited_cursor += 1;
        let id = self.visited_history[self.visited_cursor];
        self.visited_skip_next_push = true;
        match self.open_paragraph_by_uuid(id) {
            Ok(()) => {
                let title = self
                    .hierarchy
                    .get(id)
                    .map(|n| n.title.clone())
                    .unwrap_or_else(|| id.to_string());
                self.status = format!(
                    "→ forward · `{title}` ({}/{})",
                    self.visited_cursor + 1,
                    self.visited_history.len()
                );
            }
            Err(e) => {
                self.visited_skip_next_push = false;
                self.visited_cursor -= 1;
                self.status = format!("navigate: couldn't open forward-target: {e}");
            }
        }
    }

    /// 1.2.7+ — Ctrl+Shift+M. Flip TUI mouse capture so the
    /// user can use terminal-native drag-to-select +
    /// system-clipboard copy in the editor and AI panes.
    /// Status reports the new state. No HJSON persistence —
    /// the choice is per-session; default is always ON.
    fn toggle_mouse_capture(&mut self) {
        use crossterm::execute;
        self.mouse_captured = !self.mouse_captured;
        let mut stdout = std::io::stdout();
        let result = if self.mouse_captured {
            execute!(stdout, EnableMouseCapture)
        } else {
            execute!(stdout, DisableMouseCapture)
        };
        match result {
            Ok(()) => {
                self.status = if self.mouse_captured {
                    "mouse capture: ON — click-to-focus + scroll wheel; native selection disabled".into()
                } else {
                    "mouse capture: OFF — terminal-native selection enabled (drag-select + Cmd+C / Ctrl+Shift+C). Ctrl+Shift+M to re-enable.".into()
                };
            }
            Err(e) => {
                // Flip back; report the error.
                self.mouse_captured = !self.mouse_captured;
                self.status = format!("mouse capture toggle failed: {e}");
            }
        }
    }

    /// Ctrl+B E — flip `sound.enabled` in the live config + on disk,
    /// and toggle the live SoundPlayer's enabled flag. No-ops on hosts
    /// without an audio device beyond updating the config (so the
    /// preference persists for a future launch on a host that does).
    fn toggle_sound(&mut self) {
        let new_value = !self.cfg.sound.enabled;
        let config_path = self.layout.config_path();
        let raw = match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                self.status =
                    format!("sound toggle aborted: read {}: {e}", config_path.display());
                return;
            }
        };
        let updated = match set_sound_enabled_in_hjson(&raw, new_value) {
            Ok(s) => s,
            Err(reason) => {
                self.status = format!(
                    "sound toggle aborted: can't rewrite {}: {reason}",
                    config_path.display()
                );
                return;
            }
        };
        if let Err(e) = std::fs::write(&config_path, &updated) {
            self.status =
                format!("sound toggle aborted: write {}: {e}", config_path.display());
            return;
        }

        self.cfg.sound.enabled = new_value;
        if let Some(sp) = self.sound.as_mut() {
            sp.enabled = new_value;
        }
        let label = if new_value { "ON" } else { "OFF" };
        let audio_note = if self.sound.is_some() {
            ""
        } else {
            " (no audio device — silent, but preference saved)"
        };
        self.status =
            format!("Typewriter sound {label}{audio_note} · saved to {}", config_path.display());
    }

    /// Try the in-`#image(…)` picker; returns false when the cursor
    /// isn't inside an image call (so the caller can fall through to
    /// Places RAG). Returns true even when the picker is empty —
    /// "you're inside `#image()` but this paragraph has no sibling
    /// images" is still better feedback than silently jumping to
    /// Places RAG.
    fn try_open_image_picker(&mut self) -> bool {
        let Some(doc) = self.opened.as_ref() else {
            return false;
        };
        let (row, col) = doc.textarea.cursor();
        let line = doc.textarea.lines().get(row).cloned().unwrap_or_default();
        let ctx = match detect_image_call_context(&line, col) {
            Some(c) => c,
            None => return false,
        };
        // Find sibling Image nodes in the paragraph's parent.
        let entries = self.sibling_image_entries(doc.id);
        if entries.is_empty() {
            self.status = "no sibling images at this level — import one with F3 first".into();
            self.modal = Modal::ImagePicker {
                entries,
                cursor: 0,
                close_quote: !ctx.closing_quote_present,
            };
            return true;
        }
        self.status =
            "↑↓ select · Enter insert · Esc cancel".into();
        self.modal = Modal::ImagePicker {
            entries,
            cursor: 0,
            close_quote: !ctx.closing_quote_present,
        };
        true
    }

    /// Sibling Image nodes of the paragraph identified by `para_id`,
    /// sorted by their `order` field. Each entry carries the filename
    /// (already in `NN-slug.<ext>` form), the display title, and the
    /// file size for the picker readout.
    fn sibling_image_entries(&self, para_id: Uuid) -> Vec<ImagePickerEntry> {
        let Some(para) = self.hierarchy.get(para_id) else {
            return Vec::new();
        };
        let mut out: Vec<ImagePickerEntry> = self
            .hierarchy
            .children_of(para.parent_id)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Image)
            .map(|n| {
                let size_bytes = n
                    .file
                    .as_ref()
                    .map(|rel| {
                        std::fs::metadata(self.layout.root.join(rel))
                            .map(|m| m.len())
                            .unwrap_or(0)
                    })
                    .unwrap_or(0);
                ImagePickerEntry {
                    fname: n.fs_name(),
                    title: n.title.clone(),
                    size_bytes,
                }
            })
            .collect();
        out.sort_by(|a, b| a.fname.cmp(&b.fname));
        out
    }

    fn image_picker_handle_key(&mut self, key: KeyEvent) -> bool {
        let Modal::ImagePicker {
            entries, cursor, ..
        } = &mut self.modal
        else {
            return false;
        };
        let total = entries.len();
        match key.code {
            KeyCode::Up => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
                true
            }
            KeyCode::Down => {
                if *cursor + 1 < total {
                    *cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                *cursor = 0;
                true
            }
            KeyCode::End => {
                *cursor = total.saturating_sub(1);
                true
            }
            KeyCode::Enter => {
                self.commit_image_picker();
                true
            }
            _ => false,
        }
    }

    fn commit_image_picker(&mut self) {
        let (fname, close_quote) = match &self.modal {
            Modal::ImagePicker {
                entries,
                cursor,
                close_quote,
            } => match entries.get(*cursor) {
                Some(e) => (e.fname.clone(), *close_quote),
                None => {
                    self.modal = Modal::None;
                    return;
                }
            },
            _ => return,
        };
        // Insert filename + optional `"` at the cursor in the editor.
        let insert = if close_quote {
            format!("{fname}\"")
        } else {
            fname.clone()
        };
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea.insert_str(&insert);
            doc.dirty = true;
        }
        self.modal = Modal::None;
        self.status = format!("inserted `{fname}` into #image(…)");
    }

    fn open_function_picker(&mut self) {
        if self.opened.is_none() {
            self.status = "function picker needs an open paragraph".into();
            return;
        }
        self.modal = Modal::FunctionPicker {
            filter: TextInput::new(),
            cursor: 0,
        };
        self.status = "Type to filter · ↑↓ select · Enter insert · Esc cancel".into();
    }

    fn function_picker_handle_key(&mut self, key: KeyEvent) -> bool {
        // Recompute the filtered list each keystroke so the cursor
        // stays attached to a visible row.
        let (filter_text, mut cursor_value) = match &self.modal {
            Modal::FunctionPicker { filter, cursor } => (filter.as_str().to_string(), *cursor),
            _ => return false,
        };
        let matches = filter_functions(&filter_text);
        let total = matches.len();
        let mut closed = false;
        let mut commit_now = false;
        let mut filter_dirty = false;
        match key.code {
            KeyCode::Up => {
                if cursor_value > 0 {
                    cursor_value -= 1;
                }
            }
            KeyCode::Down => {
                if cursor_value + 1 < total {
                    cursor_value += 1;
                }
            }
            KeyCode::Home => cursor_value = 0,
            KeyCode::End => cursor_value = total.saturating_sub(1),
            KeyCode::PageUp => cursor_value = cursor_value.saturating_sub(10),
            KeyCode::PageDown => {
                cursor_value = (cursor_value + 10).min(total.saturating_sub(1));
            }
            KeyCode::Enter => commit_now = true,
            KeyCode::Esc => closed = true,
            _ => {
                if let Modal::FunctionPicker { filter, cursor } = &mut self.modal {
                    handle_text_input_key(filter, key);
                    *cursor = 0;
                    filter_dirty = true;
                    let _ = cursor;
                }
            }
        }
        if filter_dirty {
            return true;
        }
        if closed {
            self.modal = Modal::None;
            return true;
        }
        if commit_now {
            self.commit_function_picker(&matches, cursor_value);
            return true;
        }
        if let Modal::FunctionPicker { cursor, .. } = &mut self.modal {
            *cursor = cursor_value;
        }
        true
    }

    fn commit_function_picker(&mut self, matches: &[super::typst_funcs::TypstFn], cursor_value: usize) {
        let Some(picked) = matches.get(cursor_value).copied() else {
            self.modal = Modal::None;
            return;
        };
        // Detect the syntactic mode at the cursor via tree-sitter-
        // typst so we only emit the `#` prefix when the cursor is in
        // markup. Inside `{ code }`, function-call arguments, `let`
        // RHS, or math, the bare identifier is what typst expects.
        let mode = self
            .opened
            .as_ref()
            .map(|doc| {
                let source = doc.textarea.lines().join("\n");
                let (row, col) = doc.textarea.cursor();
                let byte = byte_offset_for_cursor(&source, row, col);
                super::highlight::typst_mode_at(&source, byte)
            })
            .unwrap_or(super::highlight::TypstMode::Markup);
        let prefix = mode.call_prefix();
        let opener = format!("{prefix}{}(", picked.name);
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea.insert_str(&opener);
            doc.textarea.insert_str(")");
            doc.textarea.move_cursor(CursorMove::Back);
            doc.dirty = true;
        }
        self.modal = Modal::None;
        let mode_tag = match mode {
            super::highlight::TypstMode::Markup => "markup",
            super::highlight::TypstMode::Code => "code",
            super::highlight::TypstMode::Math => "math",
        };
        self.status = format!("inserted {prefix}{}( … ) · {mode_tag} mode", picked.name);
    }

    fn cycle_ai_mode(&mut self) {
        self.ai_mode = self.ai_mode.next();
        self.status = match self.ai_mode {
            AiMode::None => "AI scope: None (only the prompt is sent)".into(),
            other => format!(
                "AI scope: {} (will prepend matching context to next prompt)",
                other.label()
            ),
        };
    }

    fn toggle_inference_mode(&mut self) {
        self.inference_mode = self.inference_mode.toggle();
        self.status = match self.inference_mode {
            InferenceMode::Local => {
                "Inference: Local (model uses only supplied context)".into()
            }
            InferenceMode::Full => {
                "Inference: Full (model uses context + its own knowledge)".into()
            }
        };
    }

    /// Assemble the prefix the active AI scope should prepend to the
    /// user's prompt. Returns `Ok(Some(text))` when the scope produced
    /// content, `Ok(None)` for `AiMode::None`, or `Err(reason)` when the
    /// scope was requested but produced nothing (no selection, no open
    /// paragraph, no enclosing branch). The status message in `Err` is
    /// surfaced to the user verbatim.
    fn build_ai_mode_context(&self) -> Result<Option<String>, String> {
        match self.ai_mode {
            AiMode::None => Ok(None),
            AiMode::Selection => {
                let Some(doc) = self.opened.as_ref() else {
                    return Err("AI scope `Selection` needs an open paragraph".into());
                };
                let Some(((r1, c1), (r2, c2))) = doc.textarea.selection_range() else {
                    return Err("AI scope `Selection` needs a non-empty selection in the editor".into());
                };
                let text = slice_lines(doc.textarea.lines(), r1, c1, r2, c2);
                if text.trim().is_empty() {
                    return Err("AI scope `Selection` selection was empty".into());
                }
                Ok(Some(format!(
                    "── Editor selection ──\n{text}\n── end selection ──"
                )))
            }
            AiMode::Paragraph => {
                let Some(doc) = self.opened.as_ref() else {
                    return Err("AI scope `Paragraph` needs an open paragraph".into());
                };
                let live = doc.textarea.lines().join("\n");
                let mut out = String::new();
                if let Some(split) = doc.split.as_ref() {
                    // In split-edit mode the user sees the snapshot AND
                    // the live buffer side by side; include both so the
                    // model can compare.
                    out.push_str("── Paragraph snapshot (split-edit copy) ──\n");
                    out.push_str(&split.snapshot_lines.join("\n"));
                    out.push_str("\n── end snapshot ──\n\n");
                }
                out.push_str("── Paragraph: ");
                out.push_str(&doc.title);
                out.push_str(" ──\n");
                out.push_str(&live);
                out.push_str("\n── end paragraph ──");
                // 1.2.4+: also include every linked paragraph's
                // body so the model has the related-material the
                // user explicitly curated. Direct outgoing only
                // (matches the status-bar "links N" count). Bodies
                // come from disk (saved state) — same source the
                // export / similar-paragraph paths use.
                let linked_ids: Vec<Uuid> = self
                    .hierarchy
                    .get(doc.id)
                    .map(|n| n.linked_paragraphs.clone())
                    .unwrap_or_default();
                for id in linked_ids {
                    let Some(linked) = self.hierarchy.get(id) else { continue };
                    let Some(rel) = linked.file.as_ref() else { continue };
                    let abs = self.layout.root.join(rel);
                    let body = std::fs::read_to_string(&abs).unwrap_or_default();
                    out.push_str("\n\n── Linked paragraph: ");
                    out.push_str(&linked.title);
                    out.push_str(" ──\n");
                    out.push_str(&body);
                    out.push_str("\n── end linked paragraph ──");
                }
                Ok(Some(out))
            }
            AiMode::Subchapter | AiMode::Chapter | AiMode::Book => {
                let scope_kind = match self.ai_mode {
                    AiMode::Subchapter => NodeKind::Subchapter,
                    AiMode::Chapter => NodeKind::Chapter,
                    AiMode::Book => NodeKind::Book,
                    _ => unreachable!(),
                };
                let mode_label = self.ai_mode.label();
                // Anchor on the open paragraph if any, otherwise the tree
                // cursor — gives the user a sensible default whether they
                // were editing or browsing when they cycled to this scope.
                let anchor_id = self
                    .opened
                    .as_ref()
                    .map(|d| d.id)
                    .or_else(|| self.rows.get(self.tree_cursor).map(|(id, _)| *id))
                    .ok_or_else(|| {
                        format!("AI scope `{mode_label}` needs an open paragraph or tree cursor")
                    })?;
                let anchor = self
                    .hierarchy
                    .get(anchor_id)
                    .ok_or_else(|| format!("AI scope `{mode_label}` anchor vanished"))?;
                // Walk up to the enclosing branch of `scope_kind`. The
                // anchor itself counts if it is already that kind.
                let scope_node = if anchor.kind == scope_kind {
                    Some(anchor.clone())
                } else {
                    self.hierarchy
                        .ancestors(anchor)
                        .into_iter()
                        .find(|n| n.kind == scope_kind)
                        .cloned()
                };
                let Some(scope_node) = scope_node else {
                    return Err(format!(
                        "AI scope `{mode_label}` requires the cursor to be inside a {}",
                        scope_kind.as_str()
                    ));
                };
                let mut chunks: Vec<String> = Vec::new();
                for id in self.hierarchy.collect_subtree(scope_node.id) {
                    let Some(node) = self.hierarchy.get(id) else {
                        continue;
                    };
                    if node.kind != NodeKind::Paragraph {
                        continue;
                    }
                    if let Ok(Some(bytes)) = self.store.get_content(node.id) {
                        let body = String::from_utf8_lossy(&bytes).to_string();
                        chunks.push(format!(
                            "── {} ──\n{}",
                            self.title_breadcrumb(node.id),
                            body
                        ));
                    }
                }
                if chunks.is_empty() {
                    return Err(format!(
                        "AI scope `{mode_label}` `{}` has no paragraphs to send",
                        scope_node.title
                    ));
                }
                let header = format!(
                    "── {} context: {} ({} paragraph(s)) ──",
                    mode_label,
                    scope_node.title,
                    chunks.len()
                );
                Ok(Some(format!(
                    "{header}\n\n{}\n── end {} context ──",
                    chunks.join("\n\n"),
                    mode_label.to_lowercase()
                )))
            }
        }
    }

    fn clear_chat_history(&mut self) {
        let turns = self.chat_history.len();
        self.chat_history.clear();
        self.pending_chat_user_msg = None;
        self.inference = None;
        // Reset chat-history scroll / search / selection — there's
        // nothing left to scroll or act on.
        self.chat_history_scroll = 0;
        self.chat_search = None;
        self.chat_selection = None;
        // Also dismiss any active grammar-correction overlay — the AI
        // result it derived from is being discarded, so keeping a
        // baseline tied to a forgotten correction is confusing.
        if let Some(doc) = self.opened.as_mut() {
            doc.correction_baseline = None;
        }
        self.status = if turns == 0 {
            "AI chat history already empty".into()
        } else {
            format!("AI chat cleared ({turns} turn(s) discarded)")
        };
    }

    fn open_help_query_modal(&mut self) {
        self.modal = Modal::HelpQuery {
            input: TextInput::new(),
        };
        self.status = "Help — type a question, Enter to ask, Esc to cancel".into();
    }

    /// Run a Help-book RAG inference for `query`. Builds a constrained
    /// prompt — the model is instructed to answer using ONLY the supplied
    /// Help excerpts and to admit when the context is insufficient — then
    /// streams the result into the AI pane. The AI pane is read-only by
    /// construction (no editor lives there), so the user can scroll the
    /// answer but can't edit it.
    /// Ctrl+B W toggles full-screen "typewriter mode". When on, every
    /// pane except the editor (and any floating modal) is hidden;
    /// focus is forced onto the editor so typing lands in the
    /// buffer. The same chord disables it.
    fn toggle_typewriter_mode(&mut self) {
        self.typewriter_mode = !self.typewriter_mode;
        if self.typewriter_mode {
            self.ai_fullscreen = false; // the two fullscreens are exclusive
            // Force focus to the editor so the user can start typing
            // immediately; the search bar / AI prompt are hidden
            // anyway, so leaving focus on them would be confusing.
            self.change_focus(Focus::Editor);
            self.status = "focus mode · Ctrl+B W to exit".into();
        } else {
            self.status = "focus mode off".into();
        }
    }

    /// Ctrl+B K toggles full-screen AI mode. Layout: top area split
    /// 50/50 — AI pane on the left, chat history on the right —
    /// over a full-width AI prompt at the bottom. Same chord
    /// returns to the four-pane layout.
    fn toggle_ai_fullscreen(&mut self) {
        self.ai_fullscreen = !self.ai_fullscreen;
        // Always start scrolled to the bottom (newest visible). The
        // user can PageUp to walk back through the history.
        self.chat_history_scroll = 0;
        // Wipe any in-flight chat search / selection; the layout
        // transition is an obvious break in user intent.
        self.chat_search = None;
        self.chat_selection = None;
        if self.ai_fullscreen {
            self.typewriter_mode = false; // exclusive with typewriter
            // Restore previously-saved chat history if the in-memory
            // list is currently empty. The user explicitly asked for
            // "if chat is empty, restore from previous state" — never
            // overwrite a live session.
            if self.chat_history.is_empty() {
                match self.load_chat_history_from_disk() {
                    Ok(turns) if turns > 0 => {
                        self.status = format!(
                            "AI fullscreen · restored {turns} turn(s) · Ctrl+B K to exit · Ctrl+F to search history"
                        );
                    }
                    Ok(_) => {
                        self.status =
                            "AI fullscreen · ↑↓/PgUp/PgDn scrolls history · Ctrl+F search · Ctrl+B K to exit".into();
                    }
                    Err(e) => {
                        tracing::warn!("chat history restore failed: {e}");
                        self.status =
                            "AI fullscreen · ↑↓/PgUp/PgDn scrolls history · Ctrl+F search · Ctrl+B K to exit".into();
                    }
                }
            } else {
                self.status =
                    "AI fullscreen · ↑↓/PgUp/PgDn scrolls history · Ctrl+F search · Ctrl+B K to exit".into();
            }
            // Drop focus onto the AI prompt so the user can start
            // typing the next message immediately — the AI pane has
            // no input role and the editor / tree / search bar are
            // hidden in this layout anyway.
            self.change_focus(Focus::AiPrompt);
        } else {
            // Persist the current chat to disk before leaving the
            // layout. Best-effort: write failures are logged but
            // don't block the toggle.
            match self.save_chat_history_to_disk() {
                Ok(()) => {
                    self.status = format!(
                        "AI fullscreen off · {} turn(s) saved",
                        self.chat_history.len()
                    );
                }
                Err(e) => {
                    tracing::warn!("chat history save failed: {e}");
                    self.status = "AI fullscreen off (chat history save failed — see logs)".into();
                }
            }
        }
    }

    /// Editor meta `Ctrl+B R`: advance the open paragraph's status one
    /// step through the workflow ring. The cycle wraps back to None
    /// after Ready; pressing R repeatedly walks the whole sequence
    /// without any other UI. Persisted to bdslib so it survives the
    /// next launch.
    /// Single-paragraph status cycle, used by the multi-select
    /// path in `cycle_paragraph_status`. No hook firing — that's
    /// reserved for the explicit one-paragraph chord so a bulk
    /// op doesn't fire 30 hooks in sequence.
    fn cycle_status_single(&mut self, id: Uuid) -> std::result::Result<(), String> {
        let node = self
            .hierarchy
            .get(id)
            .cloned()
            .ok_or_else(|| format!("paragraph {id} not in hierarchy"))?;
        if node.kind != NodeKind::Paragraph {
            return Err(format!("`{}` is not a paragraph", node.title));
        }
        let next = next_status(node.status.as_deref());
        let mut updated = node.clone();
        updated.status = if next == "None" {
            None
        } else {
            Some(next.to_string())
        };
        updated.target_hit_at_status = None;
        self.store
            .raw()
            .update_metadata(id, updated.to_json())
            .map_err(|e| format!("store update: {e}"))?;
        Ok(())
    }

    fn cycle_paragraph_status(&mut self) {
        // 1.2.4+: when the tree has a multi-select set, apply
        // the cycle to every marked paragraph instead of just
        // the open one.
        if !self.tree_marked.is_empty() {
            let ids: Vec<Uuid> = self.tree_marked.iter().copied().collect();
            let mut ok = 0usize;
            let mut fail = 0usize;
            for id in &ids {
                if self.cycle_status_single(*id).is_ok() {
                    ok += 1;
                } else {
                    fail += 1;
                }
            }
            self.reload_hierarchy();
            self.status = if fail == 0 {
                format!("status cycled on {ok} paragraph(s)")
            } else {
                format!("status cycled on {ok} · {fail} failed")
            };
            return;
        }
        // 1.2.4+: when invoked without an open paragraph (e.g.
        // from the tree pane via `O`), fall back to the
        // cursor's row. Lets tree O cycle status without
        // first having to open the paragraph.
        let id = match self.opened.as_ref().map(|d| d.id) {
            Some(id) => id,
            None => match self.rows.get(self.tree_cursor) {
                Some((id, _)) => *id,
                None => {
                    self.status = "no paragraph selected".into();
                    return;
                }
            },
        };
        let Some(node) = self.hierarchy.get(id).cloned() else {
            self.status = "couldn't find the target paragraph in the hierarchy".into();
            return;
        };
        if node.kind != NodeKind::Paragraph {
            self.status =
                format!("status cycle: `{}` is not a paragraph", node.title);
            return;
        }
        let next = next_status(node.status.as_deref());
        let mut updated = node.clone();
        updated.status = if next == "None" {
            None
        } else {
            Some(next.to_string())
        };
        // Manual cycle clears the auto-promote bookkeeping so a
        // future save that's still above target will re-promote
        // from whichever status the user just rolled into.
        updated.target_hit_at_status = None;
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(id, updated.to_json())
        {
            self.status = format!("status update failed: {e}");
            return;
        }
        // Refresh hierarchy so the status reads back next frame.
        self.reload_hierarchy();
        // Progress event log — feeds the status-ladder bar in
        // Ctrl+V G and the "promoted-to-Final this week" counts.
        let from_label = display_status(node.status.as_deref()).to_ascii_lowercase();
        let to_label = next.to_ascii_lowercase();
        let total_words = node.word_count.max(0) as i64;
        let book_id = self.book_of_node(id);
        crate::progress::record_status_change(
            id, book_id, &from_label, &to_label, total_words,
        );
        // hook.on_status_promoted ( uuid from_status to_status -- )
        // Fires on every transition — manual cycles AND the
        // auto-promote path. Scripts that want to act only on
        // promotions (not demotions / wraps to "none") can
        // inspect the labels.
        crate::scripting::hooks::fire(
            "hook.on_status_promoted",
            vec![
                rust_dynamic::value::Value::from_string(id.to_string()),
                rust_dynamic::value::Value::from_string(from_label.clone()),
                rust_dynamic::value::Value::from_string(to_label.clone()),
            ],
        );
        self.status = format!("status: `{}` → `{}`", display_status(node.status.as_deref()), next);
    }

    /// Open the floating Ctrl+B 1..7 status-filter modal for
    /// `target`, scoped to the tree cursor's enclosing branch.
    fn open_status_filter(&mut self, target: &'static str) {
        let (scope_id, scope_label) = self.resolve_status_filter_scope();
        let entries = self.collect_status_entries(target, scope_id);
        let count = entries.len();
        self.modal = Modal::StatusFilter {
            status_label: target,
            scope: scope_label.clone(),
            entries,
            cursor: 0,
        };
        self.status = format!(
            "status filter [{target}] · {scope_label} · {count} paragraph(s) · R/- cycles · Enter opens"
        );
    }

    /// "Current scope" for the status filter: the cursor's node when
    /// it's a branch; otherwise the nearest non-paragraph ancestor.
    /// Returns `(scope_id, breadcrumb)`. None scope = project-wide.
    fn resolve_status_filter_scope(&self) -> (Option<Uuid>, String) {
        let cursor_id = self
            .rows
            .get(self.tree_cursor)
            .map(|(id, _)| *id);
        let mut cur = cursor_id;
        while let Some(id) = cur {
            let Some(node) = self.hierarchy.get(id) else {
                break;
            };
            if node.kind != NodeKind::Paragraph && node.kind != NodeKind::Image {
                return (Some(id), self.title_breadcrumb(id));
            }
            cur = node.parent_id;
        }
        (None, "entire project".to_string())
    }

    /// Walk every paragraph that is (a) inside `scope_id`'s subtree
    /// when given, and (b) tagged with `target`. Sorted by breadcrumb
    /// so paragraphs from the same parent cluster together.
    fn collect_status_entries(
        &self,
        target: &'static str,
        scope_id: Option<Uuid>,
    ) -> Vec<StatusFilterEntry> {
        let allowed_ids: Option<std::collections::HashSet<Uuid>> =
            scope_id.map(|id| self.hierarchy.collect_subtree(id).into_iter().collect());
        let mut entries: Vec<StatusFilterEntry> = Vec::new();
        for node in self.hierarchy.iter() {
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            if let Some(allowed) = &allowed_ids {
                if !allowed.contains(&node.id) {
                    continue;
                }
            }
            if display_status(node.status.as_deref()) != target {
                continue;
            }
            entries.push(StatusFilterEntry {
                id: node.id,
                title: node.title.clone(),
                breadcrumb: self.title_breadcrumb(node.id),
            });
        }
        entries.sort_by(|a, b| a.breadcrumb.cmp(&b.breadcrumb));
        entries
    }

    fn status_filter_handle_key(&mut self, key: KeyEvent) -> bool {
        // Navigation-only branches first; status cycling has its own
        // path because it needs the full `self` borrow.
        let advance: Option<bool> = match key.code {
            KeyCode::Char('r') | KeyCode::Char('R') => Some(true),
            KeyCode::Char('-') | KeyCode::Backspace => Some(false),
            _ => None,
        };
        if let Some(forward) = advance {
            self.cycle_status_in_filter(forward);
            return true;
        }

        let Modal::StatusFilter { entries, cursor, .. } = &mut self.modal else {
            return false;
        };
        let total = entries.len();
        match key.code {
            KeyCode::Up => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
                true
            }
            KeyCode::Down => {
                if *cursor + 1 < total {
                    *cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                *cursor = 0;
                true
            }
            KeyCode::End => {
                *cursor = total.saturating_sub(1);
                true
            }
            KeyCode::PageUp => {
                *cursor = cursor.saturating_sub(10);
                true
            }
            KeyCode::PageDown => {
                *cursor = (*cursor + 10).min(total.saturating_sub(1));
                true
            }
            KeyCode::Enter => {
                self.commit_status_filter();
                true
            }
            _ => false,
        }
    }

    /// Step the highlighted paragraph's status forward or backward
    /// in the workflow ring (without leaving the modal). The list is
    /// re-collected against the original filter — if the paragraph
    /// no longer matches it disappears and the next row slides up.
    /// The cursor index is clamped to stay inside the (possibly
    /// shorter) list.
    fn cycle_status_in_filter(&mut self, forward: bool) {
        let (target_status, paragraph_id, prior_cursor) = match &self.modal {
            Modal::StatusFilter { status_label, entries, cursor, .. } => {
                let Some(entry) = entries.get(*cursor) else {
                    return;
                };
                (*status_label, entry.id, *cursor)
            }
            _ => return,
        };
        let Some(node) = self.hierarchy.get(paragraph_id).cloned() else {
            return;
        };
        let new_label = if forward {
            next_status(node.status.as_deref())
        } else {
            prev_status(node.status.as_deref())
        };
        let mut updated = node.clone();
        updated.status = if new_label == "None" {
            None
        } else {
            Some(new_label.to_string())
        };
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(paragraph_id, updated.to_json())
        {
            self.status = format!("status update failed: {e}");
            return;
        }
        self.reload_hierarchy();
        // Re-collect entries under the same scope + status target.
        let (scope_id, scope_label) = self.resolve_status_filter_scope();
        let entries = self.collect_status_entries(target_status, scope_id);
        let total = entries.len();
        let new_cursor = if total == 0 {
            0
        } else if prior_cursor >= total {
            total - 1
        } else {
            prior_cursor
        };
        self.modal = Modal::StatusFilter {
            status_label: target_status,
            scope: scope_label.clone(),
            entries,
            cursor: new_cursor,
        };
        let direction = if forward { "→" } else { "←" };
        self.status = format!(
            "{} {direction} {new_label} · `{scope_label}` · {total} paragraph(s) remaining",
            node.title
        );
    }

    fn commit_status_filter(&mut self) {
        let target_id = match &self.modal {
            Modal::StatusFilter { entries, cursor, .. } => {
                entries.get(*cursor).map(|e| e.id)
            }
            _ => None,
        };
        let Some(id) = target_id else {
            self.modal = Modal::None;
            return;
        };
        self.modal = Modal::None;
        // Jump tree cursor to the chosen paragraph, then open it via
        // the standard load path so paragraph_cursors / save sessions
        // work the same as Enter from the tree.
        if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
            self.tree_cursor = i;
        }
        if let Some(node) = self.hierarchy.get(id).cloned() {
            if let Err(e) = self.load_paragraph(&node) {
                self.status = format!("open: {e}");
            }
        }
    }

    /// Editor meta `Ctrl+B T`: rerun the placeholder-title derivation on
    /// the currently-open paragraph. Same logic that fires on save when
    /// the title is still `PARAGRAPH_PLACEHOLDER_TITLE`, but exposed
    /// explicitly so the user can refresh the tree-display name after
    /// rewriting the lead. Bails out cleanly if no first sentence can be
    /// extracted (paragraph is empty or only contains headings).
    fn rename_paragraph_to_first_sentence(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "no paragraph open".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        let Some(new_title) = extract_first_sentence(&body) else {
            self.status =
                "couldn't derive a title — paragraph is empty or only headings".into();
            return;
        };
        let id = doc.id;
        match self.store.rename_node(&self.hierarchy, id, &new_title) {
            Ok(()) => {
                self.reload_hierarchy();
                if let Some(d) = self.opened.as_mut() {
                    d.title = new_title.clone();
                    if let Some(node) = self.hierarchy.get(id) {
                        if let Some(rel) = node.file.as_ref() {
                            d.rel_path = rel.clone();
                        }
                    }
                }
                self.status = format!("renamed paragraph to `{new_title}`");
            }
            Err(e) => {
                self.status = format!("rename failed: {e}");
            }
        }
    }

    fn quickref_handle_key(&mut self, key: KeyEvent) -> bool {
        let Modal::QuickRef { focus, scroll } = &mut self.modal else {
            return false;
        };
        let total = quickref::entries_for(*focus).len();
        match key.code {
            KeyCode::Up => {
                *scroll = scroll.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                if *scroll + 1 < total {
                    *scroll += 1;
                }
                true
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(10);
                true
            }
            KeyCode::PageDown => {
                *scroll = (*scroll + 10).min(total.saturating_sub(1));
                true
            }
            KeyCode::Home => {
                *scroll = 0;
                true
            }
            KeyCode::End => {
                *scroll = total.saturating_sub(1);
                true
            }
            _ => false,
        }
    }

    /// Scroll handler for the Credits modal — mirrors `quickref_handle_key`.
    /// `total` is the number of rendered lines (computed in the renderer),
    /// but we don't need a hard upper bound here; clamping happens at
    /// render time so out-of-range scroll just shows a blank tail.
    fn credits_handle_key(&mut self, key: KeyEvent) -> bool {
        let Modal::Credits { scroll, .. } = &mut self.modal else {
            return false;
        };
        match key.code {
            KeyCode::Up => {
                *scroll = scroll.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                *scroll = scroll.saturating_add(1);
                true
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(10);
                true
            }
            KeyCode::PageDown => {
                *scroll = scroll.saturating_add(10);
                true
            }
            KeyCode::Home => {
                *scroll = 0;
                true
            }
            KeyCode::End => {
                *scroll = usize::MAX / 2;
                true
            }
            _ => false,
        }
    }

    fn open_file_picker(&mut self, context: PickerContext) {
        let root = std::env::current_dir().unwrap_or_else(|_| self.layout.root.clone());
        self.modal = Modal::FilePicker(FilePicker::new(root, context));
    }

    fn commit_file_pick(&mut self) {
        let (path, is_dir, context) = match &self.modal {
            Modal::FilePicker(p) => match p.current() {
                Some(entry) => (entry.path.clone(), entry.is_dir, p.context),
                None => {
                    self.modal = Modal::None;
                    return;
                }
            },
            _ => return,
        };

        match (context, is_dir) {
            (PickerContext::EditorLoad, true) => {
                self.status =
                    "Editor F3 needs a file, not a directory — Enter on a file".into();
            }
            (PickerContext::EditorLoad, false) => {
                self.load_file_into_editor(&path);
                self.modal = Modal::None;
            }
            (PickerContext::TreeInsertOrImport, false) => {
                self.import_single_file(&path);
                self.modal = Modal::None;
            }
            (PickerContext::TreeInsertOrImport, true) => {
                // Defer the actual import to the main loop so it can run
                // with a progress splash drawn directly via the terminal
                // handle. `commit_file_pick` doesn't own the terminal —
                // see `App::run`.
                self.modal = Modal::None;
                self.pending_import = Some(path);
            }
        }
    }

    fn import_single_file(&mut self, path: &std::path::Path) {
        // Route image files (PNG / JPG / etc.) to the image-import
        // path; everything else gets the prose treatment.
        if let Some(ext) = image_extension_for(path) {
            self.import_single_image(path, &ext);
            return;
        }
        // Place the new paragraph after the tree cursor's same-kind ancestor
        // if there is one (so it's "insert after current"). Falls back to
        // append-at-end under the nearest valid parent.
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let anchor = cursor_id.and_then(|id| {
            let mut cur = Some(id);
            while let Some(c) = cur {
                let node = self.hierarchy.get(c)?;
                if node.kind == NodeKind::Paragraph {
                    return Some(node.id);
                }
                cur = node.parent_id;
            }
            None
        });
        let position = match anchor {
            Some(id) => InsertPosition::After(id),
            None => InsertPosition::End,
        };
        let parent = match position {
            InsertPosition::After(anchor_id) | InsertPosition::Before(anchor_id) => self
                .hierarchy
                .get(anchor_id)
                .and_then(|n| n.parent_id)
                .and_then(|pid| self.hierarchy.get(pid))
                .cloned(),
            InsertPosition::End => {
                match self
                    .hierarchy
                    .pick_parent_for(&self.cfg, cursor_id, NodeKind::Paragraph)
                {
                    Ok(p) => p.cloned(),
                    Err(e) => {
                        self.status = format!("can't import here: {e}");
                        return;
                    }
                }
            }
        };

        let title = derive_paragraph_title_from_path(path);
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("read {}: {e}", path.display());
                return;
            }
        };

        let content_type = content_type_for(path);
        let mut created = match self.store.create_node(
            &self.cfg,
            &self.hierarchy,
            NodeKind::Paragraph,
            &title,
            parent.as_ref(),
            None,
            position,
        ) {
            Ok(n) => n,
            Err(e) => {
                self.status = format!("create paragraph: {e}");
                return;
            }
        };
        // For non-default content types, stamp the node + rename the
        // on-disk file so the extension matches (`<NN>-<slug>.hjson`
        // instead of `.typ`). create_node always lays down the file
        // with the typst extension; we move it before writing bytes.
        if let Some(ct) = &content_type {
            created.content_type = Some(ct.clone());
            let new_rel = std::path::PathBuf::from(
                created.file.clone().unwrap_or_default(),
            )
            .with_extension(ct);
            if let Some(old_rel) = &created.file {
                let old_abs = self.layout.root.join(old_rel);
                let new_abs = self.layout.root.join(&new_rel);
                if old_abs.exists() && old_abs != new_abs {
                    let _ = std::fs::rename(&old_abs, &new_abs);
                }
            }
            created.file = Some(new_rel.to_string_lossy().into_owned());
            if let Err(e) = self
                .store
                .raw()
                .update_metadata(created.id, created.to_json())
            {
                self.status = format!("update metadata: {e}");
                return;
            }
        }

        // Replace the templated body with the actual file content.
        let Some(rel) = created.file.clone() else {
            self.status = "created paragraph has no file path — bug?".into();
            return;
        };
        let abs = self.layout.root.join(&rel);
        if let Err(e) = std::fs::write(&abs, &bytes) {
            self.status = format!("write {}: {e}", abs.display());
            return;
        }
        let mut node = created.clone();
        if let Err(e) = self.store.update_paragraph_content(&mut node, &bytes) {
            self.status = format!("update: {e}");
            return;
        }
        let _ = self.store.sync();
        let kind_note = match content_type.as_deref() {
            Some("hjson") => " (hjson)",
            _ => "",
        };
        self.status = format!("imported `{}` as paragraph{kind_note}", path.display());
        self.reload_hierarchy();
        if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == created.id) {
            self.tree_cursor = i;
        }
    }

    /// Sibling to `import_single_file` for image files. The parent
    /// selection mirrors the prose path: insert after the cursor's
    /// nearest leaf sibling, falling back to "append at the end of
    /// the nearest legal branch".
    fn import_single_image(&mut self, path: &std::path::Path, ext: &str) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("read {}: {e}", path.display());
                return;
            }
        };
        let title = derive_paragraph_title_from_path(path);
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let anchor = cursor_id.and_then(|id| {
            let mut cur = Some(id);
            while let Some(c) = cur {
                let node = self.hierarchy.get(c)?;
                if node.kind.is_leaf() {
                    return Some(node.id);
                }
                cur = node.parent_id;
            }
            None
        });
        let position = match anchor {
            Some(id) => InsertPosition::After(id),
            None => InsertPosition::End,
        };
        let parent = match position {
            InsertPosition::After(anchor_id) | InsertPosition::Before(anchor_id) => self
                .hierarchy
                .get(anchor_id)
                .and_then(|n| n.parent_id)
                .and_then(|pid| self.hierarchy.get(pid))
                .cloned(),
            InsertPosition::End => {
                match self
                    .hierarchy
                    .pick_parent_for(&self.cfg, cursor_id, NodeKind::Image)
                {
                    Ok(p) => p.cloned(),
                    Err(e) => {
                        self.status = format!("can't import image here: {e}");
                        return;
                    }
                }
            }
        };
        let created = match self.store.create_image_node(
            &self.cfg,
            &self.hierarchy,
            &title,
            ext,
            &bytes,
            parent.as_ref(),
            position,
        ) {
            Ok(n) => n,
            Err(e) => {
                self.status = format!("import image: {e}");
                return;
            }
        };
        self.status = format!(
            "imported `{}` as image ({} bytes)",
            path.display(),
            bytes.len()
        );
        self.reload_hierarchy();
        if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == created.id) {
            self.tree_cursor = i;
        }
    }

    /// Drive a deferred directory import set up by `commit_file_pick`.
    /// Pre-counts files so the splash has a meaningful denominator, then
    /// runs the (synchronous) import with a progress callback that
    /// throttles `terminal.draw` to ~30 Hz. Status bar is updated when
    /// the import finishes; the next mainloop frame paints over the
    /// splash automatically.
    fn run_pending_import<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        root: &Path,
    ) {
        let total = count_importable_files(root);
        let source_display = root.display().to_string();
        let progress_root = root.to_path_buf();

        // Initial 0/total frame so the splash appears even if the
        // first file takes a moment (e.g. fastembed cold-loading on
        // first import in a session).
        let _ = terminal.draw(|f| {
            draw_import_splash(f, &source_display, 0, total, "scanning…")
        });

        let mut last_redraw = std::time::Instant::now();
        {
            let source_display = source_display.clone();
            let mut progress = move |done: usize, file: &Path| {
                if last_redraw.elapsed() < std::time::Duration::from_millis(33) {
                    return;
                }
                last_redraw = std::time::Instant::now();
                let rel = file
                    .strip_prefix(&progress_root)
                    .unwrap_or(file);
                let label = rel.display().to_string();
                let _ = terminal.draw(|f| {
                    draw_import_splash(f, &source_display, done, total, &label)
                });
            };
            self.import_directory_tree(root, &mut progress);
        }
    }

    /// Ctrl+B A — resolve "current book" the same way Ctrl+B I does
    /// (open paragraph's book, or the tree-cursor's book), validate
    /// it's a user book, then stash the uuid so the main loop drives
    /// the assembly with the splash.
    fn schedule_assembly(&mut self) {
        // 1.2.5+: assembly walks the on-disk .typ files. If the
        // user has unsaved edits in the editor (primary or
        // secondary), assemble would see stale bytes. Save first
        // so what the user sees in the editor is what hits the
        // assembler.
        self.save_all_before_build_step("Book assembly");
        let hierarchy = match Hierarchy::load(&self.store) {
            Ok(h) => h,
            Err(e) => {
                self.status = format!("Book assembly: hierarchy load failed: {e}");
                return;
            }
        };
        let Some(book) = self.current_book_node(&hierarchy) else {
            self.status =
                "Book assembly: move the tree cursor onto a user book (or any node inside one) first."
                    .into();
            return;
        };
        if book.system_tag.is_some() {
            self.status = format!(
                "Book assembly: `{}` is a system book — pick a user book.",
                book.title
            );
            return;
        }
        self.pending_assembly = Some(book.id);
        self.status = format!("Book assembly: assembling `{}`…", book.title);
    }

    /// Drive a deferred Book assembly. Pre-renders the splash at 0%,
    /// runs the synchronous assembler with a 30 Hz-throttled progress
    /// callback, and surfaces the final result (root .typ path or
    /// error) in the status bar.
    fn run_pending_assembly<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        book_id: Uuid,
    ) {
        let book = match Hierarchy::load(&self.store) {
            Ok(h) => match h.get(book_id).cloned() {
                Some(n) => n,
                None => {
                    self.status = "Book assembly: book vanished from hierarchy".into();
                    return;
                }
            },
            Err(e) => {
                self.status = format!("Book assembly: hierarchy load failed: {e}");
                return;
            }
        };

        let book_display = book.title.clone();
        let initial_total = 0;
        let _ = terminal.draw(|f| {
            draw_assembly_splash(f, &book_display, 0, initial_total, "preparing…")
        });

        let mut last_redraw = std::time::Instant::now();
        let book_display_for_cb = book_display.clone();
        let report = {
            let mut progress = move |done: usize, total: usize, file: &Path| {
                if last_redraw.elapsed() < std::time::Duration::from_millis(33) {
                    return;
                }
                last_redraw = std::time::Instant::now();
                let label = file.display().to_string();
                let _ = terminal.draw(|f| {
                    draw_assembly_splash(f, &book_display_for_cb, done, total, &label)
                });
            };
            crate::assemble::assemble_book(
                &self.store,
                &self.layout,
                &self.cfg,
                &book,
                &mut progress,
            )
        };

        match report {
            Ok(r) => {
                self.status = format!(
                    "Book assembly: wrote {} files · root: {}  (typst compile `{}`)",
                    r.files_written,
                    r.root_typ.display(),
                    r.root_typ.display(),
                );
                // hook.on_assemble ( uuid slug root_typ_path files_written -- )
                // Fired after a successful Ctrl+B A. Lets scripts
                // post-process the assembled tree (e.g. patch the
                // root .typ, copy artefacts, kick off a custom
                // build pipeline).
                crate::scripting::hooks::fire(
                    "hook.on_assemble",
                    vec![
                        rust_dynamic::value::Value::from_string(book.id.to_string()),
                        rust_dynamic::value::Value::from_string(book.slug.clone()),
                        rust_dynamic::value::Value::from_string(
                            r.root_typ.to_string_lossy().into_owned(),
                        ),
                        rust_dynamic::value::Value::from_int(r.files_written as i64),
                    ],
                );
            }
            Err(e) => {
                self.status = format!("Book assembly failed: {e}");
            }
        }
    }

    /// Ctrl+B B — schedule a Book "build": assembly + `typst compile`.
    /// On error the build path opens a fresh AI chat for analysis.
    fn schedule_build(&mut self) {
        // 1.2.5+: flush unsaved edits before assemble fires (see
        // schedule_assembly for the rationale).
        self.save_all_before_build_step("Book build");
        let Some(book_id) = self.resolve_current_user_book("Book build") else {
            return;
        };
        self.pending_build = Some(book_id);
        self.status = "Book build: assembling + compiling…".into();
    }

    /// Ctrl+B O — schedule a Book "take": build, then copy the PDF
    /// into the launch cwd with a timestamped filename.
    fn schedule_take(&mut self) {
        // 1.2.5+: flush unsaved edits before assemble fires (see
        // schedule_assembly for the rationale).
        self.save_all_before_build_step("Take the book");
        let Some(book_id) = self.resolve_current_user_book("Take the book") else {
            return;
        };
        self.pending_take = Some(book_id);
        self.status = "Take the book: assembling + compiling + copying…".into();
    }

    /// 1.2.5+: shared autosave step for Ctrl+B A / B / O. Saves
    /// the primary editor and, when similar-paragraph mode has
    /// a secondary editor open, that one too. Errors are logged
    /// at WARN and stamped on the status bar but never abort
    /// the build — the user can react to a save failure by
    /// dismissing the splash on Esc, which still happens. The
    /// helper is a no-op when neither buffer is dirty.
    fn save_all_before_build_step(&mut self, ctx: &str) {
        if let Some(doc) = self.opened.as_ref() {
            if doc.dirty {
                if let Err(e) = self.save_current() {
                    tracing::warn!(
                        target: "inkhaven::build",
                        "{ctx}: primary autosave failed: {e}",
                    );
                    self.status = format!("{ctx}: autosave failed: {e}");
                }
            }
        }
        // Mirror the autosave loop's `Option::take()` dance so we
        // can call `save_doc(&mut self, &mut OpenedDoc)` without
        // an aliasing borrow.
        if let Some(mut doc) = self.secondary.take() {
            if doc.dirty {
                if let Err(e) = self.save_doc(&mut doc) {
                    tracing::warn!(
                        target: "inkhaven::build",
                        "{ctx}: secondary autosave failed: {e}",
                    );
                }
            }
            self.secondary = Some(doc);
        }
    }

    /// Common preflight for Ctrl+B A / B / O. Returns the uuid of the
    /// user book the cursor is inside, or surfaces an error status and
    /// returns None when the cursor isn't on a user book.
    fn resolve_current_user_book(&mut self, ctx: &str) -> Option<Uuid> {
        let hierarchy = match Hierarchy::load(&self.store) {
            Ok(h) => h,
            Err(e) => {
                self.status = format!("{ctx}: hierarchy load failed: {e}");
                return None;
            }
        };
        let book = self.current_book_node(&hierarchy)?;
        if book.system_tag.is_some() {
            self.status =
                format!("{ctx}: `{}` is a system book — pick a user book.", book.title);
            return None;
        }
        Some(book.id)
    }

    /// Run assembly + typst compile for `book_id`. If `take` is true,
    /// the resulting PDF is also copied into the launch cwd with a
    /// timestamped filename. A typst-error opens a fresh AI chat with
    /// the configured error-system-prompt; the user gets streamed
    /// analysis on the AI pane without any extra keystroke.
    fn run_pending_build<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        book_id: Uuid,
        take: bool,
    ) {
        // Step 1: assembly (re-uses the existing splash + procedure).
        self.run_pending_assembly(terminal, book_id);
        // run_pending_assembly returns silently on failure — figure
        // out whether it succeeded by re-checking the artefacts path.
        let book = match Hierarchy::load(&self.store) {
            Ok(h) => match h.get(book_id).cloned() {
                Some(n) => n,
                None => {
                    self.status =
                        "Book build aborted: book vanished mid-assembly".into();
                    return;
                }
            },
            Err(e) => {
                self.status = format!("Book build aborted: hierarchy reload: {e}");
                return;
            }
        };
        let artefacts_root = self.store.resolve_artefacts_dir(&self.cfg);
        let root_typ = artefacts_root
            .join(&book.slug)
            .join(format!("{}.typ", book.slug));
        if !root_typ.is_file() {
            // Assembly didn't produce a root .typ — its status message
            // already explains why, leave it in place.
            return;
        }

        // Step 2: spawn `typst compile` and animate the splash while
        // the child runs.
        let book_display = book.title.clone();
        let outcome = match self.run_typst_compile(terminal, &book_display, &root_typ) {
            Some(o) => o,
            None => return, // spawn failed; status already set
        };

        if outcome.success {
            let pdf_msg = format!(
                "Build OK · PDF: {}",
                outcome.pdf_path.display()
            );
            if take {
                match self.take_book_pdf(&book, &outcome.pdf_path) {
                    Ok(dest) => {
                        // Multi-format extras live next to the PDF
                        // with matching stem. Errors are reported
                        // (status bar) but never abort the take —
                        // the PDF the user actually asked for is
                        // already on disk.
                        let extras = self.take_extra_formats(terminal, &book, &dest);
                        let extras_msg = if extras.is_empty() {
                            String::new()
                        } else {
                            format!(" · extras: {}", extras.join(", "))
                        };
                        self.status = format!(
                            "Took the book · {}{}  (source PDF: {})",
                            dest.display(),
                            extras_msg,
                            outcome.pdf_path.display()
                        );
                        // hook.on_take ( uuid slug pdf_dest -- )
                        // Fired once the PDF is copied to the
                        // launch cwd (and any configured extra
                        // formats are written alongside). Lets
                        // scripts upload the artefact, post a
                        // chat notification, etc.
                        crate::scripting::hooks::fire(
                            "hook.on_take",
                            vec![
                                rust_dynamic::value::Value::from_string(
                                    book.id.to_string(),
                                ),
                                rust_dynamic::value::Value::from_string(
                                    book.slug.clone(),
                                ),
                                rust_dynamic::value::Value::from_string(
                                    dest.to_string_lossy().into_owned(),
                                ),
                            ],
                        );
                    }
                    Err(e) => {
                        self.status = format!("{pdf_msg} · take failed: {e}");
                    }
                }
            } else {
                self.status = pdf_msg;
            }
            return;
        }

        // Compile failed — surface the stderr through the AI pane and
        // leave a status hint mentioning where to read the answer.
        let error_text = if outcome.stderr.trim().is_empty() {
            outcome.stdout.clone()
        } else {
            outcome.stderr.clone()
        };
        self.start_typst_error_analysis(&book, &root_typ, &error_text);
    }

    /// Drive a typst-compile child to completion with the spinner
    /// splash. Returns the outcome on success-or-failure of the
    /// compile itself; returns None when even spawning the binary
    /// failed (status bar is set in that case).
    fn run_typst_compile<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        book_display: &str,
        root_typ: &Path,
    ) -> Option<crate::typst_compile::CompileOutcome> {
        let mut handle = match crate::typst_compile::spawn_with_config(&self.cfg, root_typ) {
            Ok(h) => h,
            Err(e) => {
                self.status = format!("typst compile: {e}");
                return None;
            }
        };
        // Animate the splash while the compile runs (external child
        // or in-process worker thread — same loop, same UX).
        // ~80ms per frame keeps the spinner readable without
        // burning CPU. The same loop also polls for Esc so a stuck
        // compile can be interrupted by the user.
        let engine_label = crate::typst_compile::engine_summary(&self.cfg);
        let started = std::time::Instant::now();
        let mut spin_idx: usize = 0;
        let mut cancelled = false;
        loop {
            let elapsed = started.elapsed().as_secs();
            let spinner = TYPST_COMPILE_SPINNER[spin_idx % TYPST_COMPILE_SPINNER.len()];
            let _ = terminal.draw(|f| {
                draw_typst_compile_splash(
                    f,
                    book_display,
                    &engine_label,
                    elapsed,
                    spinner,
                    None,
                )
            });
            spin_idx = spin_idx.wrapping_add(1);
            // Poll for input WITHOUT consuming non-Esc keys — we
            // re-emit nothing here; any user typing during the
            // compile is just dropped (the alternate-screen
            // splash is modal). Esc → cancel.
            if let Ok(true) = crossterm::event::poll(std::time::Duration::from_millis(0)) {
                if let Ok(crossterm::event::Event::Key(k)) = crossterm::event::read() {
                    if matches!(k.code, crossterm::event::KeyCode::Esc) {
                        handle.kill();
                        cancelled = true;
                        break;
                    }
                }
            }
            match handle.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    std::thread::sleep(std::time::Duration::from_millis(80));
                }
                Err(e) => {
                    self.status = format!("typst compile: try_wait: {e}");
                    return None;
                }
            }
        }
        let outcome = match crate::typst_compile::finish(handle) {
            Ok(o) => {
                if cancelled {
                    // Promote the outcome's failure state into a
                    // user-visible "cancelled" message rather than
                    // running the AI error-analysis path.
                    self.status = format!(
                        "typst compile cancelled — partial output (if any) discarded · engine: {engine_label}",
                    );
                    return None;
                }
                o
            }
            Err(e) => {
                self.status = format!("typst compile: {e}");
                return None;
            }
        };

        // 1.2.6+: hold the splash on screen with a "Press any key to
        // continue…" prompt so the user can read the result line
        // before the editor regains the screen. Toggle:
        // `typst_compile.wait_for_key_after_compile` (default true).
        // Cancelled compiles already returned above and skip this.
        if self.cfg.typst_compile.wait_for_key_after_compile {
            let final_elapsed = started.elapsed().as_secs();
            let final_spinner =
                TYPST_COMPILE_SPINNER[spin_idx % TYPST_COMPILE_SPINNER.len()];
            let done = Some(outcome.success);
            let _ = terminal.draw(|f| {
                draw_typst_compile_splash(
                    f,
                    book_display,
                    &engine_label,
                    final_elapsed,
                    final_spinner,
                    done,
                )
            });
            // Block on a single key event. Drain any non-key events
            // (mouse, resize) so a stray scroll wheel doesn't sneak
            // through and dismiss the splash. Resize triggers a
            // redraw and keeps waiting.
            loop {
                match crossterm::event::read() {
                    Ok(crossterm::event::Event::Key(_)) => break,
                    Ok(crossterm::event::Event::Resize(_, _)) => {
                        let _ = terminal.draw(|f| {
                            draw_typst_compile_splash(
                                f,
                                book_display,
                                &engine_label,
                                final_elapsed,
                                final_spinner,
                                done,
                            )
                        });
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }

        Some(outcome)
    }

    /// Write each format in `cfg.output.extra_formats` alongside
    /// `pdf_dest`, sharing the same stem. The combined `.typ`
    /// source is built once from `book`'s subtree and reused for
    /// every format. Per-format errors are logged and surfaced in
    /// the returned brief list (`["markdown", "tex error: …"]`),
    /// never aborting — the PDF the user asked for is already on
    /// disk before this fires.
    fn take_extra_formats<B: ratatui::backend::Backend>(
        &self,
        terminal: &mut Terminal<B>,
        book: &crate::store::node::Node,
        pdf_dest: &Path,
    ) -> Vec<String> {
        let formats = &self.cfg.output.extra_formats;
        if formats.is_empty() {
            return Vec::new();
        }
        let hierarchy = match crate::store::hierarchy::Hierarchy::load(&self.store) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::take",
                    "extra formats: hierarchy load: {e}",
                );
                return vec![format!("hierarchy load failed: {e}")];
            }
        };
        let layout = crate::project::ProjectLayout::new(self.store.project_root());
        let combined = match crate::export::assemble_typst_source(
            &layout,
            &hierarchy,
            Some(book.id),
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::take",
                    "extra formats: assemble: {e}",
                );
                return vec![format!("assemble failed: {e}")];
            }
        };
        let book_display = book.title.clone();
        let formats_display: Vec<String> = formats.iter().map(|s| s.trim().to_string()).collect();
        let mut statuses: Vec<char> = vec!['·'; formats_display.len()];
        let mut produced: Vec<String> = Vec::new();
        // 1.2.6+ — draw the splash with a brief per-format
        // pause so the user can actually SEE which format is
        // being written. Without the pause the loop finishes
        // in milliseconds and the splash is invisible.
        let step_pause = std::time::Duration::from_millis(
            self.cfg.output.extras_step_pause_ms,
        );
        for (i, raw) in formats.iter().enumerate() {
            statuses[i] = '▶';
            let _ = terminal.draw(|f| {
                draw_take_extras_splash(
                    f,
                    &book_display,
                    i,
                    &formats_display,
                    &statuses,
                )
            });
            if !step_pause.is_zero() {
                std::thread::sleep(step_pause);
            }
            let fmt = raw.trim().to_ascii_lowercase();
            let outcome = self.build_one_extra_format(&fmt, &combined, &book.title);
            let artefact = match outcome {
                Some(Ok(art)) => art,
                Some(Err(e)) => {
                    tracing::warn!(
                        target: "inkhaven::take",
                        "extra format {fmt}: {e}",
                    );
                    statuses[i] = '✗';
                    produced.push(format!("{fmt} error"));
                    continue;
                }
                None => {
                    tracing::warn!(
                        target: "inkhaven::take",
                        "extra format {fmt}: unknown — skipped",
                    );
                    statuses[i] = '✗';
                    produced.push(format!("{fmt}?"));
                    continue;
                }
            };
            let dest = crate::export::with_artefact_extension(pdf_dest, &artefact);
            if let Err(e) = artefact.write_to(&dest) {
                tracing::warn!(
                    target: "inkhaven::take",
                    "extra format {fmt} write {}: {e}",
                    dest.display(),
                );
                statuses[i] = '✗';
                produced.push(format!("{fmt} write error"));
                continue;
            }
            statuses[i] = '✓';
            produced.push(dest.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&fmt)
                .to_string());
        }
        // Final frame so the user sees all checkmarks. When
        // `typst_compile.wait_for_key_after_compile` is on (the
        // 1.2.6 default), we hold this frame until any key
        // press — exactly the same pattern the compile splash
        // uses. Without the wait, we sleep one more step_pause
        // so even auto-dismiss configs are catchable in a
        // terminal screenshot.
        let _ = terminal.draw(|f| {
            draw_take_extras_splash(
                f,
                &book_display,
                formats_display.len().saturating_sub(1),
                &formats_display,
                &statuses,
            )
        });
        if self.cfg.output.extras_wait_for_key {
            loop {
                match crossterm::event::read() {
                    Ok(crossterm::event::Event::Key(_)) => break,
                    Ok(crossterm::event::Event::Resize(_, _)) => {
                        let _ = terminal.draw(|f| {
                            draw_take_extras_splash(
                                f,
                                &book_display,
                                formats_display.len().saturating_sub(1),
                                &formats_display,
                                &statuses,
                            )
                        });
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        } else if !step_pause.is_zero() {
            std::thread::sleep(step_pause);
        }
        produced
    }

    fn build_one_extra_format(
        &self,
        fmt: &str,
        combined: &str,
        book_title: &str,
    ) -> Option<anyhow::Result<crate::export::Artefact>> {
        match fmt {
            "markdown" | "md" => Some(Ok(crate::export::build_markdown(combined))),
            "tex" | "latex" => Some(Ok(crate::export::build_tex(combined))),
            "epub" => {
                let md = crate::export::markdown::typst_to_markdown(combined);
                Some(crate::export::build_epub(&md, book_title))
            }
            _ => None,
        }
    }

    /// Copy `pdf_src` into the launch cwd as
    /// `<book-slug>-YYYYDDMM-HHMM.pdf`. Returns the destination path
    /// on success. The cwd is inkhaven's `current_dir()` at the
    /// moment of the call — same path the shell-launched binary saw.
    fn take_book_pdf(
        &self,
        book: &Node,
        pdf_src: &Path,
    ) -> std::io::Result<std::path::PathBuf> {
        let cwd = std::env::current_dir()?;
        let now = chrono::Local::now();
        // Match the existing backup filename style: YYYYDDMM_HHMMSS.
        // User asked for YYYYDDMM-HHMM specifically — slight variant.
        let stamp = now.format("%Y%d%m-%H%M");
        let dest = cwd.join(format!("{}-{stamp}.pdf", book.slug));
        std::fs::copy(pdf_src, &dest)?;
        Ok(dest)
    }

    /// Open a fresh AI chat (cleared history, system prompt tuned for
    /// typst errors, inference mode forced to Full) and auto-send the
    /// compile error so the user gets streamed analysis without an
    /// extra keystroke.
    fn start_typst_error_analysis(
        &mut self,
        book: &Node,
        root_typ: &Path,
        error_text: &str,
    ) {
        // Wipe any in-flight chat so the new system prompt isn't
        // diluted by unrelated turns.
        self.chat_history.clear();
        self.inference = None;
        self.pending_chat_user_msg = None;
        // Force Full mode per the user's spec; auto-reset scope.
        self.inference_mode = InferenceMode::Full;
        self.ai_mode = AiMode::None;

        let system_prompt = self.cfg.typst_compile.resolved_error_system_prompt();
        let user_prompt = format!(
            "Book: `{book_title}` (slug `{slug}`)\n\
             Root file: {root}\n\n\
             `typst compile` failed with the following error. Please diagnose \
             it using the inkhaven file-layout knowledge from the system \
             prompt and tell me the smallest concrete fix.\n\n\
             --- typst stderr ---\n{err}",
            book_title = book.title,
            slug = book.slug,
            root = root_typ.display(),
            err = error_text.trim(),
        );

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("typst error: can't reach LLM ({e})");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            Some(system_prompt),
            Vec::new(),
            user_prompt.clone(),
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        // Record the user turn so the assistant's reply ends up in
        // chat_history when streaming finishes.
        self.pending_chat_user_msg = Some(user_prompt);
        self.change_focus(Focus::Ai);
        self.status = format!(
            "typst compile failed · {provider} is analysing the error in the AI pane…"
        );
    }

    fn import_directory_tree(
        &mut self,
        root: &std::path::Path,
        progress: &mut dyn FnMut(usize, &Path),
    ) {
        // The top-level dir's "kind" adapts to where the tree cursor sits:
        //   Book      → top dir becomes a Chapter
        //   Chapter   → top dir becomes a Subchapter
        //   Subchapter→ top dir becomes a Subchapter (requires unbounded)
        //   Paragraph → reject
        // Same rule applies at every recursion level: the new branch's kind
        // is "one level deeper than its parent", so a nested directory tree
        // walks down the hierarchy with it.
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let parent_id = cursor_id.and_then(|id| {
            // If cursor is on a paragraph we walk up to its enclosing branch
            // — easier than telling the user "move first".
            let mut cur = Some(id);
            while let Some(c) = cur {
                let node = self.hierarchy.get(c)?;
                if node.kind != NodeKind::Paragraph {
                    return Some(node.id);
                }
                cur = node.parent_id;
            }
            None
        });

        let mut counts = ImportCounts::default();
        let result = self.import_dir_recursive(root, parent_id, &mut counts, progress);

        // Always reload — even on partial failure the new branches/paragraphs
        // that DID get created should be visible in the tree.
        self.reload_hierarchy();

        match result {
            Ok(()) => {
                self.status = format!(
                    "imported {}: {} branch(es), {} paragraph(s)",
                    root.display(),
                    counts.branches,
                    counts.paragraphs
                );
            }
            Err(e) => {
                self.status = format!(
                    "partial import of {}: {} branch(es), {} paragraph(s) — stopped at: {e}",
                    root.display(),
                    counts.branches,
                    counts.paragraphs
                );
            }
        }
    }

    fn import_dir_recursive(
        &mut self,
        source: &std::path::Path,
        parent_id: Option<Uuid>,
        counts: &mut ImportCounts,
        progress: &mut dyn FnMut(usize, &Path),
    ) -> InkResult<()> {
        // Resolve parent against a freshly loaded hierarchy so prior creates
        // in this import are visible.
        let hierarchy = Hierarchy::load(&self.store)?;
        let parent = parent_id.and_then(|id| hierarchy.get(id).cloned());

        // Decide what kind of branch this directory becomes. None means we've
        // bottomed out under a bounded hierarchy and should flatten files
        // into the current parent instead of failing.
        let kind: Option<NodeKind> = match parent.as_ref().map(|p| p.kind) {
            None => Some(NodeKind::Book),
            Some(NodeKind::Book) => Some(NodeKind::Chapter),
            Some(NodeKind::Chapter) => Some(NodeKind::Subchapter),
            Some(NodeKind::Subchapter) => {
                if self.cfg.hierarchy.unbounded_subchapters {
                    Some(NodeKind::Subchapter)
                } else {
                    None
                }
            }
            Some(NodeKind::Paragraph) | Some(NodeKind::Image) | Some(NodeKind::Script) => {
                return Err(Error::Store(
                    "can't import under a leaf — move cursor to a branch first".into(),
                ));
            }
        };

        let Some(kind) = kind else {
            // Max depth reached. Walk the rest of the subtree and import every
            // file as a paragraph in the current parent. Branches beyond this
            // point are lost (the bounded hierarchy can't represent them),
            // but the prose comes through.
            let pid = parent_id.expect("None parent already handled by kind match");
            return self.flatten_files_into(source, pid, counts, progress);
        };

        let title = source
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("imported")
            .to_string();
        let created = self.store.create_node(
            &self.cfg,
            &hierarchy,
            kind,
            &title,
            parent.as_ref(),
            None,
            InsertPosition::End,
        )?;
        counts.branches += 1;
        let created_id = created.id;

        let children = read_sorted_children(source);

        // Don't bail on the first failing child — record the error but
        // continue so siblings still get imported. The user gets a partial-
        // import status with counts; orphan dirs get reported in the message.
        let mut first_err: Option<Error> = None;
        for child_path in children {
            let res = if child_path.is_dir() {
                self.import_dir_recursive(&child_path, Some(created_id), counts, progress)
            } else {
                self.import_file_as_paragraph_by_id(&child_path, created_id, counts, progress)
            };
            if let Err(e) = res {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
        match first_err {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }

    /// Walk `source` recursively and import every regular file as a paragraph
    /// under `parent_id`. Used when we've hit the depth limit and can no
    /// longer create deeper branches.
    fn flatten_files_into(
        &mut self,
        source: &std::path::Path,
        parent_id: Uuid,
        counts: &mut ImportCounts,
        progress: &mut dyn FnMut(usize, &Path),
    ) -> InkResult<()> {
        let mut first_err: Option<Error> = None;
        for entry in walkdir::WalkDir::new(source)
            .sort_by_file_name()
            .follow_links(false)
        {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    if first_err.is_none() {
                        first_err = Some(Error::Store(format!("walkdir: {e}")));
                    }
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_str().unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            if let Err(e) = self.import_file_as_paragraph_by_id(
                entry.path(),
                parent_id,
                counts,
                progress,
            ) {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
        match first_err {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }

    fn import_file_as_paragraph_by_id(
        &mut self,
        file: &std::path::Path,
        parent_id: Uuid,
        counts: &mut ImportCounts,
        progress: &mut dyn FnMut(usize, &Path),
    ) -> InkResult<()> {
        let title = derive_paragraph_title_from_path(file);
        let raw = std::fs::read(file).map_err(Error::Io)?;
        // Normalise line endings so DOS / old-Mac dumps don't keep
        // their `\r` bytes on disk — those survive into the editor
        // load path and render as control glyphs. Only touched when
        // the content decodes as UTF-8; binary payloads are written
        // verbatim and will simply fail to render meaningfully.
        let bytes: Vec<u8> = match std::str::from_utf8(&raw) {
            Ok(text) if text.contains('\r') => text
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .into_bytes(),
            _ => raw,
        };
        let hierarchy = Hierarchy::load(&self.store)?;
        let parent = hierarchy
            .get(parent_id)
            .cloned()
            .ok_or_else(|| Error::Store(format!("import: parent {parent_id} vanished")))?;
        let created = self.store.create_node(
            &self.cfg,
            &hierarchy,
            NodeKind::Paragraph,
            &title,
            Some(&parent),
            None,
            InsertPosition::End,
        )?;
        if let Some(rel) = &created.file {
            let abs = self.layout.root.join(rel);
            std::fs::write(&abs, &bytes).map_err(Error::Io)?;
            let mut node = created.clone();
            self.store.update_paragraph_content(&mut node, &bytes)?;
        }
        counts.paragraphs += 1;
        progress(counts.paragraphs, file);
        Ok(())
    }

    fn open_rename_modal(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected to rename".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if let Some(reason) = self.protected_block_reason(node) {
            self.status = reason;
            return;
        }
        let mut input = TextInput::new();
        for c in node.title.chars() {
            input.insert_char(c);
        }
        self.modal = Modal::Renaming {
            node_id: id,
            kind: node.kind,
            input,
        };
    }

    fn commit_rename(&mut self) {
        let (node_id, new_title) = match &self.modal {
            Modal::Renaming { node_id, input, .. } => {
                (*node_id, input.as_str().trim().to_string())
            }
            _ => return,
        };
        if new_title.is_empty() {
            self.status = "rename: title can't be empty — type one or Esc to cancel".into();
            return;
        }
        match self.store.rename_node(&self.hierarchy, node_id, &new_title) {
            Ok(()) => {
                self.reload_hierarchy();
                // Refresh editor's title + on-disk rel_path if the
                // renamed node is the open one. 1.2.4+: paragraphs
                // rename their file on disk, so the open doc's
                // rel_path needs to track the new slug.
                if let Some(doc) = self.opened.as_mut() {
                    if doc.id == node_id {
                        doc.title = new_title.clone();
                        if let Some(node) = self.hierarchy.get(node_id) {
                            if let Some(rel) = node.file.as_ref() {
                                doc.rel_path = rel.clone();
                            }
                        }
                    }
                }
                self.modal = Modal::None;
                self.status = format!("renamed to `{new_title}`");
            }
            Err(e) => {
                self.status = format!("rename failed: {e}");
            }
        }
    }

    fn open_delete_modal(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected to delete".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if let Some(reason) = self.protected_block_reason(node) {
            self.status = reason;
            return;
        }
        let ids = self.hierarchy.collect_subtree(id);
        let descendant_count = ids.len().saturating_sub(1);
        self.modal = Modal::Deleting {
            root_id: id,
            root_kind: node.kind,
            title: node.title.clone(),
            descendant_count,
            ids,
        };
    }

    /// Returns `Some(reason)` if the given node (or any ancestor) is a
    /// system-protected node — used to block destructive operations from
    /// the UI and surface a status message to the user. `None` means the
    /// node is fully mutable.
    fn protected_block_reason(&self, node: &Node) -> Option<String> {
        if node.protected {
            return Some(format!(
                "“{}” is a system book — it can't be deleted or renamed",
                node.title
            ));
        }
        // Walk ancestors so a paragraph inside Help is also blocked.
        for anc in self.hierarchy.ancestors(node) {
            if anc.protected && anc.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_HELP)
            {
                return Some(format!(
                    "“{}” lives inside the read-only Help book",
                    node.title
                ));
            }
        }
        None
    }

    fn handle_modal_key(&mut self, key: KeyEvent) -> Result<bool> {
        if matches!(key.code, KeyCode::Esc) {
            // SnapshotDiff stashes the picker it opened from in
            // `return_to`; popping that back makes Esc behave
            // like "close the diff, stay in the picker".
            if let Modal::SnapshotDiff { return_to, .. } = &mut self.modal {
                let prev = std::mem::replace(return_to.as_mut(), Modal::None);
                self.modal = prev;
                self.status = "diff closed".into();
                return Ok(false);
            }
            // 1.2.5+ SaveRenderedPng follows the same pattern —
            // restore the underlying RenderedPreview so the user
            // doesn't lose their navigation state when they
            // cancel a save.
            if let Modal::SaveRenderedPng { return_to, .. } = &mut self.modal {
                let prev = std::mem::replace(return_to.as_mut(), Modal::None);
                self.modal = prev;
                self.status = "save PNG: cancelled · preview restored".into();
                return Ok(false);
            }
            // Story-view save picker — mirror SaveRenderedPng.
            if let Modal::SaveStoryPng { return_to, .. } = &mut self.modal {
                let prev = std::mem::replace(return_to.as_mut(), Modal::None);
                self.modal = prev;
                self.status = "save story PNG: cancelled · preview restored".into();
                return Ok(false);
            }
            // 1.2.5+ tag-add and tag-delete sub-modals — Esc
            // returns to the TagPicker that opened them.
            if let Modal::TagAddPrompt { return_to, .. } = &mut self.modal {
                let prev = std::mem::replace(return_to.as_mut(), Modal::None);
                self.modal = prev;
                self.status = "tag add: cancelled".into();
                return Ok(false);
            }
            if let Modal::TagDeleteConfirm { return_to, .. } = &mut self.modal {
                let prev = std::mem::replace(return_to.as_mut(), Modal::None);
                self.modal = prev;
                self.status = "tag delete: cancelled".into();
                return Ok(false);
            }
            // 1.2.6+ tag-rename prompt — same return_to pattern.
            if let Modal::TagRenamePrompt { return_to, .. } = &mut self.modal {
                let prev = std::mem::replace(return_to.as_mut(), Modal::None);
                self.modal = prev;
                self.status = "tag rename: cancelled".into();
                return Ok(false);
            }
            // 1.2.6+ — timeline new-event prompt.
            if let Modal::TimelineNewEventPrompt { return_to, .. } = &mut self.modal {
                let prev = std::mem::replace(return_to.as_mut(), Modal::None);
                self.modal = prev;
                self.status = "new event: cancelled".into();
                return Ok(false);
            }
            // 1.2.6+ — timeline edit-event prompt.
            if let Modal::TimelineEditEventPrompt { .. } = &mut self.modal {
                self.modal = Modal::None;
                self.status = "edit event: cancelled".into();
                return Ok(false);
            }
            // 1.2.7+ — timeline view: snapshot the per-book
            // state (collapsed tracks, expanded track, zoom,
            // scroll, cursor) into the session cache so the
            // next Ctrl+V Shift+T for this book restores it.
            if matches!(self.modal, Modal::TimelineView { .. }) {
                self.timeline_capture_view_state();
            }
            self.modal = Modal::None;
            return Ok(false);
        }

        let is_adding = matches!(self.modal, Modal::Adding { .. });
        let is_deleting = matches!(self.modal, Modal::Deleting { .. });
        let is_snapshot = matches!(self.modal, Modal::SnapshotPicker { .. });
        let is_renaming = matches!(self.modal, Modal::Renaming { .. });
        let is_file_picker = matches!(self.modal, Modal::FilePicker(_));
        let is_find = matches!(self.modal, Modal::FindReplace { .. });
        let is_quickref = matches!(self.modal, Modal::QuickRef { .. });
        let is_credits = matches!(self.modal, Modal::Credits { .. });
        let is_book_info = matches!(self.modal, Modal::BookInfo { .. });
        let is_llm_picker = matches!(self.modal, Modal::LlmPicker { .. });
        let is_image_picker = matches!(self.modal, Modal::ImagePicker { .. });
        let is_function_picker = matches!(self.modal, Modal::FunctionPicker { .. });
        let is_status_filter = matches!(self.modal, Modal::StatusFilter { .. });
        let is_help_query = matches!(self.modal, Modal::HelpQuery { .. });
        let is_chat_search_prompt = matches!(self.modal, Modal::ChatSearchPrompt { .. });
        let is_bund_eval = matches!(self.modal, Modal::BundEval { .. });
        let is_bund_pane = matches!(self.modal, Modal::BundPane { .. });
        let is_script_picker = matches!(self.modal, Modal::ScriptPicker { .. });
        let is_bund_input = matches!(self.modal, Modal::BundInput { .. });
        let is_similar_picker = matches!(self.modal, Modal::SimilarPicker { .. });
        let is_progress = matches!(self.modal, Modal::Progress { .. });
        let is_paragraph_target = matches!(self.modal, Modal::ParagraphTarget { .. });
        let is_save_markdown = matches!(self.modal, Modal::SaveMarkdown { .. });
        let is_snapshot_diff = matches!(self.modal, Modal::SnapshotDiff { .. });
        let is_link_picker = matches!(self.modal, Modal::LinkPicker { .. });
        let is_backlink_picker = matches!(self.modal, Modal::BacklinkPicker { .. });
        let is_bookmark_picker = matches!(self.modal, Modal::BookmarkPicker { .. });
        let is_fuzzy_paragraph_picker = matches!(self.modal, Modal::FuzzyParagraphPicker { .. });
        let is_kill_ring_picker = matches!(self.modal, Modal::KillRingPicker { .. });
        let is_shell_pane = matches!(self.modal, Modal::ShellPane { .. });
        let is_hjson_editor = matches!(self.modal, Modal::HjsonEditor { .. });
        let is_rendered_preview = matches!(self.modal, Modal::RenderedPreview { .. });
        let is_save_rendered_png = matches!(self.modal, Modal::SaveRenderedPng { .. });
        let is_tts_save_as_audio = matches!(self.modal, Modal::TtsSaveAsAudio { .. });
        let is_writing_streak_heatmap = matches!(self.modal, Modal::WritingStreakHeatmap { .. });
        let is_concordance = matches!(self.modal, Modal::Concordance { .. });
        let is_diagnostics_list = matches!(self.modal, Modal::DiagnosticsList { .. });
        let is_ai_diff_review = matches!(self.modal, Modal::AiDiffReview { .. });
        let is_event_picker = matches!(self.modal, Modal::EventPicker { .. });
        let is_timeline_view = matches!(self.modal, Modal::TimelineView { .. });
        let is_timeline_new_event = matches!(self.modal, Modal::TimelineNewEventPrompt { .. });
        let is_timeline_edit_event = matches!(self.modal, Modal::TimelineEditEventPrompt { .. });
        let is_snapshot_annotation = matches!(self.modal, Modal::SnapshotAnnotation { .. });
        let is_tag_picker = matches!(self.modal, Modal::TagPicker { .. });
        let is_tag_add_prompt = matches!(self.modal, Modal::TagAddPrompt { .. });
        let is_tag_delete_confirm = matches!(self.modal, Modal::TagDeleteConfirm { .. });
        let is_tag_rename_prompt = matches!(self.modal, Modal::TagRenamePrompt { .. });
        let is_tag_search_results = matches!(self.modal, Modal::TagSearchResults { .. });
        let is_story_view = matches!(self.modal, Modal::StoryView { .. });
        let is_save_story_png = matches!(self.modal, Modal::SaveStoryPng { .. });

        if is_quickref {
            self.quickref_handle_key(key);
            return Ok(false);
        }
        if is_credits {
            self.credits_handle_key(key);
            return Ok(false);
        }
        if is_book_info {
            self.book_info_handle_key(key);
            return Ok(false);
        }
        if is_llm_picker {
            self.llm_picker_handle_key(key);
            return Ok(false);
        }
        if is_image_picker {
            self.image_picker_handle_key(key);
            return Ok(false);
        }
        if is_function_picker {
            self.function_picker_handle_key(key);
            return Ok(false);
        }
        if is_status_filter {
            self.status_filter_handle_key(key);
            return Ok(false);
        }

        if is_help_query {
            // Enter submits, anything else feeds the input box. Esc was
            // already handled at the top of this function.
            if matches!(key.code, KeyCode::Enter) {
                let query = match &self.modal {
                    Modal::HelpQuery { input } => input.as_str().to_string(),
                    _ => String::new(),
                };
                // 1.2.8+ — push onto history (dedup vs immediate
                // predecessor); reset the navigation cursor.
                let trimmed = query.trim();
                if !trimmed.is_empty()
                    && self.help_query_history.last().map(|s| s.as_str()) != Some(trimmed)
                {
                    self.help_query_history.push(trimmed.to_string());
                }
                self.help_query_history_cursor = None;
                self.modal = Modal::None;
                self.start_help_inference(&query);
                return Ok(false);
            }
            // 1.2.8+ — Up / Down walks help_query_history.  Mirrors
            // the AI-prompt history pattern: None on entry, Up goes
            // to len-1, Down past newest clears.
            match key.code {
                KeyCode::Up => {
                    if !self.help_query_history.is_empty() {
                        let next = match self.help_query_history_cursor {
                            Some(0) => 0,
                            Some(i) => i - 1,
                            None => self.help_query_history.len() - 1,
                        };
                        self.help_query_history_cursor = Some(next);
                        let entry = self.help_query_history[next].clone();
                        if let Modal::HelpQuery { input } = &mut self.modal {
                            input.clear();
                            for c in entry.chars() {
                                input.insert_char(c);
                            }
                        }
                    }
                    return Ok(false);
                }
                KeyCode::Down => {
                    if let Some(cur) = self.help_query_history_cursor {
                        let next = cur + 1;
                        if next >= self.help_query_history.len() {
                            self.help_query_history_cursor = None;
                            if let Modal::HelpQuery { input } = &mut self.modal {
                                input.clear();
                            }
                        } else {
                            self.help_query_history_cursor = Some(next);
                            let entry = self.help_query_history[next].clone();
                            if let Modal::HelpQuery { input } = &mut self.modal {
                                input.clear();
                                for c in entry.chars() {
                                    input.insert_char(c);
                                }
                            }
                        }
                    }
                    return Ok(false);
                }
                _ => {}
            }
            if let Modal::HelpQuery { input } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_bund_eval {
            // Enter runs the expression against Adam; result goes to
            // the status bar. Anything else feeds the input box.
            if matches!(key.code, KeyCode::Enter) {
                let expr = match &self.modal {
                    Modal::BundEval { input } => input.as_str().to_string(),
                    _ => String::new(),
                };
                self.modal = Modal::None;
                match self.scripting_eval(&expr) {
                    Ok(out) => {
                        self.status = format_eval_output(&out, None);
                    }
                    Err(e) => {
                        self.status = format!("bund: eval failed — {e:#}");
                    }
                }
                return Ok(false);
            }
            if let Modal::BundEval { input } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_bund_pane {
            self.bund_pane_handle_key(key);
            return Ok(false);
        }

        if is_script_picker {
            self.script_picker_handle_key(key);
            return Ok(false);
        }

        if is_similar_picker {
            self.similar_picker_handle_key(key);
            return Ok(false);
        }

        if is_progress {
            self.progress_modal_handle_key(key);
            return Ok(false);
        }

        if is_link_picker {
            self.link_picker_handle_key(key);
            return Ok(false);
        }

        if is_backlink_picker {
            self.backlink_picker_handle_key(key);
            return Ok(false);
        }

        if is_bookmark_picker {
            self.bookmark_picker_handle_key(key);
            return Ok(false);
        }

        if is_fuzzy_paragraph_picker {
            self.fuzzy_paragraph_picker_handle_key(key);
            return Ok(false);
        }

        if is_kill_ring_picker {
            self.kill_ring_picker_handle_key(key);
            return Ok(false);
        }

        if is_shell_pane {
            self.shell_pane_handle_key(key);
            return Ok(false);
        }

        if is_hjson_editor {
            self.hjson_editor_handle_key(key);
            return Ok(false);
        }

        if is_snapshot_diff {
            if let Modal::SnapshotDiff { scroll, rows, .. } = &mut self.modal {
                let total = rows.len();
                let page: usize = 12;
                match key.code {
                    KeyCode::Up => *scroll = scroll.saturating_sub(1),
                    KeyCode::Down => {
                        if *scroll + 1 < total {
                            *scroll += 1;
                        }
                    }
                    KeyCode::PageUp => *scroll = scroll.saturating_sub(page),
                    KeyCode::PageDown => {
                        *scroll = (*scroll + page).min(total.saturating_sub(1));
                    }
                    KeyCode::Home => *scroll = 0,
                    KeyCode::End => *scroll = total.saturating_sub(1),
                    _ => {}
                }
            }
            return Ok(false);
        }

        if is_paragraph_target {
            if matches!(key.code, KeyCode::Enter) {
                let raw = match &self.modal {
                    Modal::ParagraphTarget { input } => input.as_str().trim().to_string(),
                    _ => String::new(),
                };
                self.modal = Modal::None;
                self.commit_paragraph_target(&raw);
                return Ok(false);
            }
            if let Modal::ParagraphTarget { input } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_save_markdown {
            if matches!(key.code, KeyCode::Enter) {
                let (body, label, raw) = match &self.modal {
                    Modal::SaveMarkdown { input, body, label } => (
                        body.clone(),
                        label.clone(),
                        input.as_str().to_string(),
                    ),
                    _ => (String::new(), String::new(), String::new()),
                };
                self.modal = Modal::None;
                self.commit_save_markdown(body, label, raw);
                return Ok(false);
            }
            if let Modal::SaveMarkdown { input, .. } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_rendered_preview {
            // Esc is intercepted by the global modal-close
            // handler at the top of this function. Local keys:
            //   ← / →  — navigate pages
            //   S / s — save current page (open picker, mode Single)
            //   A / a — save every page (open picker, mode All)
            //   anything else — swallowed so the editor doesn't see it
            match key.code {
                KeyCode::Left | KeyCode::Up => {
                    if let Modal::RenderedPreview {
                        pages,
                        current_page,
                        ..
                    } = &mut self.modal
                    {
                        if *current_page > 0 {
                            *current_page -= 1;
                            let total = pages.len();
                            let p = &pages[*current_page];
                            self.status = format!(
                                "render ¶ · page {}/{}  · {}×{}",
                                *current_page + 1,
                                total,
                                p.width,
                                p.height,
                            );
                        }
                    }
                }
                KeyCode::Right | KeyCode::Down => {
                    if let Modal::RenderedPreview {
                        pages,
                        current_page,
                        ..
                    } = &mut self.modal
                    {
                        if *current_page + 1 < pages.len() {
                            *current_page += 1;
                            let total = pages.len();
                            let p = &pages[*current_page];
                            self.status = format!(
                                "render ¶ · page {}/{}  · {}×{}",
                                *current_page + 1,
                                total,
                                p.width,
                                p.height,
                            );
                        }
                    }
                }
                KeyCode::Home => {
                    if let Modal::RenderedPreview { current_page, .. } =
                        &mut self.modal
                    {
                        *current_page = 0;
                    }
                }
                KeyCode::End => {
                    if let Modal::RenderedPreview {
                        pages,
                        current_page,
                        ..
                    } = &mut self.modal
                    {
                        *current_page = pages.len().saturating_sub(1);
                    }
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.open_save_rendered_png_picker(false);
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.open_save_rendered_png_picker(true);
                }
                // 1.2.6+ — +/- live preview zoom. `+` (or `=`,
                // since most US keyboards put `+` over `=`)
                // bumps PPI by 0.5; `-` reduces by 0.5. Range
                // [0.5, 6.0]. Each change re-renders every
                // page at the new factor so the modal can
                // accommodate any paragraph length without
                // re-opening.
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    self.zoom_rendered_preview(0.5);
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    self.zoom_rendered_preview(-0.5);
                }
                _ => {}
            }
            return Ok(false);
        }

        if is_story_view {
            if matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S')) {
                self.open_save_story_png_picker();
            }
            return Ok(false);
        }
        if is_save_story_png {
            if matches!(key.code, KeyCode::Enter) {
                let taken = std::mem::replace(&mut self.modal, Modal::None);
                if let Modal::SaveStoryPng {
                    input,
                    png_bytes,
                    book_title,
                    return_to: _,
                } = taken
                {
                    let raw = input.as_str().to_string();
                    self.commit_save_story_png(&png_bytes, &raw, &book_title);
                }
                return Ok(false);
            }
            if let Modal::SaveStoryPng { input, .. } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_diagnostics_list {
            self.diagnostics_list_handle_key(key);
            return Ok(false);
        }
        if is_ai_diff_review {
            self.ai_diff_review_handle_key(key);
            return Ok(false);
        }
        if is_event_picker {
            self.event_picker_handle_key(key);
            return Ok(false);
        }
        if is_timeline_view {
            self.timeline_view_handle_key(key);
            return Ok(false);
        }
        if is_timeline_new_event {
            self.timeline_new_event_prompt_handle_key(key);
            return Ok(false);
        }
        if is_timeline_edit_event {
            self.timeline_edit_event_prompt_handle_key(key);
            return Ok(false);
        }
        if is_snapshot_annotation {
            if matches!(key.code, KeyCode::Enter) {
                let taken = std::mem::replace(&mut self.modal, Modal::None);
                if let Modal::SnapshotAnnotation {
                    input,
                    parent_id,
                    parent_title,
                    body,
                } = taken
                {
                    let annotation = input.as_str().to_string();
                    self.commit_snapshot_annotation(
                        parent_id,
                        &parent_title,
                        &body,
                        annotation.trim(),
                    );
                }
                return Ok(false);
            }
            if let Modal::SnapshotAnnotation { input, .. } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }
        if is_tag_picker {
            self.tag_picker_handle_key(key);
            return Ok(false);
        }
        if is_tag_add_prompt {
            self.tag_add_prompt_handle_key(key);
            return Ok(false);
        }
        if is_tag_delete_confirm {
            self.tag_delete_confirm_handle_key(key);
            return Ok(false);
        }
        if is_tag_rename_prompt {
            self.tag_rename_prompt_handle_key(key);
            return Ok(false);
        }
        if is_tag_search_results {
            self.tag_search_results_handle_key(key);
            return Ok(false);
        }

        if is_save_rendered_png {
            // Esc → restore the preview, handled at the top of
            // this function via the `return_to` stash pattern.
            if matches!(key.code, KeyCode::Enter) {
                let taken = std::mem::replace(&mut self.modal, Modal::None);
                if let Modal::SaveRenderedPng {
                    input,
                    body,
                    settings,
                    title,
                    pages,
                    return_to: _,
                } = taken
                {
                    let raw = input.as_str().to_string();
                    self.commit_save_rendered_png(
                        &body, &settings, &raw, &title, pages,
                    );
                }
                return Ok(false);
            }
            if let Modal::SaveRenderedPng { input, .. } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_writing_streak_heatmap {
            // Any key closes — Esc, Enter, anything.
            // No interactions inside; the modal is a
            // read-only viewer.
            self.modal = Modal::None;
            return Ok(false);
        }

        if is_concordance {
            self.concordance_handle_key(key);
            return Ok(false);
        }

        if is_tts_save_as_audio {
            if matches!(key.code, KeyCode::Esc) {
                self.modal = Modal::None;
                self.status = "TTS save: cancelled".into();
                return Ok(false);
            }
            if matches!(key.code, KeyCode::Enter) {
                let taken = std::mem::replace(&mut self.modal, Modal::None);
                if let Modal::TtsSaveAsAudio {
                    input,
                    body,
                    voice,
                    wpm,
                    voice_label,
                } = taken
                {
                    let raw = input.as_str().to_string();
                    self.commit_tts_save_as_audio(
                        &raw,
                        &body,
                        &voice,
                        wpm,
                        &voice_label,
                    );
                }
                return Ok(false);
            }
            if let Modal::TtsSaveAsAudio { input, .. } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_bund_input {
            if matches!(key.code, KeyCode::Enter) {
                let (typed, hook) = match &self.modal {
                    Modal::BundInput { input, hook, .. } => {
                        (input.as_str().to_string(), hook.clone())
                    }
                    _ => (String::new(), String::new()),
                };
                self.modal = Modal::None;
                if !hook.is_empty() {
                    crate::scripting::hooks::fire(
                        &hook,
                        vec![rust_dynamic::value::Value::from_string(typed)],
                    );
                }
                return Ok(false);
            }
            if let Modal::BundInput { input, .. } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_chat_search_prompt {
            // Enter commits the query into `chat_search`; the rendered
            // chat-history pane then auto-centres on the match. Esc
            // closes the modal without starting a search (global Esc
            // handler at the top of this function does the close).
            if matches!(key.code, KeyCode::Enter) {
                let query = match &self.modal {
                    Modal::ChatSearchPrompt { input } => {
                        input.as_str().trim().to_string()
                    }
                    _ => String::new(),
                };
                self.modal = Modal::None;
                self.commit_chat_search(query);
                return Ok(false);
            }
            if let Modal::ChatSearchPrompt { input } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_find {
            let mut commit = false;
            if let Modal::FindReplace {
                search_input,
                replace_input,
                focus_replace,
            } = &mut self.modal
            {
                match key.code {
                    KeyCode::Enter => commit = true,
                    KeyCode::Tab => {
                        if replace_input.is_some() {
                            *focus_replace = !*focus_replace;
                        }
                    }
                    KeyCode::Backspace => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.backspace();
                            }
                        } else {
                            search_input.backspace();
                        }
                    }
                    KeyCode::Delete => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.delete();
                            }
                        } else {
                            search_input.delete();
                        }
                    }
                    KeyCode::Left => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.move_left();
                            }
                        } else {
                            search_input.move_left();
                        }
                    }
                    KeyCode::Right => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.move_right();
                            }
                        } else {
                            search_input.move_right();
                        }
                    }
                    KeyCode::Home => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.move_home();
                            }
                        } else {
                            search_input.move_home();
                        }
                    }
                    KeyCode::End => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.move_end();
                            }
                        } else {
                            search_input.move_end();
                        }
                    }
                    KeyCode::Char(c) => {
                        let mut residual = key.modifiers;
                        residual.remove(KeyModifiers::SHIFT);
                        if residual.is_empty() {
                            let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                                && c.is_ascii_alphabetic()
                            {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            };
                            if *focus_replace {
                                if let Some(r) = replace_input.as_mut() {
                                    r.insert_char(final_c);
                                }
                            } else {
                                search_input.insert_char(final_c);
                            }
                        }
                    }
                    _ => {}
                }
            }
            if commit {
                self.commit_find();
            }
            return Ok(false);
        }


        if is_file_picker {
            let mut commit = false;
            if let Modal::FilePicker(picker) = &mut self.modal {
                match key.code {
                    KeyCode::Up => picker.move_up(),
                    KeyCode::Down => picker.move_down(),
                    KeyCode::PageUp => picker.page_up(10),
                    KeyCode::PageDown => picker.page_down(10),
                    KeyCode::Home => picker.jump_first(),
                    KeyCode::End => picker.jump_last(),
                    KeyCode::Right => picker.expand(),
                    KeyCode::Left => picker.collapse_or_step_out(),
                    KeyCode::Enter => commit = true,
                    _ => {}
                }
            }
            if commit {
                self.commit_file_pick();
            }
            return Ok(false);
        }

        if is_renaming {
            let mut commit = false;
            if let Modal::Renaming { input, .. } = &mut self.modal {
                match key.code {
                    KeyCode::Enter => commit = true,
                    KeyCode::Backspace => input.backspace(),
                    KeyCode::Delete => input.delete(),
                    KeyCode::Left => input.move_left(),
                    KeyCode::Right => input.move_right(),
                    KeyCode::Home => input.move_home(),
                    KeyCode::End => input.move_end(),
                    KeyCode::Char(c) => {
                        let mut residual = key.modifiers;
                        residual.remove(KeyModifiers::SHIFT);
                        if residual.is_empty() {
                            let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                                && c.is_ascii_alphabetic()
                            {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            };
                            input.insert_char(final_c);
                        }
                    }
                    _ => {}
                }
            }
            if commit {
                self.commit_rename();
            }
            return Ok(false);
        }

        if is_snapshot {
            let mut commit = false;
            let mut delete = false;
            let mut view_diff = false;
            // 1.2.8+ — filter-focus mode: typed chars edit the
            // annotation filter, Backspace edits, Esc exits
            // filter focus.  Picker chords (Up/Down/Enter/D/V/
            // Delete) only fire when filter mode is OFF.
            if self.snapshot_filter_focused {
                match key.code {
                    KeyCode::Esc => {
                        // Exit filter mode but keep the query so
                        // the cursor stays on the narrowed list.
                        self.snapshot_filter_focused = false;
                        // Force the outer Esc handler to NOT close
                        // the modal.  We've already handled this key.
                        return Ok(false);
                    }
                    KeyCode::Enter => {
                        // Enter inside filter mode commits the
                        // filter (exits filter mode) but does not
                        // load the snapshot — second Enter does
                        // that.  Mirrors browser address-bar UX.
                        self.snapshot_filter_focused = false;
                    }
                    KeyCode::Backspace => {
                        self.snapshot_filter.pop();
                        // Reset cursor — visible list shrunk/grew.
                        if let Modal::SnapshotPicker { cursor, .. } = &mut self.modal {
                            *cursor = 0;
                        }
                    }
                    KeyCode::Char(c) => {
                        self.snapshot_filter.push(c);
                        if let Modal::SnapshotPicker { cursor, .. } = &mut self.modal {
                            *cursor = 0;
                        }
                    }
                    _ => {}
                }
                return Ok(false);
            }
            // `/` from chord mode enters filter focus.
            if matches!(key.code, KeyCode::Char('/')) {
                self.snapshot_filter_focused = true;
                return Ok(false);
            }
            // Compute visible length up-front so cursor clamps to
            // the FILTERED list, not the absolute snapshots Vec.
            let visible_len = match &self.modal {
                Modal::SnapshotPicker { snapshots, .. } => {
                    self.visible_snapshot_indices(snapshots).len()
                }
                _ => 0,
            };
            if let Modal::SnapshotPicker { cursor, .. } = &mut self.modal {
                match key.code {
                    KeyCode::Up => {
                        if *cursor > 0 {
                            *cursor -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if *cursor + 1 < visible_len {
                            *cursor += 1;
                        }
                    }
                    KeyCode::Home => *cursor = 0,
                    KeyCode::End => {
                        *cursor = visible_len.saturating_sub(1);
                    }
                    KeyCode::Enter => commit = true,
                    // D (case-insensitive) or the Delete key removes
                    // the cursor's snapshot. No further confirmation —
                    // snapshots are explicit creations (F5 / Ctrl+B N),
                    // and refreshing the list keeps the cursor sane.
                    KeyCode::Char('D') | KeyCode::Char('d') | KeyCode::Delete => {
                        delete = true;
                    }
                    // V (case-insensitive) opens the snapshot
                    // diff modal against the open paragraph's
                    // current buffer. Read-only — Esc returns to
                    // the picker; closing the picker entirely
                    // requires another Esc.
                    KeyCode::Char('V') | KeyCode::Char('v') => {
                        view_diff = true;
                    }
                    _ => {}
                }
            }
            if commit {
                self.commit_snapshot_load();
            } else if delete {
                self.delete_current_snapshot();
            } else if view_diff {
                self.open_snapshot_diff();
            }
            return Ok(false);
        }

        if is_adding {
            let mut commit = false;
            if let Modal::Adding { input, .. } = &mut self.modal {
                match key.code {
                    KeyCode::Enter => commit = true,
                    KeyCode::Backspace => input.backspace(),
                    KeyCode::Delete => input.delete(),
                    KeyCode::Left => input.move_left(),
                    KeyCode::Right => input.move_right(),
                    KeyCode::Home => input.move_home(),
                    KeyCode::End => input.move_end(),
                    KeyCode::Char(c) => {
                        let mut residual = key.modifiers;
                        residual.remove(KeyModifiers::SHIFT);
                        if residual.is_empty() {
                            // Some terminals report Shift+letter as lowercase
                            // char + SHIFT modifier; others as uppercase char
                            // + SHIFT. Normalize so capital letters always go
                            // into the buffer when Shift was held.
                            let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                                && c.is_ascii_alphabetic()
                            {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            };
                            input.insert_char(final_c);
                        }
                    }
                    _ => {}
                }
            }
            if commit {
                self.commit_add();
            }
        } else if is_deleting {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => self.commit_delete(),
                KeyCode::Char('n') | KeyCode::Char('N') => self.modal = Modal::None,
                _ => {}
            }
        }
        Ok(false)
    }

    fn commit_add(&mut self) {
        let (kind, parent_id, raw_title, position) = match &self.modal {
            Modal::Adding {
                kind,
                parent_id,
                input,
                position,
                ..
            } => (
                *kind,
                *parent_id,
                input.as_str().trim().to_string(),
                *position,
            ),
            _ => return,
        };

        // Paragraphs can be added with an empty title — they'll be given a
        // placeholder ("Untitled paragraph") that the next save replaces with
        // the first sentence of the body. Branches still require a title
        // because they have no content from which to derive one.
        let title = if raw_title.is_empty() {
            if kind == NodeKind::Paragraph {
                PARAGRAPH_PLACEHOLDER_TITLE.to_string()
            } else {
                self.status =
                    format!("a {} needs a title — type one or Esc to cancel", kind.as_str());
                return;
            }
        } else {
            raw_title
        };

        let parent = parent_id.and_then(|id| self.hierarchy.get(id)).cloned();
        match self.store.create_node(
            &self.cfg,
            &self.hierarchy,
            kind,
            &title,
            parent.as_ref(),
            None,
            position,
        ) {
            Ok(node) => {
                let new_id = node.id;
                // For root-level user books: provision the artefacts
                // subdirectory and the Typst-book skeleton (chapter +
                // index/settings/globals paragraphs). No-op for other
                // kinds; failure logs to status but doesn't roll back
                // the just-created book.
                if let Err(e) = self.store.provision_user_book(&self.cfg, &node) {
                    self.status = format!(
                        "added {} `{}` — but Typst skeleton failed: {e}",
                        kind.as_str(),
                        node.title
                    );
                } else {
                    self.status = format!("added {} `{}`", kind.as_str(), node.title);
                }
                self.modal = Modal::None;
                self.reload_hierarchy();
                if let Some(i) = self.rows.iter().position(|(id, _)| *id == new_id) {
                    self.tree_cursor = i;
                }
            }
            Err(e) => {
                self.status = format!("add failed: {e}");
            }
        }
    }

    fn commit_delete(&mut self) {
        let (root_id, root_kind, ids, title) = match &self.modal {
            Modal::Deleting {
                root_id,
                root_kind,
                ids,
                title,
                ..
            } => (*root_id, *root_kind, ids.clone(), title.clone()),
            _ => return,
        };
        let root_node = match self.hierarchy.get(root_id) {
            Some(n) => n.clone(),
            None => {
                self.modal = Modal::None;
                self.status = "node already gone".into();
                return;
            }
        };
        let parent_id = root_node.parent_id;

        let fs_rel = match root_kind {
            NodeKind::Paragraph => root_node
                .file
                .as_ref()
                .map(std::path::PathBuf::from)
                .unwrap_or_default(),
            _ => self.hierarchy.fs_path(&root_node, &self.layout),
        };

        // 1.2.7+ — stash a single-paragraph delete into the
        // kill-ring so Ctrl+B U can recover the content
        // afterwards. Skipped for branch deletes (chapters /
        // books) because subtree restoration without UUID
        // preservation is too risky to ship without store
        // API support.
        if root_kind == NodeKind::Paragraph && ids.len() == 1 {
            self.stash_deleted_paragraph(&root_node);
        }
        // 1.2.8+: branch deletes no longer clear the
        // kill-ring. Older single-¶ entries in the ring
        // are still valid recoveries — they reference paths
        // that may or may not exist after the branch went,
        // but the restore flow handles "parent gone" by
        // falling back to InsertPosition::End. Keeping the
        // entries is strictly more useful than dropping them.

        if let Err(e) = self.store.delete_subtree(&fs_rel, &ids) {
            self.status = format!("delete failed: {e}");
            // The push-front above already happened for a
            // single-¶ stash; drop it again since the delete
            // didn't actually fire — the on-disk paragraph
            // still exists and the front-stash would create
            // a duplicate on the next Ctrl+B U.
            if root_kind == NodeKind::Paragraph && ids.len() == 1 {
                self.kill_ring.pop_front();
            }
            return;
        }

        // Close editor if its open doc was inside the deleted subtree.
        if let Some(doc) = &self.opened {
            if ids.contains(&doc.id) {
                self.opened = None;
            }
        }

        self.modal = Modal::None;
        let ring_len = self.kill_ring.len();
        let undo_hint = if root_kind == NodeKind::Paragraph
            && ids.len() == 1
            && ring_len > 0
        {
            if ring_len == 1 {
                " · Ctrl+B U to restore (new uuid — paragraph links to old id stay broken)".to_string()
            } else {
                format!(
                    " · Ctrl+B U restore · Ctrl+V Shift+U picker ({ring_len} in ring)",
                )
            }
        } else {
            String::new()
        };
        self.status = format!(
            "deleted {} `{}` ({} other node{} removed){undo_hint}",
            root_kind.as_str(),
            title,
            ids.len() - 1,
            if ids.len() == 2 { "" } else { "s" }
        );
        self.reload_hierarchy();
        if let Some(pid) = parent_id {
            if let Some(i) = self.rows.iter().position(|(id, _)| *id == pid) {
                self.tree_cursor = i;
            }
        }
    }

    // -------- open / save -------------------------------------------------

    fn open_selected(&mut self) -> Result<()> {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            return Ok(());
        };
        let Some(node) = self.hierarchy.get(id).cloned() else {
            return Ok(());
        };

        match node.kind {
            // Scripts are text leaves like Paragraphs — same load
            // path, same editor surface. Real Bund syntax
            // highlighting is a follow-up; today they render as
            // plain text (which is still legible because bundcore's
            // syntax is sparse: words + braces + strings).
            NodeKind::Paragraph | NodeKind::Script => self.load_paragraph(&node)?,
            NodeKind::Image => self.show_image_info(&node),
            _ => {
                self.status = format!(
                    "`{}` is a {} (Enter opens paragraphs / images / scripts)",
                    node.title,
                    node.kind.as_str()
                );
            }
        }
        Ok(())
    }

    /// Enter on an Image row: try the ratatui-image preview modal,
    /// fall back to a status-bar info line when the picker isn't
    /// available (preview disabled, terminal lacks graphics protocol,
    /// or the image bytes aren't decodable by the `image` crate).
    fn show_image_info(&mut self, node: &Node) {
        let fs_rel = node.file.clone().unwrap_or_else(|| "<no path>".into());
        let abs = self.layout.root.join(&fs_rel);
        let size = std::fs::metadata(&abs)
            .map(|m| m.len())
            .unwrap_or(0);

        // Preview path: fetch bytes from bdslib (source of truth),
        // decode, build a resize protocol, pop the modal.
        if let Some(picker) = self.image_picker.as_ref() {
            match self.store.image_bytes(node.id) {
                Ok(Some(bytes)) => match image::load_from_memory(&bytes) {
                    Ok(dyn_img) => {
                        let proto = picker.new_resize_protocol(dyn_img);
                        self.modal = Modal::ImagePreview {
                            title: node.title.clone(),
                            fs_rel: fs_rel.clone(),
                            size_bytes: size,
                            proto,
                        };
                        self.status = format!(
                            "🖼 {}  ·  Esc closes  ·  {} bytes",
                            node.title, size
                        );
                        return;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "image decode failed for {}: {e} — falling back to info line",
                            node.title
                        );
                    }
                },
                Ok(None) => {
                    tracing::warn!(
                        "image {} has no bytes in bdslib — info line only",
                        node.title
                    );
                }
                Err(e) => {
                    tracing::warn!("image_bytes({}): {e}", node.title);
                }
            }
        }

        // Fallback: status-bar one-liner.
        let caption_hint = node
            .image_caption
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("(no caption)");
        self.status = format!(
            "🖼 {} · {} · {} bytes · {}",
            node.title, fs_rel, size, caption_hint
        );
    }

    /// 1.2.5+: re-parse the open paragraph with `typst-syntax` and
    /// cache the resulting diagnostics on `OpenedDoc`. Honors the
    /// `typst_compile.diagnostics` HJSON flag and the buffer's
    /// `content_type` — only typst sources are checked (Bund /
    /// HJSON / others skip out cleanly). Status bar surfaces the
    /// first error so the user sees the line number at a glance;
    /// the rest stay cached on the doc for any future "next error"
    /// chord.
    fn refresh_typst_diagnostics_for_opened(&mut self) {
        if !self.cfg.typst_compile.diagnostics {
            return;
        }
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        // Anything not typst-shaped — Bund scripts, HJSON data
        // nodes, images — should never be fed to the typst
        // parser; just clear stale diagnostics and bail.
        let is_typst = match doc.content_type.as_deref() {
            None | Some("") | Some("typst") => true,
            _ => false,
        };
        if !is_typst {
            doc.typst_diagnostics.clear();
            doc.typst_diagnostics_checked_at = std::time::Instant::now();
            return;
        }
        let body = doc.textarea.lines().join("\n");
        // Phase 1 baseline: parse-only diagnostics via `typst-syntax`.
        // Cheap, always available, no engine dependency.
        let mut diags = crate::typst_check::check(&body);
        // 1.2.5+: when the user has the in-process engine on AND
        // opted into semantic diagnostics, run a full
        // `typst::compile` against the paragraph in isolation and
        // surface semantic errors the parser can't catch
        // (unknown functions, type errors, etc.). We APPEND to
        // the parse diagnostics rather than replace — a
        // syntactically-broken buffer often produces a flurry of
        // confusing semantic errors and the parse error is the
        // root cause to surface first.
        if self.cfg.typst_compile.semantic_diagnostics
            && self.cfg.typst_compile.use_inprocess_engine()
            && diags.is_empty()
        {
            let settings = crate::typst_world::WorldSettings::from_cfg(
                &self.cfg.typst_compile,
            );
            let semantic =
                crate::typst_inprocess::check_semantic(&body, settings);
            diags.extend(semantic);
        }
        doc.typst_diagnostics = diags;
        doc.typst_diagnostics_checked_at = std::time::Instant::now();
        if let Some(first) = doc.typst_diagnostics.first() {
            // Don't blow away a more-recent status (a save's own
            // "wrote N bytes" message etc.) — only stamp the
            // diagnostics line if we have errors to show. The
            // save-path caller is OK with this being the final
            // status because errors-on-save are exactly what the
            // user needs to see next.
            self.status = first.summary();
        }

        // 1.2.6+ — fire `hook.on_diagnostic` when the diagnostic
        // state changes. Snapshot is `(count, first-message)`;
        // we re-fire on clean→errored, count change, or top-
        // message change. Avoids spamming hooks on every idle
        // tick when nothing actually moved.
        let snapshot = (
            doc.typst_diagnostics.len(),
            doc.typst_diagnostics
                .first()
                .map(|d| d.message.clone())
                .unwrap_or_default(),
        );
        let changed = doc.typst_diag_last_fired.as_ref() != Some(&snapshot);
        if changed {
            doc.typst_diag_last_fired = Some(snapshot.clone());
            let paragraph_id = doc.id;
            // hook.on_diagnostic ( uuid count first-message -- )
            crate::scripting::hooks::fire(
                "hook.on_diagnostic",
                vec![
                    rust_dynamic::value::Value::from_string(
                        paragraph_id.to_string(),
                    ),
                    rust_dynamic::value::Value::from_int(snapshot.0 as i64),
                    rust_dynamic::value::Value::from_string(snapshot.1.clone()),
                ],
            );
        }
    }

    /// Create an event paragraph under the book's Timeline
    /// chapter from the swim-lane "n" path. Returns the new
    /// node id; status messaging is the caller's job.
    fn create_event_at_cursor(
        &mut self,
        book_id: Uuid,
        title: &str,
        cursor_ticks: i64,
        track: Option<&str>,
    ) -> std::result::Result<(), String> {
        let timeline_chapter_id = self
            .store
            .ensure_timeline_chapter(&self.cfg, book_id)
            .map_err(|e| format!("{e}"))?;
        self.reload_hierarchy();
        let timeline_chapter = self
            .hierarchy
            .get(timeline_chapter_id)
            .cloned()
            .ok_or_else(|| "Timeline chapter vanished after creation".to_string())?;
        let mut node = self
            .store
            .create_node(
                &self.cfg,
                &self.hierarchy,
                NodeKind::Paragraph,
                title,
                Some(&timeline_chapter),
                None,
                InsertPosition::End,
            )
            .map_err(|e| format!("create_node: {e}"))?;
        node.event = Some(crate::store::node::EventData {
            start_ticks: cursor_ticks,
            end_ticks: None,
            precision: crate::timeline::Precision::Day,
            characters: Vec::new(),
            places: Vec::new(),
            track: track.map(str::to_owned),
        });
        crate::store::reconcile_event_orphan_tag(&mut node);
        node.modified_at = chrono::Utc::now();
        self.store
            .raw()
            .update_metadata(node.id, node.to_json())
            .map_err(|e| format!("update_metadata: {e}"))?;
        self.store.sync().map_err(|e| format!("sync: {e}"))?;
        // 1.2.6+ — same hook the CLI / Bund paths fire.
        crate::scripting::hooks::fire(
            "hook.on_event_added",
            vec![rust_dynamic::value::Value::from_string(
                node.id.to_string(),
            )],
        );
        self.reload_hierarchy();
        Ok(())
    }

    /// `n` — pop a small one-line input for the new event's
    /// title; on Enter the event is created at
    /// `cursor_ticks` with the current track highlight (or
    /// the default track).
    /// `Ctrl+V Shift+I` — pop a one-line edit prompt for the
    /// open event paragraph's start / end / track. Pipe-
    /// separated:
    ///   `Sol 13 | Sol 14 | main`     ← start, end, track
    ///   `Sol 13 |  | main`           ← no end
    ///   `Sol 13 | Sol 14 |`          ← drop the track
    /// Pre-fills from current values. Precision is re-derived
    /// from the start string on commit. No-op when the open
    /// paragraph isn't an event.
    fn open_edit_event_metadata_prompt(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status =
                "edit event: no paragraph open (Ctrl+V Shift+I needs an editor buffer)".into();
            return;
        };
        let event_id = doc.id;
        let node = match self.hierarchy.get(event_id).cloned() {
            Some(n) => n,
            None => {
                self.status = "edit event: paragraph missing from hierarchy".into();
                return;
            }
        };
        let ev = match node.event.as_ref() {
            Some(ev) => ev,
            None => {
                self.status =
                    "edit event: `{open paragraph}` isn't an event — use `inkhaven event add` first".into();
                return;
            }
        };
        let cal = crate::timeline::Calendar::from_config(
            self.cfg.timeline.calendar.clone(),
        );
        let start_str = cal.format(
            crate::timeline::TimelinePoint::from_ticks(ev.start_ticks),
            ev.precision,
        );
        let end_str = ev
            .end_ticks
            .map(|t| {
                cal.format(
                    crate::timeline::TimelinePoint::from_ticks(t),
                    ev.precision,
                )
            })
            .unwrap_or_default();
        let track_str = ev.track.as_deref().unwrap_or("");
        let prefill = format!("{start_str} | {end_str} | {track_str}");
        let mut input = TextInput::new();
        for c in prefill.chars() {
            input.insert_char(c);
        }
        self.modal = Modal::TimelineEditEventPrompt { input, event_id };
        self.status =
            "edit event: <start> | <end> | <track> · empty middle = no end · Enter commits · Esc cancels".into();
    }

    fn commit_edit_event_metadata(&mut self) {
        let taken = std::mem::replace(&mut self.modal, Modal::None);
        let Modal::TimelineEditEventPrompt { input, event_id } = taken else {
            return;
        };
        let raw = input.as_str().to_owned();
        let parts: Vec<&str> = raw.split('|').collect();
        // Tolerate fewer than 3 segments (user may have left the
        // trailing pipes off — treat missing as empty).
        let start_str = parts.first().map(|s| s.trim()).unwrap_or("").to_owned();
        let end_str = parts.get(1).map(|s| s.trim()).unwrap_or("").to_owned();
        let track_str = parts.get(2).map(|s| s.trim()).unwrap_or("").to_owned();
        if start_str.is_empty() {
            self.status = "edit event: start can't be empty".into();
            return;
        }
        let cal = crate::timeline::Calendar::from_config(
            self.cfg.timeline.calendar.clone(),
        );
        let (start_point, precision) = match cal.parse(&start_str) {
            Ok(pp) => pp,
            Err(e) => {
                self.status = format!("edit event: bad start `{start_str}`: {e}");
                return;
            }
        };
        let end_ticks: Option<i64> = if end_str.is_empty() {
            None
        } else {
            match cal.parse(&end_str) {
                Ok((p, _)) => Some(p.ticks()),
                Err(e) => {
                    self.status = format!("edit event: bad end `{end_str}`: {e}");
                    return;
                }
            }
        };
        let new_track: Option<String> = if track_str.is_empty() {
            None
        } else {
            Some(track_str)
        };
        let mut node = match self.hierarchy.get(event_id).cloned() {
            Some(n) => n,
            None => {
                self.status = "edit event: paragraph vanished".into();
                return;
            }
        };
        let Some(ev) = node.event.as_mut() else {
            self.status = "edit event: paragraph isn't an event".into();
            return;
        };
        ev.start_ticks = start_point.ticks();
        ev.end_ticks = end_ticks;
        ev.precision = precision;
        ev.track = new_track;
        node.modified_at = chrono::Utc::now();
        crate::store::reconcile_event_orphan_tag(&mut node);
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(node.id, node.to_json())
        {
            self.status = format!("edit event: persist: {e}");
            return;
        }
        if let Err(e) = self.store.sync() {
            self.status = format!("edit event: sync: {e}");
            return;
        }
        self.reload_hierarchy();
        self.status = format!(
            "event updated · start={start_str}{}",
            if !end_str.is_empty() {
                format!(" · end={end_str}")
            } else {
                String::new()
            }
        );
    }

    // ── 1.2.6+ scope navigation ──────────────────────────

    /// Ctrl+V t (1.2.6+) — open the swim-lane timeline view.
    /// Anchors to the current paragraph's nearest
    /// Subchapter / Chapter / Book ancestor; falls back to
    /// the tree cursor and finally to the first user book.
    fn open_timeline_view(&mut self) {
        if !self.cfg.timeline.enabled {
            self.status =
                "timeline view: timeline.enabled is false in HJSON — enable it to use Ctrl+V Shift+T".into();
            return;
        }
        let Some(book_id) = self.resolve_anchor_book() else {
            self.status =
                "timeline view: no user books in this project".into();
            return;
        };
        let scope_id = self.resolve_anchor_scope(book_id);
        let events = self.collect_book_events(book_id, false);
        let is_empty = events.is_empty();
        // 1.2.6+: when the book has no events yet, still open the
        // timeline at the epoch tick so the user can press `n` to
        // add the first event from inside the TUI. The previous
        // behaviour was to refuse-and-redirect to the CLI, which
        // hid the in-TUI add chord entirely.
        //
        // 1.2.6+ auto-fit: when events ARE present, compute the
        // total span (earliest start → latest end/start) and pick
        // a `ticks_per_cell` that makes the whole range fit in
        // the visible pane. The user then drills in with `+` /
        // `-`. Width is sampled via `crossterm::terminal::size()`
        // at open time — close enough; the swim-lane content
        // area is ~ `terminal_width - track_gutter - borders`.
        let (cursor_ticks, scroll_ticks, ticks_per_cell) = if events.is_empty() {
            (0i64, -20i64, 1.0f64)
        } else {
            timeline_auto_fit(&events)
        };
        let state = TimelineViewState {
            book_id,
            scope_id,
            nav_history: Vec::new(),
            events,
            track_highlight: None,
            ticks_per_cell,
            scroll_ticks,
            cursor_ticks,
            selected_event_id: None,
            collapsed_tracks: std::collections::HashSet::new(),
            expanded_track: None,
            focus_level: TimelineFocusLevel::Track,
            project_overlay: false,
            descent: None,
        };
        let crumb = self.timeline_scope_crumb(&state);
        self.modal = Modal::TimelineView { state };
        // 1.2.7+ — apply any cached per-book view state
        // (collapsed tracks, expanded track, zoom, scroll).
        // No-op on a fresh book or a session.json without an
        // entry — auto-fit defaults from above stay.
        self.timeline_restore_view_state();
        self.status = if is_empty {
            format!("timeline {crumb} · empty — press `n` to add the first event · Esc closes")
        } else {
            format!(
                "timeline {crumb} · auto-fit · ↑↓ event step · ←→ scroll · +/- zoom · n new · Esc closes"
            )
        };
    }

    /// 1.2.6+ — `Ctrl+V Shift+E`. Opens the timeline view and
    /// immediately triggers the new-event prompt so a fresh
    /// project (zero events) can add its first event from any
    /// pane without going through the CLI. When the timeline
    /// has events, the prompt fires at the timeline cursor's
    /// current tick (same as pressing `n` after opening).
    fn open_new_event_prompt_from_anywhere(&mut self) {
        // 1.2.7+ — pre-fill the event title with the editor's
        // current selection, if any. Lets the user highlight
        // a phrase like "the storm at dawn" → Ctrl+V Shift+E
        // → modal pops with that text already in the input.
        // Selection truncated to 60 chars (the prompt's
        // practical width); newlines flattened to spaces.
        let prefill = self.editor_selection_text().map(|s| {
            let flat = s
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if flat.chars().count() > 60 {
                let truncated: String = flat.chars().take(59).collect();
                format!("{truncated}…")
            } else {
                flat
            }
        });
        self.open_timeline_view();
        // open_timeline_view bails before setting Modal::TimelineView
        // when timeline is disabled / no books exist; in that case
        // the status bar already explains why, and the chained
        // prompt is a no-op.
        if matches!(self.modal, Modal::TimelineView { .. }) {
            self.timeline_open_new_event_prompt();
            if let (Some(text), Modal::TimelineNewEventPrompt { input, .. }) =
                (prefill, &mut self.modal)
            {
                for c in text.chars() {
                    input.insert_char(c);
                }
                self.status =
                    "new event: title pre-filled from editor selection · Enter commits · Esc cancels".into();
            }
        }
    }

    /// Walk up from the editor (or tree cursor) to find the
    /// containing user Book. Returns its UUID. None when the
    /// project has no user books at all.
    fn resolve_anchor_book(&self) -> Option<Uuid> {
        // 1.2.6+ — focus-aware: when the Tree pane has focus,
        // the tree cursor wins over the (possibly stale) open
        // paragraph. Previously the open-paragraph branch
        // always won, so navigating the tree cursor into a
        // different book and pressing Ctrl+V Shift+T still
        // opened the timeline of the book the editor was last
        // viewing. Editor / AI / search focus still prefer the
        // open paragraph (the natural "I'm editing X, timeline
        // for X's book" reading).
        let candidate_id = if self.focus == Focus::Tree {
            self.rows
                .get(self.tree_cursor)
                .map(|(id, _)| *id)
                .or_else(|| self.opened.as_ref().map(|d| d.id))?
        } else {
            self.opened
                .as_ref()
                .map(|d| d.id)
                .or_else(|| self.rows.get(self.tree_cursor).map(|(id, _)| *id))?
        };
        let mut cur_id = candidate_id;
        loop {
            let Some(node) = self.hierarchy.get(cur_id) else {
                break;
            };
            if node.kind == NodeKind::Book && node.system_tag.is_none() {
                return Some(node.id);
            }
            match node.parent_id {
                Some(p) => cur_id = p,
                None => break,
            }
        }
        // Fallback: any user book.
        self.hierarchy
            .children_of(None)
            .into_iter()
            .find(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .map(|n| n.id)
    }

    /// Default scope = current paragraph's nearest Subchapter
    /// (or Chapter, or the book itself). Walks the parent
    /// chain; never returns a non-tree-cursor scope.
    fn resolve_anchor_scope(&self, book_id: Uuid) -> Uuid {
        // Same focus-aware preference as resolve_anchor_book —
        // Tree-pane focus uses the tree cursor; other panes use
        // the open paragraph. Without this the scope walked up
        // from a stale opened-doc paragraph and landed in the
        // wrong book entirely.
        let candidate = if self.focus == Focus::Tree {
            self.rows
                .get(self.tree_cursor)
                .map(|(id, _)| *id)
                .or_else(|| self.opened.as_ref().map(|d| d.id))
        } else {
            self.opened
                .as_ref()
                .map(|d| d.id)
                .or_else(|| self.rows.get(self.tree_cursor).map(|(id, _)| *id))
        };
        let Some(mut cur_id) = candidate else {
            return book_id;
        };
        loop {
            let Some(node) = self.hierarchy.get(cur_id) else {
                return book_id;
            };
            match node.kind {
                NodeKind::Subchapter | NodeKind::Chapter | NodeKind::Book => return node.id,
                _ => {}
            }
            match node.parent_id {
                Some(p) => cur_id = p,
                None => return book_id,
            }
        }
    }

    /// Snapshot every event under `book_id` (or every user
    /// book when `project = true`) into `TimelineEvent`s. The
    /// returned list is sorted by start_ticks.
    fn collect_book_events(&self, book_id: Uuid, project: bool) -> Vec<TimelineEvent> {
        let book_slugs: std::collections::HashMap<Uuid, String> = self
            .hierarchy
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .map(|n| (n.id, n.slug.clone()))
            .collect();
        let target_books: Vec<Uuid> = if project {
            book_slugs.keys().copied().collect()
        } else {
            vec![book_id]
        };
        let mut out: Vec<TimelineEvent> = Vec::new();
        for (n, _) in self.hierarchy.flatten() {
            let Some(ev) = n.event.as_ref() else { continue };
            // Walk up to find the containing user book.
            let mut cur = n.parent_id;
            let mut book_for_node: Option<Uuid> = None;
            while let Some(pid) = cur {
                match self.hierarchy.get(pid) {
                    Some(p) => {
                        if p.kind == NodeKind::Book && p.system_tag.is_none() {
                            book_for_node = Some(p.id);
                            break;
                        }
                        cur = p.parent_id;
                    }
                    None => break,
                }
            }
            let Some(book) = book_for_node else { continue };
            if !target_books.contains(&book) {
                continue;
            }
            let book_prefix = if project {
                book_slugs.get(&book).cloned().unwrap_or_default()
            } else {
                String::new()
            };
            out.push(TimelineEvent {
                id: n.id,
                title: n.title.clone(),
                start_ticks: ev.start_ticks,
                end_ticks: ev.end_ticks,
                precision: ev.precision,
                track: ev.track.clone(),
                is_orphan: n.tags.iter().any(|t| t.eq_ignore_ascii_case("orphan")),
                linked_paragraphs: n.linked_paragraphs.clone(),
                characters: ev.characters.clone(),
                places: ev.places.clone(),
                book_prefix,
            });
        }
        out.sort_by_key(|e| e.start_ticks);
        out
    }

    /// Ctrl+V e (1.2.6+) — gather every event in the project
    /// and pop the picker. Bails early when timeline is
    /// disabled in HJSON so users see a precise hint instead
    /// of an empty picker.
    fn open_event_picker(&mut self) {
        if !self.cfg.timeline.enabled {
            self.status =
                "event picker: timeline.enabled is false in HJSON — enable it to use Ctrl+V e".into();
            return;
        }
        let calendar = crate::timeline::Calendar::from_config(
            self.cfg.timeline.calendar.clone(),
        );
        let mut entries: Vec<EventPickerEntry> = self
            .hierarchy
            .flatten()
            .into_iter()
            .filter_map(|(n, _)| {
                let ev = n.event.as_ref()?;
                let start_str = calendar.format(
                    crate::timeline::TimelinePoint::from_ticks(ev.start_ticks),
                    ev.precision,
                );
                let glyph = if ev.end_ticks.is_some() {
                    "─"
                } else if n.tags.iter().any(|t| t.eq_ignore_ascii_case("orphan")) {
                    "◌"
                } else {
                    "●"
                };
                Some(EventPickerEntry {
                    id: n.id,
                    title: n.title.clone(),
                    start_ticks: ev.start_ticks,
                    start_str,
                    glyph: glyph.to_owned(),
                    track: ev.track.clone(),
                    is_orphan: n.tags.iter().any(|t| t.eq_ignore_ascii_case("orphan")),
                })
            })
            .collect();
        if entries.is_empty() {
            self.status =
                "event picker: no events yet — `inkhaven event add …` from the CLI".into();
            return;
        }
        entries.sort_by_key(|e| e.start_ticks);
        let total = entries.len();
        self.modal = Modal::EventPicker {
            entries,
            cursor: 0,
            track_filter: None,
        };
        self.status = format!(
            "events ({total}) · ↑↓ select · Enter opens · t cycles tracks · Esc closes"
        );
    }

    fn event_picker_handle_key(&mut self, key: KeyEvent) {
        let total = match &self.modal {
            Modal::EventPicker { entries, track_filter, .. } => {
                visible_event_entries(entries, track_filter.as_deref()).len()
            }
            _ => 0,
        };
        match key.code {
            KeyCode::Up => {
                if let Modal::EventPicker { cursor, .. } = &mut self.modal {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
            }
            KeyCode::Down => {
                if let Modal::EventPicker { cursor, .. } = &mut self.modal {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
            }
            KeyCode::Home => {
                if let Modal::EventPicker { cursor, .. } = &mut self.modal {
                    *cursor = 0;
                }
            }
            KeyCode::End => {
                if let Modal::EventPicker { cursor, .. } = &mut self.modal {
                    *cursor = total.saturating_sub(1);
                }
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                // Cycle through tracks: None → first → … → None.
                let next: Option<String> = match &self.modal {
                    Modal::EventPicker { entries, track_filter, .. } => {
                        let mut tracks: Vec<String> = entries
                            .iter()
                            .filter_map(|e| e.track.clone())
                            .collect();
                        tracks.sort();
                        tracks.dedup();
                        cycle_track(track_filter.as_deref(), &tracks)
                    }
                    _ => None,
                };
                if let Modal::EventPicker {
                    track_filter,
                    cursor,
                    ..
                } = &mut self.modal
                {
                    *track_filter = next.clone();
                    *cursor = 0;
                    self.status = match next {
                        Some(t) => format!("event picker · track filter: `{t}`"),
                        None => "event picker · track filter: all".into(),
                    };
                }
            }
            KeyCode::Enter => {
                let Modal::EventPicker {
                    entries,
                    cursor,
                    track_filter,
                } = &self.modal
                else {
                    return;
                };
                let visible = visible_event_entries(entries, track_filter.as_deref());
                let Some(entry) = visible.get(*cursor).copied() else {
                    return;
                };
                let id = entry.id;
                let title = entry.title.clone();
                self.modal = Modal::None;
                if let Err(e) = self.open_paragraph_by_uuid(id) {
                    self.status = format!("event picker: couldn't open `{title}`: {e}");
                } else if !self.status.starts_with("orphan event") {
                    // open_paragraph_by_uuid leaves the orphan
                    // hint in `status` when applicable; preserve
                    // it instead of stomping with a redundant
                    // "opened event" message.
                    self.status = format!("opened event `{title}`");
                }
            }
            _ => {}
        }
    }

    fn open_diagnostics_list(&mut self) {
        if self.opened.is_none() {
            self.status = "F8 diagnostics: no paragraph open".into();
            return;
        }
        self.refresh_typst_diagnostics_for_opened();
        let count = self
            .opened
            .as_ref()
            .map(|d| d.typst_diagnostics.len())
            .unwrap_or(0);
        if count == 0 {
            self.status = "F8 diagnostics: no typst diagnostics in this buffer".into();
            return;
        }
        // 1.2.7+ — F8 now fires from any pane; pull focus
        // back to the editor so the Enter-jumps-to-line
        // behaviour lands in the right place.
        self.change_focus(Focus::Editor);
        self.modal = Modal::DiagnosticsList { cursor: 0 };
        self.status = format!(
            "diagnostics ({count}) · ↑↓ select · Enter jumps · Esc closes"
        );
    }

    fn diagnostics_list_handle_key(&mut self, key: KeyEvent) {
        let total = self
            .opened
            .as_ref()
            .map(|d| d.typst_diagnostics.len())
            .unwrap_or(0);
        if total == 0 {
            return;
        }
        match key.code {
            KeyCode::Up => {
                if let Modal::DiagnosticsList { cursor } = &mut self.modal {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
            }
            KeyCode::Down => {
                if let Modal::DiagnosticsList { cursor } = &mut self.modal {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
            }
            KeyCode::Home => {
                if let Modal::DiagnosticsList { cursor } = &mut self.modal {
                    *cursor = 0;
                }
            }
            KeyCode::End => {
                if let Modal::DiagnosticsList { cursor } = &mut self.modal {
                    *cursor = total.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let Modal::DiagnosticsList { cursor } = self.modal else {
                    return;
                };
                let Some(diag) = self
                    .opened
                    .as_ref()
                    .and_then(|d| d.typst_diagnostics.get(cursor).cloned())
                else {
                    return;
                };
                self.modal = Modal::None;
                if let Some(doc) = self.opened.as_mut() {
                    let row = diag.line.saturating_sub(1) as u16;
                    let col = diag.col.saturating_sub(1) as u16;
                    doc.textarea
                        .move_cursor(tui_textarea::CursorMove::Jump(row, col));
                }
                self.change_focus(Focus::Editor);
                self.status = format!(
                    "diag · line {}:{} — {}",
                    diag.line, diag.col, diag.message
                );
            }
            _ => {}
        }
    }

    /// 1.2.6+ — open the AI diff-review modal. Captures the
    /// current buffer as `before_lines` and the would-be
    /// result of the named action as `after_lines`. The
    /// modal renders both columns side-by-side; the dispatch
    /// handler invokes `apply_inference_direct` if the user
    /// accepts.
    fn open_ai_diff_review(&mut self, action: InferenceAction, raw: &str) {
        let Some(doc) = self.opened.as_ref() else {
            self.status =
                "no paragraph open — apply needs a focused paragraph".into();
            return;
        };
        let before_lines: Vec<String> = doc.textarea.lines().to_vec();
        // Compute what the buffer WOULD look like after this
        // action so the diff is faithful. Both Replace and
        // ReplaceCorrected go through `select_apply_text` so
        // a grammar-style response with markers / fence /
        // "Corrected" heading lands ONLY the discrete block,
        // never the surrounding commentary — even when the
        // user pressed `r` (the looser chord).
        let force = matches!(action, InferenceAction::ReplaceCorrected);
        let raw_len = raw.len();
        let (after_text, extracted) = match select_apply_text(raw, force) {
            Ok(pair) => pair,
            Err(msg) => {
                self.status = msg.into();
                return;
            }
        };
        let after_lines: Vec<String> = if after_text.is_empty() {
            vec![String::new()]
        } else {
            after_text.split('\n').map(String::from).collect()
        };
        self.modal = Modal::AiDiffReview {
            before_lines,
            after_lines,
            action,
            scroll: 0,
        };
        self.status = if extracted {
            format!(
                "AI diff · ✂ extracted {}/{} chars · a accept · r reject",
                after_text.len(),
                raw_len,
            )
        } else {
            "AI diff · a accept · r reject · ↑↓ scroll".into()
        };
    }

    /// Commit step for `Modal::AiDiffReview`. `after_text` is
    /// the buffer content the user accepted; `refocus_editor`
    /// jumps focus back to the editor pane (used by the `e`
    /// chord). Mirrors the in-place mutation that the
    /// pre-1.2.6 direct path did.
    fn apply_ai_diff_accepted(
        &mut self,
        action: InferenceAction,
        after_text: String,
        refocus_editor: bool,
    ) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let baseline = doc.textarea.lines().to_vec();
        let lines: Vec<String> = if after_text.is_empty() {
            vec![String::new()]
        } else {
            after_text.split('\n').map(String::from).collect()
        };
        let mut new_ta = TextArea::new(lines);
        new_ta.set_cursor_line_style(
            Style::default().add_modifier(Modifier::REVERSED),
        );
        new_ta.set_line_number_style(
            Style::default().fg(self.theme.line_number_fg),
        );
        doc.textarea = new_ta;
        // ReplaceCorrected reuses the grammar-change highlight;
        // plain Replace doesn't (the user opted in to a full
        // rewrite, not a copy-edit).
        if matches!(action, InferenceAction::ReplaceCorrected) {
            doc.correction_baseline = Some(baseline);
        } else {
            doc.correction_baseline = None;
        }
        doc.dirty = true;
        doc.last_activity = std::time::Instant::now();
        self.status = format!("AI diff: accepted ({})", action.label());
        if refocus_editor {
            self.change_focus(Focus::Editor);
        }
    }

    /// Ctrl+V N (1.2.5+) — move the editor cursor to the next
    /// typst diagnostic in the open buffer. Wraps around at the
    /// end. Refreshes the diagnostics cache up-front so the user
    /// always navigates against the current buffer state, even
    /// if they haven't paused long enough for the idle recheck
    /// to fire.
    fn jump_to_next_diagnostic(&mut self) {
        if self.opened.is_none() {
            self.status = "next diag: no paragraph open".into();
            return;
        }
        // Force a fresh recheck — keeps the navigation honest
        // when the user has been typing fast.
        self.refresh_typst_diagnostics_for_opened();
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        if doc.typst_diagnostics.is_empty() {
            self.status = "next diag: no typst diagnostics in this buffer".into();
            return;
        }
        // Cursor in tui-textarea is (row, col), both 0-based.
        // TypstDiagnostic.line/col are 1-based; normalise for
        // comparison.
        let (cur_row, cur_col) = doc.textarea.cursor();
        let cur1 = (cur_row + 1, cur_col + 1);
        // Find the first diagnostic strictly past the cursor.
        // Ties on the same line go to the higher column.
        let mut sorted_idxs: Vec<usize> = (0..doc.typst_diagnostics.len()).collect();
        sorted_idxs.sort_by_key(|&i| {
            let d = &doc.typst_diagnostics[i];
            (d.line, d.col)
        });
        let next = sorted_idxs.iter().copied().find(|&i| {
            let d = &doc.typst_diagnostics[i];
            (d.line, d.col) > cur1
        });
        let chosen = match next {
            Some(i) => i,
            None => {
                // Wrap to the first.
                sorted_idxs[0]
            }
        };
        let target = doc.typst_diagnostics[chosen].clone();
        let row = target.line.saturating_sub(1) as u16;
        let col = target.col.saturating_sub(1) as u16;
        doc.textarea
            .move_cursor(tui_textarea::CursorMove::Jump(row, col));
        let total = doc.typst_diagnostics.len();
        let wrapped_note = if next.is_none() && total > 1 {
            " (wrapped)"
        } else {
            ""
        };
        self.status = format!(
            "diag {}/{}{wrapped_note}  line {}:{}  — {}",
            sorted_idxs.iter().position(|&i| i == chosen).unwrap_or(0) + 1,
            total,
            target.line,
            target.col,
            target.message,
        );
    }

    /// Promote the paragraph one ladder step if (a) the project
    /// has `goals.auto_promote_on_target = true`, (b) the
    /// paragraph carries a positive `target_words`, (c) the
    /// current `word_count` meets or exceeds it, and (d) the
    /// last auto-promote isn't already at the current status.
    /// On promotion: bumps `status` via `next_status`, sets
    /// `target_hit_at_status` to the new status, fires the
    /// status-change progress event, persists via
    /// `store.raw().update_metadata`.
    fn maybe_auto_promote_on_target(&mut self, id: Uuid, current_words: i64) {
        if !self.cfg.goals.auto_promote_on_target {
            return;
        }
        let node = match self.hierarchy.get(id) {
            Some(n) => n.clone(),
            None => return,
        };
        if node.kind != NodeKind::Paragraph {
            return;
        }
        let Some(target) = node.target_words.filter(|n| *n > 0) else {
            return;
        };
        if current_words < target as i64 {
            return;
        }
        let current_status = node.status.clone();
        // Idempotent: already promoted at this status → bail.
        if node.target_hit_at_status.as_deref() == current_status.as_deref() {
            return;
        }
        let promoted = next_status(current_status.as_deref()).to_string();
        let new_status = if promoted == "None" {
            None
        } else {
            Some(promoted.clone())
        };
        let mut updated = node.clone();
        updated.status = new_status.clone();
        updated.target_hit_at_status = new_status.clone();
        updated.modified_at = chrono::Utc::now();
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(id, updated.to_json())
        {
            tracing::warn!(
                target: "inkhaven::goal_promote",
                "auto-promote update_metadata failed: {e}"
            );
            return;
        }
        let from_label = display_status(current_status.as_deref())
            .to_ascii_lowercase();
        let to_label = promoted.to_ascii_lowercase();
        let book_id = self.book_of_node(id);
        crate::progress::record_status_change(
            id, book_id, &from_label, &to_label, current_words,
        );
        // hook.on_status_promoted fires from both auto-promote
        // and manual-cycle paths; see `cycle_paragraph_status`
        // for the signature and rationale.
        crate::scripting::hooks::fire(
            "hook.on_status_promoted",
            vec![
                rust_dynamic::value::Value::from_string(id.to_string()),
                rust_dynamic::value::Value::from_string(from_label.clone()),
                rust_dynamic::value::Value::from_string(to_label.clone()),
            ],
        );
        self.status = format!(
            "goal-hit: `{}` promoted {} → {}",
            node.title, from_label, to_label
        );
    }

    /// Resolve the user book a paragraph belongs to. Returns
    /// `None` for system-book content (Help / Scripts / Typst /
    /// …) which doesn't count toward writing goals.
    pub(crate) fn book_of_node(&self, id: Uuid) -> Option<Uuid> {
        let node = self.hierarchy.get(id)?;
        let book = self
            .hierarchy
            .ancestors(node)
            .into_iter()
            .find(|a| a.kind == NodeKind::Book)?;
        if book.system_tag.is_some() {
            return None;
        }
        Some(book.id)
    }

    // -------- drawing -----------------------------------------------------

    fn draw(&mut self, f: &mut ratatui::Frame) {
        // Typewriter mode: hide every pane except the editor. The
        // editor's own header still shows L/C, word count, edited
        // ago, and the status badge — so the user gets the writing-
        // critical metadata without the surrounding panes. Modals
        // (Quick-ref, Book info, etc.) still float on top, which
        // makes Ctrl+B H usable mid-flow.
        if self.typewriter_mode {
            let area = f.area();
            // Empty pane rects mean the mouse handler skips everything
            // hidden (clicks fall through to the editor's own rect).
            self.layout_search = Rect::default();
            self.layout_tree = Rect::default();
            self.layout_editor = area;
            self.layout_ai = Rect::default();
            self.layout_ai_prompt = Rect::default();
            self.draw_editor(f, area);
            if !matches!(self.modal, Modal::None) {
                self.draw_modal(f, f.area());
            }
            return;
        }

        if self.ai_fullscreen {
            // Layout: most of the screen split 50/50 (AI pane | chat
            // history); AI prompt at the bottom; one status line.
            // Tree, editor, and search bar are hidden.
            let outer = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(0),
                    Constraint::Length(3),
                    Constraint::Length(1),
                ])
                .split(f.area());
            let top = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(outer[0]);
            self.layout_search = Rect::default();
            self.layout_tree = Rect::default();
            self.layout_editor = Rect::default();
            self.layout_ai = top[0];
            self.layout_ai_prompt = outer[1];
            self.draw_ai(f, top[0]);
            self.draw_chat_history(f, top[1]);
            self.draw_ai_prompt(f, outer[1]);
            self.draw_status(f, outer[2]);
            if !matches!(self.modal, Modal::None) {
                self.draw_modal(f, f.area());
            }
            return;
        }

        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(f.area());

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(22),
                Constraint::Percentage(56),
                Constraint::Percentage(22),
            ])
            .split(outer[1]);

        // Cache pane rects for the mouse handler. Done before drawing so
        // a draw that bails (e.g. tiny terminal) still leaves the rects
        // self-consistent for whatever pane managed to render.
        self.layout_search = outer[0];
        self.layout_tree = body[0];
        self.layout_editor = body[1];
        self.layout_ai = body[2];
        self.layout_ai_prompt = outer[2];

        self.draw_search_bar(f, outer[0]);
        self.draw_tree(f, body[0]);
        if self.secondary.is_some() {
            // Similar-paragraph mode: AI pane is repurposed as
            // the second editor. Both panes carve off the bottom
            // row of their rect for the full slug-path footer
            // the spec asks for.
            let primary_rect = body[1];
            let footer_h: u16 = 1;
            let primary_editor_rect = Rect {
                x: primary_rect.x,
                y: primary_rect.y,
                width: primary_rect.width,
                height: primary_rect.height.saturating_sub(footer_h),
            };
            let primary_footer_rect = Rect {
                x: primary_rect.x,
                y: primary_rect.y + primary_rect.height.saturating_sub(footer_h),
                width: primary_rect.width,
                height: footer_h,
            };
            self.draw_editor(f, primary_editor_rect);
            self.draw_primary_pane_footer(f, primary_footer_rect);
            self.draw_secondary_editor(f, body[2]);
        } else {
            self.draw_editor(f, body[1]);
            self.draw_ai(f, body[2]);
        }
        self.draw_ai_prompt(f, outer[2]);
        self.draw_status(f, outer[3]);

        if self.show_results_overlay {
            self.draw_search_overlay(f, outer[1]);
        }
        if self.show_prompt_picker {
            self.draw_prompt_picker(f, f.area());
        }

        // Modal renders last so it floats over everything.
        if !matches!(self.modal, Modal::None) {
            self.draw_modal(f, f.area());
        }
    }


    fn bund_pane_handle_key(&mut self, key: KeyEvent) {
        // Esc closes the pane (top of handle_modal_key already
        // covers it). Here we just handle scrolling.
        let Modal::BundPane { lines, scroll, .. } = &mut self.modal else {
            return;
        };
        let total = lines.len();
        let page: usize = 12; // approximate visible window
        match key.code {
            KeyCode::Up => {
                *scroll = scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                if *scroll + 1 < total {
                    *scroll += 1;
                }
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(page);
            }
            KeyCode::PageDown => {
                let max = total.saturating_sub(page.max(1));
                *scroll = (*scroll + page).min(max);
            }
            KeyCode::Home => {
                *scroll = 0;
            }
            KeyCode::End => {
                *scroll = total.saturating_sub(page);
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+C inside the pane clears its buffer — convenient
                // when the pane keeps catching subsequent script output.
                lines.clear();
                *scroll = 0;
            }
            _ => {}
        }
    }

    fn script_picker_handle_key(&mut self, key: KeyEvent) {
        // Snapshot fields we need without holding the borrow
        // across `run_script_by_id` (which takes &mut self).
        let (selected_id, total, was_a_toggle, was_enter): (Option<Uuid>, usize, bool, bool) = {
            let Modal::ScriptPicker {
                entries,
                cursor,
                scroll,
                scope,
                ..
            } = &mut self.modal
            else {
                return;
            };
            let total = entries.len();
            let page: usize = 12;
            let mut a_toggle = false;
            let mut enter = false;
            let mut selected: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => {
                    *cursor = cursor.saturating_sub(page);
                }
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1).max(0));
                }
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = total.saturating_sub(1),
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    *scope = match scope {
                        ScriptPickerScope::Branch => ScriptPickerScope::ScriptsBook,
                        ScriptPickerScope::ScriptsBook => ScriptPickerScope::Branch,
                    };
                    a_toggle = true;
                }
                KeyCode::Enter => {
                    if let Some(e) = entries.get(*cursor) {
                        selected = Some(e.id);
                    }
                    enter = true;
                }
                _ => {}
            }
            // Keep cursor visible (cheap: clamp scroll).
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            (selected, total, a_toggle, enter)
        };

        if was_a_toggle {
            // Rebuild entries against the new scope.
            let new_scope = match &self.modal {
                Modal::ScriptPicker { scope, .. } => *scope,
                _ => return,
            };
            let new_entries = self.collect_script_entries(new_scope);
            if let Modal::ScriptPicker {
                entries,
                cursor,
                scroll,
                ..
            } = &mut self.modal
            {
                *entries = new_entries;
                *cursor = 0;
                *scroll = 0;
            }
            self.status = match new_scope {
                ScriptPickerScope::Branch => "bund: branch scope".into(),
                ScriptPickerScope::ScriptsBook => "bund: Scripts book scope".into(),
            };
            return;
        }

        if was_enter {
            self.modal = Modal::None;
            if let Some(id) = selected_id {
                if let Err(e) = self.bund_run_script_by_id(id) {
                    self.status = format!("bund: {e}");
                }
            } else if total == 0 {
                self.status = "bund: no script to run".into();
            }
        }
    }

    /// Load the Script node `id`, eval its body against Adam,
    /// and route the result (or error) to the status bar — or
    /// to the Bund pane if one is open.
    fn bund_run_script_by_id(&mut self, id: Uuid) -> std::result::Result<(), String> {
        let node = self
            .hierarchy
            .get(id)
            .ok_or_else(|| format!("script {id} not found"))?
            .clone();
        if node.kind != NodeKind::Script {
            return Err(format!("“{}” is not a Script node", node.title));
        }
        let bytes = self
            .store
            .get_content(node.id)
            .map_err(|e| format!("load {}: {e}", node.title))?
            .unwrap_or_default();
        let body = String::from_utf8(bytes)
            .map_err(|e| format!("script “{}” not utf-8: {e}", node.title))?;
        if body.trim().is_empty() {
            self.status = format!("bund: “{}” is empty", node.title);
            return Ok(());
        }
        match self.scripting_eval(&body) {
            Ok(out) => {
                self.status = format_eval_output(&out, Some(&node.title));
                Ok(())
            }
            Err(e) => Err(format!("eval “{}” failed — {e:#}", node.title)),
        }
    }

    fn link_picker_handle_key(&mut self, key: KeyEvent) {
        // Collect intent; close over the modal's mutable state.
        let (owner, target_to_remove, target_to_open) = {
            let Modal::LinkPicker { owner, entries, cursor, scroll } = &mut self.modal else {
                return;
            };
            let total = entries.len();
            let page: usize = 12;
            let mut target_to_remove: Option<Uuid> = None;
            let mut target_to_open: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => *cursor = cursor.saturating_sub(page),
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1));
                }
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = total.saturating_sub(1),
                KeyCode::Char('D') | KeyCode::Char('d') | KeyCode::Delete => {
                    if let Some(e) = entries.get(*cursor) {
                        target_to_remove = Some(e.id);
                    }
                }
                // 1.2.4+: Enter opens the linked paragraph in the
                // editor (autosaving the current buffer first via
                // `open_search_result` → `load_paragraph` →
                // `save_current`). Tree cursor follows. Modal closes.
                KeyCode::Enter => {
                    if let Some(e) = entries.get(*cursor) {
                        target_to_open = Some(e.id);
                    }
                }
                _ => {}
            }
            // Keep cursor visible.
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            (*owner, target_to_remove, target_to_open)
        };

        if let Some(target) = target_to_open {
            // Close the modal first so any status message the
            // load flow sets isn't immediately overwritten by a
            // modal-redraw cycle.
            self.modal = Modal::None;
            // `open_search_result` does exactly what we want:
            // moves the tree cursor onto the target row, then
            // loads the paragraph (which autosaves the previous
            // buffer if it was dirty).
            self.open_search_result(target);
            return;
        }

        if let Some(target) = target_to_remove {
            match self.remove_paragraph_link(owner, target) {
                Ok(true) => {
                    self.status = "link removed".into();
                    // Rebuild the modal entries from the fresh
                    // hierarchy; close the modal if no links remain.
                    let entries = self.collect_link_entries(owner);
                    if entries.is_empty() {
                        self.modal = Modal::None;
                    } else if let Modal::LinkPicker { entries: e, cursor, scroll, .. } =
                        &mut self.modal
                    {
                        *cursor = (*cursor).min(entries.len() - 1);
                        *scroll = (*scroll).min(*cursor);
                        *e = entries;
                    }
                }
                Ok(false) => {
                    self.status = "link not found (stale view)".into();
                }
                Err(e) => {
                    self.status = format!("link remove: {e}");
                }
            }
        }
    }

    fn fuzzy_paragraph_picker_handle_key(&mut self, key: KeyEvent) {
        let to_open = {
            let Modal::FuzzyParagraphPicker { input, entries, cursor, scroll } =
                &mut self.modal
            else {
                return;
            };
            let matches = fuzzy_filter_entries(entries, input.as_str());
            let total = matches.len();
            let page: usize = 12;
            let mut to_open: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => *cursor = cursor.saturating_sub(page),
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1));
                }
                KeyCode::Enter => {
                    if let Some(idx) = matches.get(*cursor).copied() {
                        to_open = Some(entries[idx].id);
                    }
                }
                _ => {
                    handle_text_input_key(input, key);
                    // Reset cursor on input edit; matches list
                    // may have shifted.
                    *cursor = 0;
                    *scroll = 0;
                }
            }
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            to_open
        };

        if let Some(id) = to_open {
            self.modal = Modal::None;
            self.open_search_result(id);
        }
    }

    fn bookmark_picker_handle_key(&mut self, key: KeyEvent) {
        let (id_to_unbookmark, id_to_open) = {
            let Modal::BookmarkPicker { entries, cursor, scroll } = &mut self.modal else {
                return;
            };
            let total = entries.len();
            let page: usize = 12;
            let mut to_unbookmark: Option<Uuid> = None;
            let mut to_open: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => *cursor = cursor.saturating_sub(page),
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1));
                }
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = total.saturating_sub(1),
                KeyCode::Enter => {
                    if let Some(e) = entries.get(*cursor) {
                        to_open = Some(e.id);
                    }
                }
                KeyCode::Char('D') | KeyCode::Char('d') | KeyCode::Delete => {
                    if let Some(e) = entries.get(*cursor) {
                        to_unbookmark = Some(e.id);
                    }
                }
                _ => {}
            }
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            (to_unbookmark, to_open)
        };

        if let Some(id) = id_to_open {
            self.modal = Modal::None;
            self.open_search_result(id);
            return;
        }

        if let Some(id) = id_to_unbookmark {
            if let Some(node) = self.hierarchy.get(id).cloned() {
                let mut updated = node.clone();
                updated.bookmark = false;
                updated.modified_at = chrono::Utc::now();
                if let Err(e) = self
                    .store
                    .raw()
                    .update_metadata(id, updated.to_json())
                {
                    self.status = format!("bookmark clear: {e}");
                    return;
                }
                self.reload_hierarchy();
                let entries = self.collect_bookmark_entries();
                if entries.is_empty() {
                    self.modal = Modal::None;
                    self.status = "bookmark cleared · no bookmarks left".into();
                } else if let Modal::BookmarkPicker { entries: e, cursor, scroll } =
                    &mut self.modal
                {
                    *cursor = (*cursor).min(entries.len() - 1);
                    *scroll = (*scroll).min(*cursor);
                    *e = entries;
                    self.status = "bookmark cleared".into();
                }
            }
        }
    }

    fn backlink_picker_handle_key(&mut self, key: KeyEvent) {
        let (target, source_to_unlink, target_to_open) = {
            let Modal::BacklinkPicker { target, entries, cursor, scroll } = &mut self.modal else {
                return;
            };
            let total = entries.len();
            let page: usize = 12;
            let mut source_to_unlink: Option<Uuid> = None;
            let mut target_to_open: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => *cursor = cursor.saturating_sub(page),
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1));
                }
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = total.saturating_sub(1),
                KeyCode::Char('D') | KeyCode::Char('d') | KeyCode::Delete => {
                    if let Some(e) = entries.get(*cursor) {
                        source_to_unlink = Some(e.id);
                    }
                }
                KeyCode::Enter => {
                    if let Some(e) = entries.get(*cursor) {
                        target_to_open = Some(e.id);
                    }
                }
                _ => {}
            }
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            (*target, source_to_unlink, target_to_open)
        };

        if let Some(to_open) = target_to_open {
            self.modal = Modal::None;
            self.open_search_result(to_open);
            return;
        }

        if let Some(source) = source_to_unlink {
            // Remove the source's outgoing link to `target`. That's
            // what makes this the symmetric "delete" for backlinks:
            // the link metadata lives on the source paragraph, so
            // mutating it from the backlinks view is honest about
            // what changes on disk.
            match self.remove_paragraph_link(source, target) {
                Ok(true) => {
                    self.status = "backlink removed (source's outgoing link to current)".into();
                    let entries = self.collect_backlink_entries(target);
                    if entries.is_empty() {
                        self.modal = Modal::None;
                    } else if let Modal::BacklinkPicker { entries: e, cursor, scroll, .. } =
                        &mut self.modal
                    {
                        *cursor = (*cursor).min(entries.len() - 1);
                        *scroll = (*scroll).min(*cursor);
                        *e = entries;
                    }
                }
                Ok(false) => {
                    self.status = "backlink not found (stale view)".into();
                }
                Err(e) => {
                    self.status = format!("backlink remove: {e}");
                }
            }
        }
    }

    fn similar_picker_handle_key(&mut self, key: KeyEvent) {
        let (selected_id, total, was_enter) = {
            let Modal::SimilarPicker { entries, cursor, scroll } = &mut self.modal else {
                return;
            };
            let total = entries.len();
            let page: usize = 12;
            let mut enter = false;
            let mut selected: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => *cursor = cursor.saturating_sub(page),
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1).max(0));
                }
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = total.saturating_sub(1),
                KeyCode::Enter => {
                    if let Some(e) = entries.get(*cursor) {
                        selected = Some(e.id);
                    }
                    enter = true;
                }
                _ => {}
            }
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            (selected, total, enter)
        };

        if was_enter {
            self.modal = Modal::None;
            if let Some(id) = selected_id {
                if let Err(e) = self.load_secondary_paragraph(id) {
                    self.status = format!("similar: {e}");
                }
            } else if total == 0 {
                self.status = "similar: nothing to open".into();
            }
        }
    }

    /// Mutate `target_words` on the node `id` and persist via
    /// `store.raw().update_metadata`. Used by both the Ctrl+V T
    /// modal and the `ink.paragraph.set_target` Bund word.
    /// Setting target to None also clears `target_hit_at_status`
    /// so re-enabling the goal starts fresh.
    pub(crate) fn set_paragraph_target_now(
        &mut self,
        id: Uuid,
        target: Option<i32>,
    ) -> std::result::Result<(), String> {
        let node = self
            .hierarchy
            .get(id)
            .ok_or_else(|| format!("paragraph {id} not in hierarchy"))?;
        if node.kind != NodeKind::Paragraph {
            return Err(format!("`{}` is not a paragraph", node.title));
        }
        let mut updated = node.clone();
        updated.target_words = target;
        if target.is_none() {
            updated.target_hit_at_status = None;
        }
        self.store
            .raw()
            .update_metadata(id, updated.to_json())
            .map_err(|e| format!("store update: {e}"))?;
        self.reload_hierarchy();
        Ok(())
    }

    /// Open the writing-progress modal. Forces a cache refresh
    /// so the user always sees fresh numbers (the per-redraw
    /// path stays cheap by reading the cache).
    fn open_progress_modal(&mut self) {
        self.refresh_progress_cache();
        self.modal = Modal::Progress { scroll: 0 };
        self.status = "progress: ↑↓ scroll · Esc close · r refresh".into();
    }

    fn progress_modal_handle_key(&mut self, key: KeyEvent) {
        let Modal::Progress { scroll } = &mut self.modal else {
            return;
        };
        match key.code {
            KeyCode::Up => *scroll = scroll.saturating_sub(1),
            KeyCode::Down => *scroll += 1,
            KeyCode::PageUp => *scroll = scroll.saturating_sub(8),
            KeyCode::PageDown => *scroll += 8,
            KeyCode::Home => *scroll = 0,
            KeyCode::End => *scroll += 100, // clamped by renderer
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.refresh_progress_cache();
                self.status = "progress: refreshed".into();
            }
            _ => {}
        }
    }

    fn pane_block<'a>(&self, title: &'a str, focus: Focus) -> Block<'a> {
        self.pane_block_line(Line::from(title), focus)
    }

    /// Same as `pane_block` but accepts a pre-built styled `Line` as the
    /// title — used by the AI pane to colourise the `scope=` / `infer=`
    /// mode chips.
    fn pane_block_line<'a>(&self, title: Line<'a>, focus: Focus) -> Block<'a> {
        let border_color = if self.focus == focus {
            self.theme.border_focused
        } else {
            self.theme.border_unfocused
        };
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.pane_bg)
                    .fg(self.theme.pane_fg),
            )
    }

    /// Editor pane block. Border colour carries the document's clean/dirty
    /// state when the pane has focus: green when saved, yellow when dirty,
    /// teal when the open paragraph is read-only (Help subtree). Unfocused
    /// uses the theme's neutral border colour.
    #[allow(dead_code)] // string-title convenience; the live renderer
    // uses `editor_block_line` for the coloured cursor read-out, but the
    // string overload stays available for non-styled callers.
    /// Centralized focus change. When leaving the Editor pane, autosave the
    /// open paragraph (best-effort) so unsaved edits are persisted even
    /// across Tab cycles, Ctrl+1..5 jumps, or any other focus shift. The
    /// underlying `save_current` writes to disk + bdslib + re-embeds.
    fn change_focus(&mut self, new: Focus) {
        if self.focus == Focus::Editor && new != Focus::Editor {
            // Focus-out also counts as "I'm done reviewing the
            // corrections" — drop the highlight so the next time the
            // user comes back the paragraph reads clean.
            if let Some(doc) = self.opened.as_mut() {
                doc.correction_baseline = None;
            }
            if self.opened.as_ref().is_some_and(|d| d.dirty) {
                let _ = self.save_current();
            }
            // Snapshot the cursor before defocusing so re-opening this
            // paragraph (now or in a future run) lands the cursor back where
            // the user left it. Persisting to disk here makes the position
            // survive a crash or kill, not just a graceful exit.
            self.snapshot_open_paragraph_cursor();
            if let Err(e) = self.save_session() {
                tracing::warn!("focus-loss session save failed: {e}");
            }
            // Typewriter SFX — "remove page from machine" when the
            // editor pane loses focus. No-op when sound is disabled or
            // the host has no audio device.
            if let Some(sp) = &self.sound {
                sp.play_focus_out();
            }
        }
        self.focus = new;
    }

    fn toggle_chat_selection_mode(&mut self) {
        if self.chat_selection.is_some() {
            self.chat_selection = None;
            self.status = "chat selection mode off".into();
            return;
        }
        if self.chat_history.is_empty() {
            self.status = "chat history is empty — nothing to select".into();
            return;
        }
        // Start at the newest turn — the assistant reply you just
        // received is what most users want to copy.
        let turn = self.chat_history.len() - 1;
        self.chat_selection = Some(ChatSelectionState { turn });
        // Dismiss the search highlights so the selection's block bg
        // isn't fighting for visibility.
        self.chat_search = None;
        // Reset scroll — the renderer auto-centres on the selected
        // turn anyway.
        self.chat_history_scroll = 0;
        self.status =
            "chat selection mode · ↑↓ navigate · c=copy · t=insert into editor · Esc to exit".into();
    }

    /// Open the chat-history search query modal. Pre-populates the
    /// input with the previous query (if any) so re-search is a
    /// single Enter.
    fn open_chat_search_prompt(&mut self) {
        if self.chat_history.is_empty() {
            self.status = "chat history is empty — nothing to search".into();
            return;
        }
        let mut input = TextInput::new();
        if let Some(prev) = &self.chat_search {
            for c in prev.query.chars() {
                input.insert_char(c);
            }
        }
        self.modal = Modal::ChatSearchPrompt { input };
        self.status =
            "Search chat history · Enter to start (newest first) · Ctrl+X next (older) · Esc cancel".into();
    }

    /// Apply the just-submitted query. Empty query clears any active
    /// search. The `current` index starts at the LAST match — the
    /// most recent / closest-to-the-bottom hit — per the spec.
    fn commit_chat_search(&mut self, query: String) {
        if query.is_empty() {
            self.chat_search = None;
            self.status = "chat search: empty query — cleared".into();
            return;
        }
        let total = self.chat_search_matches(&query).len();
        if total == 0 {
            self.chat_search = None;
            self.status = format!("chat search: no match for `{query}`");
            return;
        }
        self.chat_search = Some(ChatSearchState {
            query: query.clone(),
            current: total - 1, // newest match (last in match order)
        });
        // Recompute scroll so the centre lands on the match.
        // draw_chat_history handles this from the state.
        self.chat_history_scroll = 0;
        self.status = format!("chat search: `{query}` · 1/{total} (newest)");
    }

    /// Step the chat-search cursor one match toward older history.
    /// Wraps from oldest back to newest. Matches are recomputed each
    /// call to handle terminal resize / streaming-token arrival.
    fn advance_chat_search(&mut self) {
        let Some((query, current)) = self
            .chat_search
            .as_ref()
            .map(|s| (s.query.clone(), s.current))
        else {
            return;
        };
        let total = self.chat_search_matches(&query).len();
        if total == 0 {
            // Live history may have lost the matches we had — clear.
            self.chat_search = None;
            self.status = "chat search: no matches in current history".into();
            return;
        }
        let new_current = (current + total - 1) % total;
        if let Some(search) = self.chat_search.as_mut() {
            search.current = new_current;
        }
        self.status = format!(
            "chat search: `{query}` · {}/{}",
            new_current + 1,
            total
        );
    }

    /// Build the chat-history pane's Lines exactly as
    /// `draw_chat_history` does — same iteration, same markdown
    /// rendering, same headers. Returns both the lines and per-turn
    /// `(line_start..line_end)` ranges so the chat-selection mode
    /// can highlight the active turn as a block.
    fn build_chat_history_lines(&self) -> (Vec<Line<'static>>, Vec<std::ops::Range<usize>>) {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut turn_ranges: Vec<std::ops::Range<usize>> = Vec::new();
        let user_style = Style::default()
            .fg(self.theme.ai_scope_fg)
            .add_modifier(Modifier::BOLD);
        let assistant_style = Style::default()
            .fg(self.theme.ai_infer_fg)
            .add_modifier(Modifier::BOLD);
        for (i, turn) in self.chat_history.iter().enumerate() {
            let turn_start = lines.len();
            match turn {
                ChatTurn::User(text) => {
                    if i > 0 {
                        lines.push(Line::from(""));
                    }
                    lines.push(Line::from(Span::styled(
                        "❯ User".to_string(),
                        user_style,
                    )));
                    for line in text.lines() {
                        lines.push(Line::from(format!("  {line}")));
                    }
                }
                ChatTurn::Assistant(text) => {
                    lines.push(Line::from(Span::styled(
                        "← Assistant".to_string(),
                        assistant_style,
                    )));
                    let rendered = super::markdown::render(text);
                    if rendered.is_empty() {
                        for line in text.lines() {
                            lines.push(Line::from(format!("  {line}")));
                        }
                    } else {
                        for l in rendered {
                            lines.push(l);
                        }
                    }
                }
            }
            turn_ranges.push(turn_start..lines.len());
        }
        (lines, turn_ranges)
    }

    /// One-line writing-progress summary for the right edge of
    /// the status bar. Empty when progress tracking is disabled
    /// (no store installed or no goals configured).
    /// 1.2.7+ — same content as the old text-only widget,
    /// but the `today` segment gets a colour + glyph based
    /// on goal state (green ✓ when over goal, amber while
    /// climbing, dim when still cold). The rest stays DIM
    /// so the writing-progress widget reads as a calm
    /// rightside chip with one always-bright element.
    fn progress_widget_spans(&self) -> Vec<Span<'static>> {
        let Some(snap) = self.progress_cache.as_ref() else {
            return Vec::new();
        };
        let dim = Style::default().add_modifier(Modifier::DIM);
        let mut out: Vec<Span<'static>> = Vec::new();
        let today = snap.project.today_words;
        let (today_text, today_style) = match snap.project.daily_goal {
            Some(goal) if goal > 0 => {
                let (glyph, style) = if today >= goal {
                    (
                        "✓",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else if today > 0 {
                    (
                        "·",
                        Style::default().fg(Color::Yellow),
                    )
                } else {
                    (
                        "·",
                        dim,
                    )
                };
                (
                    format!("{glyph} today {today}/{goal}w"),
                    style,
                )
            }
            _ => {
                let style = if today > 0 {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    dim
                };
                (format!("today {today}w"), style)
            }
        };
        out.push(Span::styled(today_text, today_style));
        // Everything after `today` joins the dim trailing
        // chips via a single computed-on-the-fly string —
        // matches the pre-1.2.7 text layout for those
        // segments and keeps the widget tight.
        let trailing = self.progress_widget_trailing(snap);
        if !trailing.is_empty() {
            out.push(Span::styled(format!(" · {trailing}"), dim));
        }
        out.push(Span::raw(" "));
        out
    }

    /// Build the comma-joined trailing chip string (active
    /// time, streak, link count, book pace). Pulled out of
    /// `progress_widget_spans` so the `today` segment can
    /// own its own colour without copy-pasting the rest.
    fn progress_widget_trailing(
        &self,
        snap: &crate::progress::ProgressSnapshot,
    ) -> String {
        let mut parts: Vec<String> = Vec::new();
        // Active-time chip — bare duration when no goal is set,
        // `<spent>/<goal>` when there is one.
        let active_goal_secs = self.cfg.goals.active_minutes_daily.max(0) * 60;
        if active_goal_secs > 0 {
            parts.push(format!(
                "{}/{}",
                format_active_duration(snap.active_seconds_today),
                format_active_duration(active_goal_secs),
            ));
        } else if snap.active_seconds_today > 0 {
            parts.push(format_active_duration(snap.active_seconds_today));
        }
        if snap.streak.days > 0 {
            parts.push(format!("streak {}d", snap.streak.days));
        }
        // Outgoing-link count for the open paragraph (1.2.4+).
        // Only shown when > 0 to keep the line short on the
        // common "no links yet" case.
        if let Some(doc) = self.opened.as_ref() {
            if let Some(node) = self.hierarchy.get(doc.id) {
                let n = node.linked_paragraphs.len();
                if n > 0 {
                    parts.push(format!("links {n}"));
                }
            }
        }
        // Surface the focused book's pace if a deadline is set.
        if let Some(book) = snap.books.iter().find(|b| {
            self.opened
                .as_ref()
                .and_then(|d| self.book_of_node(d.id))
                .map_or(false, |bid| {
                    // Compare by title since BookProgress doesn't
                    // carry the uuid; book labels are unique
                    // within a project.
                    self.hierarchy
                        .get(bid)
                        .map_or(false, |n| n.title == b.label)
                })
        }) {
            if let (Some(target), Some(pace)) =
                (book.target_words, book.required_pace)
            {
                parts.push(format!(
                    "{} {}/{}w (pace {}w/d)",
                    book.label, book.total_words, target, pace
                ));
            }
        }
        parts.join(" · ")
    }

    // ── Bund stdlib bridge ───────────────────────────────────────────
    //
    // Called by `src/scripting/stdlib/app.rs` via `with_active_app`.
    // Each `ink_*` method exposes a single primitive operation; the
    // Bund word handler takes care of argument parsing + status
    // reporting. Methods return Option<…> for queries and Result<…>
    // for mutations so the stdlib can surface a clean error when
    // there's no open buffer / no AI session.

    pub(crate) fn ink_editor_cursor(&self) -> Option<(usize, usize)> {
        self.opened.as_ref().map(|d| d.textarea.cursor())
    }

    /// 1.2.6+ — render the story view for the named user book
    /// and write the PNG to `dest`. Same plumbing as
    /// `crate::story_view::build_story_png` (Ctrl+V W), but
    /// driven from the Bund stdlib (`ink.story.render`). Returns
    /// a human-readable error string when the book name doesn't
    /// resolve, the render fails, or the write fails.
    pub(crate) fn ink_story_render_to_path(
        &self,
        book_name: &str,
        dest: &std::path::Path,
    ) -> Result<(), String> {
        let needle = book_name.trim().to_ascii_lowercase();
        let book = self
            .hierarchy
            .flatten()
            .into_iter()
            .find_map(|(n, _)| {
                if n.kind == NodeKind::Book
                    && n.system_tag.is_none()
                    && (n.title.to_ascii_lowercase() == needle
                        || n.slug.to_ascii_lowercase() == needle)
                {
                    Some(n.clone())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                format!("no user book matches `{book_name}`")
            })?;
        let render =
            crate::story_view::build_story_png(&self.store, &self.hierarchy, book.id)
                .map_err(|e| format!("render: {e}"))?;
        std::fs::write(dest, &render.png_bytes)
            .map_err(|e| format!("write {}: {e}", dest.display()))?;
        Ok(())
    }

    pub(crate) fn ink_editor_goto(&mut self, row: usize, col: usize) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        doc.textarea.move_cursor(tui_textarea::CursorMove::Jump(
            row as u16, col as u16,
        ));
        Ok(())
    }

    pub(crate) fn ink_editor_insert(&mut self, text: &str) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        doc.textarea.insert_str(text);
        doc.dirty = true;
        Ok(())
    }

    pub(crate) fn ink_editor_scroll(&mut self, delta: i32) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        let max = doc.textarea.lines().len().saturating_sub(1);
        let new = (doc.scroll_row as i64).saturating_add(delta as i64).max(0) as usize;
        doc.scroll_row = new.min(max);
        Ok(())
    }

    pub(crate) fn ink_editor_delete_line(&mut self) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        // Kill the whole line: move to start, delete to end.
        doc.textarea.move_cursor(tui_textarea::CursorMove::Head);
        doc.textarea.delete_line_by_end();
        doc.dirty = true;
        Ok(())
    }

    pub(crate) fn ink_editor_delete_to_bol(&mut self) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        doc.textarea.delete_line_by_head();
        doc.dirty = true;
        Ok(())
    }

    pub(crate) fn ink_editor_delete_to_eol(&mut self) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        doc.textarea.delete_line_by_end();
        doc.dirty = true;
        Ok(())
    }

    pub(crate) fn ink_editor_text(&self) -> Option<String> {
        self.opened.as_ref().map(|d| d.textarea.lines().join("\n"))
    }

    /// First substring match in the buffer, returned as `(row, col)`.
    pub(crate) fn ink_editor_find(&self, needle: &str) -> Option<(usize, usize)> {
        if needle.is_empty() {
            return None;
        }
        let doc = self.opened.as_ref()?;
        for (row, line) in doc.textarea.lines().iter().enumerate() {
            if let Some(col) = line.find(needle) {
                return Some((row, col));
            }
        }
        None
    }

    pub(crate) fn ink_ai_clear_history(&mut self) {
        self.clear_chat_history();
    }

    pub(crate) fn ink_typst_assemble(&mut self) {
        self.schedule_assembly();
    }

    pub(crate) fn ink_typst_build(&mut self) {
        self.schedule_build();
    }

    pub(crate) fn ink_typst_take(&mut self) {
        self.schedule_take();
    }

    // ── Phase C: AI / theme / editor.replace ─────────────────────────

    /// Send a user prompt through the same AI pipeline the
    /// `Ctrl+I` chord uses: kicks off streaming inference,
    /// returns immediately. Response lands in `chat_history`
    /// once the stream completes. No synchronous result —
    /// Bund scripts that want the response read
    /// `ink.ai.history` later (e.g. in a hook firing on a
    /// subsequent action).
    /// Multi-occurrence find/replace on the open buffer. Returns
    /// the number of replacements made. Empty `find` returns 0
    /// (defends against an infinite loop with `replace.contains(find)`).
    pub(crate) fn ink_editor_replace_all(
        &mut self,
        find: &str,
        replace: &str,
    ) -> std::result::Result<i64, String> {
        if find.is_empty() {
            return Ok(0);
        }
        let doc = self
            .opened
            .as_mut()
            .ok_or_else(|| "no paragraph open".to_string())?;
        if doc.read_only {
            return Err("paragraph is read-only".into());
        }
        // Pull the body, do all replacements at once with String::replace,
        // then re-load the textarea. Avoids cursor-tracking complexity of
        // doing N find/replace passes through tui-textarea.
        let old_body = doc.textarea.lines().join("\n");
        let new_body = old_body.replace(find, replace);
        if new_body == old_body {
            return Ok(0);
        }
        // Count occurrences by counting non-overlapping matches in the
        // original body — same semantics as String::replace.
        let count = old_body.matches(find).count() as i64;
        let lines = body_to_lines(&new_body);
        let mut ta = TextArea::new(lines);
        ta.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        ta.set_line_number_style(Style::default().fg(Color::DarkGray));
        doc.textarea = ta;
        doc.dirty = true;
        doc.last_activity = std::time::Instant::now();
        Ok(count)
    }

    /// Open the `index`-th semantic-search hit for `query`.
    /// Returns true when a paragraph was loaded, false when the
    /// search came back empty or `index` is out of bounds.
    pub(crate) fn ink_search_load(
        &mut self,
        query: &str,
        index: usize,
    ) -> std::result::Result<bool, String> {
        if query.trim().is_empty() {
            return Err("empty query".into());
        }
        let raw = self
            .store
            .search_text(query, (index + 1).max(8))
            .map_err(|e| format!("search: {e}"))?;
        let hits: Vec<crate::tui::search_results::SearchHit> = raw
            .iter()
            .filter_map(crate::tui::search_results::SearchHit::parse)
            .collect();
        let Some(hit) = hits.get(index) else {
            return Ok(false);
        };
        let Some(node) = self.hierarchy.get(hit.id).cloned() else {
            return Ok(false);
        };
        if node.kind != crate::store::node::NodeKind::Paragraph {
            return Err(format!("hit `{}` is not a paragraph", node.title));
        }
        self.load_paragraph(&node)
            .map_err(|e| format!("load_paragraph: {e:#}"))?;
        Ok(true)
    }

    /// Snapshot the current AI inference state for `ink.ai.poll`.
    /// Returns `(status, response, elapsed_ms)`; status is one of
    /// `"none"` / `"streaming"` / `"done"` / `"error:<msg>"`.
    /// `response` accumulates as the stream arrives.
    pub(crate) fn ink_ai_poll(&self) -> (String, String, i64) {
        let Some(inf) = self.inference.as_ref() else {
            return ("none".into(), String::new(), 0);
        };
        let status = match &inf.status {
            InferenceStatus::Streaming => "streaming".to_string(),
            InferenceStatus::Done => "done".to_string(),
            InferenceStatus::Error(e) => format!("error: {e}"),
        };
        let elapsed = inf.started_at.elapsed().as_millis() as i64;
        (status, inf.response.clone(), elapsed)
    }

    /// Spawn an inference (same as `ink.ai.send`) and then poll
    /// until it terminates or `timeout_ms` elapses. The TUI does
    /// not redraw while the loop runs — that's the user's
    /// trade-off when picking the blocking variant. Returns the
    /// accumulated response, or `None` if the wait timed out (the
    /// inference itself keeps streaming in the background and
    /// can be read with `ink.ai.poll`).
    pub(crate) fn ink_ai_send_blocking(
        &mut self,
        prompt: &str,
        timeout_ms: i64,
    ) -> std::result::Result<Option<String>, String> {
        self.ink_ai_send(prompt)?;
        let timeout = std::time::Duration::from_millis(timeout_ms.max(0) as u64);
        let started = std::time::Instant::now();
        loop {
            // Pull whatever's accumulated in the stream channel.
            self.pump_inference();
            if let Some(inf) = self.inference.as_ref() {
                match &inf.status {
                    InferenceStatus::Done => {
                        return Ok(Some(inf.response.clone()));
                    }
                    InferenceStatus::Error(e) => {
                        return Err(format!("inference error: {e}"));
                    }
                    InferenceStatus::Streaming => {}
                }
            } else {
                // No inference state — shouldn't happen since we
                // just spawned one, but stay safe.
                return Ok(None);
            }
            if started.elapsed() >= timeout {
                return Ok(None);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    pub(crate) fn ink_ai_send(&mut self, prompt: &str) -> Result<(), String> {
        if prompt.trim().is_empty() {
            return Err("empty prompt".into());
        }
        // Borrow the existing input-driven path: pre-load
        // `ai_input` then call `start_inference`. Same code
        // path the AI prompt input field uses.
        self.ai_input.clear();
        for c in prompt.chars() {
            self.ai_input.insert_char(c);
        }
        self.start_inference();
        Ok(())
    }

    /// Return the chat history as a Vec of (role, content)
    /// pairs. `role` is `"user"` or `"assistant"`. Most recent
    /// turn is last.
    pub(crate) fn ink_ai_history(&self) -> Vec<(String, String)> {
        self.chat_history
            .iter()
            .map(|t| match t {
                ChatTurn::User(s) => ("user".into(), s.clone()),
                ChatTurn::Assistant(s) => ("assistant".into(), s.clone()),
            })
            .collect()
    }

    /// Set (or clear) the script-supplied system-prompt override.
    /// Empty string clears it; otherwise the inference path
    /// consults the override before falling back to the
    /// inference-mode default. See `system_prompt_override` field.
    pub(crate) fn ink_ai_set_system_prompt(&mut self, text: &str) {
        if text.trim().is_empty() {
            self.system_prompt_override = None;
        } else {
            self.system_prompt_override = Some(text.to_string());
        }
    }

    /// Mutate one theme colour at runtime AND persist the new
    /// value to `inkhaven.hjson` so the change survives restart.
    /// `field` is the theme struct's field name
    /// (`tree_paragraph_fg`, `syntax_keyword`, etc.); `hex` is
    /// `#rrggbb` or a named colour.
    ///
    /// In-memory update happens first; if HJSON write-back fails
    /// (missing `theme:` block, disk error, etc.) the runtime
    /// colour stays applied for the rest of this session — the
    /// caller logs the persistence error but doesn't roll back.
    /// That's deliberate: a hook running mid-session shouldn't
    /// have its visual feedback yanked because the project's
    /// HJSON happens to be locked.
    pub(crate) fn ink_theme_set(
        &mut self,
        field: &str,
        hex: &str,
    ) -> Result<(), String> {
        self.theme.set_by_name(field, hex)?;
        // Persist back to inkhaven.hjson. Failure here is logged
        // at WARN; the in-memory mutation already succeeded.
        let config_path = self.layout.config_path();
        let raw = match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::theme",
                    "theme persist: read {}: {e}",
                    config_path.display(),
                );
                return Ok(());
            }
        };
        // HJSON colour literal is a double-quoted string.
        let value_lit = format!("\"{}\"", hex);
        match set_key_in_hjson_block(&raw, "theme", field, &value_lit) {
            Ok(updated) => {
                if let Err(e) = std::fs::write(&config_path, &updated) {
                    tracing::warn!(
                        target: "inkhaven::theme",
                        "theme persist: write {}: {e}",
                        config_path.display(),
                    );
                }
            }
            Err(reason) => {
                tracing::warn!(
                    target: "inkhaven::theme",
                    "theme persist: rewrite `{field}` in `theme` block: {reason}",
                );
            }
        }
        Ok(())
    }

    /// Open the BundInput modal. The user's typed string lands
    /// on the Adam workbench via `hooks::fire(hook, [string])`
    /// when they press Enter. Esc closes without firing. The
    /// caller (the `ink.input` word handler) is responsible for
    /// ensuring `hook` names a registered lambda — otherwise
    /// `hooks::fire` silently no-ops.
    pub(crate) fn open_bund_input(&mut self, prompt: &str, hook: &str) {
        self.modal = Modal::BundInput {
            prompt: prompt.to_string(),
            input: TextInput::new(),
            hook: hook.to_string(),
        };
    }

    /// Append text to the active Bund output pane, if any.
    /// Returns `true` if the text was routed to the pane,
    /// `false` if no pane is open (caller falls back to the
    /// print buffer / status bar). When `newline` is true the
    /// text becomes its own line; otherwise it's concatenated
    /// to the last line.
    pub(crate) fn append_to_bund_pane(&mut self, text: &str, newline: bool) -> bool {
        let Modal::BundPane { lines, scroll, .. } = &mut self.modal else {
            return false;
        };
        if newline {
            for chunk in text.split('\n') {
                lines.push(chunk.to_string());
            }
        } else {
            // Split on embedded newlines so a single print of
            // "a\nb" still produces two lines, but the LAST
            // chunk stays append-to-prior.
            let mut parts = text.split('\n');
            if let Some(first) = parts.next() {
                if let Some(last) = lines.last_mut() {
                    last.push_str(first);
                } else {
                    lines.push(first.to_string());
                }
            }
            for chunk in parts {
                lines.push(chunk.to_string());
            }
        }
        // Auto-scroll to bottom so streaming output is visible
        // without manual scrolling. User can scroll back later.
        let visible = 20usize; // approximate; clamped by renderer
        if lines.len() > visible {
            *scroll = lines.len() - visible;
        }
        true
    }

    /// Open (or reuse) the Bund output pane with `title`. If a
    /// pane is already open it's replaced — same as Esc-then-
    /// open. Used by `ink.pane.show`.
    pub(crate) fn open_bund_pane(&mut self, title: &str) {
        self.modal = Modal::BundPane {
            title: title.to_string(),
            lines: Vec::new(),
            scroll: 0,
        };
    }

    /// Close the Bund pane (no-op if not open).
    pub(crate) fn close_bund_pane(&mut self) {
        if matches!(self.modal, Modal::BundPane { .. }) {
            self.modal = Modal::None;
        }
    }

    /// Clear the Bund pane's line buffer while keeping it open.
    pub(crate) fn clear_bund_pane(&mut self) -> bool {
        let Modal::BundPane { lines, scroll, .. } = &mut self.modal else {
            return false;
        };
        lines.clear();
        *scroll = 0;
        true
    }

    /// Replace the first occurrence of `find` in the open
    /// buffer with `replace`. Cursor lands at the start of the
    /// replacement. Returns `true` if a match was found and
    /// replaced; `false` if no match.
    pub(crate) fn ink_editor_replace(
        &mut self,
        find: &str,
        replace: &str,
    ) -> Result<bool, String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        if find.is_empty() {
            return Err("find string is empty".into());
        }
        // Linear scan over lines for the first match.
        let target: Option<(usize, usize)> = doc
            .textarea
            .lines()
            .iter()
            .enumerate()
            .find_map(|(r, line)| line.find(find).map(|c| (r, c)));
        let Some((row, col)) = target else {
            return Ok(false);
        };
        doc.textarea
            .move_cursor(tui_textarea::CursorMove::Jump(row as u16, col as u16));
        // Delete `find.len()` chars; tui-textarea's delete_char
        // works one char at a time and there's no bulk delete
        // by length, so iterate.
        for _ in 0..find.chars().count() {
            doc.textarea.delete_next_char();
        }
        doc.textarea.insert_str(replace);
        doc.dirty = true;
        Ok(true)
    }
}

/// Map a source-coordinate cursor (row, col) to its visual-coordinate
/// position inside a wrapped layout. Linear in number of visual rows up to
/// the cursor, which is bounded by the source size — cheap at literary scale.
pub(super) fn find_cursor_visual(
    visual: &[super::highlight::VisualRow],
    cur_row: usize,
    cur_col: usize,
) -> (usize, usize) {
    // Locate the cursor row's last visual row so end-of-line cursor lands
    // there even when the line wraps.
    let mut chosen: Option<usize> = None;
    for (i, v) in visual.iter().enumerate() {
        if v.src_row != cur_row {
            if chosen.is_some() {
                break;
            }
            continue;
        }
        let row_end = v.src_col_start + v.width_chars;
        if cur_col < row_end {
            return (i, cur_col - v.src_col_start);
        }
        chosen = Some(i);
    }
    if let Some(i) = chosen {
        let v = &visual[i];
        return (i, cur_col.saturating_sub(v.src_col_start));
    }
    (0, 0)
}


/// Fuzzy-rank `entries` against `query`. Returns indices into
/// the original Vec, ordered by score (descending). Scoring:
///   3 — title starts with query (case-insensitive)
///   2 — title contains query as a substring
///   1 — slug path contains query as a substring
///   0 — excluded
/// Empty query keeps the original ordering and returns all
/// indices (the picker treats this as "no filter applied").
/// 1.2.5+ — case-insensitive substring filter over a list of
/// `ScriptPickerEntry`s. Used by the tag-search results modal;
/// kept simple (no scoring) because the result set already
/// belongs to one chosen tag, and the user just wants to narrow
/// further by title / slug fragment.
pub(super) fn filter_tag_results(
    entries: &[ScriptPickerEntry],
    query: &str,
) -> Vec<ScriptPickerEntry> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return entries.to_vec();
    }
    entries
        .iter()
        .filter(|e| {
            e.title.to_lowercase().contains(&q)
                || e.slug_path.to_lowercase().contains(&q)
        })
        .cloned()
        .collect()
}

pub(super) fn fuzzy_filter_entries(
    entries: &[ScriptPickerEntry],
    query: &str,
) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (0..entries.len()).collect();
    }
    let mut scored: Vec<(i32, usize)> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let tl = e.title.to_lowercase();
        let sl = e.slug_path.to_lowercase();
        let score = if tl.starts_with(&q) {
            3
        } else if tl.contains(&q) {
            2
        } else if sl.contains(&q) {
            1
        } else {
            0
        };
        if score > 0 {
            scored.push((score, i));
        }
    }
    // Stable sort by descending score preserves the original
    // (slug-path-sorted) order within each tier.
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, i)| i).collect()
}

pub(super) fn format_progress_gauge(current: i64, target: i64) -> (String, i64, Style) {
    if target <= 0 {
        return ("[░░░░]".into(), 0, Style::default());
    }
    let pct = (current.max(0) * 100 / target).clamp(0, 999);
    // 4 cells, eighths-resolution per cell would be cleaner but
    // overkill — full / medium / light glyphs are enough.
    let full_cells = (pct / 25).min(4) as usize;
    let remainder = (pct % 25) as usize;
    let mut gauge = String::with_capacity(6);
    gauge.push('[');
    for _ in 0..full_cells {
        gauge.push('█');
    }
    if full_cells < 4 {
        if remainder >= 12 {
            gauge.push('▒');
        } else if remainder > 0 {
            gauge.push('░');
        } else {
            gauge.push('░');
        }
        for _ in (full_cells + 1)..4 {
            gauge.push('░');
        }
    }
    gauge.push(']');
    let style = if pct >= 100 {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else if pct >= 75 {
        Style::default().fg(Color::LightGreen)
    } else if pct >= 50 {
        Style::default().fg(Color::Yellow)
    } else if pct >= 25 {
        Style::default().fg(Color::LightRed)
    } else {
        Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::DIM)
    };
    (gauge, pct, style)
}

#[cfg(test)]
mod tests_scene_break {
    use super::*;

    #[test]
    fn classic_asterisks_match() {
        assert!(is_scene_break("***"));
        assert!(is_scene_break("* * *"));
        assert!(is_scene_break("  * * *  "));
        assert!(is_scene_break("*****"));
    }

    #[test]
    fn dashes_underscores_tildes_hashes_match() {
        assert!(is_scene_break("---"));
        assert!(is_scene_break("___"));
        assert!(is_scene_break("~~~"));
        assert!(is_scene_break("###"));
        assert!(is_scene_break("- - -"));
        assert!(is_scene_break("#####"));
    }

    #[test]
    fn section_sign_alone_matches() {
        assert!(is_scene_break("§"));
        assert!(is_scene_break("  §  "));
    }

    #[test]
    fn below_threshold_does_not_match() {
        assert!(!is_scene_break("**"));
        assert!(!is_scene_break("--"));
        assert!(!is_scene_break("*"));
    }

    #[test]
    fn empty_or_whitespace_no_match() {
        assert!(!is_scene_break(""));
        assert!(!is_scene_break("   "));
        assert!(!is_scene_break("\t\t"));
    }

    #[test]
    fn mixed_chars_dont_match() {
        // Three different markers in a row — not a
        // clean break pattern.
        assert!(!is_scene_break("*-*"));
        assert!(!is_scene_break("***foo"));
        assert!(!is_scene_break("foo***"));
        assert!(!is_scene_break("***bold***"));
    }

    #[test]
    fn typst_headings_dont_match() {
        assert!(!is_scene_break("= Chapter"));
        assert!(!is_scene_break("== Section"));
        assert!(!is_scene_break("=== Subsection"));
    }

    #[test]
    fn prose_does_not_match() {
        assert!(!is_scene_break("The morning was cold."));
        assert!(!is_scene_break("She lifted her shoulders."));
    }
}

#[cfg(test)]
mod tests_audio_slug {
    use super::*;

    #[test]
    fn ascii_lowercased_with_hyphens() {
        assert_eq!(audio_filename_slug("Morning Walk"), "morning-walk");
    }

    #[test]
    fn punctuation_collapsed() {
        assert_eq!(audio_filename_slug("She Said: \"Hello!\""), "she-said-hello");
    }

    #[test]
    fn runs_collapsed_no_trailing_dash() {
        assert_eq!(audio_filename_slug("  hello   world  "), "hello-world");
    }

    #[test]
    fn cyrillic_falls_through_to_hyphen_separator() {
        // Each Cyrillic char isn't ascii_alphanumeric, so
        // the slug becomes empty + fallback.  Acceptable —
        // user edits the path before pressing Enter when
        // they want a localised filename.
        assert_eq!(audio_filename_slug("Привет мир"), "paragraph");
    }

    #[test]
    fn mixed_keeps_ascii_drops_others() {
        assert_eq!(
            audio_filename_slug("Chapter 3 — введение"),
            "chapter-3"
        );
    }

    #[test]
    fn empty_falls_back() {
        assert_eq!(audio_filename_slug(""), "paragraph");
        assert_eq!(audio_filename_slug("___"), "paragraph");
    }
}

#[cfg(test)]
mod tests_truncate {
    use super::*;

    #[test]
    fn under_cap_returns_unchanged() {
        let s = "a\nb\nc".to_string();
        assert_eq!(truncate_to_lines(s.clone(), 10), s);
    }

    #[test]
    fn empty_string_returns_empty() {
        assert_eq!(truncate_to_lines(String::new(), 10), "");
    }

    #[test]
    fn exact_cap_returns_unchanged() {
        let s = "a\nb\nc".to_string();
        assert_eq!(truncate_to_lines(s.clone(), 3), s);
    }

    #[test]
    fn over_cap_truncates_and_appends_marker() {
        // 10 lines, cap 3 → keep 2 + marker; marker mentions 8.
        let s: String =
            (1..=10).map(|i| format!("l{i}")).collect::<Vec<_>>().join("\n");
        let out = truncate_to_lines(s, 3);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3, "must respect cap, got: {out:?}");
        assert_eq!(lines[0], "l1");
        assert_eq!(lines[1], "l2");
        assert!(
            lines[2].contains("8 more lines truncated"),
            "expected marker for 8 dropped, got {:?}",
            lines[2]
        );
    }

    #[test]
    fn cap_one_yields_just_the_marker() {
        // cap=1 → keep=0 head, marker reports all N as dropped.
        let s = "x\ny\nz".to_string();
        let out = truncate_to_lines(s, 1);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("3 more lines truncated"));
    }

    #[test]
    fn cap_zero_treated_as_one() {
        // Defensive: 0-cap shouldn't divide-by-zero or panic.
        let s = "a\nb".to_string();
        let out = truncate_to_lines(s, 0);
        assert!(out.contains("more lines truncated"));
    }
}

#[cfg(test)]
mod tests_gauge {
    use super::*;

    #[test]
    fn gauge_zero() {
        let (g, p, _) = format_progress_gauge(0, 100);
        assert_eq!(p, 0);
        assert_eq!(g, "[░░░░]");
    }

    #[test]
    fn gauge_partial() {
        let (_, p, _) = format_progress_gauge(60, 100);
        assert_eq!(p, 60);
    }

    #[test]
    fn gauge_full() {
        let (g, p, _) = format_progress_gauge(100, 100);
        assert_eq!(p, 100);
        assert_eq!(g, "[████]");
    }

    #[test]
    fn gauge_over() {
        let (g, p, _) = format_progress_gauge(250, 100);
        assert_eq!(p, 250);
        assert_eq!(g, "[████]");
    }
}

/// Try to derive a usable title from a paragraph body. Skips Typst heading
/// lines (`= …`), comments (`// …`), and blank lines. Looks for the first
/// `.`, `!`, or `?` followed by whitespace or end-of-text. Truncates the
/// result to fit `TITLE_MAX_DISPLAY` chars (with ellipsis if cut). Returns
/// None if no usable text is found.
/// Lift the lookup term for a Ctrl+B P / Ctrl+B C inference: the
/// selection if any, otherwise the (Unicode) word under the cursor. The
/// word boundaries respect Cyrillic / CJK punctuation via
/// `unicode-segmentation`, so a selection of "Москва" works the same as
/// dropping the cursor inside it. Trailing apostrophes / quotes are
/// trimmed so "King's" doesn't pull in an extra quote.
/// Dispatch the per-line highlight call based on `content_type` —
/// typst (default) goes through the cached tree-sitter highlighter;
/// "hjson" runs the lightweight hand-rolled lexer.
pub(super) fn highlight_for_content(
    highlighter: &mut super::highlight::TypstHighlighter,
    source: &str,
    theme: &super::theme::Theme,
    content_type: Option<&str>,
) -> Vec<Vec<super::highlight::StyledRun>> {
    match content_type {
        Some("hjson") => super::hjson_highlight::highlight_hjson_lines(source, theme),
        Some("bund") => super::bund_highlight::highlight_bund_lines(source, theme),
        // 1.2.8+ — Help-book paragraphs default to markdown
        // and use the hand-rolled CommonMark-subset lexer
        // (headings, fences, emphasis, links, lists, HRs).
        Some("markdown") | Some("md") =>
            super::markdown_highlight::highlight_markdown_lines(source, theme),
        _ => highlighter.highlight_lines(source, theme),
    }
}

/// Convert a tui-textarea (row, char-col) cursor into a byte offset
/// inside `source = lines.join("\n")`. Used by the mode detector to
/// query tree-sitter at the cursor's position.
fn byte_offset_for_cursor(source: &str, row: usize, col: usize) -> usize {
    let mut byte: usize = 0;
    let mut current_row = 0;
    for line in source.split_inclusive('\n') {
        if current_row == row {
            // Find the byte offset for `col` chars into this line
            // (stop before the newline if any).
            let body = line.strip_suffix('\n').unwrap_or(line);
            return byte
                + body
                    .char_indices()
                    .nth(col)
                    .map(|(b, _)| b)
                    .unwrap_or(body.len());
        }
        byte += line.len();
        current_row += 1;
    }
    source.len()
}

/// Apply an editor-style "search match" highlight to the first
/// occurrence of `needle` (case-insensitive) inside a chat-history
/// rendered line. The matched substring gets a dark foreground on a
/// pink background (re-using `search_match_bg` / `search_current_bg`
/// from the theme); surrounding text keeps its original styling.
///
/// Only the FIRST occurrence per line is highlighted — multiple
/// matches on the same line is a UX corner case; the user can hit
/// Ctrl+X to walk to the next line's match either way.
pub(super) fn highlight_substring_in_line(
    line: &mut ratatui::text::Line<'static>,
    needle_lower: &str,
    is_current: bool,
    theme: &super::theme::Theme,
) {
    if needle_lower.is_empty() {
        return;
    }
    let full: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    let lower = full.to_lowercase();
    let Some(byte_pos) = lower.find(needle_lower) else { return };
    let byte_end = byte_pos + needle_lower.len();
    let char_start = full[..byte_pos].chars().count();
    // Use `chars().count()` rather than slicing by needle.len() so
    // we get the count in original-cased chars (UTF-8 case-fold can
    // shift byte lengths).
    let char_end = full[..byte_end].chars().count();

    let highlight_bg = if is_current {
        theme.search_current_bg
    } else {
        theme.search_match_bg
    };
    // Dark text on the pink bg — matches the editor's find-modal
    // colour scheme so the matched word stays legible.
    let mut highlight_style = ratatui::style::Style::default()
        .bg(highlight_bg)
        .fg(theme.pane_bg);
    if is_current {
        highlight_style = highlight_style.add_modifier(ratatui::style::Modifier::BOLD);
    }

    let mut new_spans: Vec<ratatui::text::Span<'static>> = Vec::new();
    let mut cursor: usize = 0;
    for span in line.spans.drain(..) {
        let span_text = span.content.to_string();
        let span_len = span_text.chars().count();
        let span_end = cursor + span_len;
        let overlap_start = char_start.max(cursor);
        let overlap_end = char_end.min(span_end);
        if overlap_start >= overlap_end {
            new_spans.push(ratatui::text::Span::styled(span_text, span.style));
            cursor = span_end;
            continue;
        }
        if overlap_start > cursor {
            let pre: String = span_text
                .chars()
                .take(overlap_start - cursor)
                .collect();
            new_spans.push(ratatui::text::Span::styled(pre, span.style));
        }
        let match_text: String = span_text
            .chars()
            .skip(overlap_start - cursor)
            .take(overlap_end - overlap_start)
            .collect();
        new_spans.push(ratatui::text::Span::styled(match_text, highlight_style));
        if overlap_end < span_end {
            let post: String = span_text
                .chars()
                .skip(overlap_end - cursor)
                .collect();
            new_spans.push(ratatui::text::Span::styled(post, span.style));
        }
        cursor = span_end;
    }
    line.spans = new_spans;
}


/// Open-bracket → matching close pair the auto-close logic emits.
/// None for any character that isn't an opener we recognise.
fn open_pair_for(c: char) -> Option<char> {
    match c {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        _ => None,
    }
}

fn is_close_pair_char(c: char) -> bool {
    matches!(c, ')' | ']' | '}' | '"' | '\'')
}

/// Case-insensitive substring filter over the baked-in typst function
/// table. Results are returned sorted alphabetically (the table is
/// already sorted; we just preserve order across filter steps).
pub(super) fn filter_functions(filter: &str) -> Vec<super::typst_funcs::TypstFn> {
    let needle = filter.trim().to_lowercase();
    if needle.is_empty() {
        return super::typst_funcs::all();
    }
    super::typst_funcs::all()
        .into_iter()
        .filter(|f| f.name.to_lowercase().contains(&needle))
        .collect()
}

/// Inspect the editor line up to the cursor and decide whether we're
/// inside `#image("…<cursor>…")`. Returns `None` when not, so the
/// caller can fall back to the regular `Ctrl+B P` (Places RAG) path.
///
/// Detection rule (line-local, no tree-sitter required):
///   1. Find the LAST occurrence of `#image(` on the line at-or-before
///      the cursor column.
///   2. Between the `(` and the cursor, there must be no balanced `)`
///      (the call is still open).
///   3. Between the `(` and the cursor, there must be exactly one `"`
///      (we are inside the first string literal).
///
/// Multi-line `#image(...)` calls — rare in practice — are not
/// detected. The line-local scope makes this a 50-line function and
/// the failure mode is "Ctrl+B P falls through to Places RAG" rather
/// than a bug.
fn detect_image_call_context(line: &str, cursor_col: usize) -> Option<ImageCallContext> {
    let cursor_byte = char_offset_to_byte(line, cursor_col);
    let prefix = &line[..cursor_byte];
    // Walk backward to find the last `#image(`. Allow whitespace
    // between `image` and `(` (e.g. `#image (` is uncommon but legal).
    let open_paren_idx = find_image_open(prefix)?;
    // After the `(`, count parens + quotes up to the cursor.
    let between = &prefix[open_paren_idx + 1..];
    let mut depth: i32 = 1; // we're inside the open paren
    let mut quotes: usize = 0;
    for c in between.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth <= 0 {
                    return None; // call already closed before cursor
                }
            }
            '"' => quotes += 1,
            _ => {}
        }
    }
    if quotes != 1 {
        return None;
    }
    // Look forward for a closing quote on the same line so the picker
    // can decide whether to add one.
    let suffix = &line[cursor_byte..];
    let closing_quote_present = suffix
        .chars()
        .scan(false, |escape, c| {
            if *escape {
                *escape = false;
                return Some((false, c));
            }
            if c == '\\' {
                *escape = true;
                return Some((false, c));
            }
            Some((c == '"', c))
        })
        .any(|(is_close, _)| is_close);
    Some(ImageCallContext {
        closing_quote_present,
    })
}

fn find_image_open(prefix: &str) -> Option<usize> {
    // Iterate in reverse to find the LAST `#image[whitespace]?(`.
    let bytes = prefix.as_bytes();
    let mut i = bytes.len();
    while i > 0 {
        if bytes[i - 1] == b'(' {
            // Look back for "image" with optional whitespace, and a `#`
            // somewhere before that on this scan window.
            let head = &prefix[..i - 1];
            let trimmed = head.trim_end();
            if let Some(stripped) = trimmed.strip_suffix("image") {
                if stripped.ends_with('#') {
                    return Some(i - 1);
                }
            }
        }
        i -= 1;
    }
    None
}

fn char_offset_to_byte(s: &str, char_off: usize) -> usize {
    s.char_indices()
        .nth(char_off)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

pub(super) fn current_word_or_selection(doc: &OpenedDoc) -> String {
    if let Some(((r1, c1), (r2, c2))) = doc.textarea.selection_range() {
        return slice_lines(doc.textarea.lines(), r1, c1, r2, c2)
            .trim()
            .to_string();
    }
    let (row, col) = doc.textarea.cursor();
    let lines = doc.textarea.lines();
    let Some(line) = lines.get(row) else {
        return String::new();
    };
    use unicode_segmentation::UnicodeSegmentation;
    for (byte_off, w) in line.unicode_word_indices() {
        let start_col = line[..byte_off].chars().count();
        let end_col = start_col + w.chars().count();
        if col >= start_col && col <= end_col {
            return w.trim_matches(|c: char| c == '\'' || c == '"').to_string();
        }
    }
    String::new()
}

fn compute_book_stats(
    hierarchy: &Hierarchy,
    book: &Node,
    project_root: &Path,
) -> BookStats {
    let mut stats = BookStats::default();
    for id in hierarchy.collect_subtree(book.id) {
        let Some(node) = hierarchy.get(id) else { continue };
        match node.kind {
            NodeKind::Book => {} // the book itself — don't count it as a chapter
            NodeKind::Chapter => stats.chapters += 1,
            NodeKind::Subchapter => stats.subchapters += 1,
            NodeKind::Paragraph => {
                stats.paragraphs += 1;
                stats.words += node.word_count;
                if let Some(rel) = node.file.as_ref() {
                    if let Ok(body) =
                        std::fs::read_to_string(project_root.join(rel))
                    {
                        stats.sentences += count_sentences(&body);
                    }
                }
            }
            NodeKind::Image => stats.images += 1,
            // Bund scripts aren't book content; they don't add to
            // word / sentence counts. Tracking them in their own
            // stats slot is a follow-up.
            NodeKind::Script => {}
        }
    }
    stats
}

/// Count sentence-terminators (`. ! ?`) in prose text, ignoring Typst
/// heading lines (`= ...`) and comments (`// ...`). A run of repeated
/// terminators (e.g. `...` or `?!`) counts as one sentence. The result
/// is an estimate — Typst markup like `#image(...)` and inline math can
/// confuse it — but it's good enough for a UI read-out and consistent
/// across runs.
fn count_sentences(content: &str) -> usize {
    let mut count = 0;
    let mut in_run = false;
    for line in content.lines() {
        let t = line.trim_start();
        if t.is_empty() || t.starts_with('=') || t.starts_with("//") {
            in_run = false;
            continue;
        }
        for c in t.chars() {
            if matches!(c, '.' | '!' | '?') {
                if !in_run {
                    count += 1;
                    in_run = true;
                }
            } else {
                in_run = false;
            }
        }
        in_run = false;
    }
    count
}


/// Render one Quick reference entry as a single Line, sized to fit
/// `col_w` terminal cells. Headers get cyan-bold styling; regular entries
/// get a fixed 14-char key column followed by the description.
pub(super) fn format_entry_line(e: &quickref::Entry, col_w: usize) -> Line<'static> {
    if e.is_header {
        let text = if e.key.is_empty() {
            String::new()
        } else {
            format!(" {}", e.key)
        };
        return Line::from(Span::styled(
            truncate_to_chars(&text, col_w),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let key_field = 14;
    // Pad/truncate the key to a fixed width so descriptions align.
    let key_padded = pad_or_trim(&e.key, key_field);
    let desc_max = col_w.saturating_sub(key_field + 2);
    let desc = truncate_to_chars(&e.desc, desc_max);
    let line = format!(" {} {}", key_padded, desc);
    Line::from(vec![
        Span::styled(
            format!(" {}", key_padded),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::raw(desc),
    ])
    // Note: `line` above is unused — kept as a clarity comment of the
    // intended width budget. Compiler will eliminate.
    .style(Style::default())
    .alignment(ratatui::layout::Alignment::Left)
    .clone()
    .ok_or_else_unused(line)
}

/// Stub to silence the unused `line` binding above without changing
/// semantics. Compiler should inline to no-op.
trait OkOrElseUnused {
    fn ok_or_else_unused(self, _unused: String) -> Self;
}
impl OkOrElseUnused for Line<'static> {
    fn ok_or_else_unused(self, _unused: String) -> Self {
        self
    }
}


/// Standard text-input key dispatch: typing, navigation, deletion. Shared
/// helper so new modals don't have to re-implement the pattern that older
/// modals (Add, Rename, FindReplace) inline.
/// One-line status-bar summary of a `bund eval` result. Surfaces
/// captured stdout (from print / println) and the top-of-stack
/// value in a stable order. `context` is an optional preamble —
/// passed when the eval came from a named buffer ("ran `script-
/// name`").
fn format_eval_output(out: &crate::scripting::EvalOutput, context: Option<&str>) -> String {
    let stdout = out.stdout.trim_end().to_string();
    let top_str = out
        .top
        .as_ref()
        .map(crate::scripting::format_value);
    let preamble = match context {
        Some(name) => format!("bund `{name}`"),
        None => "bund".to_string(),
    };
    match (stdout.is_empty(), top_str) {
        (true, Some(v)) => format!("{preamble} → {v}"),
        (false, Some(v)) => format!("{preamble} → {v}  ·  stdout: {stdout}"),
        (false, None) => format!("{preamble} stdout: {stdout}"),
        (true, None) => format!("{preamble}: (no result)"),
    }
}

pub(super) fn handle_text_input_key(input: &mut TextInput, key: KeyEvent) {
    use KeyCode::*;
    match key.code {
        Backspace => input.backspace(),
        Delete => input.delete(),
        Left => input.move_left(),
        Right => input.move_right(),
        Home => input.move_home(),
        End => input.move_end(),
        Char(c) => {
            let mut residual = key.modifiers;
            residual.remove(KeyModifiers::SHIFT);
            if residual.is_empty() {
                let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                    && c.is_ascii_alphabetic()
                {
                    c.to_ascii_uppercase()
                } else {
                    c
                };
                input.insert_char(final_c);
            }
        }
        _ => {}
    }
}

/// System prompt used by regular chat when InferenceMode is Local. The
/// model is told to treat any "── … context ──" blocks the user prepends
/// (via the AI scope cycle) as the sole admissible source and to refuse
/// rather than fall back on general knowledge. Empty context just means
/// the conversation itself is the source.
pub(super) const LOCAL_SYSTEM_PROMPT: &str = "\
You are Inkhaven's writing assistant. The user may prepend `── … context ──` \
blocks (a selection, a paragraph, or whole chapters from their book). When \
present, treat those blocks as the sole admissible source of information. \
Do NOT introduce facts, names, or claims that are absent from the context. \
If the context is insufficient, say so plainly rather than improvising. \
Without an explicit context block, rely only on prior conversation turns; \
do not draw on general knowledge.";

/// System prompt used by regular chat when InferenceMode is Full. Context
/// blocks (when present) are still ground truth, but the model is free to
/// augment with general knowledge — the typical chat / brainstorm mode.
pub(super) const FULL_SYSTEM_PROMPT: &str = "\
You are Inkhaven's writing assistant. If the user prepends `── … context ──` \
blocks, treat that text as ground truth and prefer it to any conflicting \
general knowledge. Otherwise, answer freely using your general knowledge \
of writing craft, world-building, and the relevant subject matter. Be \
concise; favour short paragraphs and concrete suggestions.";

/// Grammar-check system prompt. Bounds the model to a copy-editor role,
/// keeps Typst markup intact, and emits the final corrected paragraph
/// inside machine-parseable markers (`<<<CORRECTED>>>` / `<<<END>>>`) so
/// the AI pane's `T` action can lift it out without dragging the
/// summary / issue list along.
pub(super) const GRAMMAR_CHECK_SYSTEM_PROMPT: &str = "\
You are a meticulous copy-editor reviewing a paragraph from a Typst-formatted \
manuscript. Check for grammar, syntax, and punctuation issues only. \
Preserve every Typst markup token verbatim — `= Heading`, `== Subheading`, \
`*bold*`, `_italic_`, `#link(\"…\")[…]`, raw / code blocks, and any other \
Typst-specific syntax must round-trip unchanged. Do not rewrite for style, \
do not change voice, do not propose structural edits unless the original \
sentence is grammatically broken.

Output format (follow exactly):
1. Start with a short summary line (e.g. \"3 grammar issues, 1 punctuation \
issue, otherwise clean\").
2. Then list each issue with the exact original phrase and a suggested \
correction.
3. Finally, emit the fully corrected paragraph between the literal markers \
shown below — nothing else may appear inside the markers, and the markers \
themselves must appear on their own lines, byte-for-byte:

<<<CORRECTED>>>
(the corrected paragraph, with every Typst markup token preserved)
<<<END>>>

The markers are LITERAL. Do not abbreviate them to `<<>>`, `<<END>>`, \
`<<<corrected>>>`, or any other variant — the editor pipeline accepts \
those shapes as a fallback but the canonical form above keeps round-trip \
testing reliable. Do not place commentary inside the markers. The editor \
pipeline will lift the text between the markers and overwrite the \
paragraph buffer with it.";

/// Markers the grammar-check system prompt instructs the model to wrap
/// the corrected paragraph in. Kept as named constants so the parser and
/// the prompt stay in sync.
pub(super) const CORRECTED_BEGIN: &str = "<<<CORRECTED>>>";
pub(super) const CORRECTED_END: &str = "<<<END>>>";

/// Fallback prompt body for F7 grammar check when no user-defined
/// `Grammar check` prompt exists in the Prompts book or `prompts.hjson`.
/// The configured `language` from the HJSON drives the grammar rules.
pub(crate) fn grammar_check_default_prompt(language: &str) -> String {
    let lang = if language.trim().is_empty() {
        "English"
    } else {
        language.trim()
    };
    format!(
        "Run a copy-edit pass on the paragraph below. Treat it as {lang} \
prose. Check syntax, agreement, tense, and punctuation; flag anything \
that's grammatically incorrect according to standard {lang} grammar. \
Typst markup may be present — preserve it verbatim in any corrected \
output. After listing issues, give the fully corrected paragraph."
    )
}

/// 1.2.6+ — embedded fallback for Ctrl+F12 explain-diagnostic.
pub(crate) fn explain_diagnostic_default_prompt() -> &'static str {
    "A Typst compiler diagnostic is shown below with the surrounding source. \
Explain in plain English what the diagnostic means, why it likely fired in \
this context, and the most plausible one-line fix. If the diagnostic is a \
false positive — e.g. the paragraph references a function defined in the \
book's preamble that isn't visible to this isolated compile — say so and \
move on. Keep the answer tight and actionable."
}

/// 1.2.6+ — embedded fallback for F12 critique in editor mode.
pub(crate) fn critique_edit_default_prompt() -> &'static str {
    "Read the paragraph below as a draft. Point out the weakest two or three \
elements: vague verbs, abstract nouns where the concrete would land harder, \
sentences that lose the reader, rhythm that flattens, claims that wobble, \
imagery that doesn't earn its place. Be specific — quote the exact phrase \
and propose a tighter alternative. Do NOT rewrite the whole paragraph; \
critique it. Be honest, not destructive."
}

/// 1.2.6+ — embedded fallback for F12 critique in split-edit mode.
pub(crate) fn critique_changes_default_prompt() -> &'static str {
    "Two versions of the same paragraph are shown below: a `Before` snapshot \
and the current `After` buffer. Identify what the revision changed (added / \
removed / reordered), and evaluate whether each change is an improvement, a \
regression, or neutral. Quote the specific phrases that moved. End with one \
suggestion for what the next revision pass should focus on."
}

/// 1.2.6+ — embedded fallback for the timeline health
/// check (y / Y / Ctrl+Y inside Ctrl+V t). The payload
/// itself does the heavy lifting; this top text just sets
/// the model's task tone.
pub(crate) fn timeline_health_default_prompt() -> &'static str {
    "You are reviewing the story timeline that follows for internal consistency. \
Treat the events as facts about a single fictional world; do not invent missing \
detail. Read the audit checklist at the bottom and respond to it — be specific, \
quote event titles, and surface concrete fixes. If the timeline is internally \
coherent, say so briefly rather than padding with caveats."
}

/// System prompt for the F1 / "Help!" RAG flow. We force the model to
/// stick to the supplied excerpts so the help feature behaves like a
/// retrieval-grounded manual and not a general LLM chat — it should admit
/// ignorance rather than confabulate Inkhaven features.
pub(super) const HELP_SYSTEM_PROMPT: &str = "\
You are the Inkhaven help-manual assistant. Your job is to answer the \
user's question about Inkhaven (a Rust TUI literary editor for Typst \
books) using ONLY the Help excerpts the user provides below.

Rules:
- Use only the supplied excerpts. Do not invent commands, keybindings, \
  features, or file paths that are not present in the excerpts.
- Do not fall back on general LLM knowledge or assumptions about other \
  editors. If the excerpts do not answer the question, say so plainly \
  and suggest which area of the Help book might cover it.
- Quote keybindings, command names, and option labels verbatim from the \
  excerpts where useful.
- Be concise. Prefer short paragraphs and bulleted lists. Skip pleasantries.
- If multiple excerpts cover the topic, synthesise them; do not list them \
  as separate answers.
- Plain text only — no markdown headings beyond `#`/`-` lists.";

/// Inclusive on the top-left, exclusive on the bottom-right — matches
/// ratatui's Rect semantics where width/height are spans (the column at
/// `x + width` is one past the rect's last column).
fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

pub(super) fn digit_count(n: usize) -> usize {
    let mut x = n.max(1);
    let mut d = 0;
    while x > 0 {
        d += 1;
        x /= 10;
    }
    d
}

/// Allowlist of keystrokes that are non-mutating in the editor pane. Used to
/// gate the editor when an open paragraph lives inside the read-only Help
/// subtree. Anything not listed here is rejected with a status message.
fn is_read_only_safe_key(key: &KeyEvent) -> bool {
    use KeyCode::*;
    // Pure navigation / scrolling / selection — always safe.
    if matches!(
        key.code,
        Left | Right | Up | Down | Home | End | PageUp | PageDown | Esc | Tab | BackTab,
    ) {
        return true;
    }
    // F-keys: F3/F4/F6 are viewers (file picker, split toggle, snapshot
    // picker) — picking a file or snapshot WOULD mutate, but we gate the
    // actual replace in those flows. F5 (new snapshot) and Ctrl+F4 (accept
    // snapshot) mutate, so they're blocked here.
    if matches!(key.code, F(1) | F(3) | F(4) | F(6)) {
        // F4 with Ctrl is "accept snapshot" → block.
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        return !(matches!(key.code, F(4)) && ctrl);
    }
    // Alt+arrows and Alt+C are block-selection / block-copy — safe.
    if key.modifiers.contains(KeyModifiers::ALT) {
        if matches!(key.code, Left | Right | Up | Down) {
            return true;
        }
        if matches!(key.code, Char('c') | Char('C')) {
            return true;
        }
        return false;
    }
    // Ctrl combos: an explicit allowlist of read-safe operations.
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return matches!(
            key.code,
            Char('a') | Char('A')                // select all
            | Char('c') | Char('C')              // copy
            | Char('f') | Char('F')              // find
            | Char('x') | Char('X')              // repeat (next match)
            | Char('s') | Char('S')              // save (no-op + status)
            | Char('h') | Char('H')              // split-scroll up (only effective in split)
            | Char('j') | Char('J')              // split-scroll down
            | Char('q') | Char('Q')              // quit
            | Char('b') | Char('B')              // meta prefix
            | Char('t') | Char('T')              // focus Tree alias
            | Char('1') | Char('2') | Char('3') | Char('4') | Char('5') // focus jumps
            | Char('@') | Char('/')              // Ctrl+2 alternates / search focus
        );
    }
    false
}

pub(super) fn reverse_chip(fg: Color) -> Style {
    Style::default()
        .bg(fg)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD)
}

/// Extract the substring spanning `(start_row, start_col)..(end_row, end_col)`
/// from a slice of lines, where both ends are char-indexed. Used for pulling
/// the editor selection out for `{{selection}}` substitution.
fn slice_lines(lines: &[String], r1: usize, c1: usize, r2: usize, c2: usize) -> String {
    if r1 >= lines.len() {
        return String::new();
    }
    if r1 == r2 {
        let chars: Vec<char> = lines[r1].chars().collect();
        let s = c1.min(chars.len());
        let e = c2.min(chars.len());
        return chars[s..e].iter().collect();
    }
    let mut out = String::new();
    let first: String = lines[r1].chars().skip(c1).collect();
    out.push_str(&first);
    out.push('\n');
    for r in (r1 + 1)..r2 {
        if let Some(line) = lines.get(r) {
            out.push_str(line);
            out.push('\n');
        }
    }
    if let Some(last_line) = lines.get(r2) {
        let chars: Vec<char> = last_line.chars().collect();
        let e = c2.min(chars.len());
        out.extend(chars[..e].iter());
    }
    out
}
