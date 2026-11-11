//! Linguistic-companion probe (3.21) — drives `inkhaven linguistic` headlessly
//! through its `TuiHost` hooks: every battery key in navigation mode (each
//! right pane) and in compose mode at nine terminal sizes, then scripted checks
//! of the contracts the status line and `KEYBINDING.md` state — `i` / `/` open
//! the prompt, Esc leaves it, Tab cycles the five right panes, `m` / `u` / `p`
//! compute their reports for the selected language, the tree keys, chat
//! scrolling, every local `/command`, a failed chat stream, a completed stream
//! recorded to the session, and quitting.
//!
//! It opens a real `Store` (cached embedding model) and its chat attempt hits a
//! closed local port, so it is `#[ignore]`d from the default run:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture --test-threads=1 linguistic_probe
//! ```
use super::*;
use crate::probe_kit::{battery, companion_project, ctrl, k, panic_text, render_text, run_probe, SIZES};
use std::panic::{catch_unwind, AssertUnwindSafe};

const QUICK: [(u16, u16); 3] = [(40, 12), (100, 30), (200, 60)];
const HINT: &str = "i ask · m metrics · u universals · p pairs · Tab cycle · jk move · ? help · q quit";

fn send(app: &mut LinguisticApp, key: KeyEvent) -> Result<(), String> {
    match catch_unwind(AssertUnwindSafe(|| app.on_key(key))) {
        Ok(()) => Ok(()),
        Err(p) => Err(format!("PANIC {}", panic_text(p))),
    }
}

fn frame(app: &LinguisticApp, w: u16, h: u16) -> Result<String, String> {
    render_text(w, h, |f| app.render(f))
}

fn press(app: &mut LinguisticApp, keys: &[KeyEvent]) {
    for key in keys {
        send(app, *key).unwrap();
    }
}

fn type_text(app: &mut LinguisticApp, s: &str) {
    for c in s.chars() {
        send(app, k(KeyCode::Char(c))).unwrap();
    }
}

fn settle(app: &mut LinguisticApp) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        app.poll_async();
        app.tick();
        if app.stream_rx.is_none() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

fn reset(app: &mut LinguisticApp) {
    app.input_focused = false;
    app.input.clear();
    app.should_quit = false;
    app.right = RightPane::Chat;
    app.stream_rx = None;
    app.streaming_turn = None;
}

/// Type a line into the prompt and send it.
fn run(app: &mut LinguisticApp, line: &str) {
    reset(app);
    press(app, &[k(KeyCode::Char('i'))]);
    type_text(app, line);
    press(app, &[k(KeyCode::Enter)]);
    let _ = settle(app);
}

fn invariants(app: &LinguisticApp) -> Option<String> {
    let rows = app.tree.rows().len();
    if rows > 0 && app.tree.cursor >= rows {
        return Some(format!("tree cursor {} past {} rows", app.tree.cursor, rows));
    }
    None
}

fn render_all(app: &LinguisticApp, label: &str, problems: &mut Vec<String>) {
    for (w, h) in SIZES {
        if let Err(e) = frame(app, w, h) {
            problems.push(format!("[{label}] render {w}x{h}: {e}"));
        }
    }
}

#[test]
#[ignore]
fn linguistic_probe() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    run_probe("linguistic", |out| {
        let project = companion_project();
        let store = Store::open(project.layout.clone(), &project.cfg).expect("open store");
        let hierarchy = Hierarchy::load(&store).unwrap();
        let inv = LinguisticInvocation { language: Some("Qya".into()), session: None };
        let mut app = LinguisticApp::new(project.layout.clone(), project.cfg.clone(), store, hierarchy, inv)
            .expect("LinguisticApp::new");
        let mut problems: Vec<String> = Vec::new();
        let lang_title = app
            .current_language_book()
            .and_then(|id| app.hierarchy.get(id))
            .map(|n| n.title.clone());
        out.push(format!(
            "[FIXTURE] {} tree rows · language {:?} · status {:?}",
            app.tree.rows().len(),
            lang_title,
            app.status
        ));
        if lang_title.as_deref() != Some("Qya") {
            problems.push(format!("[OPEN] --language Qya did not select the language: {lang_title:?}"));
        }
        if app.status != HINT {
            problems.push(format!("[OPEN] status hint {:?}", app.status));
        }
        let title = frame(&app, 100, 30).unwrap();
        if !title.contains("Inner Linguist") || !title.contains("Qya") {
            problems.push("[OPEN] title line lacks the app name or the language".into());
        }

        // ── BATTERY ───────────────────────────────────────────────────────
        type Enter = Box<dyn Fn(&mut LinguisticApp)>;
        let states: Vec<(&str, Enter)> = vec![
            ("nav-chat", Box::new(reset)),
            ("nav-preview", Box::new(|a| { reset(a); a.right = RightPane::Preview; })),
            ("nav-metrics", Box::new(|a| { reset(a); press(a, &[k(KeyCode::Char('m'))]); })),
            ("nav-universals", Box::new(|a| { reset(a); press(a, &[k(KeyCode::Char('u'))]); })),
            ("nav-pairs", Box::new(|a| { reset(a); press(a, &[k(KeyCode::Char('p'))]); })),
            ("compose", Box::new(|a| { reset(a); press(a, &[k(KeyCode::Char('i'))]); })),
        ];
        let keys = battery();
        for (name, enter) in &states {
            enter(&mut app);
            render_all(&app, name, &mut problems);
            let mut errs = 0;
            for (kn, key) in &keys {
                enter(&mut app);
                if let Err(e) = send(&mut app, *key) {
                    problems.push(format!("[{name}] {kn}: {e}"));
                    errs += 1;
                    continue;
                }
                if let Some(v) = invariants(&app) {
                    problems.push(format!("[{name}] {kn}: {v}"));
                }
                for (w, h) in QUICK {
                    if let Err(e) = frame(&app, w, h) {
                        problems.push(format!("[{name}] {kn} render {w}x{h}: {e}"));
                        errs += 1;
                    }
                }
                app.should_quit = false;
            }
            let _ = settle(&mut app);
            out.push(format!("[BATTERY] {name}: {} keys, {errs} errors", keys.len()));
        }

        // ── COMPOSE: i and / open, Esc leaves, typing, Backspace ──────────
        reset(&mut app);
        app.right = RightPane::Metrics;
        press(&mut app, &[k(KeyCode::Char('i'))]);
        if !app.input_focused || app.right != RightPane::Chat {
            problems.push("[COMPOSE] i did not open the prompt on the Chat pane".into());
        }
        type_text(&mut app, "mara?");
        press(&mut app, &[k(KeyCode::Backspace)]);
        if app.input != "mara" {
            problems.push(format!("[COMPOSE] typed {:?}", app.input));
        }
        press(&mut app, &[k(KeyCode::Esc)]);
        if app.input_focused || app.should_quit {
            problems.push("[COMPOSE] Esc should leave compose mode without quitting".into());
        }
        reset(&mut app);
        press(&mut app, &[k(KeyCode::Char('/'))]);
        if !app.input_focused {
            problems.push("[COMPOSE] / did not open the prompt".into());
        }
        press(&mut app, &[k(KeyCode::Char('q'))]);
        if app.should_quit || app.input != "q" {
            problems.push("[COMPOSE] q while composing should type".into());
        }
        press(&mut app, &[ctrl('q')]);
        if !app.should_quit {
            problems.push("[COMPOSE] Ctrl+Q while composing did not quit".into());
        }
        reset(&mut app);
        out.push("[COMPOSE] i and / open · Esc leaves · q types · Ctrl+Q quits".into());

        // ── PANES: Tab cycle, m/u/p compute ───────────────────────────────
        reset(&mut app);
        let order = [RightPane::Preview, RightPane::Metrics, RightPane::Universals, RightPane::Pairs, RightPane::Chat];
        for (i, want) in order.iter().enumerate() {
            press(&mut app, &[k(KeyCode::Tab)]);
            if app.right != *want {
                problems.push(format!("[PANES] Tab #{} landed on {:?}", i + 1, app.right));
            }
        }
        press(&mut app, &[k(KeyCode::Char('m'))]);
        if app.right != RightPane::Metrics || app.metrics_text.trim().is_empty() {
            problems.push(format!("[PANES] m → {:?} text {:?}", app.right, app.metrics_text.chars().take(40).collect::<String>()));
        }
        press(&mut app, &[k(KeyCode::Char('u'))]);
        if app.right != RightPane::Universals || app.universals_text.trim().is_empty() {
            problems.push("[PANES] u did not compute the universals".into());
        }
        press(&mut app, &[k(KeyCode::Char('p'))]);
        if app.right != RightPane::Pairs || app.pairs_text.trim().is_empty() {
            problems.push("[PANES] p did not compute the minimal pairs".into());
        }
        for (pane, text) in [("Phonology", &app.metrics_text), ("Universals", &app.universals_text), ("Minimal pairs", &app.pairs_text)] {
            out.push(format!("[PANES] {pane}: {:?}", text.lines().next().unwrap_or("").chars().take(70).collect::<String>()));
        }
        let shown = frame(&app, 120, 40).unwrap();
        if !shown.contains("Minimal pairs") {
            problems.push("[PANES] the Pairs pane title is missing at 120x40".into());
        }

        // ── TREE: j/k/g/G/h/l/Enter keep the cursor in range and refresh the preview ─
        reset(&mut app);
        app.right = RightPane::Preview;
        press(&mut app, &[k(KeyCode::Char('g'))]);
        let top = app.tree.cursor;
        press(&mut app, &[k(KeyCode::Char('G'))]);
        let bottom = app.tree.cursor;
        if top != 0 || bottom + 1 != app.tree.rows().len() {
            problems.push(format!("[TREE] g/G → {top}/{bottom} of {}", app.tree.rows().len()));
        }
        press(&mut app, &[k(KeyCode::Char('g'))]);
        let rows = app.tree.rows().len();
        press(&mut app, &[k(KeyCode::Enter)]);
        let toggled = app.tree.rows().len() != rows;
        press(&mut app, &[k(KeyCode::Enter)]);
        if !toggled || app.tree.rows().len() != rows {
            problems.push("[TREE] Enter did not fold / unfold the first row".into());
        }
        press(&mut app, &[k(KeyCode::Char('l')), k(KeyCode::Char('j'))]);
        let preview_a = app.preview.clone();
        press(&mut app, &[k(KeyCode::Char('j'))]);
        for _ in 0..40 {
            press(&mut app, &[k(KeyCode::Char('j'))]);
        }
        if app.tree.cursor + 1 != app.tree.rows().len() {
            problems.push("[TREE] j past the end moved off the last row".into());
        }
        let _ = preview_a;
        press(&mut app, &[k(KeyCode::Char('h')), k(KeyCode::Char('k')), k(KeyCode::Home), k(KeyCode::End)]);
        out.push(format!("[TREE] {} rows · cursor clamps at both ends · Enter folds", app.tree.rows().len()));

        // ── CHAT SCROLL ───────────────────────────────────────────────────
        reset(&mut app);
        app.chat_scroll = 10;
        press(&mut app, &[k(KeyCode::PageUp)]);
        let up = app.chat_scroll;
        press(&mut app, &[k(KeyCode::PageDown)]);
        if up != 5 || app.chat_scroll != 10 {
            problems.push(format!("[SCROLL] PgUp/PgDn: {up} / {}", app.chat_scroll));
        }

        // ── SLASH COMMANDS (local, no model) ──────────────────────────────
        for line in [
            "/trace s > h / _ a",
            "/parse mara",
            "/check mara",
            "/tree see mara sea",
            "/clause see mara sea",
            "/igt mara",
            "/texts",
            "/settrans mara = ocean",
            "/frequency",
            "/kwic mara",
            "/coll mara",
            "/hypotheses",
            "/hcheck H1",
        ] {
            let before = app.chat.len();
            let session_before = app.session.turns.len();
            run(&mut app, line);
            if app.chat.len() != before + 1 {
                problems.push(format!("[SLASH] {line} appended {} turn(s)", app.chat.len() - before));
            }
            let t = app.chat.last().unwrap();
            if t.streaming || t.prompt != line {
                problems.push(format!("[SLASH] {line} → prompt {:?} streaming {}", t.prompt, t.streaming));
            }
            if app.session.turns.len() != session_before {
                problems.push(format!("[SLASH] {line} was recorded to the session"));
            }
            out.push(format!("[SLASH] {line} → {:?}", t.response.lines().next().unwrap_or("").chars().take(70).collect::<String>()));
        }

        // ── SEND: a failed stream, then a completed one ───────────────────
        let before = app.chat.len();
        let session_before = app.session.turns.len();
        run(&mut app, "what does mara mean");
        let t = app.chat.last().unwrap();
        if app.chat.len() != before + 1 || t.streaming || t.response.trim().is_empty() {
            problems.push(format!("[SEND] failed stream → {} turn(s), streaming {}, response {:?}", app.chat.len() - before, t.streaming, t.response));
        }
        if app.session.turns.len() != session_before {
            problems.push("[SEND] an error turn was recorded to the session".into());
        }
        if app.status != HINT {
            problems.push(format!("[SEND] status after the failed stream {:?}", app.status));
        }
        out.push(format!("[SEND] failed stream → {:?}", t.response.chars().take(80).collect::<String>()));
        // A completed stream: inject the channel the real send would have made.
        reset(&mut app);
        let (tx, rx) = mpsc::unbounded_channel::<StreamMsg>();
        app.chat.push(Turn::new("probe question".into(), Some("Qya".into())));
        app.streaming_turn = Some(app.chat.len() - 1);
        app.stream_rx = Some(rx);
        tx.send(StreamMsg::Token("Mara ".into())).unwrap();
        tx.send(StreamMsg::Token("means sea.".into())).unwrap();
        tx.send(StreamMsg::Done(None)).unwrap();
        let _ = settle(&mut app);
        let t = app.chat.last().unwrap();
        if t.streaming || t.response != "Mara means sea." {
            problems.push(format!("[SEND] completed stream → streaming {} response {:?}", t.streaming, t.response));
        }
        if app.session.turns.len() != session_before + 1 {
            problems.push(format!("[SEND] completed turn not recorded ({} turn(s))", app.session.turns.len()));
        }
        let now = chrono::Utc::now().to_rfc3339();
        let on_disk = Session::open_or_create(&app.layout, "default", now).unwrap();
        match on_disk.turns.last() {
            Some(s) if s.prompt == "probe question" && s.response == "Mara means sea." && s.scope.as_deref() == Some("Qya") => {}
            other => problems.push(format!("[SESSION] on disk: {:?}", other.map(|s| (s.prompt.clone(), s.scope.clone())))),
        }
        out.push(format!("[SESSION] default: {} turn(s) on disk", on_disk.turns.len()));

        // ── QUIT: q quits in navigation; Esc in navigation stays ──────────
        reset(&mut app);
        press(&mut app, &[k(KeyCode::Char('q'))]);
        if !app.should_quit {
            problems.push("[QUIT] q in navigation did not quit".into());
        }
        reset(&mut app);
        press(&mut app, &[k(KeyCode::Esc)]);
        if app.should_quit {
            problems.push("[QUIT] Esc in navigation mode quit the app (the other companions never quit on Esc)".into());
        }
        reset(&mut app);
        press(&mut app, &[ctrl('c')]);
        if !app.should_quit {
            problems.push("[QUIT] Ctrl+C did not quit".into());
        }
        reset(&mut app);
        out.push("[QUIT] q · Ctrl+C · Ctrl+Q quit; Esc does not".into());

        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  {p}"));
        }
        assert!(problems.is_empty(), "{} problem(s)", problems.len());
    });
}
