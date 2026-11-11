//! Research-companion probe (3.21) — drives `inkhaven research` headlessly
//! through its `TuiHost` hooks: every battery key in every pane and overlay
//! (prompt, Facts tree, chat, help, quick-view, manual entry, tree input,
//! delete confirm, confirmation overlay, chat search, zoom) at nine terminal
//! sizes, then scripted checks of the contracts `KEYBINDING.md` and Tutorial
//! 103 state — focus cycling, the RAG mode cycle, the help overlay, the width
//! guard, every tree-editing key, the prompt's history / Esc / completion, chat
//! scrolling and search, the local `/commands`, the extraction error path, and
//! the confirmation overlay's insert / discard.
//!
//! It opens a real `Store` (cached embedding model) and its chat attempts hit a
//! closed local port, so it is `#[ignore]`d from the default run:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture --test-threads=1 research_probe
//! ```
use super::*;
use crate::probe_kit::{battery, companion_project, ctrl, k, km, panic_text, render_text, run_probe, FACTS, SIZES};
use crate::tui_host::TuiHost;
use std::panic::{catch_unwind, AssertUnwindSafe};

const QUICK: [(u16, u16); 3] = [(40, 12), (100, 30), (200, 60)];

fn send(app: &mut ResearchApp, key: KeyEvent) -> Result<(), String> {
    match catch_unwind(AssertUnwindSafe(|| app.on_key(key))) {
        Ok(()) => Ok(()),
        Err(p) => Err(format!("PANIC {}", panic_text(p))),
    }
}

fn frame(app: &ResearchApp, w: u16, h: u16) -> Result<String, String> {
    render_text(w, h, |f| app.render(f))
}

fn press(app: &mut ResearchApp, keys: &[KeyEvent]) {
    for key in keys {
        send(app, *key).unwrap();
    }
}

fn type_text(app: &mut ResearchApp, s: &str) {
    for c in s.chars() {
        send(app, k(KeyCode::Char(c))).unwrap();
    }
}

/// Pump the async pollers until nothing is in flight (or 15 s pass).
fn settle(app: &mut ResearchApp) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        app.poll_async();
        app.tick();
        if !app.is_busy() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

/// Close every overlay and return to an empty prompt.
fn reset(app: &mut ResearchApp) {
    app.confirmation = None;
    app.manual = None;
    app.tree_input = None;
    app.pending_delete = None;
    app.review = None;
    app.peek = None;
    app.show_help = false;
    app.ctrl_b_pending = false;
    app.zoom = None;
    app.chat_search = None;
    app.should_quit = false;
    app.extracting = None;
    app.stream_rx = None;
    app.focus = Focus::QueryPrompt;
    app.query = TextArea::default();
    app.prompt_history_idx = None;
}

fn select(app: &mut ResearchApp, id: Uuid) {
    app.focus = Focus::FactsTree;
    assert!(app.facts_tree.reveal(&app.hierarchy, id), "reveal {id}");
}

fn title_of(app: &ResearchApp, id: Uuid) -> String {
    app.hierarchy.get(id).map(|n| n.title.clone()).unwrap_or_default()
}

fn last_response(app: &ResearchApp) -> String {
    app.chat_history.last().map(|t| t.response.clone()).unwrap_or_default()
}

fn find_titled(app: &ResearchApp, title: &str) -> Option<Uuid> {
    app.hierarchy.iter().find(|n| n.title == title).map(|n| n.id)
}

fn confirmation(app: &mut ResearchApp, title: &str, body: &str) {
    let book_id = app.facts_tree.root.expect("Facts book");
    let mut t = TextArea::default();
    t.insert_str(title);
    let mut b = TextArea::default();
    b.insert_str(body);
    app.confirmation = Some(ConfirmationState {
        title: t,
        body: b,
        book: TargetBook::Facts,
        book_id,
        target: None,
        field: ConfirmField::Title,
        command: "/fact \"probe\"".into(),
        prov: ProvMeta { origin: "model".into(), query: "probe".into(), detail: String::new() },
        dup_warning: None,
        dup_body: None,
        fc_checked: false,
        fc_verdict: None,
        fc_detail: None,
    });
    app.focus = Focus::ConfirmationOverlay;
}

fn invariants(app: &ResearchApp) -> Option<String> {
    let rows = app.facts_tree.rows().len();
    if rows > 0 && app.facts_tree.cursor >= rows {
        return Some(format!("tree cursor {} past {} rows", app.facts_tree.cursor, rows));
    }
    if !(1..=9).contains(&app.split_ratio) {
        return Some(format!("split_ratio {} out of 1..=9", app.split_ratio));
    }
    None
}

fn render_all(app: &ResearchApp, label: &str, problems: &mut Vec<String>) {
    for (w, h) in SIZES {
        if let Err(e) = frame(app, w, h) {
            problems.push(format!("[{label}] render {w}x{h}: {e}"));
        }
    }
}

#[test]
#[ignore]
fn research_probe() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    run_probe("research", |out| {
        let project = companion_project();
        let store = Store::open(project.layout.clone(), &project.cfg).expect("open store");
        let hierarchy = Hierarchy::load(&store).unwrap();
        let mut app = ResearchApp::new(project.layout.clone(), project.cfg.clone(), store, hierarchy, None)
            .expect("ResearchApp::new");
        let mut problems: Vec<String> = Vec::new();
        let chapter = project.facts_chapter;
        let facts = project.fact_ids.clone();
        out.push(format!(
            "[FIXTURE] thread `{}` · {} tree rows · focus {:?}",
            app.thread.name,
            app.facts_tree.rows().len(),
            app.focus
        ));

        // ── BATTERY: every key in every state ─────────────────────────────
        type Enter = Box<dyn Fn(&mut ResearchApp)>;
        let f0 = facts[0];
        let f3 = facts[3];
        let states: Vec<(&str, Enter)> = vec![
            ("prompt", Box::new(reset)),
            ("facts-tree", Box::new(|a| { reset(a); a.focus = Focus::FactsTree; })),
            ("chat", Box::new(|a| { reset(a); a.focus = Focus::AiChat; })),
            ("help", Box::new(|a| { reset(a); a.show_help = true; })),
            ("peek", Box::new(move |a| { reset(a); select(a, f0); press(a, &[k(KeyCode::Enter)]); })),
            ("manual-title", Box::new(move |a| { reset(a); select(a, f0); press(a, &[k(KeyCode::Char('n'))]); })),
            ("manual-body", Box::new(move |a| {
                reset(a);
                select(a, f0);
                press(a, &[k(KeyCode::Char('n')), k(KeyCode::Char('T')), k(KeyCode::Enter)]);
            })),
            ("tree-input", Box::new(move |a| { reset(a); select(a, chapter); press(a, &[k(KeyCode::Char('R'))]); })),
            ("pending-delete", Box::new(move |a| {
                reset(a);
                if a.hierarchy.get(f3).is_some() {
                    select(a, f3);
                    press(a, &[k(KeyCode::Char('-'))]);
                }
            })),
            ("confirmation", Box::new(|a| { reset(a); confirmation(a, "Battery fact", "A body typed by the battery."); })),
            ("chat-search", Box::new(|a| { reset(a); a.focus = Focus::AiChat; press(a, &[ctrl('f')]); })),
            ("zoom-tree", Box::new(|a| { reset(a); a.focus = Focus::FactsTree; press(a, &[ctrl('z')]); })),
        ];
        let keys = battery();
        for (name, enter) in &states {
            enter(&mut app);
            let state_ok = match *name {
                "peek" => app.peek.is_some(),
                "manual-title" | "manual-body" => app.manual.is_some(),
                "tree-input" => app.tree_input.is_some(),
                "pending-delete" => app.pending_delete.is_some(),
                "confirmation" => app.confirmation.is_some(),
                "chat-search" => app.chat_search.is_some(),
                "zoom-tree" => app.zoom.is_some(),
                "help" => app.show_help,
                _ => true,
            };
            if !state_ok {
                problems.push(format!("[{name}] state did not open"));
            }
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
        reset(&mut app);
        let _ = settle(&mut app);
        app.reload_hierarchy();
        if app.hierarchy.get(f3).is_none() {
            out.push("[BATTERY] pending-delete `y` deleted the throwaway fact (expected)".into());
        }

        // ── FOCUS: Tab / Shift+Tab cycle; F10 cycles RAG ──────────────────
        reset(&mut app);
        let mut seen = vec![app.focus];
        for _ in 0..3 {
            press(&mut app, &[k(KeyCode::Tab)]);
            seen.push(app.focus);
        }
        if seen != [Focus::QueryPrompt, Focus::AiChat, Focus::FactsTree, Focus::QueryPrompt] {
            problems.push(format!("[FOCUS] Tab cycle {seen:?}"));
        }
        press(&mut app, &[k(KeyCode::BackTab)]);
        if app.focus != Focus::FactsTree {
            problems.push(format!("[FOCUS] Shift+Tab from prompt → {:?}", app.focus));
        }
        let start = app.thread.rag_mode.label();
        let mut labels = vec![start];
        for _ in 0..3 {
            press(&mut app, &[k(KeyCode::F(10))]);
            labels.push(app.thread.rag_mode.label());
        }
        if labels[3] != start || labels[0] == labels[1] || labels[1] == labels[2] {
            problems.push(format!("[FOCUS] F10 RAG cycle {labels:?}"));
        }
        out.push(format!("[FOCUS] Tab {seen:?} · F10 {labels:?}"));

        // ── HELP / HINTS / QUIT ───────────────────────────────────────────
        reset(&mut app);
        type_text(&mut app, "abc");
        press(&mut app, &[ctrl('b'), k(KeyCode::Char('h'))]);
        if !app.show_help {
            problems.push("[HELP] Ctrl+B h did not open the help overlay".into());
        }
        let shown = frame(&app, 120, 40).unwrap();
        if !shown.contains("Help (Ctrl+B h)") {
            problems.push("[HELP] overlay title missing at 120x40".into());
        }
        press(&mut app, &[k(KeyCode::Char('x'))]);
        if app.show_help || app.query_text() != "abc" {
            problems.push(format!("[HELP] closing key leaked: help={} query={:?}", app.show_help, app.query_text()));
        }
        reset(&mut app);
        let hints = app.show_hints;
        app.focus = Focus::FactsTree;
        press(&mut app, &[k(KeyCode::Char('?'))]);
        if app.show_hints == hints {
            problems.push("[HINTS] ? in the tree did not toggle the hints bar".into());
        }
        press(&mut app, &[k(KeyCode::Char('?'))]);
        app.focus = Focus::QueryPrompt;
        press(&mut app, &[k(KeyCode::Char('?'))]);
        if app.show_hints != hints || app.query_text() != "?" {
            problems.push("[HINTS] ? in the prompt should type, not toggle".into());
        }
        reset(&mut app);
        press(&mut app, &[k(KeyCode::Char('q'))]);
        if app.should_quit || app.query_text() != "q" {
            problems.push("[QUIT] q in the prompt should type".into());
        }
        press(&mut app, &[ctrl('q')]);
        if !app.should_quit {
            problems.push("[QUIT] Ctrl+Q in the prompt did not quit".into());
        }
        reset(&mut app);
        app.focus = Focus::FactsTree;
        press(&mut app, &[k(KeyCode::Char('q'))]);
        if !app.should_quit {
            problems.push("[QUIT] q in the tree did not quit".into());
        }
        reset(&mut app);
        out.push("[HELP] Ctrl+B h opens, any key closes and is swallowed · ? toggles hints outside the prompt · q/Ctrl+Q".into());

        // ── WIDTH GUARD ───────────────────────────────────────────────────
        let narrow = frame(&app, 79, 24).unwrap();
        let wide = frame(&app, 80, 24).unwrap();
        if !narrow.contains("Terminal too narrow") {
            problems.push("[WIDTH] 79 cols did not show the resize message".into());
        }
        if wide.contains("Terminal too narrow") {
            problems.push("[WIDTH] 80 cols showed the resize message".into());
        }
        out.push("[WIDTH] 79 cols → resize message · 80 cols → panes".into());

        // ── TREE: peek, manual entry, rename, add, delete, move, copy/cut/paste, undisputed, pins ──
        reset(&mut app);
        select(&mut app, facts[0]);
        press(&mut app, &[k(KeyCode::Enter)]);
        match &app.peek {
            Some(p) if p.title == FACTS[0].0 => {}
            Some(p) => problems.push(format!("[PEEK] title {:?} ≠ {:?}", p.title, FACTS[0].0)),
            None => problems.push("[PEEK] Enter on a fact did not open the quick view".into()),
        }
        press(&mut app, &[k(KeyCode::Esc)]);
        if app.peek.is_some() {
            problems.push("[PEEK] Esc did not close".into());
        }
        select(&mut app, chapter);
        let expanded_before = app.facts_tree.rows().len();
        press(&mut app, &[k(KeyCode::Enter)]);
        let folded = app.facts_tree.rows().len();
        press(&mut app, &[k(KeyCode::Enter)]);
        if app.peek.is_some() || folded >= expanded_before || app.facts_tree.rows().len() != expanded_before || app.facts_tree.selected() != Some(chapter) {
            problems.push(format!("[PEEK] Enter on a chapter should fold then unfold in place: {expanded_before} → {folded} → {} · peek {} · selected {:?}", app.facts_tree.rows().len(), app.peek.is_some(), app.facts_tree.selected().map(|id| title_of(&app, id))));
        }

        select(&mut app, facts[0]);
        press(&mut app, &[k(KeyCode::Char('n'))]);
        type_text(&mut app, "Probe manual");
        press(&mut app, &[k(KeyCode::Enter), ctrl('s')]);
        if app.manual.is_none() || !app.status_message.as_deref().unwrap_or("").contains("empty") {
            problems.push(format!("[MANUAL] Ctrl+S with an empty body: open={} · {:?}", app.manual.is_some(), app.status_message));
        }
        type_text(&mut app, "Typed by hand.");
        press(&mut app, &[ctrl('s')]);
        let manual = find_titled(&app, "Probe manual");
        if manual.is_none() || !app.status_message.as_deref().unwrap_or("").starts_with("✓ added fact") {
            problems.push(format!("[MANUAL] n → title → Ctrl+S: node={manual:?} status={:?}", app.status_message));
        }
        let manual = manual.unwrap_or(facts[0]);
        select(&mut app, manual);
        press(&mut app, &[k(KeyCode::Char('R'))]);
        type_text(&mut app, " 2");
        press(&mut app, &[k(KeyCode::Enter)]);
        if title_of(&app, manual) != "Probe manual 2" {
            problems.push(format!("[RENAME] R → {:?}", title_of(&app, manual)));
        }
        press(&mut app, &[k(KeyCode::Char('c'))]);
        type_text(&mut app, "Weather");
        press(&mut app, &[k(KeyCode::Enter)]);
        let weather = find_titled(&app, "Weather");
        let weather_parent = weather.and_then(|w| app.hierarchy.get(w)).and_then(|n| n.parent_id);
        if weather_parent != app.facts_tree.root {
            problems.push(format!("[ADD] c did not add a chapter under the Facts book ({weather_parent:?})"));
        }
        select(&mut app, facts[1]);
        press(&mut app, &[k(KeyCode::Char('s'))]);
        type_text(&mut app, "Tides");
        press(&mut app, &[k(KeyCode::Enter)]);
        let tides = find_titled(&app, "Tides");
        let tides_parent = tides.and_then(|t| app.hierarchy.get(t)).and_then(|n| n.parent_id);
        if tides_parent != Some(chapter) {
            problems.push(format!("[ADD] s did not add a subchapter under the enclosing chapter ({tides_parent:?})"));
        }
        // delete: `-` on a paragraph asks; N cancels; y deletes.
        select(&mut app, manual);
        press(&mut app, &[k(KeyCode::Char('-')), k(KeyCode::Char('N'))]);
        if app.pending_delete.is_some() || app.hierarchy.get(manual).is_none() {
            problems.push("[DELETE] N did not cancel".into());
        }
        press(&mut app, &[k(KeyCode::Char('D'))]);
        if app.pending_delete.is_some() {
            problems.push("[DELETE] D on a paragraph should refuse".into());
        }
        press(&mut app, &[k(KeyCode::Char('-')), k(KeyCode::Char('y'))]);
        if app.hierarchy.get(manual).is_some() {
            problems.push("[DELETE] - then y did not delete the paragraph".into());
        }
        // move: K on the second fact puts it first.
        let pos_of = |app: &ResearchApp, id: Uuid| {
            app.hierarchy.children_of(Some(chapter)).iter().position(|n| n.id == id).unwrap_or(usize::MAX)
        };
        select(&mut app, facts[1]);
        let p0 = pos_of(&app, facts[1]);
        press(&mut app, &[k(KeyCode::Char('K'))]);
        let p1 = pos_of(&app, facts[1]);
        press(&mut app, &[k(KeyCode::Char('J'))]);
        let p2 = pos_of(&app, facts[1]);
        if p0 == 0 || p1 + 1 != p0 || p2 != p0 || app.facts_tree.selected() != Some(facts[1]) {
            problems.push(format!("[MOVE] K/J: {p0} → {p1} → {p2} · selected {:?}", app.facts_tree.selected().map(|id| title_of(&app, id))));
        }
        // copy / paste into Weather, then cut / paste.
        if let Some(weather) = weather {
            select(&mut app, facts[2]);
            press(&mut app, &[k(KeyCode::Char('y'))]);
            select(&mut app, weather);
            press(&mut app, &[k(KeyCode::Char('p'))]);
            let copies = app.hierarchy.iter().filter(|n| n.title == FACTS[2].0).count();
            let in_weather = app.hierarchy.children_of(Some(weather)).iter().any(|n| n.title == FACTS[2].0);
            if copies != 2 || !in_weather {
                problems.push(format!("[COPY] y/p: {copies} copies, in Weather {in_weather} · {:?}", app.status_message));
            }
            select(&mut app, facts[2]);
            press(&mut app, &[k(KeyCode::Char('x'))]);
            select(&mut app, weather);
            press(&mut app, &[k(KeyCode::Char('p'))]);
            let moved = app.hierarchy.get(facts[2]).and_then(|n| n.parent_id) == Some(weather);
            if !moved {
                problems.push(format!("[CUT] x/p did not move the fact · {:?}", app.status_message));
            }
        }
        // undisputed toggle.
        select(&mut app, facts[1]);
        press(&mut app, &[k(KeyCode::Char('u'))]);
        let tagged = app.hierarchy.get(facts[1]).is_some_and(|n| n.tags.iter().any(|t| t == super::super::UNDISPUTED_TAG));
        press(&mut app, &[k(KeyCode::Char('u'))]);
        let untagged = app.hierarchy.get(facts[1]).is_some_and(|n| !n.tags.iter().any(|t| t == super::super::UNDISPUTED_TAG));
        if !tagged || !untagged {
            problems.push(format!("[UNDISPUTED] u toggle: tagged={tagged} untagged={untagged}"));
        }
        // pins: max 3.
        app.pinned_nodes.clear();
        let max = app.cfg.research.max_pinned_nodes;
        let mut pin_targets: Vec<Uuid> = app.hierarchy.children_of(Some(chapter)).iter().map(|n| n.id).collect();
        pin_targets.push(chapter);
        for id in pin_targets.iter().take(max + 1) {
            select(&mut app, *id);
            press(&mut app, &[ctrl('p')]);
        }
        if app.pinned_nodes.len() != max || !app.status_message.as_deref().unwrap_or("").starts_with("Max") {
            problems.push(format!("[PIN] {} pinned (max {max}) · {:?}", app.pinned_nodes.len(), app.status_message));
        }
        if app.thread.pinned_nodes.len() != max {
            problems.push(format!("[PIN] thread persisted {} pins", app.thread.pinned_nodes.len()));
        }
        press(&mut app, &[ctrl('p')]);
        // split + zoom.
        let r = app.split_ratio;
        press(&mut app, &[k(KeyCode::Char('>'))]);
        let r2 = app.split_ratio;
        press(&mut app, &[k(KeyCode::Char('<'))]);
        if r2 != (r + 1).min(9) || app.split_ratio != r {
            problems.push(format!("[SPLIT] > < : {r} → {r2} → {}", app.split_ratio));
        }
        press(&mut app, &[ctrl('z')]);
        let z1 = app.zoom;
        press(&mut app, &[ctrl('z')]);
        if z1 != Some(Focus::FactsTree) || app.zoom.is_some() {
            problems.push(format!("[ZOOM] Ctrl+Z toggle: {z1:?} then {:?}", app.zoom));
        }
        out.push("[TREE] peek · manual · rename · chapter/subchapter · delete · move · copy/cut/paste · undisputed · pins · split · zoom".into());

        // ── PROMPT: send, history, Esc, newline, completion ───────────────
        reset(&mut app);
        type_text(&mut app, "hello harbor");
        press(&mut app, &[k(KeyCode::Enter)]);
        let streaming = app.stream_rx.is_some();
        let settled = settle(&mut app);
        let last = app.chat_history.last().map(|t| (t.prompt.clone(), t.response.clone(), t.streaming));
        match &last {
            Some((p, r, s)) if p == "hello harbor" && !r.trim().is_empty() && !*s => {}
            other => problems.push(format!("[SEND] streaming={streaming} settled={settled} last={other:?}")),
        }
        out.push(format!(
            "[SEND] stream started={streaming} settled={settled} · response {:?}",
            last.as_ref().map(|t| t.1.chars().take(60).collect::<String>())
        ));
        press(&mut app, &[k(KeyCode::Up)]);
        if app.query_text() != "hello harbor" {
            problems.push(format!("[HISTORY] ↑ recalled {:?}", app.query_text()));
        }
        press(&mut app, &[k(KeyCode::Down)]);
        if !app.query_text().is_empty() {
            problems.push(format!("[HISTORY] ↓ past the newest left {:?}", app.query_text()));
        }
        type_text(&mut app, "draft");
        press(&mut app, &[k(KeyCode::Esc)]);
        if !app.query_text().is_empty() || app.focus != Focus::QueryPrompt {
            problems.push("[ESC] first Esc should clear the line and keep focus".into());
        }
        press(&mut app, &[k(KeyCode::Esc)]);
        if app.focus != Focus::FactsTree {
            problems.push(format!("[ESC] second Esc → {:?}", app.focus));
        }
        reset(&mut app);
        type_text(&mut app, "one");
        press(&mut app, &[km(KeyCode::Enter, KeyModifiers::ALT)]);
        type_text(&mut app, "two");
        if app.query.lines().len() != 2 {
            problems.push(format!("[NEWLINE] Alt+Enter: {} line(s)", app.query.lines().len()));
        }
        reset(&mut app);
        type_text(&mut app, "/fa");
        press(&mut app, &[k(KeyCode::Tab)]);
        if !app.query_text().starts_with("/fact") || app.focus != Focus::QueryPrompt {
            problems.push(format!("[COMPLETE] /fa Tab → {:?} focus {:?}", app.query_text(), app.focus));
        }
        reset(&mut app);
        type_text(&mut app, "/goto facts/har");
        press(&mut app, &[k(KeyCode::Tab)]);
        if !app.query_text().contains("harbor") {
            problems.push(format!("[COMPLETE] path Tab → {:?}", app.query_text()));
        }
        out.push(format!("[PROMPT] history · Esc clear-then-defocus · Alt+Enter newline · Tab completes {:?}", app.query_text()));

        // ── CHAT: scroll, search, Esc ─────────────────────────────────────
        reset(&mut app);
        app.focus = Focus::AiChat;
        app.chat_scroll = 0;
        press(&mut app, &[k(KeyCode::Char('k'))]);
        let up = app.chat_scroll;
        press(&mut app, &[k(KeyCode::Char('g'))]);
        let top = app.chat_scroll;
        press(&mut app, &[k(KeyCode::Char('G'))]);
        if up != 1 || top != u16::MAX || app.chat_scroll != 0 {
            problems.push(format!("[CHAT] scroll k/g/G: {up} {top} {}", app.chat_scroll));
        }
        press(&mut app, &[ctrl('f')]);
        type_text(&mut app, "ha");
        press(&mut app, &[k(KeyCode::Char('n'))]);
        match &app.chat_search {
            Some(s) if s.query == "ha" && s.current == 1 => {}
            other => problems.push(format!("[CHAT] search state {:?}", other.as_ref().map(|s| (s.query.clone(), s.current)))),
        }
        press(&mut app, &[k(KeyCode::Esc)]);
        if app.chat_search.is_some() || app.focus != Focus::AiChat {
            problems.push("[CHAT] Esc should close the search and stay in the chat".into());
        }
        press(&mut app, &[k(KeyCode::Esc)]);
        if app.focus != Focus::QueryPrompt {
            problems.push(format!("[CHAT] Esc → {:?}", app.focus));
        }
        out.push("[CHAT] j/k/g/G · Ctrl+F search n/N/Esc · Esc → prompt".into());

        // ── COMMANDS (local) ──────────────────────────────────────────────
        let run = |app: &mut ResearchApp, cmd: &str| {
            reset(app);
            type_text(app, cmd);
            press(app, &[k(KeyCode::Enter)]);
            let _ = settle(app);
        };
        run(&mut app, "/nonsense");
        if !app.status_message.as_deref().unwrap_or("").contains("unknown command") {
            problems.push(format!("[CMD] /nonsense → {:?}", app.status_message));
        }
        run(&mut app, "/rag facts");
        if app.thread.rag_mode != super::super::thread::RagMode::FactsOnly {
            problems.push(format!("[CMD] /rag facts → {}", app.thread.rag_mode.label()));
        }
        run(&mut app, "/save probe thread");
        if app.thread.display_name != "probe thread" || ResearchThread::load(&app.layout, &app.thread.name).is_none() {
            problems.push(format!("[CMD] /save → {:?} (on disk: {})", app.thread.display_name, ResearchThread::load(&app.layout, &app.thread.name).is_some()));
        }
        run(&mut app, "/calc 2 3 +");
        match app.chat_history.last() {
            Some(t) if t.computed && t.response.contains('5') => {}
            other => problems.push(format!("[CMD] /calc → {:?}", other.map(|t| (t.computed, t.response.clone())))),
        }
        run(&mut app, "/world");
        if !last_response(&app).contains("Astronomy") {
            problems.push(format!("[CMD] /world (bare) should list the layers: {:?}", last_response(&app).chars().take(80).collect::<String>()));
        }
        run(&mut app, "/world Astronomy");
        match app.chat_history.last() {
            Some(t) if t.simulation => {}
            other => problems.push(format!("[CMD] /world Astronomy → {:?}", other.map(|t| t.response.chars().take(80).collect::<String>()))),
        }
        for cmd in ["/sources", "/import", "/stale", "/report", "/review", "/bibliography", "/diff"] {
            let n = app.chat_history.len();
            run(&mut app, cmd);
            out.push(format!(
                "[CMD] {cmd} → +{} turn(s) · {:?}",
                app.chat_history.len() - n,
                app.status_message.as_deref().unwrap_or("").chars().take(70).collect::<String>()
            ));
        }
        run(&mut app, "/goto facts/harbor");
        if app.facts_tree.selected() != Some(chapter) || app.focus != Focus::FactsTree {
            problems.push(format!("[CMD] /goto → selected {:?} focus {:?} · {:?}", app.facts_tree.selected().map(|id| title_of(&app, id)), app.focus, app.status_message));
        }
        run(&mut app, "/clear");
        if !app.chat_history.is_empty() {
            problems.push("[CMD] /clear left turns".into());
        }
        run(&mut app, "/fact \"take the date\"");
        if !app.status_message.as_deref().unwrap_or("").contains("no research response yet") {
            problems.push(format!("[CMD] /fact without a response → {:?}", app.status_message));
        }
        app.chat_history.push(ChatTurn::with_response("when did it freeze".into(), "The harbor froze in January 1821.".into()));
        let before = app.chat_history.len();
        run(&mut app, "/fact \"take the date\"");
        let st = app.status_message.clone().unwrap_or_default();
        if app.extracting.is_some() || app.confirmation.is_some() || !st.contains("extraction error") || app.chat_history.len() != before {
            problems.push(format!("[EXTRACT] after a failed stream: extracting={} confirmation={} turns {}→{} status {st:?}", app.extracting.is_some(), app.confirmation.is_some(), before, app.chat_history.len()));
        }
        out.push(format!("[EXTRACT] failed stream → {:?}", st.chars().take(90).collect::<String>()));

        // ── CONFIRMATION OVERLAY: Tab fields, Ctrl+S inserts, Esc discards ─
        reset(&mut app);
        confirmation(&mut app, "Probe fact", "The customs house opened in 1823.");
        press(&mut app, &[k(KeyCode::Tab)]);
        if app.confirmation.as_ref().map(|c| c.field) != Some(ConfirmField::Body) {
            problems.push("[CONFIRM] Tab did not move to the body".into());
        }
        type_text(&mut app, " It still stands.");
        press(&mut app, &[k(KeyCode::Tab)]);
        if app.confirmation.as_ref().map(|c| c.field) != Some(ConfirmField::Title) {
            problems.push("[CONFIRM] Tab did not return to the title".into());
        }
        press(&mut app, &[k(KeyCode::Enter)]);
        if app.confirmation.as_ref().map(|c| c.title.lines().len()) != Some(1) {
            problems.push("[CONFIRM] Enter in the title added a line".into());
        }
        press(&mut app, &[ctrl('s')]);
        if app.confirmation.as_ref().is_some_and(|c| c.dup_warning.is_some()) {
            out.push(format!("[CONFIRM] near-duplicate warning: {:?}", app.confirmation.as_ref().and_then(|c| c.dup_warning.clone())));
            press(&mut app, &[ctrl('s')]);
        }
        let inserted = find_titled(&app, "Probe fact");
        if inserted.is_none() || app.confirmation.is_some() || app.focus != Focus::QueryPrompt {
            problems.push(format!("[CONFIRM] Ctrl+S: inserted={inserted:?} overlay={} focus={:?} · {:?}", app.confirmation.is_some(), app.focus, app.status_message));
        }
        confirmation(&mut app, "Dropped", "Never inserted.");
        press(&mut app, &[k(KeyCode::Esc)]);
        if app.confirmation.is_some() || app.focus != Focus::QueryPrompt || find_titled(&app, "Dropped").is_some() {
            problems.push("[CONFIRM] Esc did not discard".into());
        }
        confirmation(&mut app, "Empty body", "");
        press(&mut app, &[ctrl('s')]);
        if app.confirmation.is_none() || !app.status_message.as_deref().unwrap_or("").contains("empty") {
            problems.push(format!("[CONFIRM] empty body accepted · {:?}", app.status_message));
        }
        reset(&mut app);
        out.push("[CONFIRM] Tab fields · title is one line · Ctrl+S inserts · Esc discards · empty body refused".into());

        // ── PERSISTENCE ───────────────────────────────────────────────────
        let on_disk = ResearchThread::load(&app.layout, &app.thread.name);
        out.push(format!(
            "[THREAD] `{}` on disk: {} turn(s), {} pin(s), rag {}",
            app.thread.name,
            on_disk.as_ref().map(|t| t.turns.len()).unwrap_or(0),
            on_disk.as_ref().map(|t| t.pinned_nodes.len()).unwrap_or(0),
            on_disk.as_ref().map(|t| t.rag_mode.label()).unwrap_or("-")
        ));

        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  {p}"));
        }
        assert!(problems.is_empty(), "{} problem(s)", problems.len());
    });
}
