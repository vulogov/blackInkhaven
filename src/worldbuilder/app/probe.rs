//! Worldbuilder probe (3.21) — drives `inkhaven worldbuilder` headlessly
//! through its `TuiHost` hooks: every battery key in every pane, right-pane
//! view, overlay and map tool at nine terminal sizes, then scripted checks of
//! the contracts `WORLDBUILDING.md` and `KEYBINDING.md` state — the Tab and
//! Ctrl+R cycles, the size guard, every tree key, the prompt's Esc, each
//! shaping command through the y/n overlay into the pending delta and
//! `/write`, the local commands (`/compile` … `/export`), `/wfact`,
//! `/research`, the interview, the whole map editor (keys and a mouse click),
//! session switching and sizing persistence, and a failed chat stream.
//!
//! It opens a real `Store` (cached embedding model), compiles the shipped
//! Earth definition, and its chat attempt hits a closed local port, so it is
//! `#[ignore]`d from the default run:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture --test-threads=1 worldbuilder_probe
//! ```
use super::*;
use crate::probe_kit::{battery, companion_project, ctrl, k, km, mouse, panic_text, render_text, run_probe, SIZES};
use crossterm::event::{MouseButton, MouseEventKind};
use std::panic::{catch_unwind, AssertUnwindSafe};

const QUICK: [(u16, u16); 3] = [(40, 12), (100, 30), (200, 60)];

fn send(app: &mut WorldbuilderApp, key: KeyEvent) -> Result<(), String> {
    match catch_unwind(AssertUnwindSafe(|| app.on_key(key))) {
        Ok(()) => Ok(()),
        Err(p) => Err(format!("PANIC {}", panic_text(p))),
    }
}

fn frame(app: &WorldbuilderApp, w: u16, h: u16) -> Result<String, String> {
    render_text(w, h, |f| app.render(f))
}

fn press(app: &mut WorldbuilderApp, keys: &[KeyEvent]) {
    for key in keys {
        send(app, *key).unwrap();
    }
}

fn type_text(app: &mut WorldbuilderApp, s: &str) {
    for c in s.chars() {
        send(app, k(KeyCode::Char(c))).unwrap();
    }
}

fn settle(app: &mut WorldbuilderApp) -> bool {
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

fn clear_query(app: &mut WorldbuilderApp) {
    app.query.select_all();
    app.query.cut();
}

fn reset(app: &mut WorldbuilderApp) {
    clear_query(app);
    app.hjson_preview = None;
    app.map_input = None;
    app.map_tool = None;
    app.interview = None;
    app.map_edit = false;
    app.zoom = None;
    app.should_quit = false;
    app.stream_rx = None;
    app.streaming_turn = None;
    app.focus = Focus::QueryPrompt;
    app.right_pane = RightPane::Chat;
}

/// Type a line into the prompt and send it.
fn run(app: &mut WorldbuilderApp, line: &str) {
    reset(app);
    type_text(app, line);
    press(app, &[k(KeyCode::Enter)]);
    let _ = settle(app);
}

/// Run a shaping command and answer the overlay with `answer`.
fn shape(app: &mut WorldbuilderApp, line: &str, answer: char, problems: &mut Vec<String>) {
    run(app, line);
    if app.focus != Focus::ConfirmationOverlay || app.hjson_preview.is_none() {
        problems.push(format!("[SHAPE] {line} did not open the overlay · {:?}", app.status));
        return;
    }
    press(app, &[k(KeyCode::Char(answer))]);
    if app.focus != Focus::QueryPrompt || app.hjson_preview.is_some() {
        problems.push(format!("[SHAPE] {line} {answer}: focus {:?} overlay {}", app.focus, app.hjson_preview.is_some()));
    }
}

fn invariants(app: &WorldbuilderApp) -> Option<String> {
    for (name, tree) in [("facts", &app.facts_tree), ("world", &app.world_tree)] {
        let rows = tree.rows().len();
        if rows > 0 && tree.cursor >= rows {
            return Some(format!("{name} cursor {} past {rows} rows", tree.cursor));
        }
    }
    if !(2..=8).contains(&app.left_split) || !(2..=8).contains(&app.split_ratio) {
        return Some(format!("sizing out of 2..=8: {} {}", app.left_split, app.split_ratio));
    }
    if let Some((w, h)) = app.map_source_dims() {
        let (x, y) = app.map_cursor;
        if x >= w || y >= h {
            return Some(format!("map cursor ({x},{y}) outside {w}x{h}"));
        }
    }
    None
}

fn render_all(app: &WorldbuilderApp, label: &str, problems: &mut Vec<String>) {
    for (w, h) in SIZES {
        if let Err(e) = frame(app, w, h) {
            problems.push(format!("[{label}] render {w}x{h}: {e}"));
        }
    }
}

fn last_turn(app: &WorldbuilderApp) -> (String, String) {
    app.chat.last().map(|t| (t.prompt.clone(), t.response.clone())).unwrap_or_default()
}

#[test]
#[ignore]
fn worldbuilder_probe() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    run_probe("worldbuilder", |out| {
        let project = companion_project();
        let store = Store::open(project.layout.clone(), &project.cfg).expect("open store");
        let hierarchy = Hierarchy::load(&store).unwrap();
        let inv = WorldbuilderInvocation { session: None, interview: false, from_map: false };
        let mut app = WorldbuilderApp::new(project.layout.clone(), project.cfg.clone(), store, hierarchy, inv)
            .expect("WorldbuilderApp::new");
        let mut problems: Vec<String> = Vec::new();
        let facts = project.fact_ids.clone();
        out.push(format!(
            "[FIXTURE] focus {:?} · facts rows {} · world rows {} · plausibility {:?} · status {:?}",
            app.focus,
            app.facts_tree.rows().len(),
            app.world_tree.rows().len(),
            app.plausibility_score,
            app.status
        ));
        if app.plausibility_score.is_none() {
            problems.push("[OPEN] Earth.hjson gave no plausibility score".into());
        }

        // Compile once so the Map pane and editor have a grid.
        run(&mut app, "/compile");
        if app.compiled_layers.is_none() || !last_turn(&app).1.starts_with("Compiled world state") {
            problems.push(format!("[COMPILE] /compile → layers {} · {:?}", app.compiled_layers.is_some(), last_turn(&app).1.chars().take(60).collect::<String>()));
        }
        let dims = app.map_source_dims();
        out.push(format!("[COMPILE] grid {dims:?} · {:?}", app.status));

        // ── BATTERY ───────────────────────────────────────────────────────
        type Enter = Box<dyn Fn(&mut WorldbuilderApp)>;
        let states: Vec<(&str, Enter)> = vec![
            ("prompt", Box::new(reset)),
            ("facts", Box::new(|a| { reset(a); a.focus = Focus::FactsPane; })),
            ("world", Box::new(|a| { reset(a); a.focus = Focus::WorldPane; })),
            ("right-chat", Box::new(|a| { reset(a); a.focus = Focus::RightPane; })),
            ("right-research", Box::new(|a| { reset(a); a.focus = Focus::RightPane; a.right_pane = RightPane::Research; })),
            ("right-ledger", Box::new(|a| { reset(a); a.focus = Focus::RightPane; a.right_pane = RightPane::Ledger; })),
            ("right-map", Box::new(|a| { reset(a); a.focus = Focus::RightPane; a.right_pane = RightPane::Map; })),
            ("map-edit", Box::new(|a| {
                reset(a);
                a.focus = Focus::RightPane;
                a.right_pane = RightPane::Map;
                press(a, &[k(KeyCode::Char('e'))]);
            })),
            ("map-river", Box::new(|a| {
                reset(a);
                a.focus = Focus::RightPane;
                a.right_pane = RightPane::Map;
                press(a, &[k(KeyCode::Char('e')), k(KeyCode::Char('r'))]);
            })),
            ("map-input", Box::new(|a| {
                reset(a);
                a.focus = Focus::RightPane;
                a.right_pane = RightPane::Map;
                press(a, &[k(KeyCode::Char('e')), k(KeyCode::Char('t'))]);
            })),
            ("overlay", Box::new(|a| {
                reset(a);
                type_text(a, "/set name Battery");
                press(a, &[k(KeyCode::Enter)]);
            })),
            ("interview", Box::new(|a| {
                reset(a);
                type_text(a, "/interview");
                press(a, &[k(KeyCode::Enter)]);
            })),
        ];
        let keys = battery();
        for (name, enter) in &states {
            enter(&mut app);
            let state_ok = match *name {
                "map-edit" => app.map_edit,
                "map-river" => app.map_tool.is_some(),
                "map-input" => app.map_input.is_some(),
                "overlay" => app.focus == Focus::ConfirmationOverlay,
                "interview" => app.interview.is_some(),
                _ => true,
            };
            if !state_ok {
                problems.push(format!("[{name}] state did not open · {:?}", app.status));
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
        // The battery's `y` on the overlay and its map placements left deltas; drop them.
        run(&mut app, "/reset");
        if !app.pending_ops.is_empty() {
            problems.push(format!("[RESET] {} op(s) survived /reset", app.pending_ops.len()));
        }
        app.facts_pins.clear();
        app.world_pins.clear();
        app.facts_filter_world = false;
        app.map_terrain = None;

        // ── FOCUS: Tab / Shift+Tab; Ctrl+R; size guard; ? and quit ────────
        reset(&mut app);
        let mut seen = vec![app.focus];
        for _ in 0..4 {
            press(&mut app, &[k(KeyCode::Tab)]);
            seen.push(app.focus);
        }
        if seen != [Focus::QueryPrompt, Focus::RightPane, Focus::FactsPane, Focus::WorldPane, Focus::QueryPrompt] {
            problems.push(format!("[FOCUS] Tab cycle {seen:?}"));
        }
        press(&mut app, &[k(KeyCode::BackTab)]);
        if app.focus != Focus::WorldPane {
            problems.push(format!("[FOCUS] Shift+Tab from the prompt → {:?}", app.focus));
        }
        app.focus = Focus::QueryPrompt;
        let mut panes = vec![app.right_pane];
        for _ in 0..4 {
            press(&mut app, &[ctrl('r')]);
            panes.push(app.right_pane);
        }
        if panes != [RightPane::Chat, RightPane::Research, RightPane::Map, RightPane::Ledger, RightPane::Chat] {
            problems.push(format!("[FOCUS] Ctrl+R cycle {panes:?}"));
        }
        if !app.status.starts_with("right pane:") {
            problems.push(format!("[FOCUS] Ctrl+R status {:?}", app.status));
        }
        out.push(format!("[FOCUS] Tab {seen:?} · Ctrl+R {panes:?}"));
        reset(&mut app);
        for (w, h, small) in [(39, 10, true), (40, 9, true), (40, 10, false), (120, 40, false)] {
            let text = frame(&app, w, h).unwrap();
            if text.contains("Terminal too small") != small {
                problems.push(format!("[SIZE] {w}x{h}: small-guard shown = {}", !small));
            }
        }
        app.focus = Focus::FactsPane;
        let hints = app.show_hints;
        press(&mut app, &[k(KeyCode::Char('?'))]);
        if app.show_hints == hints {
            problems.push("[HINTS] ? in a pane did not toggle".into());
        }
        press(&mut app, &[k(KeyCode::Char('?'))]);
        reset(&mut app);
        press(&mut app, &[k(KeyCode::Char('?'))]);
        if app.show_hints != hints || app.query.lines().join("") != "?" {
            problems.push("[HINTS] ? in the prompt should type".into());
        }
        reset(&mut app);
        press(&mut app, &[k(KeyCode::Char('q'))]);
        if app.should_quit {
            problems.push("[QUIT] q in the prompt quit".into());
        }
        press(&mut app, &[ctrl('q')]);
        let ctrl_q = app.should_quit;
        reset(&mut app);
        app.focus = Focus::WorldPane;
        press(&mut app, &[k(KeyCode::Char('q'))]);
        if !ctrl_q || !app.should_quit {
            problems.push(format!("[QUIT] Ctrl+Q {ctrl_q} · q in a pane {}", app.should_quit));
        }
        reset(&mut app);
        out.push("[SIZE] 39x10 and 40x9 → guard · 40x10 → panes · ? · q / Ctrl+Q".into());

        // ── TREES: nav, pin, fact:world tag, filter, zoom, sizing, edit stubs ─
        reset(&mut app);
        app.focus = Focus::FactsPane;
        press(&mut app, &[k(KeyCode::Char('g'))]);
        if app.facts_tree.rows().len() > 1 {
            press(&mut app, &[k(KeyCode::Enter)]); // fold the chapter first
        }
        let rows = app.facts_tree.rows().len();
        press(&mut app, &[k(KeyCode::Char('l'))]);
        let opened = app.facts_tree.rows().len();
        press(&mut app, &[k(KeyCode::Char('h'))]);
        if rows != 1 || opened <= rows || app.facts_tree.rows().len() != rows {
            problems.push(format!("[TREE] l/h: {rows} → {opened} → {}", app.facts_tree.rows().len()));
        }
        press(&mut app, &[k(KeyCode::Enter), k(KeyCode::Char('G'))]);
        if app.facts_tree.cursor + 1 != app.facts_tree.rows().len() {
            problems.push("[TREE] G did not reach the last row".into());
        }
        assert!(app.facts_tree.reveal(&app.hierarchy, facts[0]));
        press(&mut app, &[ctrl('p')]);
        let pinned = app.facts_pins.clone();
        press(&mut app, &[ctrl('p')]);
        if pinned != vec![facts[0]] || !app.facts_pins.is_empty() {
            problems.push(format!("[TREE] Ctrl+P pin toggle: {pinned:?} then {:?}", app.facts_pins));
        }
        press(&mut app, &[ctrl('t')]);
        let tagged = app.hierarchy.get(facts[0]).is_some_and(|n| n.tags.iter().any(|t| t == FACT_WORLD_TAG));
        if !tagged || !app.status.contains("tagged fact:world") {
            problems.push(format!("[TREE] Ctrl+T did not tag: {tagged} · {:?}", app.status));
        }
        let shown = frame(&app, 120, 40).unwrap();
        if !shown.contains('◎') {
            problems.push("[TREE] the ◎ glyph is missing after Ctrl+T".into());
        }
        press(&mut app, &[k(KeyCode::Char('F'))]);
        let filtered = app.facts_filter_world;
        let filtered_rows = app.facts_tree.rows().len();
        press(&mut app, &[k(KeyCode::Char('F'))]);
        if !filtered || app.facts_filter_world {
            problems.push("[TREE] F did not toggle the fact:world filter".into());
        }
        out.push(format!("[TREE] filter rows {filtered_rows} (of {})", app.facts_tree.rows().len()));
        press(&mut app, &[ctrl('t')]);
        if app.hierarchy.get(facts[0]).is_some_and(|n| n.tags.iter().any(|t| t == FACT_WORLD_TAG)) {
            problems.push("[TREE] second Ctrl+T did not untag".into());
        }
        press(&mut app, &[k(KeyCode::Char('z'))]);
        let z = app.zoom;
        press(&mut app, &[k(KeyCode::Char('z'))]);
        if z != Some(Focus::FactsPane) || app.zoom.is_some() {
            problems.push(format!("[TREE] z toggle: {z:?} then {:?}", app.zoom));
        }
        let (ls, sr) = (app.left_split, app.split_ratio);
        press(&mut app, &[k(KeyCode::Char('}')), k(KeyCode::Char(']'))]);
        let grown = (app.left_split, app.split_ratio);
        press(&mut app, &[k(KeyCode::Char('{')), k(KeyCode::Char('['))]);
        if grown != ((ls + 1).min(8), (sr + 1).min(8)) || (app.left_split, app.split_ratio) != (ls, sr) {
            problems.push(format!("[TREE] sizing: ({ls},{sr}) → {grown:?} → ({},{})", app.left_split, app.split_ratio));
        }
        for _ in 0..10 {
            press(&mut app, &[k(KeyCode::Char('{'))]);
        }
        if app.left_split != 2 || app.session.left_split != 2 {
            problems.push(format!("[TREE] {{ clamps at 2 and persists: {} / {}", app.left_split, app.session.left_split));
        }
        for _ in 0..(ls - 2) {
            press(&mut app, &[k(KeyCode::Char('}'))]);
        }
        press(&mut app, &[k(KeyCode::Char('n'))]);
        if !app.status.contains("later phase") {
            problems.push(format!("[TREE] n in the Facts pane → {:?}", app.status));
        }
        app.focus = Focus::WorldPane;
        press(&mut app, &[k(KeyCode::Char('g')), k(KeyCode::Char('e'))]);
        out.push(format!("[TREE] World pane e → {:?}", app.status));
        reset(&mut app);

        // ── PROMPT: Esc clears, then returns to the previous pane ─────────
        app.focus = Focus::WorldPane;
        press(&mut app, &[k(KeyCode::Char('j')), k(KeyCode::Tab)]);
        if app.focus != Focus::QueryPrompt {
            problems.push(format!("[PROMPT] Tab from World → {:?}", app.focus));
        }
        type_text(&mut app, "draft");
        press(&mut app, &[k(KeyCode::Esc)]);
        if !app.query.is_empty() || app.focus != Focus::QueryPrompt {
            problems.push("[PROMPT] first Esc should clear and keep focus".into());
        }
        press(&mut app, &[k(KeyCode::Esc)]);
        if app.focus != Focus::WorldPane {
            problems.push(format!("[PROMPT] second Esc → {:?} (expected the pane we came from)", app.focus));
        }
        out.push("[PROMPT] Esc clears, then returns to the previous pane".into());

        // ── SHAPING: overlay y/n, pending delta, /diff /undo /reset /write ─
        shape(&mut app, "/set name Velmar", 'n', &mut problems);
        if !app.pending_ops.is_empty() || !app.status.contains("discarded") {
            problems.push(format!("[SHAPE] n left {} op(s) · {:?}", app.pending_ops.len(), app.status));
        }
        shape(&mut app, "/set name Velmar", 'y', &mut problems);
        if app.pending_ops.len() != 1 || !app.status.starts_with('✓') {
            problems.push(format!("[SHAPE] y → {} op(s) · {:?}", app.pending_ops.len(), app.status));
        }
        let preview = frame(&app, 120, 40).unwrap();
        let _ = preview;
        run(&mut app, "/diff");
        if !app.status.starts_with("1 pending delta") {
            problems.push(format!("[SHAPE] /diff → {:?}", app.status));
        }
        for (line, ops) in [
            ("/star G", 2usize),
            ("/tilt 23", 1),
            ("/moon Luna 27", 1),
            ("/magic on", 1),
            ("/rule hard fire,water Fire beats water", 2),
            ("/nation Velmar 3 4", 1),
            ("/orogeny active", 1),
        ] {
            let before = app.pending_ops.len();
            shape(&mut app, line, 'y', &mut problems);
            let added = app.pending_ops.len() - before;
            if added == 0 {
                problems.push(format!("[SHAPE] {line} added no op · {:?}", app.status));
            }
            out.push(format!("[SHAPE] {line} → +{added} op(s) (expected ~{ops}) · {:?}", app.status.chars().take(60).collect::<String>()));
        }
        let n = app.pending_ops.len();
        run(&mut app, "/undo");
        if app.pending_ops.len() >= n {
            problems.push(format!("[SHAPE] /undo: {n} → {}", app.pending_ops.len()));
        }
        if app.session.pending_ops.len() != app.pending_ops.len() {
            problems.push("[SHAPE] the session did not mirror the pending delta".into());
        }
        let score_before = app.plausibility_score;
        run(&mut app, "/write");
        let world_text = std::fs::read_to_string(app.layout.root.join("world.hjson")).unwrap_or_default();
        if !app.pending_ops.is_empty() || !world_text.contains("Velmar") {
            problems.push(format!("[WRITE] pending {} · Velmar in world.hjson {} · {:?}", app.pending_ops.len(), world_text.contains("Velmar"), app.status));
        }
        out.push(format!("[WRITE] committed · plausibility {score_before:?} → {:?} · {:?}", app.plausibility_score, app.status));
        run(&mut app, "/write");
        if app.status != "nothing to write" {
            problems.push(format!("[WRITE] empty /write → {:?}", app.status));
        }
        shape(&mut app, "/set name Battery", 'y', &mut problems);
        run(&mut app, "/reset");
        if !app.pending_ops.is_empty() || !app.status.starts_with("reset") {
            problems.push(format!("[SHAPE] /reset → {} op(s) · {:?}", app.pending_ops.len(), app.status));
        }

        // ── LOCAL COMMANDS ────────────────────────────────────────────────
        run(&mut app, "/validate");
        if !last_turn(&app).0.starts_with("/validate") {
            problems.push(format!("[CMD] /validate → {:?} · {:?}", last_turn(&app).0, app.status));
        }
        run(&mut app, "/roll 2");
        let (p, body) = last_turn(&app);
        if p != "/roll 2" || !body.contains("candidate") {
            problems.push(format!("[CMD] /roll 2 → {p:?} {:?}", body.chars().take(60).collect::<String>()));
        }
        if let Some(def) = app.current_world_def() {
            let seed = def.seed_u64().wrapping_add(1);
            shape(&mut app, &format!("/adopt 0x{seed:x}"), 'y', &mut problems);
            if app.pending_ops.len() != 1 {
                problems.push(format!("[CMD] /adopt → {} op(s) · {:?}", app.pending_ops.len(), app.status));
            }
            run(&mut app, "/reset");
        }
        run(&mut app, "/map");
        if app.right_pane != RightPane::Map || !app.status.contains("ASCII") {
            problems.push(format!("[CMD] /map → {:?} · {:?}", app.right_pane, app.status));
        }
        let map_text = frame(&app, 160, 50).unwrap();
        out.push(format!("[CMD] /map ASCII pane rendered ({} non-space cells)", map_text.chars().filter(|c| !c.is_whitespace()).count()));
        run(&mut app, "/mapcheck");
        out.push(format!("[CMD] /mapcheck → {:?}", last_turn(&app).1.lines().next().unwrap_or("").chars().take(70).collect::<String>()));
        run(&mut app, "/terrain");
        if !app.status.contains("no terrain edits yet") {
            problems.push(format!("[CMD] /terrain without edits → {:?}", app.status));
        }
        for cmd in ["/journey", "/sessions"] {
            run(&mut app, cmd);
            if last_turn(&app).0 != cmd {
                problems.push(format!("[CMD] {cmd} → {:?}", last_turn(&app).0));
            }
        }
        run(&mut app, "/export");
        let exports = app.layout.root.join("exports");
        let n_exports = std::fs::read_dir(&exports).map(|d| d.count()).unwrap_or(0);
        if n_exports == 0 || !app.status.starts_with("exported") {
            problems.push(format!("[CMD] /export → {n_exports} file(s) · {:?}", app.status));
        }
        run(&mut app, "/frobnicate");
        if app.status.is_empty() {
            problems.push("[CMD] an unknown command gave no status".into());
        }
        out.push(format!("[CMD] unknown → {:?}", app.status));
        run(&mut app, "/switch probe two");
        if app.session.name != "probe two" {
            problems.push(format!("[SESSION] /switch → {:?} · {:?}", app.session.name, app.status));
        }
        run(&mut app, "/switch default");
        if app.session.name != "default" {
            problems.push(format!("[SESSION] /switch back → {:?}", app.session.name));
        }
        let now = chrono::Utc::now().to_rfc3339();
        let on_disk = WorldbuilderSession::open_or_create(&app.layout, "default", now).unwrap();
        if on_disk.left_split != app.left_split || on_disk.split_ratio != app.split_ratio {
            problems.push(format!("[SESSION] sizing on disk ({},{}) ≠ live ({},{})", on_disk.left_split, on_disk.split_ratio, app.left_split, app.split_ratio));
        }
        out.push(format!("[SESSION] default: {} turn(s) on disk · sizing ({},{})", on_disk.turns.len(), on_disk.left_split, on_disk.split_ratio));

        // ── WORLD FACTS: /wfact, /research, the Research pane ─────────────
        run(&mut app, "/wfact The fleet wintered in Velmar harbour after the 1821 freeze.");
        let wfact = app.hierarchy.iter().find(|n| n.title.starts_with("The fleet wintered")).cloned();
        match &wfact {
            Some(n) if n.tags.iter().any(|t| t == FACT_WORLD_TAG) => {}
            other => problems.push(format!("[WFACT] {:?} · {:?}", other.as_ref().map(|n| n.tags.clone()), app.status)),
        }
        run(&mut app, "/research harbour freeze");
        if app.right_pane != RightPane::Research || app.research_hits.is_empty() {
            problems.push(format!("[RESEARCH] {} hit(s) · {:?} · {:?}", app.research_hits.len(), app.right_pane, app.status));
        }
        out.push(format!("[RESEARCH] {} hit(s) · top {:?}", app.research_hits.len(), app.research_hits.first().map(|h| h.id)));
        app.focus = Focus::RightPane;
        press(&mut app, &[k(KeyCode::Char('j'))]);
        let moved = app.research_cursor;
        press(&mut app, &[k(KeyCode::Char('k')), k(KeyCode::Char('a'))]);
        let top_id = app.research_hits.first().map(|h| h.id);
        let promoted = top_id.and_then(|id| app.hierarchy.get(id)).is_some_and(|n| n.tags.iter().any(|t| t == FACT_WORLD_TAG));
        if (app.research_hits.len() > 1 && moved != 1) || !promoted {
            problems.push(format!("[RESEARCH] j → {moved} · a promoted {promoted} · {:?}", app.status));
        }
        let shown = frame(&app, 120, 40).unwrap();
        if !shown.contains('◎') {
            problems.push("[RESEARCH] the ◎ marker is missing in the Research pane".into());
        }

        // ── INTERVIEW ─────────────────────────────────────────────────────
        run(&mut app, "/interview");
        let progress = app.interview.as_ref().map(|iv| iv.progress());
        if app.interview.is_none() || app.focus != Focus::QueryPrompt || app.right_pane != RightPane::Chat {
            problems.push(format!("[INTERVIEW] /interview → {} {:?} {:?}", app.interview.is_some(), app.focus, app.right_pane));
        }
        type_text(&mut app, "Velmar");
        press(&mut app, &[k(KeyCode::Enter)]);
        let after = app.interview.as_ref().map(|iv| iv.progress());
        if after <= progress || app.pending_ops.is_empty() {
            problems.push(format!("[INTERVIEW] answer: {progress:?} → {after:?} · {} op(s) · {:?}", app.pending_ops.len(), app.status));
        }
        type_text(&mut app, "/diff");
        press(&mut app, &[k(KeyCode::Enter)]);
        if app.interview.is_none() || !app.status.contains("pending delta") {
            problems.push(format!("[INTERVIEW] /diff mid-interview → {:?}", app.status));
        }
        press(&mut app, &[k(KeyCode::Esc)]);
        if app.interview.is_some() || !app.status.contains("interview left") || app.pending_ops.is_empty() {
            problems.push(format!("[INTERVIEW] Esc → interview {} · {} op(s) · {:?}", app.interview.is_some(), app.pending_ops.len(), app.status));
        }
        out.push(format!("[INTERVIEW] {progress:?} → {after:?} · Esc keeps {} pending op(s)", app.pending_ops.len()));
        run(&mut app, "/reset");

        // ── MAP EDITOR ────────────────────────────────────────────────────
        reset(&mut app);
        app.focus = Focus::RightPane;
        app.right_pane = RightPane::Map;
        press(&mut app, &[k(KeyCode::Char('e'))]);
        let (w, h) = app.map_source_dims().unwrap_or((0, 0));
        if !app.map_edit || w == 0 {
            problems.push(format!("[MAP] e → edit {} dims {w}x{h} · {:?}", app.map_edit, app.status));
        }
        app.map_cursor = (0, 0);
        press(&mut app, &[k(KeyCode::Char('l')), k(KeyCode::Char('j'))]);
        let coarse = app.map_cursor;
        press(&mut app, &[k(KeyCode::Char('L')), km(KeyCode::Down, KeyModifiers::SHIFT)]);
        let fine = app.map_cursor;
        if coarse.0 == 0 || coarse.1 == 0 || fine != (coarse.0 + 1, coarse.1 + 1) {
            problems.push(format!("[MAP] cursor moves: coarse {coarse:?} fine {fine:?}"));
        }
        for _ in 0..200 {
            press(&mut app, &[k(KeyCode::Char('h')), k(KeyCode::Char('k'))]);
        }
        if app.map_cursor != (0, 0) {
            problems.push(format!("[MAP] cursor did not clamp at the origin: {:?}", app.map_cursor));
        }
        for _ in 0..400 {
            press(&mut app, &[k(KeyCode::Right), k(KeyCode::Down)]);
        }
        if app.map_cursor != (w - 1, h - 1) {
            problems.push(format!("[MAP] cursor did not clamp at the far corner: {:?} of {w}x{h}", app.map_cursor));
        }
        app.map_cursor = (w / 2, h / 2);
        let ops = app.pending_ops.len();
        press(&mut app, &[k(KeyCode::Char('t'))]);
        if app.map_input.as_ref().map(|m| m.label) != Some("Town name") {
            problems.push(format!("[MAP] t → {:?}", app.map_input.as_ref().map(|m| m.label)));
        }
        type_text(&mut app, "Velport");
        press(&mut app, &[k(KeyCode::Enter)]);
        if app.map_input.is_some() || app.pending_ops.len() != ops + 1 {
            problems.push(format!("[MAP] town placement → {} op(s) · {:?}", app.pending_ops.len() - ops, app.status));
        }
        press(&mut app, &[k(KeyCode::Char('n'))]);
        type_text(&mut app, "Old Light");
        press(&mut app, &[k(KeyCode::Enter)]);
        press(&mut app, &[k(KeyCode::Char('g'))]);
        let region_prompt = app.map_input.is_some();
        press(&mut app, &[k(KeyCode::Esc)]);
        if !region_prompt || app.map_input.is_some() || !app.status.contains("cancelled") {
            problems.push(format!("[MAP] g then Esc: prompt {region_prompt} · {:?}", app.status));
        }
        let ops = app.pending_ops.len();
        press(&mut app, &[k(KeyCode::Char('r'))]);
        if !matches!(app.map_tool, Some(MapTool::River { source: None })) {
            problems.push("[MAP] r did not start the river tool".into());
        }
        press(&mut app, &[k(KeyCode::Enter)]);
        if !matches!(app.map_tool, Some(MapTool::River { source: Some(_) })) {
            problems.push(format!("[MAP] Enter did not set the river source · {:?}", app.status));
        }
        press(&mut app, &[k(KeyCode::Char('j')), k(KeyCode::Char('j')), k(KeyCode::Char('l')), k(KeyCode::Enter)]);
        if app.map_input.is_none() {
            problems.push(format!("[MAP] river mouth did not ask for a name · {:?}", app.status));
        }
        type_text(&mut app, "Ebb");
        press(&mut app, &[k(KeyCode::Enter)]);
        if app.map_tool.is_some() || app.pending_ops.len() != ops + 1 {
            problems.push(format!("[MAP] river → {} op(s) · tool {} · {:?}", app.pending_ops.len() - ops, app.map_tool.is_some(), app.status));
        }
        press(&mut app, &[k(KeyCode::Char('o'))]);
        let road = matches!(app.map_tool, Some(MapTool::Road { .. }));
        press(&mut app, &[k(KeyCode::Esc)]);
        if !road || app.map_tool.is_some() || !app.map_edit {
            problems.push(format!("[MAP] o then Esc: road {road} · tool {} · edit {}", app.map_tool.is_some(), app.map_edit));
        }
        press(&mut app, &[k(KeyCode::Char('+'))]);
        if app.map_terrain.is_none() || !app.status.contains("raised") {
            problems.push(format!("[MAP] + did not sculpt · {:?}", app.status));
        }
        press(&mut app, &[k(KeyCode::Char('-'))]);
        let brush = app.map_brush;
        press(&mut app, &[k(KeyCode::Char('.'))]);
        let bigger = app.map_brush;
        press(&mut app, &[k(KeyCode::Char(','))]);
        if bigger != brush + 1 || app.map_brush != brush {
            problems.push(format!("[MAP] brush . , : {brush} → {bigger} → {}", app.map_brush));
        }
        run_terrain(&mut app, &mut problems, out);
        app.focus = Focus::RightPane;
        app.right_pane = RightPane::Map;
        if !app.map_edit {
            press(&mut app, &[k(KeyCode::Char('e'))]);
        }
        app.map_cursor = (w / 2, h / 2);
        let ops = app.pending_ops.len();
        press(&mut app, &[k(KeyCode::Char('d'))]);
        out.push(format!("[MAP] d at the town → {} op(s) · {:?}", app.pending_ops.len() as i64 - ops as i64, app.status));
        press(&mut app, &[k(KeyCode::Char('f'))]);
        out.push(format!("[MAP] f → {:?}", app.status));
        press(&mut app, &[k(KeyCode::Esc)]);
        if app.map_edit {
            problems.push("[MAP] Esc did not leave edit mode".into());
        }
        // Mouse: a left-click inside the Map pane enters edit mode at that cell.
        app.focus = Focus::QueryPrompt;
        app.right_pane = RightPane::Map;
        let _ = frame(&app, 160, 50).unwrap();
        match app.map_pane_rect.get() {
            Some(r) => {
                let (cx, cy) = (r.x + r.width / 2, r.y + r.height / 2);
                app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), cx, cy));
                if !app.map_edit || app.focus != Focus::RightPane || !app.status.starts_with("cursor (") {
                    problems.push(format!("[MOUSE] click: edit {} focus {:?} · {:?}", app.map_edit, app.focus, app.status));
                }
                out.push(format!("[MOUSE] click ({cx},{cy}) → {:?}", app.status));
                app.on_mouse(mouse(MouseEventKind::ScrollDown, cx, cy));
                app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 0, 0));
            }
            None => problems.push("[MOUSE] the Map pane did not record its rect".into()),
        }
        run(&mut app, "/reset");

        // ── CHAT: a failed stream ends cleanly ────────────────────────────
        reset(&mut app);
        let before = app.chat.len();
        run(&mut app, "how cold does the harbour get");
        let (p, body) = last_turn(&app);
        let streaming = app.chat.last().is_some_and(|t| t.streaming);
        if app.chat.len() != before + 1 || p != "how cold does the harbour get" || body.trim().is_empty() || streaming || app.status != "ready" {
            problems.push(format!("[CHAT] failed stream → +{} turn(s) · streaming {streaming} · {:?} · {:?}", app.chat.len() - before, body.chars().take(60).collect::<String>(), app.status));
        }
        out.push(format!("[CHAT] failed stream → {:?} · status {:?}", body.chars().take(80).collect::<String>(), app.status));
        app.focus = Focus::RightPane;
        app.chat_scroll = u16::MAX;
        press(&mut app, &[k(KeyCode::Char('g'))]);
        let top = app.chat_scroll;
        press(&mut app, &[k(KeyCode::Char('j')), k(KeyCode::Char('G'))]);
        if top != 0 || app.chat_scroll != u16::MAX {
            problems.push(format!("[CHAT] g/G scroll: {top} / {}", app.chat_scroll));
        }

        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  {p}"));
        }
        assert!(problems.is_empty(), "{} problem(s)", problems.len());
    });
}

/// `/terrain` after a sculpt writes the DEM and records the `geology.dem` edit.
fn run_terrain(app: &mut WorldbuilderApp, problems: &mut Vec<String>, out: &mut Vec<String>) {
    let ops = app.pending_ops.len();
    run(app, "/terrain");
    let maps = app.layout.root.join("assets").join("maps");
    let n = std::fs::read_dir(&maps).map(|d| d.count()).unwrap_or(0);
    if n == 0 || app.pending_ops.len() != ops + 1 {
        problems.push(format!("[TERRAIN] /terrain → {n} file(s) · +{} op(s) · {:?}", app.pending_ops.len() - ops, app.status));
    }
    out.push(format!("[TERRAIN] {n} DEM file(s) · {:?}", app.status.chars().take(80).collect::<String>()));
}
