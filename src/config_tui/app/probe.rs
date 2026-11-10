//! Config-editor probe (3.20) — drives `inkhaven config` headlessly: every
//! battery key on the tree and inside every modal (each typed widget, the
//! colour sliders, the path picker, the list box, save / rollback / inspect /
//! annotate / map entries) at nine terminal sizes, then scripted checks of
//! the contracts `KEYBINDING.md` states — a chorded letter never fires a
//! bare-key action, every text box takes the shared chord set, lists window
//! around their cursor, the help pane scrolls, a save round-trips through the
//! surgical writer with comments and unknown fields intact.
//!
//! It needs no store, so it runs in the default suite:
//!
//! ```text
//! cargo test --bin inkhaven -- config_tui_probe --nocapture
//! ```
use super::*;
use crate::probe_kit::{alt, battery, ctrl, k, panic_text, render_text, run_probe, SIZES};
use crossterm::event::KeyCode;
use serde_json::json;
use std::panic::{catch_unwind, AssertUnwindSafe};

const SMALL: [(u16, u16); 3] = [(20, 6), (80, 24), (200, 60)];

fn fixture() -> (tempfile::TempDir, App) {
    let tmp = tempfile::tempdir().unwrap();
    let mut v = serde_json::to_value(Config::default()).unwrap();
    v["probe_unknown"] = json!({ "kept": true });
    // A second provider so the map-entry chords have something to delete.
    let first = v["llm"]["providers"]
        .as_object()
        .and_then(|m| m.values().next().cloned())
        .unwrap_or_else(|| json!({}));
    v["llm"]["providers"]["probe-provider"] = first;
    let text = format!(
        "// probe config — leading comment\n# hash comment\n{}\n",
        serde_json::to_string_pretty(&v).unwrap()
    );
    let path = tmp.path().join("inkhaven.hjson");
    std::fs::write(&path, text).unwrap();
    let app = App::load(tmp.path().to_path_buf(), &path).unwrap();
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

fn goto(app: &mut App, path: &str) -> bool {
    let idx = app.rows().iter().position(|(_, n)| n.path == path);
    match idx {
        Some(i) => {
            app.cursor = i;
            true
        }
        None => false,
    }
}

fn leaf_where(app: &App, pred: impl Fn(&SchemaNode) -> bool) -> Option<String> {
    app.rows().iter().find(|(_, n)| n.is_leaf() && pred(n)).map(|(_, n)| n.path.clone())
}

fn node_value(app: &App, path: &str) -> Option<Value> {
    find_in_tree(&app.schema, path).map(|n| n.current.clone())
}

/// `n` backup files with distinct timestamps, so the rollback picker has a
/// list longer than its box (a real save per second would collide on the
/// filename).
fn seed_backups(app: &App, n: usize) {
    let dir = app.project_root.join(".config-backups");
    std::fs::create_dir_all(&dir).unwrap();
    let body = std::fs::read_to_string(&app.cfg_path).unwrap();
    for i in 0..n {
        let name = format!("inkhaven_20260101_10{:02}{:02}.hjson", i / 60, i % 60);
        std::fs::write(dir.join(name), &body).unwrap();
    }
}

fn modal_name(m: &Modal) -> String {
    match m {
        Modal::None => "None".into(),
        Modal::Help { .. } => "Help".into(),
        Modal::Edit { widget, .. } => format!(
            "Edit/{}",
            match widget {
                Widget::Bool(_) => "Bool",
                Widget::Int(_) => "Int",
                Widget::Float(_) => "Float",
                Widget::Text(_) => "Text",
                Widget::Color(_) => "Color",
                Widget::Path(_) => "Path",
                Widget::Enum(_) => "Enum",
                Widget::List(_) => "List",
                Widget::Unsupported(_) => "Unsupported",
            }
        ),
        Modal::SaveConfirm { .. } => "SaveConfirm".into(),
        Modal::Saved { .. } => "Saved".into(),
        Modal::DiscardConfirm { .. } => "DiscardConfirm".into(),
        Modal::Rollback { .. } => "Rollback".into(),
        Modal::RollbackDelete { .. } => "RollbackDelete".into(),
        Modal::RollbackPreview { .. } => "RollbackPreview".into(),
        Modal::Inspector { .. } => "Inspector".into(),
        Modal::Annotate { .. } => "Annotate".into(),
        Modal::AddMapEntry { .. } => "AddMapEntry".into(),
        Modal::DeleteMapEntryConfirm { .. } => "DeleteMapEntryConfirm".into(),
    }
}

fn invariants(app: &App) -> Option<String> {
    let n = app.rows().len();
    if n > 0 && app.cursor >= n {
        return Some(format!("cursor {} past the {} rows", app.cursor, n));
    }
    if app.scroll > app.cursor {
        return Some(format!("scroll {} past cursor {}", app.scroll, app.cursor));
    }
    None
}

#[test]
fn config_tui_probe() {
    run_probe("config-tui", |out| {
        let (_tmp, mut app) = fixture();
        let mut problems: Vec<String> = Vec::new();
        out.push(format!(
            "[FIXTURE] {} rows · {} unknown · {} providers",
            app.rows().len(),
            app.unknowns.len(),
            find_in_tree(&app.schema, "llm.providers").map(|n| n.children.len()).unwrap_or(0)
        ));
        assert!(!app.unknowns.is_empty(), "the unknown field did not survive the load");

        // ── Baseline: every size, no modal ────────────────────────────
        for (w, h) in SIZES {
            if let Err(e) = frame(&mut app, w, h) {
                problems.push(format!("[BASELINE] {w}x{h}: {e}"));
            }
        }

        // ── Battery on the tree ───────────────────────────────────────
        let mut fired = 0usize;
        for (name, key) in battery() {
            app.modal = Modal::None;
            fired += 1;
            if let Err(e) = send(&mut app, key) {
                problems.push(format!("[TREE] {name}: {e}"));
            }
            for (w, h) in SMALL {
                if let Err(e) = frame(&mut app, w, h) {
                    problems.push(format!("[TREE] {name} render {w}x{h}: {e}"));
                }
            }
            // After a frame: the renderer reconciles the scroll with the cursor.
            if let Some(bad) = invariants(&app) {
                problems.push(format!("[TREE] {name}: {bad}"));
            }
        }
        out.push(format!("[BATTERY tree] {fired} keys"));

        // ── Leaves of every widget kind ───────────────────────────────
        let bool_path = leaf_where(&app, |n| matches!(n.ty, ConfigType::Bool)).expect("a bool leaf");
        let int_path = leaf_where(&app, |n| matches!(n.ty, ConfigType::Int)).expect("an int leaf");
        let float_path = leaf_where(&app, |n| matches!(n.ty, ConfigType::Float));
        let text_path = leaf_where(&app, |n| matches!(n.ty, ConfigType::String)).expect("a string leaf");
        let color_path = leaf_where(&app, |n| matches!(n.ty, ConfigType::Color));
        let path_path = leaf_where(&app, |n| matches!(n.ty, ConfigType::Path));
        let enum_path = leaf_where(&app, |n| matches!(n.ty, ConfigType::Enum(_)));
        let list_path = leaf_where(&app, |n| matches!(n.ty, ConfigType::StringList));
        let other_path = leaf_where(&app, |n| {
            !matches!(
                n.ty,
                ConfigType::Bool
                    | ConfigType::Int
                    | ConfigType::Float
                    | ConfigType::String
                    | ConfigType::Color
                    | ConfigType::Path
                    | ConfigType::Enum(_)
                    | ConfigType::StringList
            )
        });
        let stanza_path = app
            .rows()
            .iter()
            .find(|(_, n)| !n.is_leaf() && !n.children.is_empty())
            .map(|(_, n)| n.path.clone())
            .expect("a stanza");
        let provider_entry = find_in_tree(&app.schema, "llm.providers")
            .and_then(|n| n.children.first())
            .map(|n| n.path.clone())
            .expect("a provider entry");
        out.push(format!(
            "[LEAVES] bool={bool_path} int={int_path} float={float_path:?} text={text_path} color={color_path:?} path={path_path:?} enum={enum_path:?} list={list_path:?} other={other_path:?}"
        ));

        type Opener = Box<dyn FnMut(&mut App)>;
        let open_leaf = |path: String, extra: Vec<crossterm::event::KeyEvent>| -> Opener {
            Box::new(move |a: &mut App| {
                assert!(goto(a, &path), "no row for {path}");
                press(a, &[k(KeyCode::Enter)]);
                press(a, &extra);
            })
        };
        let mut openers: Vec<(String, String, Opener)> = Vec::new();
        {
            let p = text_path.clone();
            openers.push(("Help/leaf".into(), "Help".into(), Box::new(move |a| {
                goto(a, &p);
                press(a, &[k(KeyCode::Char('?'))]);
            })));
            let p = stanza_path.clone();
            openers.push(("Help/stanza".into(), "Help".into(), Box::new(move |a| {
                goto(a, &p);
                press(a, &[ctrl('h')]);
            })));
        }
        openers.push(("Edit/Bool".into(), "Edit/Bool".into(), open_leaf(bool_path.clone(), vec![])));
        openers.push(("Edit/Int".into(), "Edit/Int".into(), open_leaf(int_path.clone(), vec![])));
        if let Some(p) = float_path.clone() {
            openers.push(("Edit/Float".into(), "Edit/Float".into(), open_leaf(p, vec![])));
        }
        openers.push(("Edit/Text".into(), "Edit/Text".into(), open_leaf(text_path.clone(), vec![])));
        if let Some(p) = color_path.clone() {
            openers.push(("Edit/Color".into(), "Edit/Color".into(), open_leaf(p.clone(), vec![])));
            openers.push(("Edit/Color+hsl".into(), "Edit/Color".into(), open_leaf(p, vec![k(KeyCode::Char('h'))])));
        }
        if let Some(p) = path_path.clone() {
            openers.push(("Edit/Path".into(), "Edit/Path".into(), open_leaf(p.clone(), vec![])));
            openers.push(("Edit/Path+picker".into(), "Edit/Path".into(), open_leaf(p, vec![k(KeyCode::F(3))])));
        }
        if let Some(p) = enum_path.clone() {
            openers.push(("Edit/Enum".into(), "Edit/Enum".into(), open_leaf(p, vec![])));
        }
        if let Some(p) = list_path.clone() {
            openers.push(("Edit/List".into(), "Edit/List".into(), open_leaf(p.clone(), vec![])));
            openers.push(("Edit/List+add".into(), "Edit/List".into(), open_leaf(p, vec![k(KeyCode::Char('a'))])));
        }
        if let Some(p) = other_path.clone() {
            openers.push(("Edit/Unsupported".into(), "Edit/Unsupported".into(), open_leaf(p, vec![])));
        }
        // Staging an int increment always yields a diff, so the save paths
        // can open as often as the battery needs them to.
        let stage_int = |a: &mut App, p: &str| {
            assert!(goto(a, p), "no row for {p}");
            press(a, &[k(KeyCode::Enter), k(KeyCode::Up), k(KeyCode::Enter)]);
        };
        {
            let p = int_path.clone();
            openers.push(("SaveConfirm".into(), "SaveConfirm".into(), Box::new(move |a| {
                stage_int(a, &p);
                press(a, &[ctrl('s')]);
            })));
            let p = int_path.clone();
            openers.push(("Saved".into(), "Saved".into(), Box::new(move |a| {
                stage_int(a, &p);
                press(a, &[ctrl('s'), k(KeyCode::Char('y'))]);
            })));
            let p = int_path.clone();
            openers.push(("DiscardConfirm".into(), "DiscardConfirm".into(), Box::new(move |a| {
                stage_int(a, &p);
                press(a, &[k(KeyCode::Esc)]);
            })));
        }
        openers.push(("Rollback".into(), "Rollback".into(), Box::new(|a| press(a, &[ctrl('r')]))));
        openers.push(("RollbackPreview".into(), "RollbackPreview".into(), Box::new(|a| press(a, &[ctrl('r'), k(KeyCode::Char('v'))]))));
        openers.push(("RollbackDelete".into(), "RollbackDelete".into(), Box::new(|a| press(a, &[ctrl('r'), k(KeyCode::Char('d'))]))));
        {
            let p = text_path.clone();
            openers.push(("Annotate".into(), "Annotate".into(), Box::new(move |a| {
                goto(a, &p);
                press(a, &[ctrl('a')]);
            })));
            let p = text_path.clone();
            openers.push(("Inspector".into(), "Inspector".into(), Box::new(move |a| {
                goto(a, &p);
                press(a, &[ctrl('i')]);
            })));
        }
        openers.push(("AddMapEntry".into(), "AddMapEntry".into(), Box::new(|a| {
            goto(a, "llm.providers");
            press(a, &[k(KeyCode::Char('a'))]);
        })));
        {
            let p = provider_entry.clone();
            openers.push(("DeleteMapEntryConfirm".into(), "DeleteMapEntryConfirm".into(), Box::new(move |a| {
                goto(a, &p);
                press(a, &[k(KeyCode::Char('d'))]);
            })));
        }

        // ── Battery inside every modal ────────────────────────────────
        seed_backups(&app, 40);
        for (label, expected, opener) in openers.iter_mut() {
            app.modal = Modal::None;
            opener(&mut app);
            let got = modal_name(&app.modal);
            if &got != expected {
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
            // Esc reaches the main view within three presses.
            app.modal = Modal::None;
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

        // ── CHECK chorded letters are not bare keys ───────────────────
        goto(&mut app, &text_path);
        for key in [ctrl('e'), alt('e'), alt('a'), ctrl('d'), alt('d')] {
            send(&mut app, key).unwrap();
            if !matches!(app.modal, Modal::None) {
                problems.push(format!("[CHORD] {key:?} opened {}", modal_name(&app.modal)));
                app.modal = Modal::None;
            }
        }
        goto(&mut app, &provider_entry);
        send(&mut app, ctrl('d')).unwrap();
        if !matches!(app.modal, Modal::None) {
            problems.push("[CHORD] Ctrl+D on a provider entry opened the delete confirm".into());
            app.modal = Modal::None;
        }
        out.push("[CHECK chord] Ctrl/Alt letters leave the tree alone".into());

        // ── CHECK the text box takes the shared chord set ─────────────
        goto(&mut app, &text_path);
        press(&mut app, &[k(KeyCode::Enter), ctrl('u')]);
        for c in "abc".chars() {
            press(&mut app, &[k(KeyCode::Char(c))]);
        }
        press(&mut app, &[k(KeyCode::Left), k(KeyCode::Char('x')), k(KeyCode::Home), k(KeyCode::Char('>')), k(KeyCode::End), k(KeyCode::Char('<')), k(KeyCode::Enter)]);
        let got = node_value(&app, &text_path);
        if got != Some(Value::String(">abxc<".into())) {
            problems.push(format!("[TEXT] expected `>abxc<`, got {got:?}"));
        }
        out.push(format!("[CHECK text] {text_path} = {got:?}"));
        goto(&mut app, &int_path);
        press(&mut app, &[k(KeyCode::Enter), ctrl('u'), k(KeyCode::Char('4')), k(KeyCode::Char('2')), k(KeyCode::Char('z')), k(KeyCode::Up), k(KeyCode::Enter)]);
        let got = node_value(&app, &int_path);
        if got != Some(json!(43)) {
            problems.push(format!("[INT] expected 43, got {got:?}"));
        }
        out.push(format!("[CHECK int] {int_path} = {got:?}"));
        if let Some(p) = color_path.clone() {
            goto(&mut app, &p);
            press(&mut app, &[k(KeyCode::Enter), ctrl('u')]);
            for c in "#AbCdEfz".chars() {
                press(&mut app, &[k(KeyCode::Char(c))]);
            }
            press(&mut app, &[k(KeyCode::Enter)]);
            let got = node_value(&app, &p);
            if got != Some(Value::String("#abcdef".into())) {
                problems.push(format!("[COLOR] expected `#abcdef`, got {got:?}"));
            }
            out.push(format!("[CHECK color] {p} = {got:?}"));
        }

        // ── CHECK AddMapEntry validates and stages ────────────────────
        app.modal = Modal::None;
        goto(&mut app, "llm.providers");
        press(&mut app, &[k(KeyCode::Char('a'))]);
        for c in "bad name".chars() {
            press(&mut app, &[k(KeyCode::Char(c))]);
        }
        press(&mut app, &[k(KeyCode::Enter)]);
        let shown = frame(&mut app, 80, 24).unwrap();
        if !matches!(app.modal, Modal::AddMapEntry { error: Some(_), .. }) || !shown.contains('⚠') {
            problems.push("[ADD] an invalid name did not show an error".into());
        }
        press(&mut app, &[ctrl('u')]);
        for c in "probe_added".chars() {
            press(&mut app, &[k(KeyCode::Char(c))]);
        }
        press(&mut app, &[k(KeyCode::Enter)]);
        if !matches!(app.modal, Modal::None) || !app.added_map_entries.contains("llm.providers.probe_added") {
            problems.push(format!("[ADD] staging failed: modal {} · status {}", modal_name(&app.modal), app.status));
        }
        // Dropping the staged entry shortens the row list; the cursor stays on a row.
        if goto(&mut app, "llm.providers.probe_added") {
            let last = app.rows().len() - 1;
            app.cursor = last;
            goto(&mut app, "llm.providers.probe_added");
            press(&mut app, &[k(KeyCode::Char('d')), k(KeyCode::Char('y'))]);
            if let Some(bad) = invariants(&app) {
                problems.push(format!("[ADD] after dropping the entry: {bad}"));
            }
        } else {
            problems.push("[ADD] the staged entry has no row".into());
        }
        out.push("[CHECK add-entry] validation + staging + drop".into());

        // ── CHECK the save round-trips comments and unknown fields ────
        app.modal = Modal::None;
        stage_int(&mut app, &int_path);
        let want = node_value(&app, &int_path).unwrap();
        press(&mut app, &[ctrl('s'), k(KeyCode::Char('y'))]);
        if !matches!(app.modal, Modal::Saved { .. }) {
            problems.push(format!("[SAVE] no Saved overlay: {}", modal_name(&app.modal)));
        }
        let saved = std::fs::read_to_string(&app.cfg_path).unwrap();
        let reloaded: Value = serde_hjson::from_str(&saved).unwrap();
        let key = int_path.rsplit('.').next().unwrap();
        let mut cur = &reloaded;
        for seg in int_path.split('.') {
            cur = &cur[seg];
        }
        if cur != &want {
            problems.push(format!("[SAVE] {int_path} on disk is {cur}, staged {want} (key {key})"));
        }
        if !saved.starts_with("// probe config — leading comment\n# hash comment\n") {
            problems.push("[SAVE] the leading comments were not preserved".into());
        }
        if reloaded["probe_unknown"]["kept"] != json!(true) {
            problems.push("[SAVE] the unknown field was dropped".into());
        }
        out.push(format!("[CHECK save] {int_path} = {want} · comments + unknown kept"));

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
                let cursor = match &app.modal {
                    Modal::Rollback { cursor, .. } => *cursor,
                    _ => 0,
                };
                let shown = frame(&mut app, 80, 24).unwrap();
                if !shown.contains('▶') {
                    problems.push(format!("[ROLLBACK] {label}: cursor {cursor} of {n} is not on the frame"));
                }
            }
        }
        out.push(format!("[CHECK rollback] {n} backups · cursor visible at Home / mid / End"));

        // ── CHECK the help pane scrolls to its end and no further ─────
        app.modal = Modal::None;
        let longest = {
            let mut best = (0usize, text_path.clone());
            let paths: Vec<String> = app.rows().iter().filter(|(_, n)| n.is_leaf()).map(|(_, n)| n.path.clone()).collect();
            for p in paths {
                goto(&mut app, &p);
                open_help(&mut app);
                if let Modal::Help { body, .. } = &app.modal {
                    let lines = body.lines().count();
                    if lines > best.0 {
                        best = (lines, p.clone());
                    }
                }
                app.modal = Modal::None;
            }
            best
        };
        goto(&mut app, &longest.1);
        press(&mut app, &[k(KeyCode::Char('?'))]);
        frame(&mut app, 80, 24).unwrap();
        press(&mut app, &[k(KeyCode::End)]);
        frame(&mut app, 80, 24).unwrap();
        let (scroll, max) = match &app.modal {
            Modal::Help { body, scroll } => (*scroll, help_scroll_max(Rect::new(0, 0, 80, 24), body)),
            _ => (0, 0),
        };
        if scroll != max {
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
        out.push(format!("[CHECK help] longest body {} lines ({}) · End → {max} · Up → {after_up}", longest.0, longest.1));

        out.push(format!("PROBLEMS ({}):", problems.len()));
        out.extend(problems.iter().cloned());
        assert!(problems.is_empty(), "{} problem(s) — see the report", problems.len());
    });
}
