//! Prompts-editor probe (3.20) — drives `inkhaven prompts-editor` headlessly:
//! every battery key in each of the three panes and inside every modal at
//! nine terminal sizes, then scripted checks of the contracts `KEYBINDING.md`
//! states — `?` types in the editor and the AI prompt, a chorded letter never
//! fires a list action, the AI prompt takes the shared chord set and keeps
//! the draft across a history walk, the AI response pane scrolls and clamps,
//! the help pane scrolls, the rollback picker windows around its cursor, and
//! a save round-trips to disk.
//!
//! No store and no network (the AI runtime is a stub client that is never
//! asked anything), so it runs in the default suite:
//!
//! ```text
//! cargo test --bin inkhaven -- prompts_tui_probe --nocapture
//! ```
use super::*;
use crate::probe_kit::{alt, battery, ctrl, k, km, panic_text, render_text, run_probe, SIZES};
use crossterm::event::{KeyCode, KeyModifiers};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

const SMALL: [(u16, u16); 3] = [(20, 6), (80, 24), (200, 60)];

fn fixture() -> (tempfile::TempDir, App) {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("prompts.hjson");
    std::fs::write(&path, DEFAULT_PROMPTS).unwrap();
    let mut app = App::load(tmp.path().to_path_buf(), &path).unwrap();
    // A stub runtime so the response pane renders its three sections; no
    // send ever reaches it (the probe fills `last_send` by hand).
    app.ai = Some(AiRuntime {
        client: AiClient { client: Arc::new(genai::Client::default()), default_provider: "probe".into() },
        model: "probe-model".into(),
        provider: "probe".into(),
    });
    (tmp, app)
}

fn send(app: &mut App, key: crossterm::event::KeyEvent) -> Result<bool, String> {
    match catch_unwind(AssertUnwindSafe(|| handle_key(app, key))) {
        Ok(Ok(b)) => Ok(b),
        Ok(Err(e)) => Err(format!("Err {e:#}")),
        Err(p) => Err(format!("PANIC {}", panic_text(p))),
    }
}

fn frame(app: &mut App, w: u16, h: u16) -> Result<String, String> {
    render_text(w, h, |f| render(f, app))
}

fn press(app: &mut App, keys: &[crossterm::event::KeyEvent]) {
    for key in keys {
        send(app, *key).unwrap();
    }
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, &[k(KeyCode::Char(c))]);
    }
}

/// `n` backup files with distinct timestamps, so the rollback picker has a
/// list longer than its box (a real save per second would collide on the
/// filename).
fn seed_backups(app: &App, n: usize) {
    let dir = app.project_root.join(".prompts-backups");
    std::fs::create_dir_all(&dir).unwrap();
    let body = std::fs::read_to_string(&app.prompts_path).unwrap();
    for i in 0..n {
        let name = format!("prompts_20260101_10{:02}{:02}.hjson", i / 60, i % 60);
        std::fs::write(dir.join(name), &body).unwrap();
    }
}

fn modal_name(m: &Modal) -> &'static str {
    match m {
        Modal::None => "None",
        Modal::Help { .. } => "Help",
        Modal::SaveConfirm { .. } => "SaveConfirm",
        Modal::Saved { .. } => "Saved",
        Modal::AddPrompt { .. } => "AddPrompt",
        Modal::DeletePromptConfirm { .. } => "DeletePromptConfirm",
        Modal::DiscardConfirm { .. } => "DiscardConfirm",
        Modal::Rollback { .. } => "Rollback",
        Modal::RollbackDelete { .. } => "RollbackDelete",
        Modal::RollbackPreview { .. } => "RollbackPreview",
    }
}

fn invariants(app: &App) -> Option<String> {
    let n = app.library.prompts.len();
    if n > 0 && app.cursor >= n {
        return Some(format!("cursor {} past the {} prompts", app.cursor, n));
    }
    if app.list_scroll > app.cursor {
        return Some(format!("list scroll {} past cursor {}", app.list_scroll, app.cursor));
    }
    if app.editor.lines().is_empty() {
        return Some("editor has no lines".into());
    }
    None
}

/// A finished response of `lines` numbered lines in the AI pane.
fn seed_response(app: &mut App, lines: usize) {
    let response: String = (1..=lines).map(|i| format!("response line {i}")).collect::<Vec<_>>().join("\n");
    app.last_send = Some(Send {
        prompt_name: app.current_prompt().map(|p| p.name.clone()),
        template_under_review: "Tighten {{selection}}".into(),
        analysis_request: DEFAULT_ANALYSIS_REQUEST.into(),
        response,
        started_at: Instant::now(),
        duration: Some(Duration::from_millis(1234)),
        failed: false,
    });
    app.inference = None;
}

#[test]
fn prompts_tui_probe() {
    run_probe("prompts-tui", |out| {
        // A plain Enter in the AI prompt spawns the stream on tokio; the stub
        // client fails at once, which is the state the pane must survive.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        let (_tmp, mut app) = fixture();
        let mut problems: Vec<String> = Vec::new();
        out.push(format!("[FIXTURE] {} prompts · ai stub {}", app.library.prompts.len(), app.ai.is_some()));
        assert!(app.library.prompts.len() >= 3, "the embedded defaults did not load");

        // ── Baseline: every size, each focus, welcome banner on and off ──
        for first in [true, false] {
            app.first_launch = first;
            for focus in [Focus::List, Focus::Editor, Focus::AiPrompt] {
                app.focus = focus;
                for (w, h) in SIZES {
                    if let Err(e) = frame(&mut app, w, h) {
                        problems.push(format!("[BASELINE first={first} {focus:?}] {w}x{h}: {e}"));
                    }
                }
            }
        }
        // The pane states: streaming, failed, done.
        seed_response(&mut app, 40);
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel::<StreamMsg>();
        app.inference = Some(Inference { rx, started_at: Instant::now() });
        for (w, h) in SIZES {
            if let Err(e) = frame(&mut app, w, h) {
                problems.push(format!("[BASELINE streaming] {w}x{h}: {e}"));
            }
        }
        app.inference = None;
        if let Some(s) = app.last_send.as_mut() {
            s.failed = true;
        }
        for (w, h) in SIZES {
            if let Err(e) = frame(&mut app, w, h) {
                problems.push(format!("[BASELINE failed] {w}x{h}: {e}"));
            }
        }
        if let Some(s) = app.last_send.as_mut() {
            s.failed = false;
        }

        // ── Battery in each pane ──────────────────────────────────────
        for focus in [Focus::List, Focus::Editor, Focus::AiPrompt] {
            let mut fired = 0usize;
            for (name, key) in battery() {
                app.modal = Modal::None;
                app.focus = focus;
                app.meta_pending = false;
                fired += 1;
                if let Err(e) = send(&mut app, key) {
                    problems.push(format!("[{focus:?}] {name}: {e}"));
                }
                for (w, h) in SMALL {
                    if let Err(e) = frame(&mut app, w, h) {
                        problems.push(format!("[{focus:?}] {name} render {w}x{h}: {e}"));
                    }
                }
                // After a frame: the renderer reconciles the scroll with the cursor.
                if let Some(bad) = invariants(&app) {
                    problems.push(format!("[{focus:?}] {name}: {bad}"));
                }
                if app.ai_scroll > app.ai_scroll_max.get() {
                    problems.push(format!("[{focus:?}] {name}: ai_scroll {} past max {}", app.ai_scroll, app.ai_scroll_max.get()));
                }
            }
            // The meta prefix followed by every key.
            for (name, key) in battery() {
                app.modal = Modal::None;
                app.focus = focus;
                app.meta_pending = false;
                press(&mut app, &[ctrl('b')]);
                if let Err(e) = send(&mut app, key) {
                    problems.push(format!("[{focus:?}] Ctrl+B {name}: {e}"));
                }
                for (w, h) in SMALL {
                    if let Err(e) = frame(&mut app, w, h) {
                        problems.push(format!("[{focus:?}] Ctrl+B {name} render {w}x{h}: {e}"));
                    }
                }
            }
            out.push(format!("[BATTERY {focus:?}] {fired} keys, plus each behind Ctrl+B"));
        }
        app.meta_pending = false;
        app.focus = Focus::List;

        // ── Battery inside every modal ────────────────────────────────
        type Opener = Box<dyn FnMut(&mut App)>;
        let dirty_one = |a: &mut App| {
            a.focus = Focus::List;
            press(a, &[k(KeyCode::Char('l'))]);
        };
        let mut openers: Vec<(&str, &str, Opener)> = vec![
            ("Help/list", "Help", Box::new(|a| {
                a.focus = Focus::List;
                press(a, &[k(KeyCode::Char('?'))]);
            })),
            ("Help/editor", "Help", Box::new(|a| {
                a.focus = Focus::Editor;
                press(a, &[ctrl('h')]);
            })),
            ("Help/ai", "Help", Box::new(|a| {
                a.focus = Focus::AiPrompt;
                press(a, &[ctrl('h')]);
            })),
            ("AddPrompt", "AddPrompt", Box::new(|a| {
                a.focus = Focus::List;
                press(a, &[k(KeyCode::Char('a'))]);
            })),
            ("DeletePromptConfirm", "DeletePromptConfirm", Box::new(|a| {
                a.focus = Focus::List;
                a.removed.clear();
                press(a, &[k(KeyCode::Char('d'))]);
            })),
            ("SaveConfirm", "SaveConfirm", Box::new(move |a| {
                dirty_one(a);
                press(a, &[ctrl('s')]);
            })),
            ("Saved", "Saved", Box::new(move |a| {
                dirty_one(a);
                press(a, &[ctrl('s'), k(KeyCode::Char('y'))]);
            })),
            ("DiscardConfirm", "DiscardConfirm", Box::new(move |a| {
                dirty_one(a);
                press(a, &[k(KeyCode::Esc)]);
            })),
            ("Rollback", "Rollback", Box::new(|a| press(a, &[ctrl('r')]))),
            ("RollbackPreview", "RollbackPreview", Box::new(|a| press(a, &[ctrl('r'), k(KeyCode::Char('v'))]))),
            ("RollbackDelete", "RollbackDelete", Box::new(|a| press(a, &[ctrl('r'), k(KeyCode::Char('d'))]))),
        ];
        seed_backups(&app, 40);
        for (label, expected, opener) in openers.iter_mut() {
            app.modal = Modal::None;
            app.meta_pending = false;
            opener(&mut app);
            let got = modal_name(&app.modal);
            if got != *expected {
                problems.push(format!("[MODAL {label}] opener produced {got}, expected {expected}"));
                continue;
            }
            for (w, h) in SIZES {
                if let Err(e) = frame(&mut app, w, h) {
                    problems.push(format!("[MODAL {label}] render {w}x{h}: {e}"));
                }
            }
            let mut fired = 0usize;
            for (name, key) in battery() {
                if modal_name(&app.modal) != *expected {
                    app.modal = Modal::None;
                    app.meta_pending = false;
                    opener(&mut app);
                }
                fired += 1;
                if let Err(e) = send(&mut app, key) {
                    problems.push(format!("[MODAL {label}] {name}: {e}"));
                }
                for (w, h) in SMALL {
                    if let Err(e) = frame(&mut app, w, h) {
                        problems.push(format!("[MODAL {label}] {name} render {w}x{h}: {e}"));
                    }
                }
                if let Some(bad) = invariants(&app) {
                    problems.push(format!("[MODAL {label}] {name}: {bad}"));
                }
            }
            app.modal = Modal::None;
            app.meta_pending = false;
            opener(&mut app);
            let mut steps = 0;
            while !matches!(app.modal, Modal::None) && steps < 4 {
                send(&mut app, k(KeyCode::Esc)).unwrap();
                steps += 1;
            }
            if !matches!(app.modal, Modal::None) {
                problems.push(format!("[MODAL {label}] Esc×{steps} did not reach the main view ({})", modal_name(&app.modal)));
            }
            out.push(format!("[MODAL {label}] {fired} keys · 9 sizes · Esc×{steps}"));
        }
        app.modal = Modal::None;
        app.meta_pending = false;

        // ── CHECK `?` types in the text panes, opens help in the list ─
        app.focus = Focus::Editor;
        app.reload_editor();
        let before = app.editor.lines().join("\n");
        press(&mut app, &[k(KeyCode::Char('?'))]);
        let after = app.editor.lines().join("\n");
        if !matches!(app.modal, Modal::None) || after.matches('?').count() != before.matches('?').count() + 1 {
            problems.push(format!("[QMARK] editor: modal {} · `?` count {} → {}", modal_name(&app.modal), before.matches('?').count(), after.matches('?').count()));
        }
        app.modal = Modal::None;
        app.focus = Focus::AiPrompt;
        app.ai_input.clear();
        press(&mut app, &[k(KeyCode::Char('?'))]);
        if app.ai_input.as_str() != "?" || !matches!(app.modal, Modal::None) {
            problems.push(format!("[QMARK] ai prompt: `{}` · modal {}", app.ai_input.as_str(), modal_name(&app.modal)));
        }
        app.focus = Focus::List;
        press(&mut app, &[k(KeyCode::Char('?'))]);
        if !matches!(app.modal, Modal::Help { .. }) {
            problems.push(format!("[QMARK] list: `?` opened {}", modal_name(&app.modal)));
        }
        app.modal = Modal::None;
        out.push("[CHECK ?] types in the editor and AI prompt, help in the list".into());

        // ── CHECK chorded letters are not list actions ────────────────
        app.focus = Focus::List;
        let dirty_before = app.dirty.len();
        for key in [ctrl('a'), alt('a'), ctrl('d'), alt('d'), ctrl('l'), alt('l')] {
            send(&mut app, key).unwrap();
            if !matches!(app.modal, Modal::None) {
                problems.push(format!("[CHORD] {key:?} opened {}", modal_name(&app.modal)));
                app.modal = Modal::None;
            }
        }
        if app.dirty.len() != dirty_before {
            problems.push("[CHORD] a chorded `l` cycled the language tag".into());
        }
        out.push("[CHECK chord] Ctrl/Alt letters leave the list alone".into());

        // ── CHECK the AI prompt: shared chords + draft recall ─────────
        app.focus = Focus::AiPrompt;
        app.ai_input.clear();
        app.ai_history = vec!["one".into(), "two".into()];
        reset_recall(&mut app.ai_history_cursor, &mut app.ai_history_draft);
        type_str(&mut app, "draft text");
        press(&mut app, &[km(KeyCode::Left, KeyModifiers::CONTROL), k(KeyCode::Char('x'))]);
        if app.ai_input.as_str() != "draft xtext" {
            problems.push(format!("[AIPROMPT] Ctrl+← then x gave `{}`", app.ai_input.as_str()));
        }
        press(&mut app, &[k(KeyCode::Up)]);
        let s1 = app.ai_input.as_str().to_string();
        press(&mut app, &[k(KeyCode::Up)]);
        let s2 = app.ai_input.as_str().to_string();
        press(&mut app, &[k(KeyCode::Down), k(KeyCode::Down)]);
        let s3 = app.ai_input.as_str().to_string();
        if s1 != "two" || s2 != "one" || s3 != "draft xtext" {
            problems.push(format!("[AIPROMPT] history walk gave `{s1}` `{s2}` `{s3}`"));
        }
        press(&mut app, &[alt('b'), ctrl('w'), ctrl('u')]);
        if app.ai_input.as_str() != "xtext" {
            problems.push(format!("[AIPROMPT] Alt+B Ctrl+W Ctrl+U left `{}`", app.ai_input.as_str()));
        }
        press(&mut app, &[ctrl('l')]);
        if !app.ai_input.is_empty() {
            problems.push("[AIPROMPT] Ctrl+L did not clear".into());
        }
        out.push(format!("[CHECK ai prompt] walk `{s1}` `{s2}` back to `{s3}` · word chords ok"));

        // ── CHECK the AI response pane scrolls and clamps ─────────────
        seed_response(&mut app, 200);
        app.ai_scroll = 0;
        app.focus = Focus::List;
        frame(&mut app, 80, 24).unwrap();
        let max80 = app.ai_scroll_max.get();
        if max80 == 0 {
            problems.push("[AISCROLL] a 200-line response has no scroll range at 80x24".into());
        }
        press(&mut app, &[ctrl('b'), k(KeyCode::End)]);
        frame(&mut app, 80, 24).unwrap();
        if app.ai_scroll != max80 {
            problems.push(format!("[AISCROLL] Ctrl+B End → {} (max {max80})", app.ai_scroll));
        }
        let shown = frame(&mut app, 80, 24).unwrap();
        if !shown.contains("response line 200") {
            problems.push("[AISCROLL] the last response line is not on the frame after End".into());
        }
        press(&mut app, &[ctrl('b'), k(KeyCode::Up)]);
        if app.ai_scroll != max80 - 1 {
            problems.push(format!("[AISCROLL] Ctrl+B ↑ after End → {}", app.ai_scroll));
        }
        press(&mut app, &[ctrl('b'), k(KeyCode::Home)]);
        if app.ai_scroll != 0 {
            problems.push(format!("[AISCROLL] Ctrl+B Home → {}", app.ai_scroll));
        }
        app.focus = Focus::AiPrompt;
        press(&mut app, &[k(KeyCode::PageDown), k(KeyCode::PageDown)]);
        if app.ai_scroll != 20 {
            problems.push(format!("[AISCROLL] PgDn×2 from the AI prompt → {}", app.ai_scroll));
        }
        press(&mut app, &[ctrl('b'), k(KeyCode::PageDown)]);
        if app.ai_scroll != 30 {
            problems.push(format!("[AISCROLL] Ctrl+B PgDn → {}", app.ai_scroll));
        }
        app.ai_scroll = usize::MAX / 4;
        frame(&mut app, 20, 6).unwrap();
        if app.ai_scroll > app.ai_scroll_max.get() {
            problems.push(format!("[AISCROLL] 20x6 left ai_scroll {} past max {}", app.ai_scroll, app.ai_scroll_max.get()));
        }
        out.push(format!("[CHECK ai scroll] max at 80x24 = {max80} · End/Up/Home/PgDn · clamps at 20x6"));

        // ── CHECK AddPrompt takes the shared chord set and stages ─────
        app.focus = Focus::List;
        app.modal = Modal::None;
        press(&mut app, &[k(KeyCode::Char('a'))]);
        type_str(&mut app, "bad name");
        press(&mut app, &[k(KeyCode::Enter)]);
        let shown = frame(&mut app, 80, 24).unwrap();
        if !matches!(app.modal, Modal::AddPrompt { error: Some(_), .. }) || !shown.contains('⚠') {
            problems.push("[ADD] an invalid name did not show an error".into());
        }
        press(&mut app, &[ctrl('u')]);
        type_str(&mut app, "probeadded");
        press(&mut app, &[k(KeyCode::Home), k(KeyCode::Char('x')), k(KeyCode::Left), k(KeyCode::Delete), k(KeyCode::Char('p')), k(KeyCode::Enter)]);
        if !matches!(app.modal, Modal::None) || !app.added.contains("pprobeadded") || app.focus != Focus::Editor {
            problems.push(format!("[ADD] staging failed: modal {} · added {:?} · focus {:?} · {}", modal_name(&app.modal), app.added, app.focus, app.status));
        }
        out.push("[CHECK add-prompt] validation + Home/Left/Delete edits + staging".into());

        // ── CHECK a save round-trips the edited template ──────────────
        app.modal = Modal::None;
        app.focus = Focus::List;
        press(&mut app, &[k(KeyCode::Home), k(KeyCode::Enter)]);
        let name = app.current_prompt().map(|p| p.name.clone()).unwrap();
        press(&mut app, &[km(KeyCode::Home, KeyModifiers::CONTROL)]);
        type_str(&mut app, "PROBE-EDIT ");
        press(&mut app, &[ctrl('s')]);
        if !matches!(app.modal, Modal::SaveConfirm { .. }) {
            problems.push(format!("[SAVE] Ctrl+S opened {}", modal_name(&app.modal)));
        }
        press(&mut app, &[k(KeyCode::Char('y'))]);
        let on_disk = std::fs::read_to_string(&app.prompts_path).unwrap();
        let lib: PromptLibrary = serde_hjson::from_str(&on_disk).unwrap();
        let saved = lib.prompts.iter().find(|p| p.name == name).map(|p| p.template.clone()).unwrap_or_default();
        if !saved.starts_with("PROBE-EDIT ") {
            problems.push(format!("[SAVE] `{name}` on disk starts with {:?}", saved.chars().take(20).collect::<String>()));
        }
        if !lib.prompts.iter().any(|p| p.name == "pprobeadded") {
            problems.push("[SAVE] the added prompt is not on disk".into());
        }
        out.push(format!("[CHECK save] `{name}` edited + `pprobeadded` written"));

        // ── CHECK the rollback picker windows around its cursor ───────
        app.modal = Modal::None;
        seed_backups(&app, 40);
        press(&mut app, &[ctrl('r')]);
        let n = match &app.modal {
            Modal::Rollback { entries, .. } => entries.len(),
            other => {
                problems.push(format!("[ROLLBACK] Ctrl+R opened {}", modal_name(other)));
                0
            }
        };
        if n > 0 {
            for (label, keys) in [("End", vec![k(KeyCode::End)]), ("mid", vec![k(KeyCode::Home), k(KeyCode::PageDown), k(KeyCode::PageDown)]), ("Home", vec![k(KeyCode::Home)])] {
                press(&mut app, &keys);
                let shown = frame(&mut app, 80, 24).unwrap();
                if !shown.contains('▶') {
                    problems.push(format!("[ROLLBACK] {label}: the cursor row is not on the frame ({n} backups)"));
                }
            }
        }
        out.push(format!("[CHECK rollback] {n} backups · cursor visible at Home / mid / End"));

        // ── CHECK the help pane scrolls to its end and no further ─────
        app.modal = Modal::None;
        app.focus = Focus::Editor;
        press(&mut app, &[ctrl('h')]);
        frame(&mut app, 80, 24).unwrap();
        press(&mut app, &[k(KeyCode::End)]);
        frame(&mut app, 80, 24).unwrap();
        let (scroll, max) = match &app.modal {
            Modal::Help { body, scroll } => (*scroll, help_scroll_max(Rect::new(0, 0, 80, 24), body)),
            _ => (0, 0),
        };
        if scroll != max || max == 0 {
            problems.push(format!("[HELP] End left scroll at {scroll}, max {max}"));
        }
        press(&mut app, &[k(KeyCode::Up)]);
        let after_up = match &app.modal {
            Modal::Help { scroll, .. } => *scroll,
            _ => usize::MAX,
        };
        if max > 0 && after_up != max - 1 {
            problems.push(format!("[HELP] Up after End moved to {after_up}, expected {}", max - 1));
        }
        press(&mut app, &[k(KeyCode::Char('x'))]);
        if !matches!(app.modal, Modal::None) {
            problems.push("[HELP] a plain key did not close the help".into());
        }
        out.push(format!("[CHECK help] editor help: End → {max} · Up → {after_up}"));

        out.push(format!("PROBLEMS ({}):", problems.len()));
        out.extend(problems.iter().cloned());
        assert!(problems.is_empty(), "{} problem(s) — see the report", problems.len());
    });
}
